//! Deep tests for process_mgmt module — Round 2.
//!
//! Targets untested code paths in process_mgmt.rs (72.45% coverage).
//! Focus: PmixAppBuilder complex scenarios, spawn validation, connect/disconnect,
//! resolve_peers/nodes, abort, panic safety.

use pmix::process_mgmt::*;
use pmix::{InfoBuilder, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// PmixAppBuilder — construction edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_builder_default() {
    let builder = PmixAppBuilder::new();
    let app = builder.build().expect("empty builder builds");
    assert!(app.cmd().is_none());
    assert!(app.argv().is_empty());
    assert!(app.env_vars().is_empty());
    assert!(app.cwd().is_none());
    assert_eq!(app.maxprocs(), 0);
}

#[test]
fn test_builder_full() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .arg("--flag")
        .arg("value")
        .env("FOO=bar")
        .cwd("/tmp")
        .maxprocs(4)
        .build()
        .expect("build app");
    assert_eq!(app.cmd(), Some("/bin/test"));
    assert_eq!(app.argv().len(), 2);
    assert_eq!(app.env_vars().len(), 1);
    assert_eq!(app.cwd(), Some("/tmp"));
    assert_eq!(app.maxprocs(), 4);
}

#[test]
fn test_builder_cmd_only() {
    let app = PmixApp::builder()
        .cmd("/usr/bin/echo")
        .build()
        .expect("build");
    assert_eq!(app.cmd(), Some("/usr/bin/echo"));
    assert!(app.argv().is_empty());
}

#[test]
fn test_builder_args_batch() {
    let app = PmixApp::builder()
        .args(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        .build()
        .expect("build");
    assert_eq!(app.argv().len(), 3);
}

#[test]
fn test_builder_envs_batch() {
    let app = PmixApp::builder()
        .envs(vec!["A=1".to_string(), "B=2".to_string()])
        .build()
        .expect("build");
    assert_eq!(app.env_vars().len(), 2);
}

#[test]
fn test_builder_nul_in_cmd() {
    let result = PmixApp::builder().cmd("bad\x00cmd").build();
    assert!(result.is_err());
}

#[test]
fn test_builder_nul_in_arg() {
    let result = PmixApp::builder()
        .cmd("/bin/test")
        .arg("bad\x00arg")
        .build();
    assert!(result.is_err());
}

#[test]
fn test_builder_nul_in_env() {
    let result = PmixApp::builder()
        .cmd("/bin/test")
        .env("KEY=\x00value")
        .build();
    assert!(result.is_err());
}

#[test]
fn test_builder_nul_in_cwd() {
    let result = PmixApp::builder()
        .cmd("/bin/test")
        .cwd("/tmp\x00dir")
        .build();
    assert!(result.is_err());
}

#[test]
fn test_builder_maxprocs_zero() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .maxprocs(0)
        .build()
        .expect("build");
    assert_eq!(app.maxprocs(), 0);
}

#[test]
fn test_builder_maxprocs_large() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .maxprocs(10000)
        .build()
        .expect("build");
    assert_eq!(app.maxprocs(), 10000);
}

#[test]
fn test_builder_maxprocs_negative() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .maxprocs(-1)
        .build()
        .expect("build");
    assert_eq!(app.maxprocs(), -1);
}

#[test]
fn test_builder_unicode_cmd() {
    let app = PmixApp::builder().cmd("/path/αβγ").build().expect("build");
    assert_eq!(app.cmd(), Some("/path/αβγ"));
}

#[test]
fn test_builder_debug_format() {
    let builder = PmixAppBuilder::new();
    let debug = format!("{:?}", builder);
    assert!(!debug.is_empty());
}

#[test]
fn test_builder_multiple_args_and_envs() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .arg("first")
        .args(vec!["second".to_string(), "third".to_string()])
        .arg("fourth")
        .env("X=1")
        .envs(vec!["Y=2".to_string(), "Z=3".to_string()])
        .env("W=4")
        .build()
        .expect("build");
    assert_eq!(app.argv().len(), 4);
    assert_eq!(app.env_vars().len(), 4);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixApp — field accessors
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_app_debug_format() {
    let app = PmixApp::builder().cmd("/bin/test").build().expect("build");
    let debug = format!("{:?}", app);
    assert!(!debug.is_empty());
}

#[test]
fn test_app_no_cmd() {
    let app = PmixApp::builder().build().expect("build");
    assert!(app.cmd().is_none());
    assert!(app.cwd().is_none());
}

#[test]
fn test_app_argv_returns_slice() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .arg("arg1")
        .build()
        .expect("build");
    let _argv: &[std::ffi::CString] = app.argv();
}

#[test]
fn test_app_env_vars_returns_slice() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .env("A=1")
        .build()
        .expect("build");
    let _env: &[std::ffi::CString] = app.env_vars();
}

// ─────────────────────────────────────────────────────────────────────────────
// spawn — validation tests (no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_empty_apps_rejected() {
    let result = spawn(&[], &[]);
    assert!(result.is_err());
}

#[test]
fn test_spawn_empty_apps_with_info_rejected() {
    let info = vec![InfoBuilder::new().build()];
    let result = spawn(&info, &[]);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// spawn_nb — validation tests (no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_nb_empty_apps_rejected() {
    let cb = SpawnCallbackWrapper::new(|_s, _n| {});
    let result = spawn_nb(&[], &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// connect — validation tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_connect_empty_procs_rejected() {
    let result = connect(&[], &[]);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// disconnect — validation tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_empty_procs_rejected() {
    let result = disconnect(&[], &[]);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrappers — compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_callback_wrapper() {
    let _cb = SpawnCallbackWrapper::new(|_status, _nspace| {});
}

#[test]
fn test_connect_callback_wrapper() {
    let _cb = ConnectCallbackWrapper::new(|_status| {});
}

#[test]
fn test_disconnect_callback_wrapper() {
    let _cb = DisconnectCallbackWrapper::new(|_status| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_does_not_panic_on_empty_apps() {
    let result = std::panic::catch_unwind(|| {
        let _ = spawn(&[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_connect_does_not_panic_on_empty_procs() {
    let result = std::panic::catch_unwind(|| {
        let _ = connect(&[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_disconnect_does_not_panic_on_empty_procs() {
    let result = std::panic::catch_unwind(|| {
        let _ = disconnect(&[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_resolve_peers_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _ = resolve_peers(None, None);
    });
    assert!(result.is_ok());
}

#[test]
fn test_resolve_nodes_does_not_panic_on_empty() {
    let result = std::panic::catch_unwind(|| {
        let _ = resolve_nodes("");
    });
    assert!(result.is_ok());
}

#[test]
fn test_abort_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _ = abort(PmixStatus::from_raw(-1), None, None);
    });
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

// ── spawn ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_spawn_single_app() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let app = PmixApp::builder()
        .cmd("/bin/true")
        .maxprocs(1)
        .build()
        .expect("build");
    let result = spawn(&[], &[app]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_spawn_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let app = PmixApp::builder().cmd("/bin/true").build().expect("build");
    let info = vec![InfoBuilder::new().build()];
    let result = spawn(&info, &[app]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_spawn_multiple_apps() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let app1 = PmixApp::builder()
        .cmd("/bin/true")
        .maxprocs(1)
        .build()
        .expect("build");
    let app2 = PmixApp::builder()
        .cmd("/bin/false")
        .maxprocs(1)
        .build()
        .expect("build");
    let result = spawn(&[], &[app1, app2]);
    let _ = result;
}

// ── spawn_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_spawn_nb_single_app() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let app = PmixApp::builder().cmd("/bin/true").build().expect("build");
    let cb = SpawnCallbackWrapper::new(|_s, _n| {});
    let result = spawn_nb(&[], &[app], cb);
    assert!(result.is_ok());
}

// ── connect ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_connect_single_proc() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let result = connect(&[proc], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_connect_multiple_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let p1 = Proc::new("ns", 0).expect("p1");
    let p2 = Proc::new("ns", 1).expect("p2");
    let result = connect(&[p1, p2], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_connect_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let info = vec![InfoBuilder::new().build()];
    let result = connect(&[proc], &info);
    let _ = result;
}

// ── connect_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_connect_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let cb = ConnectCallbackWrapper::new(|_| {});
    let result = connect_nb(&[proc], &[], cb);
    assert!(result.is_ok());
}

// ── disconnect ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_disconnect_single_proc() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let result = disconnect(&[proc], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_disconnect_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let info = vec![InfoBuilder::new().build()];
    let result = disconnect(&[proc], &info);
    let _ = result;
}

// ── disconnect_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_disconnect_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let cb = DisconnectCallbackWrapper::new(|_| {});
    let result = disconnect_nb(&[proc], &[], cb);
    assert!(result.is_ok());
}

// ── resolve_peers ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_resolve_peers_no_args() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = resolve_peers(None, None);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_resolve_peers_with_nodename() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = resolve_peers(Some("localhost"), None);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_resolve_peers_with_nspace() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = resolve_peers(None, Some("test_ns"));
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_resolve_peers_with_both() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = resolve_peers(Some("localhost"), Some("test_ns"));
    let _ = result;
}

// ── resolve_nodes ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_resolve_nodes_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = resolve_nodes("test_ns");
    let _ = result;
}

// ── abort ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_abort_no_msg_no_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = abort(PmixStatus::from_raw(1), None, None);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_abort_with_msg() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = abort(PmixStatus::from_raw(1), Some("test message"), None);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_abort_with_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let result = abort(PmixStatus::from_raw(1), None, Some(&[proc]));
    let _ = result;
}

// ── Lifecycle ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_spawn_connect_disconnect() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let app = PmixApp::builder()
        .cmd("/bin/true")
        .maxprocs(1)
        .build()
        .expect("build");
    let _ = spawn(&[], &[app]);
    let proc = Proc::new("ns", 0).expect("proc");
    let _ = connect(&[proc.clone()], &[]);
    let _ = disconnect(&[proc], &[]);
}
