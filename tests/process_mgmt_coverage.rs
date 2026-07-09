//! Additional structural coverage for process_mgmt module — TASK-032.
//!
//! Focus on pure-Rust code paths that don't require PMIx_Init:
//! - PmixAppBuilder edge cases and accessor tests
//! - spawn/spawn_nb validation (empty apps rejection)
//! - connect/disconnect validation (empty procs rejection)
//! - Callback wrapper construction and trait bounds
//! - PmixApp field accessor edge cases
//!
//! Tests that require FFI calls without PMIx_Init are excluded —
//! the PMIx library may segfault rather than return an error code
//! when called without initialization.

use pmix::process_mgmt::*;
use pmix::{InfoBuilder, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// spawn — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_empty_apps_rejected() {
    let result = spawn(&[], &[]);
    assert!(
        result.is_err(),
        "spawn with empty apps should fail at validation"
    );
}

#[test]
fn test_spawn_empty_apps_with_info_rejected() {
    let info = vec![InfoBuilder::new().build()];
    let result = spawn(&info, &[]);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// spawn_nb — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_nb_empty_apps_rejected() {
    let cb = SpawnCallbackWrapper::new(|_s, _n| {});
    let result = spawn_nb(&[], &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// connect — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_connect_empty_procs_rejected() {
    let result = connect(&[], &[]);
    assert!(
        result.is_err(),
        "connect with empty procs should fail at validation"
    );
}

#[test]
fn test_connect_empty_procs_with_info_rejected() {
    let info = vec![InfoBuilder::new().build()];
    let result = connect(&[], &info);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// disconnect — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_empty_procs_rejected() {
    let result = disconnect(&[], &[]);
    assert!(
        result.is_err(),
        "disconnect with empty procs should fail at validation"
    );
}

#[test]
fn test_disconnect_empty_procs_with_info_rejected() {
    let info = vec![InfoBuilder::new().build()];
    let result = disconnect(&[], &info);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// connect_nb — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_connect_nb_empty_procs_rejected() {
    let cb = ConnectCallbackWrapper::new(|_| {});
    let result = connect_nb(&[], &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// disconnect_nb — validation tests (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_nb_empty_procs_rejected() {
    let cb = DisconnectCallbackWrapper::new(|_| {});
    let result = disconnect_nb(&[], &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrappers — construction and trait bounds
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_callback_wrapper_basic() {
    let _cb = SpawnCallbackWrapper::new(|_status, _nspace| {});
}

#[test]
fn test_spawn_callback_wrapper_with_state() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _cb = SpawnCallbackWrapper::new(move |_status, _nspace| {
        called_clone.store(true, Ordering::SeqCst);
    });
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn test_connect_callback_wrapper_basic() {
    let _cb = ConnectCallbackWrapper::new(|_status| {});
}

#[test]
fn test_disconnect_callback_wrapper_basic() {
    let _cb = DisconnectCallbackWrapper::new(|_status| {});
}

#[test]
fn test_spawn_callback_wrapper_send() {
    fn assert_send<T: Send>() {}
    assert_send::<SpawnCallbackWrapper>();
}

#[test]
fn test_connect_callback_wrapper_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ConnectCallbackWrapper>();
}

#[test]
fn test_disconnect_callback_wrapper_send() {
    fn assert_send<T: Send>() {}
    assert_send::<DisconnectCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixAppBuilder — edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_builder_empty_then_full() {
    // Build empty, then build full — verify builder is reusable concept
    let empty = PmixApp::builder().build().expect("empty build");
    assert!(empty.cmd().is_none());

    let full = PmixApp::builder()
        .cmd("/bin/test")
        .arg("--flag")
        .env("FOO=bar")
        .cwd("/tmp")
        .maxprocs(4)
        .build()
        .expect("full build");
    assert_eq!(full.cmd(), Some("/bin/test"));
}

#[test]
fn test_builder_args_then_args() {
    let app = PmixApp::builder()
        .args(vec!["a".to_string(), "b".to_string()])
        .args(vec!["c".to_string(), "d".to_string()])
        .build()
        .expect("build");
    assert_eq!(app.argv().len(), 4);
}

#[test]
fn test_builder_envs_then_envs() {
    let app = PmixApp::builder()
        .envs(vec!["A=1".to_string()])
        .envs(vec!["B=2".to_string()])
        .build()
        .expect("build");
    assert_eq!(app.env_vars().len(), 2);
}

#[test]
fn test_builder_args_empty_iterator() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .args(Vec::<String>::new())
        .build()
        .expect("build");
    assert!(app.argv().is_empty());
}

#[test]
fn test_builder_envs_empty_iterator() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .envs(Vec::<String>::new())
        .build()
        .expect("build");
    assert!(app.env_vars().is_empty());
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

// ─────────────────────────────────────────────────────────────────────────────
// PmixApp — accessor tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_app_cmd_returns_none_for_empty_builder() {
    let app = PmixApp::builder().build().expect("build");
    assert_eq!(app.cmd(), None);
}

#[test]
fn test_app_argv_empty_for_no_args() {
    let app = PmixApp::builder().cmd("/bin/test").build().expect("build");
    assert!(app.argv().is_empty());
}

#[test]
fn test_app_env_vars_empty_for_no_env() {
    let app = PmixApp::builder().cmd("/bin/test").build().expect("build");
    assert!(app.env_vars().is_empty());
}

#[test]
fn test_app_maxprocs_default_zero() {
    let app = PmixApp::builder().build().expect("build");
    assert_eq!(app.maxprocs(), 0);
}

#[test]
fn test_app_maxprocs_large() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .maxprocs(10000)
        .build()
        .expect("build");
    assert_eq!(app.maxprocs(), 10000);
}

#[test]
fn test_app_maxprocs_negative() {
    let app = PmixApp::builder()
        .cmd("/bin/test")
        .maxprocs(-1)
        .build()
        .expect("build");
    assert_eq!(app.maxprocs(), -1);
}

#[test]
fn test_app_builder_returns_mut_self() {
    let _app = PmixApp::builder()
        .cmd("/bin/test")
        .arg("a")
        .arg("b")
        .env("X=1")
        .cwd("/tmp")
        .maxprocs(2)
        .build()
        .expect("build");
}

#[test]
fn test_app_debug_format() {
    let app = PmixApp::builder().cmd("/bin/test").build().expect("build");
    let debug = format!("{:?}", app);
    assert!(!debug.is_empty());
}

#[test]
fn test_builder_debug_format() {
    let builder = PmixAppBuilder::new();
    let debug = format!("{:?}", builder);
    assert!(!debug.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc — construction tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_new_valid() {
    let proc = Proc::new("test_namespace", 0).expect("valid proc");
    let _ = proc;
}

#[test]
fn test_proc_new_wildcard_rank() {
    let proc = Proc::new("test_namespace", pmix::RANK_WILDCARD).expect("valid proc");
    let _ = proc;
}

#[test]
fn test_proc_new_nul_in_namespace() {
    let result = Proc::new("bad\x00ns", 0);
    assert!(result.is_err());
}

#[test]
fn test_proc_type_name() {
    let name = std::any::type_name::<pmix::Proc>();
    assert!(name.contains("Proc"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety — pure Rust paths only
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
fn test_spawn_nb_does_not_panic_on_empty_apps() {
    let result = std::panic::catch_unwind(|| {
        let cb = SpawnCallbackWrapper::new(|_s, _n| {});
        let _ = spawn_nb(&[], &[], cb);
    });
    assert!(result.is_ok());
}
