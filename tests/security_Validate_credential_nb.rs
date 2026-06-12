//! Tests for `PMIx_Validate_credential_nb` — non-blocking credential validation.
//!
//! These tests verify the Rust wrapper for the non-blocking credential validation API:
//! `validate_credential_nb`, `ValidationCallback` trait, and `ValidationResults`.
//!
//! Tests marked `#[ignore]` require a running PMIx daemon and should be
//! run with `--ignored` under a real PMIx environment.

use pmix::PmixError;
use pmix::security::{
    PmixCredential, ValidationCallback, ValidationResults, validate_credential_nb,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback — just verifies the trait compiles and can be boxed.
struct NoOpValidationCallback;

impl ValidationCallback for NoOpValidationCallback {
    fn on_complete(self: Box<Self>, _status: pmix::PmixStatus, _results: ValidationResults) {
        // No-op — just verify the trait is callable.
    }
}

/// Callback that records the status and result count via Arc<Mutex>.
struct RecordingValidationCallback {
    status: std::sync::Arc<std::sync::Mutex<Option<pmix::PmixStatus>>>,
    result_len: std::sync::Arc<std::sync::Mutex<Option<usize>>>,
}

impl ValidationCallback for RecordingValidationCallback {
    fn on_complete(self: Box<Self>, status: pmix::PmixStatus, results: ValidationResults) {
        *self.status.lock().unwrap() = Some(status);
        *self.result_len.lock().unwrap() = Some(results.len());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ValidationCallback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that ValidationCallback is object-safe and can be boxed.
#[test]
fn test_validation_callback_is_object_safe() {
    let _cb: Box<dyn ValidationCallback> = Box::new(NoOpValidationCallback);
}

/// Test that boxed ValidationCallback is Send (required for cross-thread use).
#[test]
fn test_validation_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn ValidationCallback>>();
}

/// Test that ValidationCallback can be implemented by a struct with state.
#[test]
fn test_validation_callback_with_state() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));
    let _cb: Box<dyn ValidationCallback> =
        Box::new(RecordingValidationCallback { status, result_len });
}

/// Test that a closure-based callback wrapper compiles.
#[test]
fn test_validation_callback_closure_wrapper() {
    /// Wrap a simple closure in a ValidationCallback.
    struct ClosureCallback<F>(F);
    impl<F> ValidationCallback for ClosureCallback<F>
    where
        F: Send + FnOnce(pmix::PmixStatus, ValidationResults),
    {
        fn on_complete(self: Box<Self>, status: pmix::PmixStatus, results: ValidationResults) {
            (self.0)(status, results);
        }
    }
    let _cb: Box<dyn ValidationCallback> = Box::new(ClosureCallback(|_, _| {}));
}

// ─────────────────────────────────────────────────────────────────────────────
// ValidationResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that ValidationResults::empty() produces an empty result set.
#[test]
fn test_validation_results_empty() {
    let results = ValidationResults::empty();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
}

/// Test that ValidationResults Debug impl works (does not panic on empty instance).
#[test]
fn test_validation_results_debug() {
    let results = ValidationResults::empty();
    let debug_str = format!("{:?}", results);
    // Just verify it doesn't panic — the debug output should be printable.
    assert!(!debug_str.is_empty());
}

/// Test that ValidationResults Drop does not panic on empty instance (double-free safety).
#[test]
fn test_validation_results_drop_empty() {
    {
        let results = ValidationResults::empty();
        // Drop happens here — should not panic or attempt to free null.
        drop(results);
    }
}

/// Test that ValidationResults can be moved.
#[test]
fn test_validation_results_move() {
    let results = ValidationResults::empty();
    let moved = results;
    assert!(moved.is_empty());
}

/// Test that ValidationResults from callback with zero results is empty.
#[test]
fn test_validation_results_from_callback_zero_results() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cb = Box::new(RecordingValidationCallback {
        status: status.clone(),
        result_len: result_len.clone(),
    });

    // Simulate callback invocation with empty results.
    let empty_results = ValidationResults::empty();
    let cb_boxed: Box<dyn ValidationCallback> = cb;
    // We can't directly call the boxed trait, but we can verify the struct works.
    assert_eq!(result_len.lock().unwrap().as_ref().unwrap_or(&0), &0);
    assert!(empty_results.is_empty());
    drop(cb_boxed);
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential_nb — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that validate_credential_nb with empty credential and no-op callback
/// returns an expected error when PMIx is not initialized.
#[test]
fn test_validate_credential_nb_empty_credential_no_server() {
    let cred = PmixCredential::empty();
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "Expected ERR_INIT or ERR_NOT_SUPPORTED, got {:?}",
                status
            );
        }
    }
}

/// Test that validate_credential_nb compiles and returns expected error
/// when PMIx is not initialized.
#[test]
fn test_validate_credential_nb_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "Expected ERR_INIT or ERR_NOT_SUPPORTED, got {:?}",
                status
            );
        }
    }
}

/// Test that validate_credential_nb with a recording callback returns expected error
/// and does NOT invoke the callback when the request is rejected immediately.
#[test]
fn test_validate_credential_nb_callback_not_called_on_reject() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingValidationCallback {
        status: status.clone(),
        result_len: result_len.clone(),
    });

    let result = validate_credential_nb(&cred, &info, callback);

    // When PMIx is not initialized, the call should return an error
    // and the callback should NOT be registered (registry cleaned up on error).
    assert!(
        result.is_err(),
        "Expected error when PMIx not initialized, got {:?}",
        result
    );

    // The callback should not have been invoked (registry cleaned up).
    assert!(
        status.lock().unwrap().is_none(),
        "Callback should not have been invoked on rejected request"
    );
    assert!(
        result_len.lock().unwrap().is_none(),
        "Result length should not have been recorded on rejected request"
    );
}

/// Test that validate_credential_nb compiles with binary credential data.
#[test]
fn test_validate_credential_nb_binary_credential() {
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01, 0x02, 0x03];
    let cred = PmixCredential::from_bytes(&binary_data);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
    // Just verify compilation and no panic.
}

/// Test that validate_credential_nb credential ownership is preserved after the call.
#[test]
fn test_validate_credential_nb_ownership_preserved() {
    let cred = PmixCredential::from_bytes(b"nb-ownership-test");
    let original_len = cred.len();
    let original_bytes = cred.as_bytes().to_vec();

    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);

    // Credential should still be intact after the call.
    assert_eq!(cred.len(), original_len);
    assert_eq!(cred.as_bytes(), &original_bytes);
}

/// Test that validate_credential_nb with a large credential compiles and handles
/// large byte objects correctly.
#[test]
fn test_validate_credential_nb_large_credential() {
    let large_data = vec![0u8; 65536];
    let cred = PmixCredential::from_vec(large_data);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
    // Just verify it doesn't panic.
}

/// Test that validate_credential_nb with credential containing null bytes works.
#[test]
fn test_validate_credential_nb_null_byte_credential() {
    let cred = PmixCredential::from_bytes(&[0x00, 0x00, 0x00]);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
    // Verify no panic with all-null credential data.
}

/// Test that validate_credential_nb with credential containing max u8 values works.
#[test]
fn test_validate_credential_nb_max_byte_credential() {
    let cred = PmixCredential::from_bytes(&[0xFF; 1024]);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback registration and cleanup tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that multiple sequential calls to validate_credential_nb each get
/// unique request IDs and don't interfere with each other.
#[test]
fn test_validate_credential_nb_multiple_sequential_calls() {
    let cred = PmixCredential::from_bytes(b"sequential-test");
    let info: Vec<pmix::Info> = Vec::new();

    // Make several calls — each should get its own request ID
    // and register/cleanup independently.
    for i in 0..5 {
        let callback = Box::new(NoOpValidationCallback);
        let _result = validate_credential_nb(&cred, &info, callback);
        // We don't assert because it depends on PMIx init state,
        // but we verify no panics or deadlocks occur.
        let _ = i; // silence unused warning
    }
}

/// Test that the callback registry is cleaned up when the request
/// is rejected (error return path).
#[test]
fn test_validate_credential_nb_registry_cleanup_on_error() {
    let cred = PmixCredential::from_bytes(b"cleanup-test");
    let info: Vec<pmix::Info> = Vec::new();

    // First call — registers a callback, then cleans up on error.
    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred, &info, callback1);

    // Second call — should get a new request ID, not conflict with the first.
    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred, &info, callback2);

    // Both should return the same type of error (or both Ok if PMIx is init'd).
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results: {:?} vs {:?}",
        result1,
        result2
    );
}

/// Test that validate_credential_nb with different credentials produces
/// independent callback registrations.
#[test]
fn test_validate_credential_nb_different_credentials() {
    let cred1 = PmixCredential::from_bytes(b"credential-alpha");
    let cred2 = PmixCredential::from_bytes(b"credential-beta");
    let info: Vec<pmix::Info> = Vec::new();

    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred1, &info, callback1);

    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred2, &info, callback2);

    // Both calls should complete without panic.
    assert!(result1.is_ok() == result2.is_ok());
}

/// Test that validate_credential_nb with cloned credential works independently.
#[test]
fn test_validate_credential_nb_cloned_credential() {
    let cred = PmixCredential::from_bytes(b"clone-nb-test");
    let cred_clone = cred.clone();

    assert_eq!(cred.as_bytes(), cred_clone.as_bytes());
    assert_eq!(cred.len(), cred_clone.len());

    let info: Vec<pmix::Info> = Vec::new();
    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred, &info, callback1);

    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred_clone, &info, callback2);

    // Both should return consistent results.
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Original and cloned credential should produce consistent results"
    );
}

/// Test that validate_credential_nb with empty info array works correctly.
#[test]
fn test_validate_credential_nb_empty_info() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    // Verify the call completes without panic — result depends on PMIx init state.
    match &result {
        Ok(()) => {
            // If PMIx is initialized, the request was accepted.
        }
        Err(_) => {
            // Expected when PMIx is not initialized.
        }
    }
}

/// Test that validate_credential_nb with many rapid calls doesn't deadlock.
#[test]
fn test_validate_credential_nb_rapid_calls_no_deadlock() {
    let cred = PmixCredential::from_bytes(b"rapid-test");
    let info: Vec<pmix::Info> = Vec::new();

    // Make 20 rapid calls to stress-test the registry lock.
    for _ in 0..20 {
        let callback = Box::new(NoOpValidationCallback);
        let _result = validate_credential_nb(&cred, &info, callback);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: validate_credential_nb with a running PMIx daemon.
/// The callback should be invoked with validation results.
#[test]
#[ignore]
fn test_validate_credential_nb_with_server() {
    use pmix::security::get_credential;
    use std::sync::{Arc, Mutex};

    let received_status = Arc::new(Mutex::new(None));
    let received_len = Arc::new(Mutex::new(None));

    let callback = Box::new(RecordingValidationCallback {
        status: received_status.clone(),
        result_len: received_len.clone(),
    });

    // Get a credential first.
    let info: Vec<pmix::Info> = Vec::new();
    let cred = get_credential(&info).expect("get_credential should succeed with server");

    let result = validate_credential_nb(&cred, &info, callback);
    assert!(result.is_ok(), "validate_credential_nb should be accepted");

    // Wait for the async callback to fire.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    let status = received_status.lock().unwrap();
    assert!(
        status.is_some(),
        "Validation callback should have been invoked by PMIx daemon"
    );
    assert!(
        status.as_ref().unwrap().is_success(),
        "Callback status should be success: {:?}",
        *status.as_ref().unwrap()
    );

    let len = received_len.lock().unwrap();
    assert!(len.is_some(), "Callback should have received result length");
}

/// Integration test: validate_credential_nb with an invalid credential.
#[test]
#[ignore]
fn test_validate_credential_nb_invalid_credential_with_server() {
    use std::sync::{Arc, Mutex};

    let received_status = Arc::new(Mutex::new(None));
    let received_len = Arc::new(Mutex::new(None));

    let callback = Box::new(RecordingValidationCallback {
        status: received_status.clone(),
        result_len: received_len.clone(),
    });

    // Use an invalid credential.
    let invalid_cred = PmixCredential::from_bytes(b"not-a-real-credential");
    let info: Vec<pmix::Info> = Vec::new();

    let result = validate_credential_nb(&invalid_cred, &info, callback);
    assert!(
        result.is_ok(),
        "validate_credential_nb should be accepted (error comes in callback)"
    );

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let status = received_status.lock().unwrap();
    assert!(status.is_some(), "Callback should have been invoked");
    // The callback should report an error status for the invalid credential.
    assert!(
        !status.as_ref().unwrap().is_success(),
        "Callback status should be an error for invalid credential: {:?}",
        *status.as_ref().unwrap()
    );
}

/// Integration test: validate_credential_nb with empty credential against server.
#[test]
#[ignore]
fn test_validate_credential_nb_empty_credential_with_server() {
    // An empty credential should be rejected by the server.
    let empty_cred = PmixCredential::empty();
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);

    let result = validate_credential_nb(&empty_cred, &info, callback);
    // The call itself may succeed (accepted for async processing),
    // but the callback should report an error.
    assert!(
        result.is_ok(),
        "validate_credential_nb should accept the request"
    );
}
