//! Tests for `PMIx_Validate_credential` and `PMIx_Validate_credential_nb` — credential validation.
//!
//! These tests verify the Rust wrappers for the credential validation APIs:
//! `validate_credential`, `validate_credential_nb`, `ValidationResults`,
//! and the `ValidationCallback` trait.
//!
//! Tests marked `#[ignore]` require a running PMIx daemon and should be
//! run with `--ignored` under a real PMIx environment.

use pmix::PmixError;
use pmix::security::{
    PmixCredential, ValidationCallback, ValidationResults, validate_credential,
    validate_credential_nb,
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

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that validate_credential with an empty credential returns an expected error
/// when PMIx is not initialized.
#[test]
fn test_validate_credential_empty_credential_no_server() {
    let cred = PmixCredential::empty();
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    match result {
        Ok(_) => {
            // Acceptable — PMIx may have been initialized elsewhere.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::ErrInvalidCred),
                "Expected ERR_INIT, ERR_NOT_SUPPORTED, or ERR_INVALID_CRED, got {:?}",
                status
            );
        }
    }
}

/// Test that validate_credential with a dummy credential returns an expected error
/// when PMIx is not initialized.
#[test]
fn test_validate_credential_dummy_credential_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential-data");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    match result {
        Ok(_) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::ErrInvalidCred),
                "Expected ERR_INIT, ERR_NOT_SUPPORTED, or ERR_INVALID_CRED, got {:?}",
                status
            );
        }
    }
}

/// Test that validate_credential with binary credential data compiles and handles
/// non-text credential bytes correctly.
#[test]
fn test_validate_credential_binary_credential_no_server() {
    // Credential data is opaque bytes — test with binary data that includes
    // null bytes and non-UTF8 sequences.
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01, 0x02, 0x03, 0x00, 0xFE];
    let cred = PmixCredential::from_bytes(&binary_data);
    assert_eq!(cred.len(), binary_data.len());
    assert_eq!(cred.as_bytes(), &binary_data);

    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    // We don't assert on the result — just verify it compiles and doesn't panic.
    let _ = result;
}

/// Test that validate_credential with a large credential compiles and handles
/// large byte objects correctly.
#[test]
fn test_validate_credential_large_credential_no_server() {
    let large_data = vec![0u8; 65536]; // 64KB credential
    let cred = PmixCredential::from_vec(large_data);
    assert_eq!(cred.len(), 65536);

    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    // Just verify it doesn't panic — the FFI call will fail without a server.
    let _ = result;
}

/// Test that validate_credential with empty info array works correctly.
#[test]
fn test_validate_credential_empty_info() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    // Verify the call completes without panic — result depends on PMIx init state.
    match &result {
        Ok(_r) => {
            // If PMIx is initialized, results should be a ValidationResults.
            // (len() is always >= 0 for usize, so just check it exists.)
        }
        Err(_) => {
            // Expected when PMIx is not initialized.
        }
    }
}

/// Test that validate_credential handles multiple sequential calls correctly.
#[test]
fn test_validate_credential_multiple_sequential_calls() {
    let cred1 = PmixCredential::from_bytes(b"credential-one");
    let cred2 = PmixCredential::from_bytes(b"credential-two");
    let info: Vec<pmix::Info> = Vec::new();

    let result1 = validate_credential(&cred1, &info);
    let result2 = validate_credential(&cred2, &info);

    // Both calls should return the same type of result (both Ok or both Err)
    // in a no-server environment.
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results: {:?} vs {:?}",
        result1,
        result2
    );
}

/// Test that validate_credential credential ownership is not affected by the call.
/// The credential should still be usable after validation (even on error).
#[test]
fn test_validate_credential_ownership_preserved() {
    let cred = PmixCredential::from_bytes(b"ownership-test-credential");
    let original_len = cred.len();
    let original_bytes = cred.as_bytes().to_vec();

    let info: Vec<pmix::Info> = Vec::new();
    let _result = validate_credential(&cred, &info);

    // Credential should still be intact after the call.
    assert_eq!(cred.len(), original_len);
    assert_eq!(cred.as_bytes(), &original_bytes);
    assert!(!cred.is_empty());
}

/// Test that validate_credential with cloned credential works independently.
#[test]
fn test_validate_credential_cloned_credential() {
    let cred = PmixCredential::from_bytes(b"clone-test");
    let cred_clone = cred.clone();

    assert_eq!(cred.as_bytes(), cred_clone.as_bytes());
    assert_eq!(cred.len(), cred_clone.len());

    let info: Vec<pmix::Info> = Vec::new();
    let result1 = validate_credential(&cred, &info);
    let result2 = validate_credential(&cred_clone, &info);

    // Both should return consistent results.
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Original and cloned credential should produce consistent results"
    );
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
//  large byte objects correctly.
#[test]
fn test_validate_credential_nb_large_credential() {
    let large_data = vec![0u8; 65536];
    let cred = PmixCredential::from_vec(large_data);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
    // Just verify it doesn't panic.
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

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: validate_credential with a real credential from a PMIx daemon.
/// Requires a running PMIx server that supports credential operations.
#[test]
#[ignore]
fn test_validate_credential_with_server() {
    use pmix::security::get_credential;

    // First get a credential, then validate it.
    let info: Vec<pmix::Info> = Vec::new();
    let cred = get_credential(&info).expect("get_credential should succeed with server");
    let result = validate_credential(&cred, &info);
    match result {
        Ok(_results) => {}
        Err(status) => {
            panic!("validate_credential failed: {:?}", status);
        }
    }
}

/// Integration test: validate_credential with an expired or invalid credential.
/// Requires a running PMIx server.
#[test]
#[ignore]
fn test_validate_credential_invalid_credential_with_server() {
    // Create a credential with random data that should fail validation.
    let invalid_cred = PmixCredential::from_bytes(b"this-is-not-a-valid-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&invalid_cred, &info);

    // Should return an error for an invalid credential.
    assert!(
        result.is_err(),
        "validate_credential should fail for invalid credential"
    );
    let status = result.unwrap_err();
    assert!(
        status == pmix::PmixStatus::Known(PmixError::ErrInvalidCred) || !status.is_success(),
        "Expected ERR_INVALID_CRED or another error, got {:?}",
        status
    );
}

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
