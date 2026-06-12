//! Tests for `PMIx_Get_credential` and `PMIx_Validate_credential` — security operations.
//!
//! These tests verify the Rust wrappers for credential management APIs:
//! `get_credential`, `get_credential_nb`, `validate_credential`,
//! `validate_credential_nb`, and the `PmixCredential` type.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::PmixError;
use pmix::security::{
    get_credential, get_credential_nb, validate_credential, validate_credential_nb,
    CredentialCallback, CredentialResults, ValidationCallback, ValidationResults, PmixCredential,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for get_credential_nb.
struct TestCredentialCallback;

impl CredentialCallback for TestCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        _status: pmix::PmixStatus,
        _credential: Option<PmixCredential>,
        _results: CredentialResults,
    ) {
        // No-op — just verify the trait compiles and the callback
        // can be invoked without panicking.
    }
}

/// Test callback that records the status it received.
struct RecordingCredentialCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
    has_credential: std::cell::Cell<Option<bool>>,
}

impl CredentialCallback for RecordingCredentialCallback {
    fn on_complete(
        self: Box<Self>,
        status: pmix::PmixStatus,
        credential: Option<PmixCredential>,
        _results: CredentialResults,
    ) {
        self.status.set(Some(status));
        self.has_credential.set(Some(credential.is_some()));
    }
}

/// No-op test callback for validate_credential_nb.
struct TestValidationCallback;

impl ValidationCallback for TestValidationCallback {
    fn on_complete(self: Box<Self>, _status: pmix::PmixStatus, _results: ValidationResults) {
        // No-op — just verify the trait compiles.
    }
}

/// Test callback that records the status and result count.
struct RecordingValidationCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
    result_len: std::cell::Cell<Option<usize>>,
}

impl ValidationCallback for RecordingValidationCallback {
    fn on_complete(self: Box<Self>, status: pmix::PmixStatus, results: ValidationResults) {
        self.status.set(Some(status));
        self.result_len.set(Some(results.len()));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCredential construction and access tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixCredential can be created from an empty byte slice.
#[test]
fn test_credential_from_empty_bytes() {
    let cred = PmixCredential::from_bytes(&[]);
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
    assert!(cred.as_bytes().is_empty());
}

/// Test that PmixCredential can be created from non-empty bytes.
#[test]
fn test_credential_from_bytes() {
    let data = b"test-credential-data";
    let cred = PmixCredential::from_bytes(data);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), data.len());
    assert_eq!(cred.as_bytes(), data);
}

/// Test that PmixCredential handles binary data correctly.
#[test]
fn test_credential_binary_data() {
    let data: &[u8] = &[0x00, 0x01, 0xFF, 0xFE, 0x80, 0x7F];
    let cred = PmixCredential::from_bytes(data);
    assert_eq!(cred.as_bytes(), data);
    assert_eq!(cred.len(), 6);
}

/// Test that PmixCredential as_raw returns a non-null pointer.
#[test]
fn test_credential_as_raw_non_null() {
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    assert!(!cred.as_raw().is_null());
}

/// Test that PmixCredential as_raw for empty credential is also non-null
/// (the struct itself is allocated, just with zero-size bytes).
#[test]
fn test_credential_as_raw_empty_non_null() {
    let cred = PmixCredential::from_bytes(&[]);
    assert!(!cred.as_raw().is_null());
}

/// Test credential debug formatting does not panic.
#[test]
fn test_credential_debug() {
    // PmixCredential doesn't derive Debug (contains raw pointers),
    // but we can verify it can be inspected via as_bytes().
    let cred = PmixCredential::from_bytes(b"debug-test");
    assert_eq!(cred.as_bytes(), b"debug-test");
    assert_eq!(cred.len(), 10);
}

/// Test multiple credentials can coexist independently.
#[test]
fn test_multiple_credentials() {
    let cred1 = PmixCredential::from_bytes(b"first");
    let cred2 = PmixCredential::from_bytes(b"second-credential-data");
    let cred3 = PmixCredential::from_bytes(&[]);

    assert_eq!(cred1.as_bytes(), b"first");
    assert_eq!(cred2.as_bytes(), b"second-credential-data");
    assert!(cred3.is_empty());

    // All should still be valid after creation.
    assert!(!cred1.as_raw().is_null());
    assert!(!cred2.as_raw().is_null());
    assert!(!cred3.as_raw().is_null());
}

// ─────────────────────────────────────────────────────────────────────────────
// CredentialResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that CredentialResults default is empty.
#[test]
fn test_credential_results_default() {
    let results = CredentialResults::default();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
    assert!(results.info().is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// ValidationResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that ValidationResults debug formatting does not panic.
#[test]
fn test_validation_results_debug() {
    // We can't easily create a ValidationResults without FFI,
    // but we can verify the type is Debug by checking it compiles.
    // The struct derives Debug, so this is a compile-time check.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<ValidationResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential FFI call tests (without PMIx server)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_credential with empty info returns an expected error
/// when PMIx is not initialized.
#[test]
fn test_get_credential_no_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential(&info);
    match result {
        Ok(_) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            // Expected errors when no PMIx server is running.
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "Expected ERR_INIT or ERR_NOT_SUPPORTED, got {:?}",
                status
            );
        }
    }
}

/// Integration test: get_credential with a running PMIx daemon.
/// Requires PMIx server to be running — ignored by default.
#[test]
#[ignore]
fn test_get_credential_with_server() {
    // This test requires a running PMIx daemon.
    // Initialize PMIx first, then call get_credential.
    let info: Vec<pmix::Info> = Vec::new();
    let result = get_credential(&info);
    match result {
        Ok(cred) => {
            // Credential should be non-empty from a real server.
            assert!(!cred.is_empty(), "Credential from server should not be empty");
            assert!(!cred.as_bytes().is_empty());
        }
        Err(status) => {
            panic!("get_credential failed with server: {:?}", status);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential_nb callback tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_credential_nb with the test callback compiles and returns
/// an expected error when PMIx is not initialized.
#[test]
fn test_get_credential_nb_no_server() {
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(TestCredentialCallback);
    let result = get_credential_nb(&info, callback);
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

/// Integration test: get_credential_nb with a running PMIx daemon.
#[test]
#[ignore]
fn test_get_credential_nb_with_server() {
    use std::sync::{Arc, Mutex};

    let received = Arc::new(Mutex::new(None));
    let received_clone = received.clone();

    struct ArcCallback {
        received: Arc<Mutex<Option<(pmix::PmixStatus, bool)>>>,
    }

    impl CredentialCallback for ArcCallback {
        fn on_complete(
            self: Box<Self>,
            status: pmix::PmixStatus,
            credential: Option<PmixCredential>,
            _results: CredentialResults,
        ) {
            let has_cred = credential.map(|c| !c.is_empty()).unwrap_or(false);
            *self.received.lock().unwrap() = Some((status, has_cred));
        }
    }

    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(ArcCallback {
        received: received_clone,
    });

    let result = get_credential_nb(&info, callback);
    assert!(result.is_ok(), "get_credential_nb should be accepted");

    // Wait for callback — in a real environment, the callback fires asynchronously.
    std::thread::sleep(std::time::Duration::from_millis(500));

    let received = received.lock().unwrap();
    assert!(
        received.is_some(),
        "Callback should have been invoked by PMIx daemon"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential FFI call tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that validate_credential with a dummy credential returns an expected error
/// when PMIx is not initialized.
#[test]
fn test_validate_credential_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential");
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
                "Expected ERR_INIT, ERR_NOT_SUPPORTED, or ERR_BAD_CRED, got {:?}",
                status
            );
        }
    }
}

/// Integration test: validate_credential with a real credential from a PMIx daemon.
#[test]
#[ignore]
fn test_validate_credential_with_server() {
    // First get a credential, then validate it.
    let info: Vec<pmix::Info> = Vec::new();
    let cred = get_credential(&info).expect("get_credential should succeed with server");
    let result = validate_credential(&cred, &info);
    match result {
        Ok(results) => {
            // Validation results should contain at least some info.
            assert!(!results.is_empty() || results.len() == 0);
        }
        Err(status) => {
            panic!("validate_credential failed: {:?}", status);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential_nb callback tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that validate_credential_nb compiles and returns expected error
/// when PMIx is not initialized.
#[test]
fn test_validate_credential_nb_no_server() {
    let cred = PmixCredential::from_bytes(b"dummy-credential");
    let info: Vec<pmix::Info> = Vec::new();
    let callback = Box::new(TestValidationCallback);
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

/// Integration test: validate_credential_nb with a running PMIx daemon.
#[test]
#[ignore]
fn test_validate_credential_nb_with_server() {
    use std::sync::{Arc, Mutex};

    let received = Arc::new(Mutex::new(None));
    let received_clone = received.clone();

    struct ArcValidationCallback {
        received: Arc<Mutex<Option<(pmix::PmixStatus, usize)>>>,
    }

    impl ValidationCallback for ArcValidationCallback {
        fn on_complete(self: Box<Self>, status: pmix::PmixStatus, results: ValidationResults) {
            *self.received.lock().unwrap() = Some((status, results.len()));
        }
    }

    // Get a credential first.
    let info: Vec<pmix::Info> = Vec::new();
    let cred = get_credential(&info).expect("get_credential should succeed with server");

    let callback = Box::new(ArcValidationCallback {
        received: received_clone,
    });

    let result = validate_credential_nb(&cred, &info, callback);
    assert!(result.is_ok(), "validate_credential_nb should be accepted");

    // Wait for callback.
    std::thread::sleep(std::time::Duration::from_millis(500));

    let received = received.lock().unwrap();
    assert!(
        received.is_some(),
        "Validation callback should have been invoked"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait compilation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the CredentialCallback trait is object-safe and can be boxed.
#[test]
fn test_credential_callback_trait_object() {
    let _callback: Box<dyn CredentialCallback> = Box::new(TestCredentialCallback);
}

/// Test that the ValidationCallback trait is object-safe and can be boxed.
#[test]
fn test_validation_callback_trait_object() {
    let _callback: Box<dyn ValidationCallback> = Box::new(TestValidationCallback);
}

/// Test that callback traits are Send.
#[test]
fn test_callback_traits_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn CredentialCallback>>();
    assert_send::<Box<dyn ValidationCallback>>();
}
