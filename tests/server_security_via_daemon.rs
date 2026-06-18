//! Round 6 — P3: server_get_credential via prte-beast daemon.
//!
//! These tests connect to the running prte-beast daemon using PMIX_SERVER_URI
//! to exercise server_get_credential from a server context.
//!
//! NOTE: server_get_credential delegates to crate::security::get_credential,
//! which wraps PMIx_Get_credential. Without proper authentication setup
//! (no client credential to retrieve), this returns an error. The tests
//! verify correct error handling and type signatures in this scenario.
//!
//! IMPORTANT: PMIx server state is global C-level state. Calling server_init
//! and server_finalize multiple times in the same process causes double-free
//! crashes. Each daemon test must be its own isolated test with exactly one
//! init/finalize pair. We use #[ignore] to force running them individually.
//!
//! Run individually:
//!   cargo test --test server_security_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::server::{server_finalize, server_get_credential, server_init, PmixServerModule};
use pmix::{InfoBuilder, PmixStatus};

// Dummy callbacks for testing module with callbacks set.
// All PmixServerModule callbacks are Option<unsafe extern "C" fn()>
extern "C" fn dummy_callback() {}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (always run — verify compile-time type correctness)
// ─────────────────────────────────────────────────────────────────────────────

/// server_get_credential signature returns Result<PmixCredential, PmixStatus>.
/// (Compile-time type check — verifies the function is callable.)
#[test]
fn test_server_get_credential_type_check() {
    let _f: fn(
        &pmix::server::PmixServerHandle,
        &[pmix::Info],
    ) -> Result<pmix::security::PmixCredential, PmixStatus> = server_get_credential;
}

/// PmixCredential is constructible as a type reference.
#[test]
fn test_pmix_credential_type_exists() {
    let _ = std::any::type_name::<pmix::security::PmixCredential>();
}

/// PmixCredential::empty() is constructible.
#[test]
fn test_pmix_credential_empty_constructible() {
    let cred = pmix::security::PmixCredential::empty();
    assert!(cred.as_bytes().is_empty(), "empty credential should have no bytes");
}

/// PmixCredential::from_bytes is constructible.
#[test]
fn test_pmix_credential_from_bytes() {
    let data = vec![1u8, 2, 3, 4];
    let cred = pmix::security::PmixCredential::from_bytes(&data);
    assert_eq!(cred.as_bytes(), &data[..]);
}

/// PmixCredential::from_vec is constructible.
#[test]
fn test_pmix_credential_from_vec() {
    let data = vec![5u8, 6, 7, 8];
    let cred = pmix::security::PmixCredential::from_vec(data);
    assert_eq!(cred.as_bytes(), &[5u8, 6, 7, 8]);
}

/// PmixCredential is Clone.
#[test]
fn test_pmix_credential_clone() {
    let cred = pmix::security::PmixCredential::empty();
    let cloned = cred.clone();
    assert_eq!(cloned.as_bytes(), cred.as_bytes());
}

/// PmixCredential is Debug.
#[test]
fn test_pmix_credential_debug() {
    let cred = pmix::security::PmixCredential::empty();
    let debug_str = format!("{:?}", cred);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

/// PmixCredential is Debug (with data).
#[test]
fn test_pmix_credential_debug_with_data() {
    let cred = pmix::security::PmixCredential::from_bytes(&[1u8, 2, 3]);
    let debug_str = format!("{:?}", cred);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — each does exactly ONE init/finalize cycle.
// Run individually with --ignored to avoid C-level state corruption.
//
// NOTE: server_get_credential wraps PMIx_Get_credential. Without proper
// authentication setup (no client credential to retrieve), it returns an
// error. These tests verify the wrapper function correctly propagates the
// error and that the server lifecycle (init/finalize) works correctly around
// the call.
// ─────────────────────────────────────────────────────────────────────────────

/// Daemon: server_get_credential returns error without auth setup.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_get_credential_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Get credential — returns error without auth setup.
    let result = server_get_credential(&handle, &[]);
    assert!(
        result.is_err(),
        "server_get_credential without auth should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_get_credential with info directives returns error.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_get_credential_with_info_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // Get credential with info directives — still returns error without auth.
    let cred_info = vec![InfoBuilder::new().build()];
    let result = server_get_credential(&handle, &cred_info);
    assert!(
        result.is_err(),
        "server_get_credential with info from server context should return Err"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_get_credential returns correct Result type.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_get_credential_returns_credential_type_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let result: Result<pmix::security::PmixCredential, PmixStatus> =
        server_get_credential(&handle, &[]);
    assert!(
        result.is_err(),
        "server_get_credential should return Err(PmixStatus) without auth"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: server_get_credential with callbacks module returns error.
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_get_credential_with_callbacks_module_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let mut module = PmixServerModule::default();
    module.abort = Some(dummy_callback);
    module.fence_nb = Some(dummy_callback);

    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    let result = server_get_credential(&handle, &[]);
    assert!(
        result.is_err(),
        "server_get_credential with callbacks module should return Err without auth"
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Daemon: Multiple get_credential attempts in one init/finalize cycle (all error).
#[test]
#[ignore = "daemon isolation — one init/finalize per test"]
fn test_server_get_credential_multiple_attempts_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("daemon available");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed with daemon");

    // First attempt — empty info.
    let result1 = server_get_credential(&handle, &[]);
    assert!(result1.is_err(), "first get_credential should return Err");

    // Second attempt — with info directives.
    let cred_info = vec![InfoBuilder::new().build()];
    let result2 = server_get_credential(&handle, &cred_info);
    assert!(result2.is_err(), "second get_credential should return Err");

    // Third attempt — with multiple info directives.
    let cred_info2 = vec![InfoBuilder::new().build(), InfoBuilder::new().build()];
    let result3 = server_get_credential(&handle, &cred_info2);
    assert!(result3.is_err(), "third get_credential should return Err");

    server_finalize(handle).expect("server_finalize should succeed");
}
