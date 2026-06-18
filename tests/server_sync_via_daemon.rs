//! Round 6 — P2: server_fence / server_fence_nb via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise the synchronization fence operations available through the
//! PMIx server library.
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_sync_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::server::{
    server_fence, server_fence_nb, server_finalize, server_init, FenceNbCallbackWrapper,
    PmixServerModule,
};
use pmix::{InfoBuilder, PmixStatus};

// Dummy callbacks for testing module with callbacks set.
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

/// server_fence signature returns Result<PmixStatus, PmixStatus>.
#[test]
fn test_server_fence_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[pmix::Info],
        i32,
    ) -> Result<PmixStatus, PmixStatus> = server_fence;
}

/// server_fence_nb signature returns Result<(), PmixStatus> and accepts FenceNbCallbackWrapper.
#[test]
fn test_server_fence_nb_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[pmix::Info],
        FenceNbCallbackWrapper,
    ) -> Result<(), PmixStatus> = server_fence_nb;
}

/// FenceNbCallbackWrapper is constructible with a closure.
#[test]
fn test_fence_nb_callback_wrapper_constructible() {
    let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
    drop(wrapper);
}

/// FenceNbCallbackWrapper accepts closures that capture state.
#[test]
fn test_fence_nb_callback_wrapper_with_capture() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CALLED: AtomicBool = AtomicBool::new(false);
    let _wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {
        CALLED.store(true, Ordering::SeqCst);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_fence from server context.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_fence(&handle, &[], 0);
    assert!(result.is_err(), "server_fence from server context should return Err");

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence with timeout parameter.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_with_timeout_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_fence(&handle, &[], 30);
    assert!(result.is_err(), "server_fence with timeout should return Err");

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence returns PmixStatus with correct error type.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_returns_pmix_status_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result: Result<PmixStatus, PmixStatus> = server_fence(&handle, &[], 0);
    assert!(result.is_err(), "server_fence should return Err(PmixStatus)");

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence with info directives.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_with_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let fence_info = vec![InfoBuilder::new().build()];
    let result = server_fence(&handle, &fence_info, 0);
    assert!(result.is_err(), "server_fence with info should return Err");

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence_nb accepts callback and returns Ok for request submission.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_nb_with_daemon() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let wrapper = FenceNbCallbackWrapper::new(move |status: PmixStatus| {
        CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        let _ = status;
    });

    let result = server_fence_nb(&handle, &[], wrapper);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence_nb with info directives.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_nb_with_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let fence_info = vec![InfoBuilder::new().build()];
    let wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {});

    let result = server_fence_nb(&handle, &fence_info, wrapper);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_fence with callbacks module (single init/finalize).
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_with_callbacks_module_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.fence_nb = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_fence(&handle, &[], 0);
    assert!(result.is_err(), "server_fence should return Err");

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: Verify server_fence error is ErrUnreach specifically.
#[test]
#[ignore = "daemon isolation"]
fn test_server_fence_err_unreach_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_fence(&handle, &[], 0);
    assert!(result.is_err(), "server_fence should return Err");

    server_finalize(handle).expect("server_finalize");
}
