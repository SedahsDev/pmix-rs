//! Round 6 — P2: server_connect / server_disconnect via prte-beast daemon.
//!
//! IMPORTANT: PMIx server state is global C-level state. Each daemon test
//! must be its own isolated test with exactly one init/finalize pair.
//! We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_connection_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::server::{
    FenceNbCallbackWrapper, PmixServerModule, server_connect, server_connect_nb, server_disconnect,
    server_disconnect_nb, server_finalize, server_init,
};
use pmix::{InfoBuilder, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_server_connect_type_check() {
    let _f: fn(&pmix::server::PmixServerHandle, &[Proc], &[pmix::Info]) -> Result<(), PmixStatus> =
        server_connect;
}

#[test]
fn test_server_disconnect_type_check() {
    let _f: fn(&pmix::server::PmixServerHandle, &[Proc], &[pmix::Info]) -> Result<(), PmixStatus> =
        server_disconnect;
}

#[test]
fn test_server_connect_nb_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[Proc],
        &[pmix::Info],
        FenceNbCallbackWrapper,
    ) -> Result<(), PmixStatus> = server_connect_nb;
}

#[test]
fn test_server_disconnect_nb_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[Proc],
        &[pmix::Info],
        FenceNbCallbackWrapper,
    ) -> Result<(), PmixStatus> = server_disconnect_nb;
}

#[test]
fn test_connect_nb_callback_constructible() {
    let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
    drop(wrapper);
}

#[test]
fn test_disconnect_nb_callback_constructible() {
    let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
    drop(wrapper);
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_connect with empty procs returns ErrBadParam.
#[test]
#[ignore = "daemon isolation"]
fn test_server_connect_empty_procs_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_connect(&handle, &[], &[]);
    assert!(
        result.is_err(),
        "server_connect with empty procs should return Err"
    );

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_connect with a proc.
#[test]
#[ignore = "daemon isolation"]
fn test_server_connect_with_proc_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("test_nspace", 0).expect("invalid nspace")];
    let result = server_connect(&handle, &procs, &[]);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_connect with info directives.
#[test]
#[ignore = "daemon isolation"]
fn test_server_connect_with_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("info_connect_test", 0).expect("invalid nspace")];
    let connect_info = vec![InfoBuilder::new().build()];
    let result = server_connect(&handle, &procs, &connect_info);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_connect_nb accepts callback.
#[test]
#[ignore = "daemon isolation"]
fn test_server_connect_nb_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("nb_connect_test", 0).expect("invalid nspace")];
    let wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {});
    let result = server_connect_nb(&handle, &procs, &[], wrapper);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_disconnect with empty procs returns ErrBadParam.
#[test]
#[ignore = "daemon isolation"]
fn test_server_disconnect_empty_procs_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let result = server_disconnect(&handle, &[], &[]);
    assert!(
        result.is_err(),
        "server_disconnect with empty procs should return Err"
    );

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_disconnect with a proc.
#[test]
#[ignore = "daemon isolation"]
fn test_server_disconnect_with_proc_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("disconnect_test", 0).expect("invalid nspace")];
    let result = server_disconnect(&handle, &procs, &[]);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_disconnect with info directives.
#[test]
#[ignore = "daemon isolation"]
fn test_server_disconnect_with_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("info_disconnect_test", 0).expect("invalid nspace")];
    let disconnect_info = vec![InfoBuilder::new().build()];
    let result = server_disconnect(&handle, &procs, &disconnect_info);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: server_disconnect_nb accepts callback.
#[test]
#[ignore = "daemon isolation"]
fn test_server_disconnect_nb_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("nb_disconnect_test", 0).expect("invalid nspace")];
    let wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {});
    let result = server_disconnect_nb(&handle, &procs, &[], wrapper);
    let _ = result;

    server_finalize(handle).expect("server_finalize");
}

/// Daemon: connect then disconnect cycle.
#[test]
#[ignore = "daemon isolation"]
fn test_server_connect_disconnect_cycle_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let procs = vec![Proc::new("connect_disconnect_cycle", 0).expect("invalid nspace")];

    // Try connect
    let connect_result = server_connect(&handle, &procs, &[]);
    let _ = connect_result;

    // Try disconnect
    let disconnect_result = server_disconnect(&handle, &procs, &[]);
    let _ = disconnect_result;

    server_finalize(handle).expect("server_finalize");
}
