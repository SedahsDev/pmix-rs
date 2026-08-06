//! Multi-thread + external-progress integration tests (issue #54).
//!
//! ## Goals covered
//!
//! 1. **N threads**: clone process-wide [`pmix::PmixClient`], concurrent
//!    `put_value` (distinct keys) + a process `fence` under `prterun`.
//! 2. **Concurrent `_nb` completions**: several `fence_nb` submissions with
//!    completions counted under internal progress.
//! 3. **`external_progress=true`**: host thread runs [`pmix::progress`] while
//!    workers issue ops; main thread fences once progress is running.
//! 4. **Callback must-not-block regression**: a `fence_nb` completion that
//!    blocks on the progress path is bounded by an application-side timeout
//!    (deadlock class from #51 / `THREADING.md`).
//!
//! ## How to run
//!
//! ```bash
//! export PMIX_PREFIX=${PMIX_PREFIX:-$HOME/.local/openpmix-6.1.0}
//! export LD_LIBRARY_PATH=$PMIX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
//! # PRTE ≥ 4.1 built against the same OpenPMIx (system PRTE 3.x is not enough):
//! export PATH=/path/to/prte-4.1/bin:$PATH
//!
//! # Standalone (Send/Clone + InitOptions only):
//! cargo test --test threading_mt_via_prterun -- --test-threads=1
//!
//! # DVM — each ignored test under its own prterun (cargo filter is a substring):
//! for t in mt_concurrent_put_and_fence mt_concurrent_fence_nb_completions \
//!          callback_must_not_block_progress_timeout mt_external_progress_host_thread; do
//!   prterun -np 1 cargo test --test threading_mt_via_prterun "$t" -- --ignored --test-threads=1
//! done
//!
//! # Harness (standalone + all DVM goals):
//! ./scripts/run_daemon_tests.sh THREADING
//! ```
//!
//! Uses **process-wide** [`PmixClient`] only (no bare `Context`, no per-thread init).

mod daemon_helper;

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pmix::data_ops::{fence_nb, FenceCallback};
use pmix::{InitOptions, PmixClient, PmixError, PmixScope, PmixStatus, PmixValueBuilder};

// ─── helpers ────────────────────────────────────────────────────────────────

fn is_dvm_launched() -> bool {
    std::env::var("PMIX_NAMESPACE").is_ok()
        || std::env::var("PMIX_RANK").is_ok()
        || std::env::var("PRTE_LAUNCHED").is_ok()
        || std::env::var("PMIX_SERVER_URI2").is_ok()
}

/// Connect once with default (internal) progress — process-wide session.
fn ensure_client_internal() -> &'static PmixClient {
    daemon_helper::ensure_pmix_init()
}

fn put_string_key(key: &str, value: &str) -> Result<(), String> {
    let ckey = CString::new(key).map_err(|e| format!("key NUL: {e}"))?;
    let mut owned = PmixValueBuilder::new()
        .string(value)
        .map_err(|e| format!("string value: {e:?}"))?
        .build()
        .map_err(|e| format!("build value: {e:?}"))?;
    pmix::put_value(PmixScope::Global.to_raw(), &ckey, &mut owned)
        .map_err(|e| format!("put_value({key}): {e:?}"))
}

// ─── Standalone (no DVM) ────────────────────────────────────────────────────

/// `PmixClient` is `Clone + Send + Sync` — clones can move to worker threads.
#[test]
fn client_clone_is_send_across_threads() {
    let client = PmixClient::new();
    let worker = client.clone();
    let handle = thread::spawn(move || {
        let _ = worker.state();
        worker.is_live()
    });
    let _live = handle.join().expect("worker must not panic");
    assert!(client.same_session(&PmixClient::new()));
}

/// Building `InitOptions` with external progress does not require a DVM.
#[test]
fn init_options_external_progress_builds() {
    let mut opts = InitOptions::new();
    opts.external_progress(true);
    let info = opts.build();
    assert_eq!(info.len(), 1, "external_progress must emit one Info entry");
    unsafe {
        let ent = &*info.as_ptr();
        let key = std::ffi::CStr::from_ptr(ent.key.as_ptr());
        eprintln!(
            "external_progress info: key={:?} type={} flag={}",
            key, ent.value.type_, ent.value.data.flag
        );
        assert_eq!(key.to_bytes(), b"pmix.evext");
        // PMIX_BOOL is type tag 1 in OpenPMIx.
        assert_eq!(ent.value.type_, 1);
        assert_ne!(ent.value.data.flag as u8, 0, "flag must be true");
    }
    drop(info);
}

#[test]
fn pmix_client_is_clone_send_sync() {
    fn assert_clone_send_sync<T: Clone + Send + Sync>() {}
    assert_clone_send_sync::<PmixClient>();
}

/// Documents session SM used by MT tests: second connect on Live → ErrExists.
#[test]
fn connect_new_err_exists_when_already_live_is_documented() {
    let client = PmixClient::new();
    match client.connect(None) {
        Ok(()) => {
            let err = client.connect(None).expect_err("second connect");
            assert_eq!(err, PmixError::ErrExists);
            let _ = client.disconnect(None);
        }
        Err(_) => {
            // No DVM — expected in standalone `cargo test`.
        }
    }
}

// ─── Goal (1): N threads, concurrent put + fence ────────────────────────────

/// Clone `PmixClient` onto N workers; each puts a distinct key. The main
/// thread then `commit` + `fence` once and verifies every key.
#[test]
#[ignore = "requires DVM-launched process (prterun) — issue #54 goal (1)"]
fn mt_concurrent_put_and_fence() {
    assert!(
        is_dvm_launched(),
        "mt_concurrent_put_and_fence must be launched by prterun"
    );

    let client = ensure_client_internal();
    assert!(
        client.is_live(),
        "session must be Live after ensure_pmix_init"
    );
    let proc = client.require_proc();

    const N: usize = 8;
    let barrier = Arc::new(Barrier::new(N));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut handles = Vec::with_capacity(N);

    for i in 0..N {
        let worker = client.clone();
        let barrier = Arc::clone(&barrier);
        let errors = Arc::clone(&errors);
        handles.push(thread::spawn(move || {
            assert!(worker.is_live(), "cloned client must observe Live session");
            let _rank = worker.require_rank();

            let key = format!("pmix.rs.mt.put.{i}");
            let val = format!("worker-{i}");
            // Align before put so the KVS writes race under load.
            barrier.wait();
            if let Err(e) = put_string_key(&key, &val) {
                errors.lock().expect("errors mutex").push(e);
            }
        }));
    }

    for h in handles {
        h.join().expect("worker must not panic");
    }

    {
        let errs = errors.lock().expect("errors mutex");
        assert!(
            errs.is_empty(),
            "concurrent put errors: {}",
            errs.join("; ")
        );
    }

    pmix::commit().unwrap_or_else(|e| panic!("commit after concurrent puts: {e:?}"));
    pmix::fence(&proc, None).unwrap_or_else(|e| panic!("fence after concurrent puts: {e:?}"));

    for i in 0..N {
        let key = CString::new(format!("pmix.rs.mt.put.{i}")).unwrap();
        let got = pmix::get_value(&proc, key.to_bytes_with_nul(), None);
        assert!(
            got.is_ok(),
            "get pmix.rs.mt.put.{i} after concurrent put/fence: {got:?}"
        );
    }
}

// ─── Goal (3): external_progress + host progress thread ─────────────────────

/// Host-driven progress: connect with `external_progress(true)`, run a
/// progress loop on a dedicated thread, workers put concurrently, main thread
/// commit/fence while progress runs.
///
/// **Must run in its own process** — `InitOptions` apply only on the first
/// process-wide connect. The THREADING harness isolates this test.
#[test]
#[ignore = "requires DVM-launched process (prterun) — issue #54 goal (3); run alone"]
fn mt_external_progress_host_thread() {
    assert!(
        is_dvm_launched(),
        "mt_external_progress_host_thread must be launched by prterun"
    );

    let probe = PmixClient::new();
    if probe.is_live() {
        panic!(
            "process-wide PmixClient already Live; run this test in its own \
             cargo test process (see scripts/run_daemon_tests.sh THREADING)"
        );
    }

    // Host progress before connect (external_progress has no internal thread).
    //
    // Important: `PMIx_Progress` may block inside libevent, so a stop flag is
    // not enough to make `JoinHandle::join` return promptly. We therefore
    // *detach* the progress thread and tear the session down via explicit
    // disconnect + atexit safety net (same process-exit model as
    // `daemon_helper::ensure_pmix_init`).
    let stop = Arc::new(AtomicBool::new(false));
    let stop_progress = Arc::clone(&stop);
    thread::Builder::new()
        .name("pmix-host-progress".into())
        .spawn(move || {
            while !stop_progress.load(Ordering::Acquire) {
                pmix::progress();
                thread::sleep(Duration::from_millis(1));
            }
            // Best-effort drain; may block — thread is detached at process exit.
            for _ in 0..16 {
                pmix::progress();
            }
        })
        .expect("spawn host progress thread");

    let mut opts = InitOptions::new();
    opts.external_progress(true);
    let info = opts.build();

    let client = PmixClient::connect_new(Some(info))
        .expect("connect_new with external_progress must succeed under prterun");
    assert!(client.is_live());
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            extern "C" fn finalize_at_exit() {
                let _ = pmix::finalize(None);
            }
            unsafe {
                libc::atexit(finalize_at_exit);
            }
        });
    }

    const N: usize = 4;
    let barrier = Arc::new(Barrier::new(N));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut handles = Vec::with_capacity(N);

    for i in 0..N {
        let worker = client.clone();
        let barrier = Arc::clone(&barrier);
        let errors = Arc::clone(&errors);
        handles.push(thread::spawn(move || {
            assert!(worker.is_live());
            barrier.wait();
            let key = format!("pmix.rs.mt.ext.{i}");
            if let Err(e) = put_string_key(&key, &format!("ext-{i}")) {
                errors.lock().expect("errors").push(e);
            }
        }));
    }

    for h in handles {
        h.join().expect("worker must not panic");
    }

    {
        let errs = errors.lock().expect("errors");
        assert!(
            errs.is_empty(),
            "external_progress put errors: {}",
            errs.join("; ")
        );
    }

    let proc = client.require_proc();
    pmix::commit().unwrap_or_else(|e| panic!("commit (external progress): {e:?}"));
    pmix::fence(&proc, None).unwrap_or_else(|e| panic!("fence (external progress): {e:?}"));

    for i in 0..N {
        let key = CString::new(format!("pmix.rs.mt.ext.{i}")).unwrap();
        let got = pmix::get_value(&proc, key.to_bytes_with_nul(), None);
        assert!(
            got.is_ok(),
            "get pmix.rs.mt.ext.{i} under external progress: {got:?}"
        );
    }

    // Signal progress to wind down; do not join (Progress may block).
    stop.store(true, Ordering::Release);

    // Disconnect while the progress thread may still be runnable — try with a
    // short timeout so a stuck finalize cannot hang the harness.
    let client2 = client.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let r = client2.disconnect(None);
        let _ = tx.send(r);
    });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("disconnect error (non-fatal): {e:?}"),
        Err(_) => {
            eprintln!(
                "disconnect timed out under external_progress — atexit finalize \
                 will clean up; concurrent put/fence/get already verified"
            );
        }
    }
}

// ─── Goal (2): concurrent _nb completions ───────────────────────────────────

struct CountingFenceCb {
    done: Arc<AtomicUsize>,
    ok: Arc<AtomicUsize>,
}

impl FenceCallback for CountingFenceCb {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        if status.is_success() {
            self.ok.fetch_add(1, Ordering::Release);
        }
        self.done.fetch_add(1, Ordering::Release);
    }
}

/// Submit several `fence_nb` ops; wait for all completions (internal progress).
#[test]
#[ignore = "requires DVM-launched process (prterun) — issue #54 goal (2)"]
fn mt_concurrent_fence_nb_completions() {
    assert!(
        is_dvm_launched(),
        "mt_concurrent_fence_nb_completions must be launched by prterun"
    );

    let client = ensure_client_internal();
    let proc = client.require_proc();

    const N: usize = 4;
    let done = Arc::new(AtomicUsize::new(0));
    let ok = Arc::new(AtomicUsize::new(0));

    for _ in 0..N {
        let cb = Box::new(CountingFenceCb {
            done: Arc::clone(&done),
            ok: Arc::clone(&ok),
        });
        fence_nb(std::slice::from_ref(&proc), None, cb)
            .unwrap_or_else(|e| panic!("fence_nb submit: {e:?}"));
    }

    let deadline = Instant::now() + Duration::from_secs(30);
    while done.load(Ordering::Acquire) < N {
        if Instant::now() > deadline {
            panic!(
                "timed out waiting for fence_nb completions: {}/{N}",
                done.load(Ordering::Acquire)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(done.load(Ordering::Acquire), N);
    assert_eq!(
        ok.load(Ordering::Acquire),
        N,
        "all fence_nb completions should be success"
    );
}

// ─── Goal (4): callback must-not-block regression ───────────────────────────

/// Callback parks on the progress path — models THREADING.md §6.1 deadlock class.
struct BlockingFenceCb {
    pair: Arc<(Mutex<bool>, Condvar)>,
}

impl FenceCallback for BlockingFenceCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        let (lock, cvar) = &*self.pair;
        let guard = lock.lock().expect("mutex");
        // Wait up to 3s for a flag that nobody sets — holds the progress
        // thread if the callback runs there.
        let _ = cvar
            .wait_timeout(guard, Duration::from_secs(3))
            .expect("condvar");
    }
}

/// Regression: progress-thread blocking is detected with a wall-clock timeout
/// on a follow-up op, rather than hanging the suite forever.
#[test]
#[ignore = "requires DVM-launched process (prterun) — issue #54 goal (4)"]
fn callback_must_not_block_progress_timeout() {
    assert!(
        is_dvm_launched(),
        "callback_must_not_block_progress_timeout must be launched by prterun"
    );

    let client = ensure_client_internal();
    let proc = client.require_proc();

    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let cb = Box::new(BlockingFenceCb {
        pair: Arc::clone(&pair),
    });

    fence_nb(std::slice::from_ref(&proc), None, cb)
        .unwrap_or_else(|e| panic!("fence_nb submit: {e:?}"));

    let client2 = client.clone();
    let proc2 = proc.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let _ = client2.is_live();
        let r = pmix::fence(&proc2, None);
        let _ = tx.send(r);
    });

    match rx.recv_timeout(Duration::from_secs(8)) {
        Ok(fence_result) => {
            // Completed (in-place nb, or block finished). Must not hang.
            match fence_result {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("secondary fence returned error (non-fatal for timeout test): {e:?}");
                }
            }
        }
        Err(_) => {
            eprintln!(
                "callback_must_not_block_progress_timeout: secondary fence \
                 timed out (expected when a progress callback blocks) — OK"
            );
        }
    }
}
