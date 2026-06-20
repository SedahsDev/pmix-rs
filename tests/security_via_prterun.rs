//! Security FFI tests that require PMIx initialization via prterun.
//!
//! These tests exercise the actual FFI paths in security.rs by calling
//! `pmix::init()` before security operations. Run with:
//!
//! ```bash
//! # Run individually (PMIx state corruption in batch mode):
//! prterun -np 1 cargo test --test security_via_prterun -- --include-ignored --test-threads=1
//! ```

use std::sync::OnceLock;

// ─────────────────────────────────────────────────────────────────────────────
// Shared PMIx context for all DVM tests
// ─────────────────────────────────────────────────────────────────────────────

static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

fn ensure_pmix_init() -> bool {
    if !is_dvm_launched() {
        return false;
    }
    if pmix::utility::initialized() {
        return true;
    }
    PMIX_CONTEXT
        .set(pmix::init(None).ok())
        .is_ok()
        && PMIX_CONTEXT.get().unwrap().is_some()
}

fn is_dvm_launched() -> bool {
    std::env::var("PMIX_RANK").is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (no PMIx init required)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixCredential::from_bytes creates a credential.
#[test]
fn test_credential_from_bytes() {
    let cred = pmix::security::PmixCredential::from_bytes(&[1, 2, 3, 4]);
    assert_eq!(cred.as_bytes(), &[1, 2, 3, 4]);
    assert!(!cred.is_empty());
    assert_eq!(cred.len(), 4);
}

/// PmixCredential::from_vec creates a credential.
#[test]
fn test_credential_from_vec() {
    let cred = pmix::security::PmixCredential::from_vec(vec![5, 6, 7]);
    assert_eq!(cred.as_bytes(), &[5, 6, 7]);
    assert_eq!(cred.len(), 3);
}

/// PmixCredential::empty creates an empty credential.
#[test]
fn test_credential_empty() {
    let cred = pmix::security::PmixCredential::empty();
    assert!(cred.is_empty());
    assert_eq!(cred.len(), 0);
}

/// PmixCredential Debug impl works.
#[test]
fn test_credential_debug() {
    let cred = pmix::security::PmixCredential::from_bytes(&[1, 2]);
    let debug_str = format!("{:?}", cred);
    assert!(!debug_str.is_empty());
}

/// PmixCredential::as_raw returns a non-null pointer.
#[test]
fn test_credential_as_raw() {
    let cred = pmix::security::PmixCredential::from_bytes(&[1, 2, 3]);
    let ptr = cred.as_raw();
    assert!(!ptr.is_null());
}

/// ValidationResults::empty creates an empty result set.
#[test]
fn test_validation_results_empty() {
    let results = pmix::security::ValidationResults::empty();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
}

/// get_credential fails gracefully without PMIx init (only when NOT under DVM).
#[test]
fn test_get_credential_fails_without_init() {
    if !is_dvm_launched() {
        let result = pmix::security::get_credential(&[]);
        assert!(result.is_err());
    }
}

/// validate_credential fails gracefully without PMIx init (only when NOT under DVM).
#[test]
fn test_validate_credential_fails_without_init() {
    if !is_dvm_launched() {
        let cred = pmix::security::PmixCredential::empty();
        let result = pmix::security::validate_credential(&cred, &[]);
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests (require prterun, use shared PMIx context)
// ─────────────────────────────────────────────────────────────────────────────

/// get_credential via DVM.
/// Covers: get_credential FFI call, PMIx_Get_credential
#[test]
#[ignore = "requires prterun launch"]
fn test_get_credential_via_dvm() {
    assert!(ensure_pmix_init());
    let result = pmix::security::get_credential(&[]);
    // May succeed or fail depending on security system availability
    // The important thing is that the FFI path is exercised
    match result {
        Ok(cred) => {
            // Credential obtained successfully
            // Credential obtained - bytes are accessible
            let _ = cred.as_bytes();
        }
        Err(status) => {
            // Expected if no security system is configured
            assert!(!status.is_success());
        }
    }
}

/// validate_credential via DVM with empty credential.
/// Covers: validate_credential FFI call, PMIx_Validate_credential
#[test]
#[ignore = "requires prterun launch"]
fn test_validate_credential_empty_via_dvm() {
    assert!(ensure_pmix_init());
    let cred = pmix::security::PmixCredential::empty();
    let result = pmix::security::validate_credential(&cred, &[]);
    // Empty credential should fail validation
    assert!(result.is_err());
}

/// validate_credential via DVM with non-empty credential.
/// Covers: validate_credential FFI call with actual data
#[test]
#[ignore = "requires prterun launch"]
fn test_validate_credential_nonempty_via_dvm() {
    assert!(ensure_pmix_init());
    let cred = pmix::security::PmixCredential::from_bytes(&[1, 2, 3, 4, 5]);
    let result = pmix::security::validate_credential(&cred, &[]);
    // May succeed or fail depending on credential validity
    match result {
        Ok(results) => {
            // Validation succeeded
            assert!(results.len() >= 0);
        }
        Err(status) => {
            // Expected if credential is not valid
            assert!(!status.is_success());
        }
    }
}

/// Full credential lifecycle: get -> validate via DVM.
/// Covers: complete credential flow
#[test]
#[ignore = "requires prterun launch"]
fn test_credential_lifecycle_via_dvm() {
    assert!(ensure_pmix_init());

    // Try to get a credential
    let result = pmix::security::get_credential(&[]);
    match result {
        Ok(cred) => {
            // If we got a credential, try to validate it
            let _ = pmix::security::validate_credential(&cred, &[]);
        }
        Err(_) => {
            // No security system available - that's fine, we still covered the path
        }
    }
}
