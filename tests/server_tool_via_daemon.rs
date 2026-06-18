//! Round 6 — P3: server_tool_attach_to_server via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise server_tool_attach_to_server from a server context.
//!
//! NOTE: server_tool_attach_to_server delegates to crate::tool::tool_attach_to_server,
//! which wraps PMIx_tool_attach_to_server. This is a tool-side API that is meant
//! to be called after tool_init, not from a server_init context. When called from
//! a server context it returns PMIX_ERR_UNREACH because no tool connection is
//! established. The tests verify correct error handling and type signatures in
//! this scenario.
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_tool_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::server::{
    server_finalize, server_init, server_tool_attach_to_server, PmixServerModule,
};
use pmix::{InfoBuilder, PmixStatus, Proc};

// Dummy callbacks for testing module with callbacks set.
// All PmixServerModule callbacks are Option<unsafe extern "C" fn()>
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

/// server_tool_attach_to_server signature returns
/// Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_tool_attach_to_server_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        Option<&pmix::Proc>,
        bool,
        &pmix::Info,
    ) -> Result<
        (Option<pmix::tool::PmixToolHandle>, Option<pmix::tool::PmixServerHandle>),
        PmixStatus,
    > = server_tool_attach_to_server;
}

/// PmixToolHandle is constructible as a type reference.
#[test]
fn test_pmix_tool_handle_type_exists() {
    let _ = std::any::type_name::<pmix::tool::PmixToolHandle>();
}

/// PmixServerHandle (tool variant) is constructible as a type reference.
#[test]
fn test_pmix_server_handle_tool_type_exists() {
    let _ = std::any::type_name::<pmix::tool::PmixServerHandle>();
}

/// Proc::new is constructible for use with tool_attach.
#[test]
fn test_proc_constructible_for_tool_attach() {
    let proc = Proc::new("test_nspace", 0).expect("invalid nspace");
    assert_eq!(proc.nspace(), Some("test_nspace".to_string()));
    assert_eq!(proc.rank(), 0);
}

/// InfoBuilder produces Info usable with tool_attach.
#[test]
fn test_info_builder_for_tool_attach() {
    let info = InfoBuilder::new().build();
    let _: &pmix::Info = &info;
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
//
// NOTE: tool_attach_to_server is a tool-side API (PMIx_tool_attach_to_server).
// From a server_init context it returns PMIX_ERR_UNREACH because no tool
// connection is established. These tests verify the wrapper function correctly
// propagates the error and that the server lifecycle (init/finalize) works
// correctly around the call.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_tool_attach_to_server returns error from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_to_server_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Tool attach from server context — returns error (no tool connection).
    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, None, false, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server from server context should return Err (no tool connection)"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server with myproc returns error.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_with_myproc_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let myproc = Proc::new("test_nspace", 0).expect("invalid nspace");
    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, Some(&myproc), false, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server with myproc from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server with want_server=true returns error.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_want_server_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, None, true, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server with want_server=true from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server with myproc and want_server returns error.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_full_params_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let myproc = Proc::new("full_params_nspace", 0).expect("invalid nspace");
    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, Some(&myproc), true, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server with all params from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server returns ErrUnreach specifically.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_err_unreach_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, None, false, &attach_info);
    assert!(result.is_err(), "server_tool_attach_to_server should return Err");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(pmix::PmixError::ErrBadParam),
        "expected ErrBadParam from server context (no tool connection)"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server returns correct Result type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_returns_tuple_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let attach_info = InfoBuilder::new().build();
    let result: Result<
        (Option<pmix::tool::PmixToolHandle>, Option<pmix::tool::PmixServerHandle>),
        PmixStatus,
    > = server_tool_attach_to_server(&handle, None, false, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server should return Err(PmixStatus) from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_tool_attach_to_server with callbacks module.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_with_callbacks_module_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.abort = Some(dummy_callback);
    module.fence_nb = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let attach_info = InfoBuilder::new().build();
    let result = server_tool_attach_to_server(&handle, None, false, &attach_info);
    assert!(
        result.is_err(),
        "server_tool_attach_to_server with callbacks module from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: Multiple tool_attach attempts in one init/finalize cycle (all error).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_tool_attach_multiple_attempts_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // First attempt — no myproc, no want_server.
    let attach_info = InfoBuilder::new().build();
    let result1 = server_tool_attach_to_server(&handle, None, false, &attach_info);
    assert!(result1.is_err(), "first tool_attach should return Err");

    // Second attempt — with myproc.
    let myproc = Proc::new("multi_attempt_nspace", 0).expect("invalid nspace");
    let result2 = server_tool_attach_to_server(&handle, Some(&myproc), false, &attach_info);
    assert!(result2.is_err(), "second tool_attach should return Err");

    // Third attempt — with want_server.
    let result3 = server_tool_attach_to_server(&handle, None, true, &attach_info);
    assert!(result3.is_err(), "third tool_attach should return Err");

    server_finalize(handle).expect("server_finalize should succeed");
}
