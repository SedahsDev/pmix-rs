//! Tests for `PMIx_Spawn` and `PMIx_Spawn_nb` via the safe `process_mgmt` module wrapper.
//!
//! Derived from C test patterns in:
//! - `test/simple/simpdyn.c` — rank 0 calls `PMIx_Spawn` after `PMIx_App_create`,
//!   sets cmd, argv, env, maxprocs, then calls spawn with NULL job_info.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::process_mgmt::{PmixApp, SpawnCallbackWrapper, spawn, spawn_nb};

// ─────────────────────────────────────────────────────────────────────────────
// PmixApp builder tests (no PMIx_Init required)
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal PmixApp with just a command.
#[test]
fn app_builder_minimal() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .build()
        .expect("build app");
    assert_eq!(app.cmd(), Some("./myapp"));
    assert!(app.argv().is_empty());
    assert!(app.env_vars().is_empty());
    assert!(app.cwd().is_none());
    assert_eq!(app.maxprocs(), 0);
}

/// Build a PmirApp with all fields set — mirrors the simpdyn.c pattern.
#[test]
fn app_builder_full() {
    let app = PmixApp::builder()
        .cmd("./simpclient")
        .arg("simpclient")
        .arg("-n")
        .arg("2")
        .env("PMIX_ENV_VALUE=3")
        .cwd("/tmp")
        .maxprocs(2)
        .build()
        .expect("build app");

    assert_eq!(app.cmd(), Some("./simpclient"));
    assert_eq!(app.argv().len(), 3);
    assert_eq!(app.argv()[0].to_str().unwrap(), "simpclient");
    assert_eq!(app.argv()[1].to_str().unwrap(), "-n");
    assert_eq!(app.argv()[2].to_str().unwrap(), "2");
    assert_eq!(app.env_vars().len(), 1);
    assert_eq!(app.env_vars()[0].to_str().unwrap(), "PMIX_ENV_VALUE=3");
    assert_eq!(app.cwd(), Some("/tmp"));
    assert_eq!(app.maxprocs(), 2);
}

/// Builder with multiple args via `args()`.
#[test]
fn app_builder_args() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .args(vec!["a1".to_string(), "a2".to_string(), "a3".to_string()])
        .build()
        .expect("build app");
    assert_eq!(app.argv().len(), 3);
}

/// Builder with multiple env vars via `envs()`.
#[test]
fn app_builder_envs() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .envs(vec!["FOO=bar".to_string(), "BAZ=qux".to_string()])
        .build()
        .expect("build app");
    assert_eq!(app.env_vars().len(), 2);
}

/// Builder rejects NUL bytes in command path.
#[test]
fn app_builder_rejects_nul_in_cmd() {
    let result = PmixApp::builder().cmd("./my\0app").build();
    assert!(result.is_err(), "should reject NUL in cmd");
}

/// Builder rejects NUL bytes in arguments.
#[test]
fn app_builder_rejects_nul_in_arg() {
    let result = PmixApp::builder().cmd("./myapp").arg("a\0rg").build();
    assert!(result.is_err(), "should reject NUL in arg");
}

/// Builder rejects NUL bytes in environment variables.
#[test]
fn app_builder_rejects_nul_in_env() {
    let result = PmixApp::builder().cmd("./myapp").env("KEY=\0VALUE").build();
    assert!(result.is_err(), "should reject NUL in env");
}

/// Builder rejects NUL bytes in working directory.
#[test]
fn app_builder_rejects_nul_in_cwd() {
    let result = PmixApp::builder().cmd("./myapp").cwd("/tmp/\0dir").build();
    assert!(result.is_err(), "should reject NUL in cwd");
}

// ─────────────────────────────────────────────────────────────────────────────
// spawn() without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `spawn` without `PMIx_Init` must return an error rather than
/// panic or segfault — the library should detect that it is not initialized.
///
/// Derived from `test/simple/simpdyn.c` — the C test calls PMIx_Spawn
/// only after PMIx_Init, so calling it without init is an error path.
#[test]
fn spawn_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .maxprocs(1)
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(
        result.is_err(),
        "spawn without PMIx_Init should fail, got {:?}\n\
         NOTE: if this returns Ok, it means PMIx_Spawn succeeded without\n\
         PMIx_Init — that would indicate unexpected library behavior.",
        result
    );
}

/// Spawn with empty app list should return PMIX_ERR_BAD_PARAM.
#[test]
fn spawn_empty_apps_returns_bad_param() {
    let result = spawn(&[], &[]);
    assert!(result.is_err(), "spawn with empty apps should fail");
}

/// Spawn with multiple apps without init — should fail.
#[test]
fn spawn_multiple_apps_without_init_fails() {
    let app1 = PmixApp::builder()
        .cmd("./app1")
        .maxprocs(1)
        .build()
        .expect("build app1");
    let app2 = PmixApp::builder()
        .cmd("./app2")
        .maxprocs(2)
        .build()
        .expect("build app2");
    let result = spawn(&[], &[app1, app2]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with full app descriptor (cmd + argv + env + cwd + maxprocs).
#[test]
fn spawn_full_app_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./simpclient")
        .arg("simpclient")
        .arg("-n")
        .arg("2")
        .env("PMIX_ENV_VALUE=3")
        .cwd("/tmp")
        .maxprocs(2)
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with app that has no maxprocs (default 0 = unlimited).
#[test]
fn spawn_default_maxprocs_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .build()
        .expect("build app");
    assert_eq!(app.maxprocs(), 0, "default maxprocs should be 0");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with app that has only env vars (no argv).
#[test]
fn spawn_env_only_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .env("PATH=/usr/bin")
        .env("LD_LIBRARY_PATH=/usr/lib")
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with app that has only argv (no env).
#[test]
fn spawn_argv_only_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .arg("--verbose")
        .arg("--count=10")
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// spawn_nb() without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `spawn_nb` without `PMIx_Init` must return an error.
#[test]
fn spawn_nb_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .maxprocs(1)
        .build()
        .expect("build app");
    let callback = SpawnCallbackWrapper::new(|_status, _nspace| {
        // This callback should not be called if the spawn fails synchronously.
    });
    let result = spawn_nb(&[], &[app], callback);
    assert!(
        result.is_err(),
        "spawn_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Spawn_nb with empty app list should return PMIX_ERR_BAD_PARAM.
#[test]
fn spawn_nb_empty_apps_returns_bad_param() {
    let callback = SpawnCallbackWrapper::new(|_status, _nspace| {});
    let result = spawn_nb(&[], &[], callback);
    assert!(result.is_err(), "spawn_nb with empty apps should fail");
}

/// Spawn_nb with a callback that captures values.
#[test]
fn spawn_nb_callback_capture() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .build()
        .expect("build app");
    let callback = SpawnCallbackWrapper::new(|_status, _nspace| {
        // Callback closure can capture external state.
        // In practice this callback won't fire because spawn_nb fails
        // without PMIx_Init, but the wrapper should still be valid.
    });
    let result = spawn_nb(&[], &[app], callback);
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn with a command path containing spaces (valid on Unix).
#[test]
fn spawn_cmd_with_spaces_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./my app")
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with a very long command path.
#[test]
fn spawn_long_cmd_path_without_init_fails() {
    let long_path = format!("/very/long/path/{}", "a".repeat(500));
    let app = PmixApp::builder()
        .cmd(&long_path)
        .build()
        .expect("build app");
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with many arguments.
#[test]
fn spawn_many_args_without_init_fails() {
    let mut builder = PmixApp::builder();
    builder.cmd("./myapp");
    for i in 0..100 {
        builder.arg(&format!("arg{}", i));
    }
    let app = builder.build().expect("build app");
    assert_eq!(app.argv().len(), 100);
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with many environment variables.
#[test]
fn spawn_many_env_without_init_fails() {
    let mut builder = PmixApp::builder();
    builder.cmd("./myapp");
    for i in 0..50 {
        builder.env(&format!("VAR{}=value{}", i, i));
    }
    let app = builder.build().expect("build app");
    assert_eq!(app.env_vars().len(), 50);
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with maxprocs set to a large value.
#[test]
fn spawn_large_maxprocs_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .maxprocs(10000)
        .build()
        .expect("build app");
    assert_eq!(app.maxprocs(), 10000);
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

/// Spawn with maxprocs set to negative value (invalid, but test the wrapper).
#[test]
fn spawn_negative_maxprocs_without_init_fails() {
    let app = PmixApp::builder()
        .cmd("./myapp")
        .maxprocs(-1)
        .build()
        .expect("build app");
    assert_eq!(app.maxprocs(), -1);
    let result = spawn(&[], &[app]);
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration test (requires PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: Init → Spawn → verify namespace.
///
/// This test mirrors `test/simple/simpdyn.c`:
/// 1. PMIx_Init
/// 2. PMIx_App_create(app, 1)
/// 3. Set app->cmd = "./simpclient", app->maxprocs = 2
/// 4. Add argv and env
/// 5. PMIx_Spawn(NULL, 0, app, 1, nsp2)
/// 6. PMIx_App_free(app, 1)
/// 7. Verify returned namespace
///
/// NOTE: This test requires a running PMIx server and must be run
/// under `pmixrun` or equivalent. It is ignored by default.
///
/// ```sh
/// pmixrun -n 2 -- cargo test --test process_mgmt_Spawn spawn_integration -- --include-ignored
/// ```
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn spawn_integration() {
    // This would require PMIx_Init which needs a daemon.
    // In a real integration test environment:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Build PmixApp with cmd = "./simpclient", maxprocs = 2
    // 3. Call spawn(&[], &[app])
    // 4. Verify returned namespace is non-empty
    // 5. Optionally PMIx_Get the job size of the spawned namespace
    //
    // Because PMIx_Spawn blocks until the applications are launched,
    // we can verify the result synchronously.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Non-blocking spawn integration test.
///
/// Mirrors the _nb variant pattern:
/// 1. PMIx_Init
/// 2. Build PmixApp
/// 3. Call spawn_nb with a callback that captures the namespace
/// 4. Verify the callback was invoked with the expected status
///
/// Ignored by default — requires PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn spawn_nb_integration() {
    // In a real integration test:
    //
    // 1. Use a std::sync::Arc<Mutex<>> to share state with the callback.
    // 2. Call spawn_nb(&[], &[app], callback).
    // 3. Wait for the callback to be invoked (e.g., with a Condvar).
    // 4. Verify the callback received PMIX_SUCCESS and a valid namespace.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
