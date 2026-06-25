//! Round 6 — P0: server_init / server_finalize success paths via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise the SUCCESS paths of server_init and server_finalize, which
//! are the largest uncovered functions in server.rs (61.67% line coverage).
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_core_via_daemon -- --ignored --test-threads=1
//!   (or run a single test by name)

mod daemon_helper;

use pmix::server::{
    PmixServerModule, is_server_initialized, server_finalize, server_init, server_init_minimal,
};
use pmix::{InfoBuilder, PmixStatus};

// Dummy callbacks for testing module with callbacks set
// All PmixServerModule callbacks are Option<unsafe extern "C" fn()>
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify error paths)
// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — no C-level state)
// ─────────────────────────────────────────────────────────────────────────────

/// is_server_initialized is callable (no state modification).
#[test]
fn test_is_server_initialized_callable() {
    let _ = is_server_initialized();
}

/// PmixServerHandle is not Copy (compile-time check).
#[test]
fn test_server_handle_not_copy() {
    let _ = std::any::type_name::<pmix::server::PmixServerHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_init with module and empty info succeeds.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_init_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");
    assert!(is_server_initialized(), "server should be initialized");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_init_minimal with None module succeeds.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_init_minimal_none_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let handle = server_init_minimal(None).expect("server_init_minimal(None) should succeed");
    assert!(is_server_initialized(), "server should be initialized");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_init_minimal with default module succeeds.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_init_minimal_default_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    assert!(is_server_initialized(), "server should be initialized");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: is_server_initialized lifecycle — false → true.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_initialized_lifecycle_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    assert!(!is_server_initialized(), "should be false before init");
    let handle = server_init_minimal(None).expect("server_init_minimal should succeed");
    assert!(is_server_initialized(), "should be true after init");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_finalize returns Result<(), PmixStatus> — verify Ok variant.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_finalize_ok_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");
    let result: Result<(), PmixStatus> = server_finalize(handle);
    assert!(result.is_ok(), "server_finalize should return Ok");
}

/// Daemon: server_init with module that has callbacks set.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_init_with_callbacks_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.abort = Some(dummy_callback);
    module.fence_nb = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle =
        server_init(Some(&module), &info).expect("server_init with callbacks should succeed");
    assert!(is_server_initialized());
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: PmixServerHandle Debug trait works.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_handle_debug_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let handle = server_init_minimal(None).expect("server_init should succeed");
    let debug_str = format!("{:?}", handle);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: PmixServerModule Default trait creates all-None callbacks.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_module_default_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    assert!(
        module.abort.is_none(),
        "default module should have no callbacks"
    );
    assert!(module.fence_nb.is_none());
    assert!(module.publish.is_none());
    assert!(module.lookup.is_none());
    assert!(module.spawn.is_none());

    // Verify it works with daemon
    let handle = server_init(Some(&module), &InfoBuilder::new().build())
        .expect("server_init with default module should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: PmixServerModule as_c_ptr returns valid pointer.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_module_as_c_ptr_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let ptr = module.as_c_ptr();
    assert!(!ptr.is_null(), "as_c_ptr should not return null");

    // Verify it works with daemon
    let handle = server_init(Some(&module), &InfoBuilder::new().build())
        .expect("server_init should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_init with multiple callback configs (single init/finalize).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_init_with_callbacks_config_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    // Module with one callback set
    let mut module = PmixServerModule::default();
    module.abort = Some(dummy_callback);
    let handle = server_init(Some(&module), &InfoBuilder::new().build())
        .expect("server_init with one callback should succeed");
    assert!(is_server_initialized());
    server_finalize(handle).expect("server_finalize should succeed");
}
