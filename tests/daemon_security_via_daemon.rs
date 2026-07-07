//! Round 8 - P6: security.rs module via prte-beast daemon.
//!
//! Uses the shared tool handle from `daemon_helper` - tool_init is called
//! only once for the entire test binary, avoiding PMIx global state corruption.
//!
//! Run individually:
//!   cargo test --test daemon_security_via_daemon -- --ignored --test-threads=1

mod daemon_helper;

use pmix::security::{
    CredentialCallback, CredentialResults, PmixCredential, ValidationCallback, ValidationResults,
    get_credential, get_credential_nb, validate_credential, validate_credential_nb,
};
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_credential_type() {
    let _f: fn(&[pmix::Info]) -> Result<PmixCredential, PmixStatus> = get_credential;
}

#[test]
fn test_get_credential_nb_type() {
    let _f: fn(&[pmix::Info], Box<dyn CredentialCallback>) -> Result<(), PmixStatus> =
        get_credential_nb;
}

#[test]
fn test_validate_credential_type() {
    let _f: fn(&PmixCredential, &[pmix::Info]) -> Result<ValidationResults, PmixStatus> =
        validate_credential;
}

#[test]
fn test_validate_credential_nb_type() {
    let _f: fn(
        &PmixCredential,
        &[pmix::Info],
        Box<dyn ValidationCallback>,
    ) -> Result<(), PmixStatus> = validate_credential_nb;
}

#[test]
fn test_pmix_credential_from_bytes_type() {
    let _f: fn(&[u8]) -> PmixCredential = PmixCredential::from_bytes;
}

#[test]
fn test_pmix_credential_from_vec_type() {
    let _f: fn(Vec<u8>) -> PmixCredential = PmixCredential::from_vec;
}

#[test]
fn test_pmix_credential_empty_type() {
    let _f: fn() -> PmixCredential = PmixCredential::empty;
}

#[test]
fn test_pmix_credential_as_bytes_type() {
    let _f: fn(&PmixCredential) -> &[u8] = PmixCredential::as_bytes;
}

#[test]
fn test_pmix_credential_is_empty_type() {
    let _f: fn(&PmixCredential) -> bool = PmixCredential::is_empty;
}

#[test]
fn test_pmix_credential_len_type() {
    let _f: fn(&PmixCredential) -> usize = PmixCredential::len;
}

#[test]
fn test_pmix_credential_as_raw_exists() {
    let cred = PmixCredential::from_vec(vec![1u8, 2, 3]);
    let _raw: *const std::ffi::c_void = cred.as_raw() as *const std::ffi::c_void;
}

#[test]
fn test_credential_results_len_type() {
    let _f: fn(&CredentialResults) -> usize = CredentialResults::len;
}

#[test]
fn test_credential_results_is_empty_type() {
    let _f: fn(&CredentialResults) -> bool = CredentialResults::is_empty;
}

#[test]
fn test_credential_results_info_type() {
    let _f: fn(&CredentialResults) -> &[pmix::Info] = CredentialResults::info;
}

#[test]
fn test_credential_callback_trait_object() {
    struct TestCredCb;
    impl CredentialCallback for TestCredCb {
        fn on_complete(
            self: Box<Self>,
            _status: PmixStatus,
            _credential: Option<PmixCredential>,
            _results: CredentialResults,
        ) {
        }
    }
    let _cb: Box<dyn CredentialCallback> = Box::new(TestCredCb);
}

#[test]
fn test_validation_callback_trait_object() {
    struct TestValCb;
    impl ValidationCallback for TestValCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: ValidationResults) {}
    }
    let _cb: Box<dyn ValidationCallback> = Box::new(TestValCb);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCredential construction tests (always run - no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_credential_empty_construction() {
    let cred = PmixCredential::empty();
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
    assert!(cred.as_bytes().is_empty());
}

#[test]
fn test_pmix_credential_from_bytes() {
    let data = vec![1u8, 2, 3, 4, 5];
    let cred = PmixCredential::from_bytes(&data);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), data.len());
    assert_eq!(cred.as_bytes(), data.as_slice());
}

#[test]
fn test_pmix_credential_from_vec() {
    let data = vec![10u8, 20, 30, 40];
    let cred = PmixCredential::from_vec(data);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), 4);
    assert_eq!(cred.as_bytes(), [10u8, 20, 30, 40]);
}

#[test]
fn test_pmix_credential_as_raw() {
    let data = vec![42u8, 43, 44];
    let cred = PmixCredential::from_vec(data);
    let raw = cred.as_raw();
    assert!(!raw.is_null());
}

#[test]
fn test_pmix_credential_empty_as_raw() {
    let cred = PmixCredential::empty();
    let raw = cred.as_raw();
    let _ = raw;
}

// ─────────────────────────────────────────────────────────────────────────────
// Note: "before init" tests removed - with the shared handle pattern,
// tool_init is called once at the start of the test binary. The ErrInit
// error path is covered by daemon_server tests instead.
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Consolidated daemon test.
//
// PMIx C library has internal state that corrupts when multiple FFI
// security calls run in sequence across separate test functions, even
// with a shared tool handle. All security FFI operations must run
// within a single test function to avoid segfaults.
//
// This is a known PMIx limitation - the C library maintains global
// state that degrades after certain FFI calls (get_credential,
// validate_credential). The only workaround is to batch all FFI calls
// into a single test.
#[test]
#[ignore = "daemon isolation"]
fn test_security_all_ffi_via_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon handle");

    let directives = vec![InfoBuilder::new().build()];

    // ── 1. PmixCredential lifecycle ──
    let empty = PmixCredential::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(empty.as_bytes().is_empty());

    let data = vec![10u8, 20, 30, 40, 50];
    let cred = PmixCredential::from_bytes(&data);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), 5);
    assert_eq!(cred.as_bytes(), data.as_slice());
    assert!(!cred.as_raw().is_null());

    // ── 2. get_credential ──
    {
        let cred_result = get_credential(&directives);
        match &cred_result {
            Ok(cred) => {
                let _bytes = cred.as_bytes();
                let _len = cred.len();
                let _empty = cred.is_empty();
                let _raw = cred.as_raw();
            }
            Err(status) => {
                assert_ne!(
                    *status,
                    PmixStatus::Known(PmixError::ErrInit),
                    "get_credential returned ErrInit after tool_init"
                );
            }
        }
    }

    // ── 3. validate_credential with empty credential ──
    {
        let validate_result = validate_credential(&empty, &directives);
        match &validate_result {
            Ok(results) => {
                let len = results.len();
                let is_empty = results.is_empty();
                assert_eq!(is_empty, len == 0);
            }
            Err(status) => {
                assert_ne!(
                    *status,
                    PmixStatus::Known(PmixError::ErrInit),
                    "validate_credential returned ErrInit after tool_init"
                );
            }
        }
    }

    // ── 4. validate_credential with non-empty credential ──
    {
        let non_empty_cred = PmixCredential::from_bytes(&[1, 2, 3, 4]);
        let validate_result2 = validate_credential(&non_empty_cred, &directives);
        match &validate_result2 {
            Ok(results) => {
                let _ = results.len();
                let _ = results.is_empty();
            }
            Err(status) => {
                assert_ne!(
                    *status,
                    PmixStatus::Known(PmixError::ErrInit),
                    "validate_credential returned ErrInit after tool_init"
                );
            }
        }
    }
}
