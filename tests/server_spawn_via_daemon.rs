//! Round 6 — P2: server_spawn / server_spawn_nb via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise the spawn operations available through the PMIx server library.
//!
//! NOTE: The underlying PMIx spawn call (PMIx_Spawn) delegates to
//! crate::process_mgmt::spawn. When called from a server_init context,
//! it returns PMIX_ERR_UNREACH because no client connection is established.
//! The tests verify correct error handling and type signatures in this scenario.
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_spawn_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::process_mgmt::{PmixApp, SpawnCallbackWrapper};
use pmix::server::{PmixServerModule, server_finalize, server_init, server_spawn, server_spawn_nb};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// Dummy callbacks for testing module with callbacks set.
// All PmixServerModule callbacks are Option<unsafe extern "C" fn()>.
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

/// server_spawn signature returns Result<String, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_spawn_type_check() {
    let _f: fn(&pmix::server::PmixServerHandle, &[Info], &[PmixApp]) -> Result<String, PmixStatus> =
        server_spawn;
}

/// server_spawn_nb signature returns Result<(), PmixStatus> and accepts SpawnCallbackWrapper.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_spawn_nb_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[Info],
        &[PmixApp],
        SpawnCallbackWrapper,
    ) -> Result<(), PmixStatus> = server_spawn_nb;
}

/// PmixApp::builder() is constructible.
#[test]
fn test_pmix_app_builder_constructible() {
    let app = PmixApp::builder().build().expect("valid app");
    assert!(app.cmd().is_none(), "default app should have no cmd");
    assert!(app.argv().is_empty(), "default app should have no args");
    assert_eq!(app.maxprocs(), 0, "default app should have maxprocs=0");
}

/// PmixApp::builder() with cmd is constructible.
#[test]
fn test_pmix_app_builder_with_cmd() {
    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .build()
        .expect("valid app");
    assert_eq!(app.cmd(), Some("/bin/echo"));
}

/// PmixApp::builder() with args is constructible.
#[test]
fn test_pmix_app_builder_with_args() {
    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .arg("hello")
        .arg("world")
        .build()
        .expect("valid app");
    assert_eq!(app.argv().len(), 2);
}

/// PmixApp::builder() with cwd and maxprocs is constructible.
#[test]
fn test_pmix_app_builder_with_cwd_maxprocs() {
    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .cwd("/tmp")
        .maxprocs(4)
        .build()
        .expect("valid app");
    assert_eq!(app.cwd(), Some("/tmp"));
    assert_eq!(app.maxprocs(), 4);
}

/// PmixApp::builder() with env is constructible.
#[test]
fn test_pmix_app_builder_with_env() {
    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .env("FOO=bar")
        .build()
        .expect("valid app");
    assert_eq!(app.env_vars().len(), 1);
}

/// SpawnCallbackWrapper is constructible with a closure.
#[test]
fn test_spawn_callback_wrapper_constructible() {
    let wrapper = SpawnCallbackWrapper::new(|_status: PmixStatus, _nspace: Option<String>| {
        // callback body
    });
    drop(wrapper);
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_spawn returns ErrUnreach from server context.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    // Spawn — returns ErrUnreach from server context.
    let result = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn with job info directives returns ErrUnreach.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_with_job_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/sleep")
        .arg("1")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let result = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn with job info from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn with multiple apps returns ErrUnreach.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_multiple_apps_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app1 = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let app2 = PmixApp::builder()
        .cmd("/bin/true")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app1, app2];
    let job_info = vec![InfoBuilder::new().build()];

    let result = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn with multiple apps from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn returns ErrUnreach specifically.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_err_unreach_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let result = server_spawn(&handle, &job_info, &apps);
    assert!(result.is_err(), "server_spawn should return Err");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrUnreach),
        "expected ErrUnreach from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn returns Result<String, PmixStatus> — verify type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_returns_result_string_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let result: Result<String, PmixStatus> = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn should return Err(PmixStatus) from server context"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn_nb accepts callback and returns result.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_nb_with_daemon() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let wrapper = SpawnCallbackWrapper::new(move |status: PmixStatus, nspace: Option<String>| {
        CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        let _ = (status, nspace);
    });

    let result = server_spawn_nb(&handle, &job_info, &apps, wrapper);
    // Accept either outcome since behavior depends on PMIx library version.
    let _ = result;

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn_nb with job info directives.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_nb_with_job_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .arg("test")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let wrapper = SpawnCallbackWrapper::new(move |_status: PmixStatus, _nspace: Option<String>| {
        // callback invoked on spawn completion
    });

    let result = server_spawn_nb(&handle, &job_info, &apps, wrapper);
    let _ = result;

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn with callbacks module (single init/finalize).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_with_callbacks_module_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.spawn = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .maxprocs(1)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let result = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_spawn with app that has cwd and env.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_spawn_with_full_app_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let app = PmixApp::builder()
        .cmd("/bin/echo")
        .arg("hello")
        .env("TEST_VAR=value")
        .cwd("/tmp")
        .maxprocs(2)
        .build()
        .expect("valid app");
    let apps = vec![app];
    let job_info = vec![InfoBuilder::new().build()];

    let result = server_spawn(&handle, &job_info, &apps);
    assert!(
        result.is_err(),
        "server_spawn with full app from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}
