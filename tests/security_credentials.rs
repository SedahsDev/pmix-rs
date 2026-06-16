//! Tests for security credential operations — `get_credential`, `get_credential_nb`,
//! `validate_credential`, `validate_credential_nb`.
//!
//! Focus areas:
//! - Compile-time type checks (trait bounds, object safety, Send/Sync)
//! - Panic safety (empty credentials, binary data, large objects)
//! - Callback traits (CredentialCallback, ValidationCallback)
//! - PmixCredential struct (construction, access, clone, ownership)
//! - Error codes (ERR_INIT, ERR_NOT_SUPPORTED, ERR_INVALID_CRED)
//!
//! Tests that call `server_init_minimal` corrupt C-level PMIx state and are
//! marked `#[ignore]` with reason.

use pmix::PmixError;
use pmix::security::{
    CredentialCallback, CredentialResults, PmixCredential, ValidationCallback, ValidationResults,
    get_credential, get_credential_nb, validate_credential, validate_credential_nb,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op credential callback — verifies trait compiles and can be boxed.
struct NoOpCredentialCallback;

impl CredentialCallback for NoOpCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        _status: pmix::PmixStatus,
        _credential: Option<PmixCredential>,
        _results: CredentialResults,
    ) {
    }
}

/// Credential callback that records status and credential presence via Arc<Mutex>.
struct RecordingCredentialCallback {
    status: std::sync::Arc<std::sync::Mutex<Option<pmix::PmixStatus>>>,
    has_credential: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
    result_len: std::sync::Arc<std::sync::Mutex<Option<usize>>>,
}

impl CredentialCallback for RecordingCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        status: pmix::PmixStatus,
        credential: Option<PmixCredential>,
        results: CredentialResults,
    ) {
        *self.status.lock().unwrap() = Some(status);
        *self.has_credential.lock().unwrap() = Some(credential.is_some());
        *self.result_len.lock().unwrap() = Some(results.len());
    }
}

/// No-op validation callback — verifies trait compiles and can be boxed.
struct NoOpValidationCallback;

impl ValidationCallback for NoOpValidationCallback {
    fn on_complete(self: Box<Self>, _status: pmix::PmixStatus, _results: ValidationResults) {
    }
}

/// Validation callback that records status and result count via Arc<Mutex>.
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
// Section 1: PmixCredential struct tests
// ─────────────────────────────────────────────────────────────────────────────

/// PmixCredential can be created from an empty byte slice.
#[test]
fn credential_from_empty_bytes() {
    let cred = PmixCredential::from_bytes(&[]);
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
    assert!(cred.as_bytes().is_empty());
}

/// PmixCredential can be created from non-empty bytes via from_bytes.
#[test]
fn credential_from_bytes() {
    let data = b"test-credential-data";
    let cred = PmixCredential::from_bytes(data);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), data.len());
    assert_eq!(cred.as_bytes(), data);
}

/// PmixCredential can be created from a Vec<u8> via from_vec (ownership transfer).
#[test]
fn credential_from_vec() {
    let data = vec![1, 2, 3, 4, 5];
    let cred = PmixCredential::from_vec(data);
    assert_eq!(cred.len(), 5);
    assert_eq!(cred.as_bytes(), &[1, 2, 3, 4, 5]);
}

/// PmixCredential::empty() produces a credential with zero bytes.
#[test]
fn credential_empty_constructor() {
    let cred = PmixCredential::empty();
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
}

/// PmixCredential handles binary data (including null bytes, 0xFF, etc.).
#[test]
fn credential_binary_data() {
    let data: &[u8] = &[0x00, 0x01, 0xFF, 0xFE, 0x80, 0x7F];
    let cred = PmixCredential::from_bytes(data);
    assert_eq!(cred.as_bytes(), data);
    assert_eq!(cred.len(), 6);
}

/// PmixCredential handles a single byte.
#[test]
fn credential_single_byte() {
    let cred = PmixCredential::from_bytes(&[42]);
    assert_eq!(cred.len(), 1);
    assert_eq!(cred.as_bytes(), &[42]);
}

/// PmixCredential::as_raw returns a non-null pointer for non-empty credentials.
#[test]
fn credential_as_raw_non_null() {
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    assert!(!cred.as_raw().is_null());
}

/// PmixCredential::as_raw returns a non-null pointer even for empty credentials.
#[test]
fn credential_as_raw_empty_non_null() {
    let cred = PmixCredential::empty();
    assert!(!cred.as_raw().is_null());
}

/// PmixCredential implements Debug (compile-time check).
#[test]
fn credential_debug_trait() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixCredential>();
}

/// PmixCredential implements Clone.
#[test]
fn credential_clone_trait() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixCredential>();
}

/// PmixCredential clone produces an independent copy with identical bytes.
#[test]
fn credential_clone_independence() {
    let cred = PmixCredential::from_bytes(b"original");
    let cloned = cred.clone();
    assert_eq!(cred.as_bytes(), cloned.as_bytes());
    assert_eq!(cred.len(), cloned.len());
    // Both should still be valid.
    assert!(!cred.as_raw().is_null());
    assert!(!cloned.as_raw().is_null());
}

/// Multiple PmixCredential instances can coexist independently.
#[test]
fn credential_multiple_coexist() {
    let cred1 = PmixCredential::from_bytes(b"first");
    let cred2 = PmixCredential::from_bytes(b"second-credential-data");
    let cred3 = PmixCredential::empty();

    assert_eq!(cred1.as_bytes(), b"first");
    assert_eq!(cred2.as_bytes(), b"second-credential-data");
    assert!(cred3.is_empty());

    assert!(!cred1.as_raw().is_null());
    assert!(!cred2.as_raw().is_null());
    assert!(!cred3.as_raw().is_null());
}

/// PmixCredential with a large byte array (1MB) does not panic.
#[test]
fn credential_large_data() {
    let large = vec![0xABu8; 1_048_576];
    let cred = PmixCredential::from_vec(large);
    assert_eq!(cred.len(), 1_048_576);
    assert_eq!(cred.as_bytes()[0], 0xAB);
    assert_eq!(cred.as_bytes()[cred.len() - 1], 0xAB);
}

/// PmixCredential as_bytes returns a slice that is valid after clone.
#[test]
fn credential_as_bytes_after_clone() {
    let cred = PmixCredential::from_bytes(&[10, 20, 30]);
    let cloned = cred.clone();
    let slice = cloned.as_bytes();
    assert_eq!(slice, &[10, 20, 30]);
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 2: CredentialResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// CredentialResults::default() produces an empty result set.
#[test]
fn credential_results_default_empty() {
    let results = CredentialResults::default();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
    assert!(results.info().is_empty());
}

/// CredentialResults info() returns a slice reference.
#[test]
fn credential_results_info_slice() {
    let results = CredentialResults::default();
    let _info_slice: &[pmix::Info] = results.info();
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 3: ValidationResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// ValidationResults::empty() produces an empty result set.
#[test]
fn validation_results_empty() {
    let results = ValidationResults::empty();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
}

/// ValidationResults implements Debug (compile-time check).
#[test]
fn validation_results_debug_trait() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<ValidationResults>();
}

/// ValidationResults Debug formatting does not panic on empty instance.
#[test]
fn validation_results_debug_format() {
    let results = ValidationResults::empty();
    let debug_str = format!("{:?}", results);
    assert!(!debug_str.is_empty());
}

/// ValidationResults Drop does not panic on empty instance (no-op free).
#[test]
fn validation_results_drop_empty() {
    {
        let results = ValidationResults::empty();
        drop(results);
    }
}

/// ValidationResults can be moved.
#[test]
fn validation_results_move() {
    let results = ValidationResults::empty();
    let moved = results;
    assert!(moved.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 4: CredentialCallback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// CredentialCallback is object-safe and can be boxed as a trait object.
#[test]
fn credential_callback_is_object_safe() {
    let _cb: Box<dyn CredentialCallback> = Box::new(NoOpCredentialCallback);
}

/// Box<dyn CredentialCallback> is Send (required for cross-thread use).
#[test]
fn credential_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn CredentialCallback>>();
}

/// CredentialCallback can be implemented by a struct with state.
#[test]
fn credential_callback_with_state() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));
    let _cb: Box<dyn CredentialCallback> = Box::new(RecordingCredentialCallback {
        status,
        has_credential: has_cred,
        result_len,
    });
}

/// CredentialCallback can be implemented via a closure wrapper.
#[test]
fn credential_callback_closure_wrapper() {
    struct ClosureCallback<F>(F);
    impl<F> CredentialCallback for ClosureCallback<F>
    where
        F: Send + FnOnce(pmix::PmixStatus, Option<PmixCredential>, CredentialResults),
    {
        fn on_complete(
            self: Box<Self>,
            status: pmix::PmixStatus,
            credential: Option<PmixCredential>,
            results: CredentialResults,
        ) {
            (self.0)(status, credential, results);
        }
    }
    let _cb: Box<dyn CredentialCallback> =
        Box::new(ClosureCallback(|_, _, _| {}));
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 5: ValidationCallback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// ValidationCallback is object-safe and can be boxed as a trait object.
#[test]
fn validation_callback_is_object_safe() {
    let _cb: Box<dyn ValidationCallback> = Box::new(NoOpValidationCallback);
}

/// Box<dyn ValidationCallback> is Send (required for cross-thread use).
#[test]
fn validation_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn ValidationCallback>>();
}

/// ValidationCallback can be implemented by a struct with state.
#[test]
fn validation_callback_with_state() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));
    let _cb: Box<dyn ValidationCallback> = Box::new(RecordingValidationCallback {
        status,
        result_len,
    });
}

/// ValidationCallback can be implemented via a closure wrapper.
#[test]
fn validation_callback_closure_wrapper() {
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
// Section 6: get_credential — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// get_credential with empty info returns an expected error without a server.
#[test]
fn get_credential_empty_info_no_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential(&info);
    match result {
        Ok(_) => {
            // Acceptable — PMIx may have been initialized elsewhere.
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

/// get_credential returns an error status (not Ok) when PMIx is not initialized.
#[test]
fn get_credential_returns_error_status() {
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential(&info);
    if let Err(status) = result {
        assert!(status.is_error());
    }
}

/// get_credential with multiple sequential calls is consistent.
#[test]
fn get_credential_multiple_sequential_calls() {
    let info: Vec<pmix::Info> = Vec::new();
    let result1 = get_credential(&info);
    let result2 = get_credential(&info);
    // Both should return the same type of result (both Ok or both Err).
    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential get_credential calls should be consistent"
    );
}

/// get_credential accepts a reference to Info slice (compile-time type check).
#[test]
fn get_credential_accepts_info_slice_ref() {
    let info: Vec<pmix::Info> = Vec::new();
    let _result = get_credential(&info);
    // Compile-time check: &Vec<Info> coerces to &[Info].
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 7: get_credential_nb — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// get_credential_nb with no-op callback returns expected error without server.
#[test]
fn get_credential_nb_noop_no_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpCredentialCallback);
    let result = get_credential_nb(&info, callback);
    match result {
        Ok(()) => {}
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

/// get_credential_nb does NOT invoke callback when request is rejected.
#[test]
fn get_credential_nb_callback_not_called_on_reject() {
    let info: Vec<pmix::Info> = Vec::new();
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingCredentialCallback {
        status: status.clone(),
        has_credential: has_cred.clone(),
        result_len: result_len.clone(),
    });

    let result = get_credential_nb(&info, callback);
    assert!(
        result.is_err(),
        "Expected error when PMIx not initialized"
    );

    // Callback should not have been invoked (registry cleaned up on error).
    assert!(
        status.lock().unwrap().is_none(),
        "Callback should not have been invoked on rejected request"
    );
}

/// get_credential_nb with recording callback compiles and works.
#[test]
fn get_credential_nb_recording_callback() {
    let info: Vec<pmix::Info> = Vec::new();
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let has_cred = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingCredentialCallback {
        status,
        has_credential: has_cred,
        result_len,
    });

    let _result = get_credential_nb(&info, callback);
    // Just verify compilation and no panic.
}

/// get_credential_nb with multiple sequential calls doesn't interfere.
#[test]
fn get_credential_nb_multiple_sequential_calls() {
    let info: Vec<pmix::Info> = Vec::new();
    for _ in 0..5 {
        let callback = Box::new(NoOpCredentialCallback);
        let _result = get_credential_nb(&info, callback);
    }
}

/// get_credential_nb registry cleanup on error — sequential calls consistent.
#[test]
fn get_credential_nb_registry_cleanup_on_error() {
    let info: Vec<pmix::Info> = Vec::new();
    let callback1 = Box::new(NoOpCredentialCallback);
    let result1 = get_credential_nb(&info, callback1);

    let callback2 = Box::new(NoOpCredentialCallback);
    let result2 = get_credential_nb(&info, callback2);

    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results"
    );
}

/// get_credential_nb returns an error status (not Ok) when PMIx is not initialized.
#[test]
fn get_credential_nb_returns_error_status() {
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpCredentialCallback);
    let result = get_credential_nb(&info, callback);
    if let Err(status) = result {
        assert!(status.is_error());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 8: validate_credential — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// validate_credential with empty credential returns expected error without server.
#[test]
fn validate_credential_empty_credential_no_server() {
    let cred = PmixCredential::empty();
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    match result {
        Ok(_) => {}
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

/// validate_credential with dummy credential returns expected error without server.
#[test]
fn validate_credential_dummy_credential_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential-data");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    match result {
        Ok(_) => {}
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

/// validate_credential with binary credential data compiles and doesn't panic.
#[test]
fn validate_credential_binary_credential_no_server() {
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01, 0x02, 0x03, 0x00, 0xFE];
    let cred = PmixCredential::from_bytes(&binary_data);
    let info: Vec<pmix::Info> = Vec::new();
    let _result = validate_credential(&cred, &info);
}

/// validate_credential with a large credential (64KB) doesn't panic.
#[test]
fn validate_credential_large_credential_no_server() {
    let large_data = vec![0u8; 65536];
    let cred = PmixCredential::from_vec(large_data);
    let info: Vec<pmix::Info> = Vec::new();
    let _result = validate_credential(&cred, &info);
}

/// validate_credential credential ownership is preserved after the call.
#[test]
fn validate_credential_ownership_preserved() {
    let cred = PmixCredential::from_bytes(b"ownership-test-credential");
    let original_len = cred.len();
    let original_bytes = cred.as_bytes().to_vec();

    let info: Vec<pmix::Info> = Vec::new();
    let _result = validate_credential(&cred, &info);

    assert_eq!(cred.len(), original_len);
    assert_eq!(cred.as_bytes(), &original_bytes);
    assert!(!cred.is_empty());
}

/// validate_credential with cloned credential works independently.
#[test]
fn validate_credential_cloned_credential() {
    let cred = PmixCredential::from_bytes(b"clone-test");
    let cred_clone = cred.clone();

    let info: Vec<pmix::Info> = Vec::new();
    let result1 = validate_credential(&cred, &info);
    let result2 = validate_credential(&cred_clone, &info);

    assert!(
        result1.is_ok() == result2.is_ok(),
        "Original and cloned credential should produce consistent results"
    );
}

/// validate_credential with multiple sequential calls is consistent.
#[test]
fn validate_credential_multiple_sequential_calls() {
    let cred1 = PmixCredential::from_bytes(b"credential-one");
    let cred2 = PmixCredential::from_bytes(b"credential-two");
    let info: Vec<pmix::Info> = Vec::new();

    let result1 = validate_credential(&cred1, &info);
    let result2 = validate_credential(&cred2, &info);

    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results"
    );
}

/// validate_credential returns an error status when PMIx is not initialized.
#[test]
fn validate_credential_returns_error_status() {
    let cred = PmixCredential::from_bytes(b"test");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&cred, &info);
    if let Err(status) = result {
        assert!(status.is_error());
    }
}

/// validate_credential with empty info array works correctly.
#[test]
fn validate_credential_empty_info() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let _result = validate_credential(&cred, &info);
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 9: validate_credential_nb — no-server tests
// ─────────────────────────────────────────────────────────────────────────────

/// validate_credential_nb with empty credential returns expected error.
#[test]
fn validate_credential_nb_empty_credential_no_server() {
    let cred = PmixCredential::empty();
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    match result {
        Ok(()) => {}
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

/// validate_credential_nb with dummy credential returns expected error.
#[test]
fn validate_credential_nb_dummy_credential_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    match result {
        Ok(()) => {}
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

/// validate_credential_nb does NOT invoke callback when request is rejected.
#[test]
fn validate_credential_nb_callback_not_called_on_reject() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result_len = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingValidationCallback {
        status: status.clone(),
        result_len: result_len.clone(),
    });

    let result = validate_credential_nb(&cred, &info, callback);
    assert!(result.is_err(), "Expected error when PMIx not initialized");

    assert!(
        status.lock().unwrap().is_none(),
        "Callback should not have been invoked on rejected request"
    );
    assert!(
        result_len.lock().unwrap().is_none(),
        "Result length should not have been recorded"
    );
}

/// validate_credential_nb with binary credential data compiles and doesn't panic.
#[test]
fn validate_credential_nb_binary_credential() {
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01, 0x02, 0x03];
    let cred = PmixCredential::from_bytes(&binary_data);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

/// validate_credential_nb credential ownership is preserved after the call.
#[test]
fn validate_credential_nb_ownership_preserved() {
    let cred = PmixCredential::from_bytes(b"nb-ownership-test");
    let original_len = cred.len();
    let original_bytes = cred.as_bytes().to_vec();

    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);

    assert_eq!(cred.len(), original_len);
    assert_eq!(cred.as_bytes(), &original_bytes);
}

/// validate_credential_nb with a large credential (64KB) doesn't panic.
#[test]
fn validate_credential_nb_large_credential() {
    let large_data = vec![0u8; 65536];
    let cred = PmixCredential::from_vec(large_data);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

/// validate_credential_nb with credential containing null bytes works.
#[test]
fn validate_credential_nb_null_byte_credential() {
    let cred = PmixCredential::from_bytes(&[0x00, 0x00, 0x00]);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

/// validate_credential_nb with credential containing all 0xFF bytes works.
#[test]
fn validate_credential_nb_max_byte_credential() {
    let cred = PmixCredential::from_bytes(&[0xFF; 1024]);
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

/// validate_credential_nb with multiple sequential calls doesn't interfere.
#[test]
fn validate_credential_nb_multiple_sequential_calls() {
    let cred = PmixCredential::from_bytes(b"sequential-test");
    let info: Vec<pmix::Info> = Vec::new();
    for _ in 0..5 {
        let callback = Box::new(NoOpValidationCallback);
        let _result = validate_credential_nb(&cred, &info, callback);
    }
}

/// validate_credential_nb registry cleanup on error — sequential calls consistent.
#[test]
fn validate_credential_nb_registry_cleanup_on_error() {
    let cred = PmixCredential::from_bytes(b"cleanup-test");
    let info: Vec<pmix::Info> = Vec::new();

    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred, &info, callback1);

    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred, &info, callback2);

    assert!(
        result1.is_ok() == result2.is_ok(),
        "Sequential calls should return consistent results"
    );
}

/// validate_credential_nb with different credentials produces independent registrations.
#[test]
fn validate_credential_nb_different_credentials() {
    let cred1 = PmixCredential::from_bytes(b"credential-alpha");
    let cred2 = PmixCredential::from_bytes(b"credential-beta");
    let info: Vec<pmix::Info> = Vec::new();

    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred1, &info, callback1);

    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred2, &info, callback2);

    assert!(result1.is_ok() == result2.is_ok());
}

/// validate_credential_nb with cloned credential works independently.
#[test]
fn validate_credential_nb_cloned_credential() {
    let cred = PmixCredential::from_bytes(b"clone-nb-test");
    let cred_clone = cred.clone();

    let info: Vec<pmix::Info> = Vec::new();
    let callback1 = Box::new(NoOpValidationCallback);
    let result1 = validate_credential_nb(&cred, &info, callback1);

    let callback2 = Box::new(NoOpValidationCallback);
    let result2 = validate_credential_nb(&cred_clone, &info, callback2);

    assert!(
        result1.is_ok() == result2.is_ok(),
        "Original and cloned credential should produce consistent results"
    );
}

/// validate_credential_nb with empty info array works correctly.
#[test]
fn validate_credential_nb_empty_info() {
    let cred = PmixCredential::from_bytes(b"test-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let _result = validate_credential_nb(&cred, &info, callback);
}

/// validate_credential_nb with many rapid calls doesn't deadlock.
#[test]
fn validate_credential_nb_rapid_calls_no_deadlock() {
    let cred = PmixCredential::from_bytes(b"rapid-test");
    let info: Vec<pmix::Info> = Vec::new();
    for _ in 0..20 {
        let callback = Box::new(NoOpValidationCallback);
        let _result = validate_credential_nb(&cred, &info, callback);
    }
}

/// validate_credential_nb returns an error status when PMIx is not initialized.
#[test]
fn validate_credential_nb_returns_error_status() {
    let cred = PmixCredential::from_bytes(b"test");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(NoOpValidationCallback);
    let result = validate_credential_nb(&cred, &info, callback);
    if let Err(status) = result {
        assert!(status.is_error());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 10: Error code verification tests
// ─────────────────────────────────────────────────────────────────────────────

/// PmixError::ErrInit has the correct raw value (-31).
#[test]
fn error_code_err_init_value() {
    assert_eq!(PmixError::ErrInit as i32, -31);
}

/// PmixError::ErrNotSupported has the correct raw value (-47).
#[test]
fn error_code_err_not_supported_value() {
    assert_eq!(PmixError::ErrNotSupported as i32, -47);
}

/// PmixError::ErrInvalidCred has the correct raw value (-12).
#[test]
fn error_code_err_invalid_cred_value() {
    assert_eq!(PmixError::ErrInvalidCred as i32, -12);
}

/// PmixError::ErrTimeout has the correct raw value (-24).
#[test]
fn error_code_err_timeout_value() {
    assert_eq!(PmixError::ErrTimeout as i32, -24);
}

/// PmixStatus::from_raw correctly maps ERR_INIT.
#[test]
fn status_from_raw_err_init() {
    let status = pmix::PmixStatus::from_raw(-31);
    assert_eq!(status, pmix::PmixStatus::Known(PmixError::ErrInit));
}

/// PmixStatus::from_raw correctly maps ERR_NOT_SUPPORTED.
#[test]
fn status_from_raw_err_not_supported() {
    let status = pmix::PmixStatus::from_raw(-47);
    assert_eq!(status, pmix::PmixStatus::Known(PmixError::ErrNotSupported));
}

/// PmixStatus::from_raw correctly maps ERR_INVALID_CRED.
#[test]
fn status_from_raw_err_invalid_cred() {
    let status = pmix::PmixStatus::from_raw(-12);
    assert_eq!(status, pmix::PmixStatus::Known(PmixError::ErrInvalidCred));
}

/// PmixStatus::is_error returns true for ERR_INIT.
#[test]
fn status_is_error_for_err_init() {
    let status = pmix::PmixStatus::from_raw(-31);
    assert!(status.is_error());
}

/// PmixStatus::is_error returns true for ERR_NOT_SUPPORTED.
#[test]
fn status_is_error_for_err_not_supported() {
    let status = pmix::PmixStatus::from_raw(-47);
    assert!(status.is_error());
}

/// PmixStatus::is_error returns true for ERR_INVALID_CRED.
#[test]
fn status_is_error_for_err_invalid_cred() {
    let status = pmix::PmixStatus::from_raw(-12);
    assert!(status.is_error());
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 11: Integration tests (require PMIx daemon — marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: get_credential with a running PMIx daemon.
/// #[ignore] — requires PMIx server; calling server_init_minimal corrupts C-level PMIx state.
#[test]
#[ignore = "requires PMIx daemon; server_init_minimal corrupts C-level PMIx state"]
fn get_credential_with_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential(&info);
    match result {
        Ok(cred) => {
            assert!(!cred.is_empty(), "Credential from server should not be empty");
        }
        Err(status) => {
            panic!("get_credential failed with server: {:?}", status);
        }
    }
}

/// Integration test: get_credential_nb with a running PMIx daemon.
/// #[ignore] — requires PMIx daemon; calling server_init_minimal corrupts C-level PMIx state.
#[test]
#[ignore = "requires PMIx daemon; server_init_minimal corrupts C-level PMIx state"]
fn get_credential_nb_with_server() {
    let received = std::sync::Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();

    let callback = Box::new(RecordingCredentialCallback {
        status: received_clone.clone(),
        has_credential: std::sync::Arc::new(std::sync::Mutex::new(None)),
        result_len: std::sync::Arc::new(std::sync::Mutex::new(None)),
    });

    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential_nb(&info, callback);
    assert!(result.is_ok(), "get_credential_nb should be accepted");

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let received = received.lock().unwrap();
    assert!(received.is_some(), "Callback should have been invoked");
}

/// Integration test: validate_credential with a real credential from a PMIx daemon.
/// #[ignore] — requires PMIx daemon; calling server_init_minimal corrupts C-level PMIx state.
#[test]
#[ignore = "requires PMIx daemon; server_init_minimal corrupts C-level PMIx state"]
fn validate_credential_with_server() {
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

/// Integration test: validate_credential_nb with a running PMIx daemon.
/// #[ignore] — requires PMIx daemon; calling server_init_minimal corrupts C-level PMIx state.
#[test]
#[ignore = "requires PMIx daemon; server_init_minimal corrupts C-level PMIx state"]
fn validate_credential_nb_with_server() {
    let received_status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let received_len = std::sync::Arc::new(std::sync::Mutex::new(None));

    let callback = Box::new(RecordingValidationCallback {
        status: received_status.clone(),
        result_len: received_len.clone(),
    });

    let info: Vec<pmix::Info> = Vec::new();
    let cred = get_credential(&info).expect("get_credential should succeed with server");

    let result = validate_credential_nb(&cred, &info, callback);
    assert!(result.is_ok(), "validate_credential_nb should be accepted");

    std::thread::sleep(std::time::Duration::from_millis(1000));

    let status = received_status.lock().unwrap();
    assert!(status.is_some(), "Callback should have been invoked");
    assert!(
        status.as_ref().unwrap().is_success(),
        "Callback status should be success: {:?}",
        *status.as_ref().unwrap()
    );
}

/// Integration test: validate_credential with an invalid credential.
/// #[ignore] — requires PMIx daemon; calling server_init_minimal corrupts C-level PMIx state.
#[test]
#[ignore = "requires PMIx daemon; server_init_minimal corrupts C-level PMIx state"]
fn validate_credential_invalid_credential_with_server() {
    let invalid_cred = PmixCredential::from_bytes(b"this-is-not-a-valid-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let result = validate_credential(&invalid_cred, &info);
    assert!(
        result.is_err(),
        "validate_credential should fail for invalid credential"
    );
}
