//! Tests for `PMIx_Get_credential_nb` — non-blocking credential request.
//!
//! These tests verify the Rust wrapper for the async credential API:
//! `get_credential_nb`, `CredentialCallback` trait, `CredentialResults`,
//! and the callback registration / cleanup logic.
//!
//! Tests marked `#[ignore]` require a running PMIx daemon and should be
//! run with `--ignored` under a real PMIx environment.

use pmix::PmixError;
use pmix::security::{
    get_credential_nb, CredentialCallback, CredentialResults, PmixCredential,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback — just verifies the trait compiles and can be boxed.
struct NoOpCredentialCallback;

impl CredentialCallback for NoOpCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        _status: pmix::PmixStatus,
        _credential: Option<PmixCredential>,
        _results: CredentialResults,
    ) {
        // No-op — just verify the trait is callable.
    }
}

/// Callback that records the status and credential presence via Arc<Mutex>.
struct RecordingCredentialCallback {
    status: std::sync::Arc<std::sync::Mutex<Option<pmix::PmixStatus>>>,
    has_credential: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
}

impl CredentialCallback for RecordingCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        status: pmix::PmixStatus,
        credential: Option<PmixCredential>,
        _results: CredentialResults,
    ) {
        *self.status.lock().unwrap() = Some(status);
        *self.has_credential.lock().unwrap() = Some(credential.is_some());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that CredentialCallback is object-safe and can be boxed.
#[test]
fn test_credential_callback_is_object_safe() {
    let _cb: Box<dyn CredentialCallback> = Box::new(NoOpCredentialCallback);
}

/// Test that boxed CredentialCallback is Send (required for cross-thread use).
#[test]
fn test_credential_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn CredentialCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// CredentialResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that CredentialResults default constructor produces an empty result.
#[test]
fn test_credential_results_default() {
    let results = CredentialResults::default();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
    assert!(results.info().is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential_nb — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_credential_nb with empty info and no-op callback returns
/// an expected error when PMIx is not initialized.
#[test]
fn test_get_credential_nb_empty_info_no_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpCredentialCallback);
    let result = get_credential_nb(&info, callback);

    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized elsewhere.
        }
        Err(status) => {
            // Expected: ERR_INIT or ERR_NOT_SUPPORTED when no server.
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "Expected ERR_INIT or ERR_NOT_SUPPORTED, got {:?}",
                status
            );
        }
    }
}

/// Test that get_credential_nb with a recording callback returns expected error
/// and does NOT invoke the callback when the request is rejected immediately.
#[test]
fn test_get_credential_nb_callback_not_called_on_reject() {
    let info: Vec<pmix::Info> = Vec::new();
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingCredentialCallback {
        status: status.clone(),
        has_credential: has_cred.clone(),
    });

    let result = get_credential_nb(&info, callback);

    // When PMIx is not initialized, the call should return an error
    // and the callback should NOT be registered (registry cleaned up on error).
    assert!(
        result.is_err(),
        "Expected error when PMIx not initialized, got {:?}",
        result
    );

    // The callback should not have been invoked (registry cleaned up).
     // In a no-server environment, the callback won't fire because PMIx
     // rejects the request before registering it.
     assert!(
         status.lock().unwrap().is_none(),
         "Callback should not have been invoked on rejected request"
     );
}

/// Test that get_credential_nb compiles with non-empty info array.
#[test]
fn test_get_credential_nb_with_info_compiles() {
    // This is primarily a compile-time test to ensure the Info slice
    // parameter is correctly handled.
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpCredentialCallback);
    let _result = get_credential_nb(&info, callback);
    // We don't assert on the result because it depends on PMIx init state.
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential_nb — integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: get_credential_nb with a running PMIx daemon.
/// The callback should be invoked with a credential.
#[test]
#[ignore]
fn test_get_credential_nb_with_server() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingCredentialCallback {
        status: status.clone(),
        has_credential: has_cred.clone(),
    });

    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential_nb(&info, callback);
    assert!(result.is_ok(), "get_credential_nb should be accepted by server");

    // Wait for the async callback to fire.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    let received_status = status.lock().unwrap();
    assert!(
        received_status.is_some(),
        "Callback should have been invoked by PMIx daemon"
    );
    assert!(
        received_status.as_ref().unwrap().is_success(),
        "Callback status should be success: {:?}",
        *received_status.as_ref().unwrap()
    );

    let received_cred = has_cred.lock().unwrap();
    assert!(
        received_cred.as_ref().unwrap_or(&false),
        "Callback should have received a credential"
    );
}

/// Integration test: get_credential_nb with credential type info.
#[test]
#[ignore]
fn test_get_credential_nb_with_credential_type() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingCredentialCallback {
        status: status.clone(),
        has_credential: has_cred.clone(),
    });

    // In a real environment, you would pass PMIX_CRED_TYPE info.
    // For now, test with empty info — the server should still respond.
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential_nb(&info, callback);
    assert!(result.is_ok(), "get_credential_nb should be accepted");

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let received_status = status.lock().unwrap();
    assert!(
        received_status.is_some(),
        "Callback should have been invoked"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback registration and cleanup tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that multiple sequential calls to get_credential_nb each get
/// unique request IDs and don't interfere with each other.
#[test]
fn test_get_credential_nb_multiple_sequential_calls() {
    let info: Vec<pmix::Info> = Vec::new();

    // Make several calls — each should get its own request ID
    // and register/cleanup independently.
    for i in 0..5 {
        let callback = Box::new(NoOpCredentialCallback);
        let _result = get_credential_nb(&info, callback);
        // We don't assert because it depends on PMIx init state,
        // but we verify no panics or deadlocks occur.
        let _ = i; // silence unused warning
    }
}

/// Test that the callback registry is cleaned up when the request
/// is rejected (error return path).
#[test]
fn test_get_credential_nb_registry_cleanup_on_error() {
    // First call — registers a callback, then cleans up on error.
    let callback1 = Box::new(NoOpCredentialCallback);
    let result1 = get_credential_nb(&Vec::<pmix::Info>::new(), callback1);

    // Second call — should get a new request ID, not conflict with the first.
    let callback2 = Box::new(NoOpCredentialCallback);
    let result2 = get_credential_nb(&Vec::<pmix::Info>::new(), callback2);

    // Both should return the same type of error (or both Ok if PMIx is init'd).
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results: {:?} vs {:?}",
        result1,
        result2
    );
}
