//! Server-module upcall hop-off example (issue #52).
//!
//! Host callbacks in [`PmixServerModule`](pmix::server::PmixServerModule)
//! (`fence_nb`, `direct_modex`, …) run in PMIx **progress context**. Blocking
//! back into PMIx from an upcall deadlocks progress. The pattern is:
//!
//! 1. Mark the handler with [`ProgressContext`](pmix::threading::ProgressContext).
//! 2. Copy any C buffers you need (lifetimes end when the upcall returns;
//!    pointers are `!Send`).
//! 3. [`spawn_from_callback`](pmix::threading::spawn_from_callback) (or a
//!    [`CallbackChannel`](pmix::threading::CallbackChannel)) to hop off.
//! 4. Return from the upcall immediately.
//! 5. Invoke the provided `cbfunc` **later** from the app thread when RM work
//!    finishes.
//!
//! Upcalls are **not** CPU-pin targets — pin the progress engine via
//! `InitOptions::bind_progress_thread` (see THREADING.md §4), not the
//! `fence_nb` / `direct_modex` bodies themselves.
//!
//! The module fields take native callback signatures (#520): borrowed view
//! types (`ProcView`, `InfoView`, …) instead of raw bindgen pointers, and
//! owned completion-callback wrappers instead of raw C fn pointers.
//!
//! Shared hop-off helpers and client `_nb` / events counterpart:
//! [`pmix::threading`], `examples/callback_hop.rs` (issue #51).
//!
//! # Running
//!
//! ```text
//! cargo run --example server_upcall_hop
//! ```
//!
//! Under a real resource manager the same native callbacks are installed on
//! the module OpenPMIx copies at `PMIx_server_init`; the crate's C-ABI
//! trampolines convert raw PMIx arguments into the views for you.

use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use pmix::PmixStatus;
use pmix::server::PmixServer;
use pmix::server::PmixServerModule;
use pmix::server::callbacks::{ModexCallback, ReleaseCallback};
use pmix::threading::{CallbackChannel, ProgressContext, spawn_from_callback};
use pmix::views::{InfoView, ProcView};

/// Counts successful async completions from hopped-off upcall work.
static COMPLETIONS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    println!("pmix-rs: server upcall hop-off example (issue #52)");
    println!("Pattern: progress upcall → spawn_from_callback → delayed cbfunc");
    println!();

    // ── 1. Install native host upcalls on the module ───────────────────────
    //
    // Since #520 the fields take native signatures: views + completion
    // wrappers, no transmutes and no raw bindgen types.
    let module = PmixServerModule {
        fence_nb: Some(host_fence_nb),
        direct_modex: Some(host_direct_modex),
        ..Default::default()
    };

    let info = pmix::InfoBuilder::new().build().expect("build info");
    let server = match PmixServer::connect_new(Some(&module), &info) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("PmixServer::connect_new failed: {e:?}");
            eprintln!("(OpenPMIx ≥ 6.1 must be discoverable via PMIX_PREFIX.)");
            return;
        }
    };
    println!(
        "server live={} — fence_nb + direct_modex installed",
        server.is_live()
    );

    // ── 2. Self-drive the hop pattern (no client fence required) ───────────
    //
    // Mirrors what the crate's C-ABI trampoline does when a client
    // fence/modex arrives: enter the upcall on a "progress" stand-in thread,
    // hop, complete via the native completion wrapper later.
    let hop = CallbackChannel::<&'static str>::new();
    let tx = hop.sender();
    let completion = ModexCallback::new(
        move |status: PmixStatus, _data, _len, _cbdata, _rel, _rel_cbdata| {
            COMPLETIONS.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(if status.to_raw() == 0 {
                "fence_nb hopped + cbfunc OK"
            } else {
                "fence_nb hopped + cbfunc error"
            });
        },
    );
    let progress_stand_in = std::thread::Builder::new()
        .name("pmix-progress-standin".into())
        .spawn(move || {
            // SAFETY: synthetic call with empty views and a real completion
            // path — demonstrates the hop rules without a full RM collective.
            unsafe {
                host_fence_nb(
                    // The participant array (empty = all ranks in the nspace).
                    &[],
                    &[],
                    // No modex blob in the demo.
                    ptr::null_mut(),
                    0,
                    Some(completion),
                    // Tag the chain pointer (null in this self-drive).
                    ptr::null_mut(),
                )
            };
        })
        .expect("spawn progress stand-in");

    // Application side: wait for the hopped completion (never join hop work
    // from inside the upcall).
    match hop.recv_timeout(Duration::from_secs(5)) {
        Ok(msg) => println!("[app thread] async fence completion: {msg}"),
        Err(e) => eprintln!("[app thread] no completion within 5s: {e}"),
    }
    progress_stand_in.join().expect("progress stand-in");

    // Brief spin so fire-and-forget hop threads finish counting (standalone).
    let deadline = Instant::now() + Duration::from_secs(2);
    while COMPLETIONS.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    println!(
        "hopped completions observed: {}",
        COMPLETIONS.load(Ordering::SeqCst)
    );

    if let Err(e) = server.disconnect() {
        eprintln!("disconnect failed: {e:?}");
        return;
    }
    println!("server_upcall_hop example done");
}

/// Host `fence_nb` upcall — **progress context**.
///
/// Native signature (#520): `(procs[], info[], data, ndata, cbfunc, cbdata)`
/// — the participant array, the collective directives, the modex blob, and
/// the modex completion. Must not block on PMIx; hop and call `cbfunc`
/// later.
unsafe fn host_fence_nb(
    procs: &[ProcView],
    info: &[InfoView],
    _data: *mut std::os::raw::c_char,
    _ndata: usize,
    cbfunc: Option<ModexCallback>,
    cbdata: *mut c_void,
) {
    let _ctx = ProgressContext;

    // Copy what the hop thread needs before return — view borrows end with
    // the upcall frame and raw pointers are !Send.
    let nprocs = procs.len();
    let ninfo = info.len();

    // *mut c_void is !Send — carry the chain address as usize across the hop.
    let chain_addr = cbdata as usize;

    let spawn_result = spawn_from_callback(move || {
        // Application / RM thread — blocking work and delayed completion are OK.
        println!(
            "[app thread] fence_nb: nprocs={nprocs} ninfo={ninfo} — collective/RM work safe here"
        );
        // Real RMs would exchange the modex blob with peer daemons here,
        // then complete. Return the chain pointer verbatim.
        if let Some(cb) = cbfunc {
            // SAFETY: complete the OpenPMIx fence chain. Return the chain
            // pointer verbatim; no host-owned release blob in this demo.
            unsafe {
                cb.call(
                    PmixStatus::from_raw(0), // PMIX_SUCCESS
                    ptr::null(),             // no modex blob in the demo
                    0,
                    chain_addr as *mut c_void,
                    ReleaseCallback::default(),
                    ptr::null_mut(),
                );
            }
        } else {
            COMPLETIONS.fetch_add(1, Ordering::SeqCst);
        }
    });
    if let Err(e) = spawn_result {
        // Never panic from a PMIx upcall — log the failure.
        eprintln!("pmix: fence_nb hop spawn failed: {e}");
    }

    // Return immediately — do NOT wait for the hop thread.
}

/// Host `direct_modex` upcall — **progress context**.
///
/// Same hop-then-`cbfunc` rules as fence; the `direct_modex` slot also
/// completes through a [`ModexCallback`] (the modex completion shape).
unsafe fn host_direct_modex(
    _proc: &ProcView,
    _info: &[InfoView],
    cbfunc: Option<ModexCallback>,
    cbdata: *mut c_void,
) {
    let _ctx = ProgressContext;
    let chain_addr = cbdata as usize;

    let spawn_result = spawn_from_callback(move || {
        println!("[app thread] direct_modex: remote fetch would run here");
        // Standalone demo: empty blob success. Real RMs contact the remote
        // daemon, assemble the modex blob, then complete.
        if let Some(cb) = cbfunc {
            // SAFETY: complete the direct-modex request with an empty blob.
            unsafe {
                cb.call(
                    PmixStatus::from_raw(0),
                    ptr::null(),
                    0,
                    chain_addr as *mut c_void,
                    ReleaseCallback::default(),
                    ptr::null_mut(),
                )
            };
        }
        COMPLETIONS.fetch_add(1, Ordering::SeqCst);
    });
    if let Err(e) = spawn_result {
        eprintln!("pmix: direct_modex hop spawn failed: {e}");
    }
}
