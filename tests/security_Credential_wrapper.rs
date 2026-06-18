//! Wrapper tests for security.rs — Credential Core.
//!
//! Tests exercise `PmixCredential`, `get_credential`, `get_credential_nb`,
//! `validate_credential`, `validate_credential_nb` wrapper logic without PMIx_Init.
//! FFI calls return errors gracefully (no segfault).
//!
//! Coverage targets:
//!   - PmixCredential (lines 60-158) — constructor, accessors, empty
//!   - get_credential (lines 236-274) — FFI call + error path
//!   - get_credential_nb (lines 477-530) — callback reg + FFI + cleanup
//!   - validate_credential (lines 602-650) — FFI call + error path
//!   - validate_credential_nb (lines 786-847) — callback reg + FFI + cleanup

use pmix::security::*;
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixCredential — credential struct
// ─────────────────────────────────────────────────────────────────────────────

/// PmixCredential::from_bytes creates a credential from a byte slice.
#[test]
fn test_credential_from_bytes() {
    let cred = PmixCredential::from_bytes(&[1, 2, 3, 4]);
    assert_eq!(cred.len(), 4);
    assert!(!cred.is_empty());
}

/// PmixCredential::from_bytes with empty slice.
#[test]
fn test_credential_from_bytes_empty() {
    let cred = PmixCredential::from_bytes(&[]);
    assert_eq!(cred.len(), 0);
    assert!(cred.is_empty());
}

/// PmixCredential::from_vec creates a credential from a Vec<u8>.
#[test]
fn test_credential_from_vec() {
    let cred = PmixCredential::from_vec(vec![1, 2, 3, 4]);
    assert_eq!(cred.len(), 4);
    assert!(!cred.is_empty());
}

/// PmixCredential::from_vec with empty vec.
#[test]
fn test_credential_from_vec_empty() {
    let cred = PmixCredential::from_vec(vec![]);
    assert_eq!(cred.len(), 0);
    assert!(cred.is_empty());
}

/// PmixCredential::empty creates an empty credential.
#[test]
fn test_credential_empty() {
    let cred = PmixCredential::empty();
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
}

/// PmixCredential::as_bytes returns the credential bytes.
#[test]
fn test_credential_as_bytes() {
    let bytes = [1u8, 2, 3, 4, 5];
    let cred = PmixCredential::from_bytes(&bytes);
    assert_eq!(cred.as_bytes(), &bytes);
}

/// PmixCredential::as_bytes on empty credential.
#[test]
fn test_credential_as_bytes_empty() {
    let cred = PmixCredential::empty();
    assert!(cred.as_bytes().is_empty());
}

/// PmixCredential::len returns correct length.
#[test]
fn test_credential_len() {
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    assert_eq!(cred.len(), 3);
}

/// PmixCredential::is_empty on non-empty credential.
#[test]
fn test_credential_is_empty_false() {
    let cred = PmixCredential::from_bytes(&[1]);
    assert!(!cred.is_empty());
}

/// PmixCredential with large data.
#[test]
fn test_credential_large() {
    let large: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let cred = PmixCredential::from_vec(large.clone());
    assert_eq!(cred.len(), 4096);
    assert_eq!(cred.as_bytes(), large.as_slice());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential — synchronous credential retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// get_credential with empty info returns error.
#[test]
fn test_get_credential_empty_info() {
    let info: Vec<pmix::Info> = vec![];
    let result = get_credential(&info);
    assert!(result.is_err());
}

/// get_credential with info returns error (not initialized).
#[test]
fn test_get_credential_with_info() {
    let info = InfoBuilder::new().build();
    let result = get_credential(&[info]);
    assert!(result.is_err());
}

/// get_credential is deterministic.
#[test]
fn test_get_credential_deterministic() {
    let info: Vec<pmix::Info> = vec![];
    let r1 = get_credential(&info);
    let r2 = get_credential(&info);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential_nb — non-blocking credential retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// get_credential_nb with empty info returns error, callback not invoked.
#[test]
fn test_get_credential_nb_empty_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl CredentialCallback for Cb {
        fn on_complete(
            self: Box<Self>,
            _status: PmixStatus,
            _credential: Option<PmixCredential>,
            _results: CredentialResults,
        ) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info: Vec<pmix::Info> = vec![];
    let result = get_credential_nb(&info, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// get_credential_nb with info returns error, callback not invoked.
#[test]
fn test_get_credential_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl CredentialCallback for Cb {
        fn on_complete(
            self: Box<Self>,
            _status: PmixStatus,
            _credential: Option<PmixCredential>,
            _results: CredentialResults,
        ) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info = InfoBuilder::new().build();
    let result = get_credential_nb(&[info], Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential — synchronous credential validation
// ─────────────────────────────────────────────────────────────────────────────

/// validate_credential with empty credential and info returns error.
#[test]
fn test_validate_credential_empty() {
    let info: Vec<pmix::Info> = vec![];
    let cred = PmixCredential::empty();
    let result = validate_credential(&cred, &info);
    assert!(result.is_err());
}

/// validate_credential with data returns error (not initialized).
#[test]
fn test_validate_credential_with_data() {
    let info: Vec<pmix::Info> = vec![];
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    let result = validate_credential(&cred, &info);
    assert!(result.is_err());
}

/// validate_credential with info returns error.
#[test]
fn test_validate_credential_with_info() {
    let info = InfoBuilder::new().build();
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    let result = validate_credential(&cred, &[info]);
    assert!(result.is_err());
}

/// validate_credential is deterministic.
#[test]
fn test_validate_credential_deterministic() {
    let info: Vec<pmix::Info> = vec![];
    let cred = PmixCredential::empty();
    let r1 = validate_credential(&cred, &info);
    let r2 = validate_credential(&cred, &info);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential_nb — non-blocking credential validation
// ─────────────────────────────────────────────────────────────────────────────

/// validate_credential_nb with empty credential returns error, callback not invoked.
#[test]
fn test_validate_credential_nb_empty() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl ValidationCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: ValidationResults) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info: Vec<pmix::Info> = vec![];
    let cred = PmixCredential::empty();
    let result = validate_credential_nb(&cred, &info, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// validate_credential_nb with credential data returns error, callback not invoked.
#[test]
fn test_validate_credential_nb_with_data() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl ValidationCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: ValidationResults) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info: Vec<pmix::Info> = vec![];
    let cred = PmixCredential::from_bytes(&[1, 2, 3]);
    let result = validate_credential_nb(&cred, &info, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ValidationResults
// ─────────────────────────────────────────────────────────────────────────────

/// ValidationResults::empty creates an empty result set.
#[test]
fn test_validation_results_empty() {
    let results = ValidationResults::empty();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
}
