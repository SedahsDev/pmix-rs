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
//! 4. Return `PMIX_SUCCESS` from the upcall immediately.
//! 5. Invoke the provided `cbfunc` **later** from the app thread when RM work
//!    finishes.
//!
//! Upcalls are **not** CPU-pin targets — pin the progress engine via
//! `InitOptions::bind_progress_thread` (see THREADING.md §4), not the
//! `fence_nb` / `direct_modex` bodies themselves.
//!
//! Shared hop-off helpers and client `_nb` / events counterpart:
//! [`pmix::threading`], `examples/callback_hop.rs` (issue #51).
//!
//! # Running
//!
//! Compiles and runs standalone (no clients → no real upcalls). The example
//! also **self-drives** the hop pattern once so the async `cbfunc` path is
//! exercised without a full RM:
//!
//! ```text
//! cargo run --example server_upcall_hop
//! ```
//!
//! Under a real resource manager the same function pointers are installed on
//! the module OpenPMIx copies at `PMIx_server_init`.

use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pmix::server::{PmixServer, PmixServerModule};
use pmix::threading::{spawn_from_callback, CallbackChannel, ProgressContext};
use pmix::InfoBuilder;

/// Counts successful async completions from hopped-off upcall work.
static COMPLETIONS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    println!("pmix-rs: server upcall hop-off example (issue #52)");
    println!("Pattern: progress upcall → spawn_from_callback → delayed cbfunc");
    println!();

    // ── 1. Install typed host upcalls on the module ────────────────────────
    //
    // `PmixServerModule` stores `Option<unsafe extern "C" fn()>` so the layout
    // matches a C function-pointer table. Real OpenPMIx signatures are richer;
    // transmute is the established install path (see server module docs).
    let mut module = PmixServerModule::default();
    // SAFETY: `host_fence_nb` / `host_direct_modex` match the C typedefs
    // `pmix_server_fencenb_fn_t` / `pmix_server_dmodex_req_fn_t`. OpenPMIx
    // only calls them through those typed slots after `server_init`.
    module.fence_nb = Some(unsafe {
        std::mem::transmute::<
            unsafe extern "C" fn(
                *const c_void,
                usize,
                *const c_void,
                usize,
                *mut i8,
                usize,
                Option<ModexCb>,
                *mut c_void,
            ) -> i32,
            unsafe extern "C" fn(),
        >(host_fence_nb)
    });
    module.direct_modex = Some(unsafe {
        std::mem::transmute::<
            unsafe extern "C" fn(
                *const c_void,
                *const c_void,
                usize,
                Option<ModexCb>,
                *mut c_void,
            ) -> i32,
            unsafe extern "C" fn(),
        >(host_direct_modex)
    });

    let info = InfoBuilder::new().build();
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
    // Mirrors what OpenPMIx does when a client fence/modex arrives: enter the
    // upcall on a "progress" stand-in thread, hop, complete via cbfunc later.
    let hop = CallbackChannel::<&'static str>::new();
    let tx = hop.sender();
    let progress_stand_in = std::thread::Builder::new()
        .name("pmix-progress-standin".into())
        .spawn(move || {
            // SAFETY: synthetic call with null procs/info and a real completion
            // path — demonstrates the hop rules without a full RM collective.
            let status = unsafe {
                host_fence_nb(
                    ptr::null(),
                    0,
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    0,
                    Some(demo_modex_complete),
                    // Tag the cbdata so demo_modex_complete can signal the app.
                    Box::into_raw(Box::new(tx)) as *mut c_void,
                )
            };
            assert_eq!(status, 0, "upcall must return PMIX_SUCCESS immediately");
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

/// `pmix_modex_cbfunc_t` — OpenPMIx completion for fence / direct-modex upcalls.
type ModexCb = unsafe extern "C" fn(
    status: i32,
    data: *const i8,
    ndata: usize,
    cbdata: *mut c_void,
    release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    release_cbdata: *mut c_void,
);

/// Demo completion used by the self-driven path: signals the app channel.
unsafe extern "C" fn demo_modex_complete(
    status: i32,
    _data: *const i8,
    _ndata: usize,
    cbdata: *mut c_void,
    release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    release_cbdata: *mut c_void,
) {
    COMPLETIONS.fetch_add(1, Ordering::SeqCst);
    if !cbdata.is_null() {
        // SAFETY: cbdata is the Box'd Sender we passed from main.
        let tx = unsafe { Box::from_raw(cbdata as *mut std::sync::mpsc::Sender<&'static str>) };
        let _ = tx.send(if status == 0 {
            "fence_nb hopped + cbfunc OK"
        } else {
            "fence_nb hopped + cbfunc error"
        });
    }
    if let Some(rel) = release_fn {
        // SAFETY: release pair is whatever the host passed; null is fine.
        unsafe { rel(release_cbdata) };
    }
}

/// Host `fence_nb` upcall — **progress context**.
///
/// Matches `pmix_server_fencenb_fn_t`. Must not block on PMIx; hop and call
/// `cbfunc` later.
unsafe extern "C" fn host_fence_nb(
    _procs: *const c_void,
    nprocs: usize,
    _info: *const c_void,
    _ninfo: usize,
    data: *mut i8,
    ndata: usize,
    cbfunc: Option<ModexCb>,
    cbdata: *mut c_void,
) -> i32 {
    let _ctx = ProgressContext;

    // Copy contributed blob before return — C buffer lifetime ends with the
    // upcall frame; the pointer is also !Send.
    let blob: Arc<Vec<u8>> = Arc::new(if data.is_null() || ndata == 0 {
        Vec::new()
    } else {
        // SAFETY: OpenPMIx guarantees `data` is ndata bytes for the call.
        unsafe { std::slice::from_raw_parts(data as *const u8, ndata) }.to_vec()
    });

    // *mut c_void is !Send — carry the chain address as usize across the hop.
    let chain_addr = cbdata as usize;
    let nprocs = nprocs;

    let spawn_result = spawn_from_callback(move || {
        // Application / RM thread — blocking work and delayed completion are OK.
        println!(
            "[app thread] fence_nb: nprocs={nprocs} blob_len={} — collective/RM work safe here",
            blob.len()
        );
        // Real RMs would exchange `blob` with peer daemons here, then complete.
        if let Some(cb) = cbfunc {
            // SAFETY: complete the OpenPMIx fence chain. Return the chain
            // pointer verbatim; no host-owned release blob in this demo.
            unsafe {
                cb(
                    0, // PMIX_SUCCESS
                    if blob.is_empty() {
                        ptr::null()
                    } else {
                        blob.as_ptr() as *const i8
                    },
                    blob.len(),
                    chain_addr as *mut c_void,
                    None,
                    ptr::null_mut(),
                );
            }
        } else {
            COMPLETIONS.fetch_add(1, Ordering::SeqCst);
        }
    });
    if let Err(e) = spawn_result {
        // Never panic from a PMIx upcall — log and fail the request.
        eprintln!("pmix: fence_nb hop spawn failed: {e}");
        return -1; // PMIX_ERROR
    }

    // Return immediately — do NOT wait for the hop thread.
    0 // PMIX_SUCCESS
}

/// Host `direct_modex` upcall — **progress context**.
///
/// Matches `pmix_server_dmodex_req_fn_t`. Same hop-then-`cbfunc` rules as fence.
unsafe extern "C" fn host_direct_modex(
    _proc: *const c_void,
    _info: *const c_void,
    _ninfo: usize,
    cbfunc: Option<ModexCb>,
    cbdata: *mut c_void,
) -> i32 {
    let _ctx = ProgressContext;
    let chain_addr = cbdata as usize;

    let spawn_result = spawn_from_callback(move || {
        println!("[app thread] direct_modex: remote fetch would run here");
        // Standalone demo: empty blob success. Real RMs contact the remote
        // daemon, assemble the modex blob, then complete.
        if let Some(cb) = cbfunc {
            unsafe {
                cb(
                    0,
                    ptr::null(),
                    0,
                    chain_addr as *mut c_void,
                    None,
                    ptr::null_mut(),
                );
            }
        }
        COMPLETIONS.fetch_add(1, Ordering::SeqCst);
    });
    if let Err(e) = spawn_result {
        eprintln!("pmix: direct_modex hop spawn failed: {e}");
        return -1;
    }
    0
}
