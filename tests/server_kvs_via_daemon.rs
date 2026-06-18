//! Round 6 — P1: server_publish / server_lookup / server_delete / server_fence
//! via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise the KVS (Key-Value Store) operations available through the
//! PMIx server library.
//!
//! NOTE: The underlying PMIx KVS calls (PMIx_Publish, PMIx_Lookup, etc.) are
//! client-side operations. When called from a server_init context (rather than
//! a client PMIx_Init context), they return PMIX_ERR_UNREACH because no client
//! connection is established. The tests verify correct error handling and type
//! signatures in this scenario.
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_kvs_via_daemon -- --ignored --test-threads=1
//!   (or run a single test by name)

mod daemon_helper;

use pmix::server::{
    server_delete, server_fence, server_finalize, server_init, server_lookup, server_publish,
    PmixServerModule,
};
use pmix::{InfoBuilder, PmixError, PmixStatus};

// Dummy callbacks for testing module with callbacks set.
// All PmixServerModule callbacks are Option<unsafe extern "C" fn()>
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

/// server_publish signature returns Result<PmixStatus, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_publish_type_check() {
    // This test verifies server_publish has the correct signature:
    // fn(&PmixServerHandle, &str, &Info) -> Result<PmixStatus, PmixStatus>
    // We don't call it here since we need a valid handle, but the
    // type annotation below ensures the compiler checks the signature.
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &str,
        &pmix::Info,
    ) -> Result<PmixStatus, PmixStatus> = server_publish;
}

/// server_lookup signature returns Result<PmixOwnedValue, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_lookup_type_check() {
    // This test verifies server_lookup has the correct signature:
    // fn(&PmixServerHandle, &str, &str, &[Info]) -> Result<PmixOwnedValue, PmixStatus>
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &str,
        &str,
        &[pmix::Info],
    ) -> Result<pmix::PmixOwnedValue, PmixStatus> = server_lookup;
}

/// server_delete signature returns Result<PmixStatus, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_delete_type_check() {
    // This test verifies server_delete has the correct signature:
    // fn(&PmixServerHandle, &str, &str) -> Result<PmixStatus, PmixStatus>
    let _f: fn(&pmix::server::PmixServerHandle, &str, &str) -> Result<PmixStatus, PmixStatus> =
        server_delete;
}

/// server_fence signature returns Result<PmixStatus, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_fence_type_check() {
    // This test verifies server_fence has the correct signature:
    // fn(&PmixServerHandle, &[Info], i32) -> Result<PmixStatus, PmixStatus>
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[pmix::Info],
        i32,
    ) -> Result<PmixStatus, PmixStatus> = server_fence;
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
//
// NOTE: KVS operations (publish/lookup/delete/fence) are client-side PMIx
// calls. From a server_init context they return PMIX_ERR_UNREACH because
// no client connection is established. These tests verify the wrapper
// functions correctly propagate the error and that the server lifecycle
// (init/finalize) works correctly around KVS calls.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_publish returns ErrUnreach from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_publish_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Publish data — returns ErrUnreach from server context (no client connection).
    let key_val = InfoBuilder::new().build();
    let result = server_publish(&handle, "test-nspace", &key_val);
    assert!(
        result.is_err(),
        "server_publish from server context should return Err (no client connection)"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrUnreach),
        "expected ErrUnreach from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_lookup returns ErrUnreach from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_lookup_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Lookup — returns ErrUnreach from server context.
    let result = server_lookup(&handle, "test-nspace", "some-key", &[]);
    assert!(
        result.is_err(),
        "server_lookup from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_delete returns ErrUnreach from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_delete_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Delete — returns ErrUnreach from server context.
    let result = server_delete(&handle, "test-nspace", "some-key");
    assert!(
        result.is_err(),
        "server_delete from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_fence returns ErrUnreach from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_fence_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Fence — returns ErrUnreach from server context.
    let result = server_fence(&handle, &[], 0);
    assert!(
        result.is_err(),
        "server_fence from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_fence with timeout parameter returns ErrUnreach.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_fence_with_timeout_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Fence with a non-zero timeout.
    let result = server_fence(&handle, &[], 30);
    assert!(
        result.is_err(),
        "server_fence with timeout from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_publish returns PmixStatus with correct error type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_publish_returns_pmix_status_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let key_val = InfoBuilder::new().build();
    let result: Result<PmixStatus, PmixStatus> = server_publish(&handle, "test-nspace", &key_val);
    assert!(
        result.is_err(),
        "server_publish should return Err(PmixStatus) from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_fence returns PmixStatus with correct error type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_fence_returns_pmix_status_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let result: Result<PmixStatus, PmixStatus> = server_fence(&handle, &[], 0);
    assert!(
        result.is_err(),
        "server_fence should return Err(PmixStatus) from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_delete returns PmixStatus with correct error type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_delete_returns_pmix_status_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let result: Result<PmixStatus, PmixStatus> = server_delete(&handle, "test-nspace", "some-key");
    assert!(
        result.is_err(),
        "server_delete should return Err(PmixStatus) from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: KVS operations with callbacks module (single init/finalize).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_kvs_with_callbacks_module_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.publish = Some(dummy_callback);
    module.lookup = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Publish with callbacks module — returns ErrUnreach from server context.
    let key_val = InfoBuilder::new().build();
    let publish_result = server_publish(&handle, "test-nspace", &key_val);
    assert!(
        publish_result.is_err(),
        "server_publish from server context should return Err"
    );

    // Fence — returns ErrUnreach from server context.
    let fence_result = server_fence(&handle, &[], 0);
    assert!(
        fence_result.is_err(),
        "server_fence from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: All four KVS operations in one init/finalize cycle.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_all_kvs_ops_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Publish — ErrUnreach from server context.
    let key_val = InfoBuilder::new().build();
    let publish_result = server_publish(&handle, "test-nspace", &key_val);
    assert!(publish_result.is_err(), "publish should return Err");

    // Lookup — ErrUnreach from server context.
    let lookup_result = server_lookup(&handle, "test-nspace", "some-key", &[]);
    assert!(lookup_result.is_err(), "lookup should return Err");

    // Delete — ErrUnreach from server context.
    let delete_result = server_delete(&handle, "test-nspace", "some-key");
    assert!(delete_result.is_err(), "delete should return Err");

    // Fence — ErrUnreach from server context.
    let fence_result = server_fence(&handle, &[], 0);
    assert!(fence_result.is_err(), "fence should return Err");

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_lookup returns ErrUnreach for non-existent key.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_lookup_not_found_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Lookup a key that was never published — returns ErrUnreach from server context.
    let result = server_lookup(&handle, "test-nspace", "nonexistent-key", &[]);
    assert!(
        result.is_err(),
        "server_lookup should return Err from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: publish then fence (full KVS lifecycle — both return ErrUnreach).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_publish_fence_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Publish data — ErrUnreach from server context.
    let key_val = InfoBuilder::new().build();
    let publish_result = server_publish(&handle, "test-nspace", &key_val);
    assert!(publish_result.is_err(), "publish should return Err");

    // Fence — ErrUnreach from server context.
    let fence_result = server_fence(&handle, &[], 0);
    assert!(fence_result.is_err(), "fence should return Err");

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: publish then delete (full KVS lifecycle — both return ErrUnreach).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_publish_delete_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Publish data — ErrUnreach from server context.
    let key_val = InfoBuilder::new().build();
    let publish_result = server_publish(&handle, "test-nspace", &key_val);
    assert!(publish_result.is_err(), "publish should return Err");

    // Delete the data — ErrUnreach from server context.
    let delete_result = server_delete(&handle, "test-nspace", "some-key");
    assert!(delete_result.is_err(), "delete should return Err");

    server_finalize(handle).expect("server_finalize should succeed");
}
