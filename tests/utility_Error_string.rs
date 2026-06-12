//! Integration tests for `PMIx_Error_string` via the safe `error_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Error_string` only looks up a static string table
//! inside the library.

use pmix::{utility::error_string, PmixStatus};

/// `error_string` returns `Ok(String)` for PMIX_SUCCESS (0).
///
/// The PMIx spec defines `PMIx_Error_string` as returning a non-null,
/// null-terminated string for any valid `pmix_status_t`. For PMIX_SUCCESS,
/// the library returns "success".
#[test]
fn error_string_success_returns_ok() {
    let status = PmixStatus::from_raw(0);
    let result = error_string(status);
    assert!(
        result.is_ok(),
        "error_string(PMIX_SUCCESS) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "error_string should not return an empty string"
    );
}

/// `error_string` returns a readable description for PMIX_ERROR (-1).
#[test]
fn error_string_generic_error_returns_ok() {
    let status = PmixStatus::from_raw(-1);
    let result = error_string(status);
    assert!(
        result.is_ok(),
        "error_string(PMIX_ERROR) should return Ok, got {:?}",
        result
    );
}

/// `error_string` handles various negative error codes across subsystems.
///
/// Tests timeout (-24), bad parameter (-27), and not found (-33) to cover
/// different error categories defined in the PMIx standard.
#[test]
fn error_string_various_error_codes() {
    let codes: Vec<(i32, &str)> = vec![
        (0, "PMIX_SUCCESS"),
        (-1, "PMIX_ERROR"),
        (-24, "PMIX_ERR_TIMEOUT"),
        (-27, "PMIX_ERR_BAD_PARAM"),
        (-33, "PMIX_ERR_NOT_FOUND"),
    ];
    for (code, name) in codes {
        let status = PmixStatus::from_raw(code);
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string({} = {}) should return Ok, got {:?}",
            code,
            name,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "error_string({} = {}) should not return empty string",
            code,
            name
        );
    }
}

/// `error_string` handles unknown/user-defined status codes gracefully.
///
/// PMIx reserves values below PMIX_EXTERNAL_ERR_BASE (-9999) for
/// user/implementation-defined codes. The C library should still return
/// a valid string (typically "external error" or similar).
#[test]
fn error_string_unknown_code_returns_ok() {
    let status = PmixStatus::from_raw(-10001);
    let result = error_string(status);
    assert!(
        result.is_ok(),
        "error_string should handle unknown codes gracefully, got {:?}",
        result
    );
}

/// `error_string` is deterministic — the same status code always
/// produces the same string description.
#[test]
fn error_string_is_deterministic() {
    let status = PmixStatus::from_raw(-24); // PMIX_ERR_TIMEOUT
    let first = error_string(status).unwrap();
    let second = error_string(status).unwrap();
    assert_eq!(
        first, second,
        "error_string must be deterministic for the same input"
    );
}

/// `error_string` returns different strings for different status codes.
///
/// SUCCESS and ERROR must produce distinct descriptions.
#[test]
fn error_string_distinct_for_different_codes() {
    let success = error_string(PmixStatus::from_raw(0)).unwrap();
    let error = error_string(PmixStatus::from_raw(-1)).unwrap();
    assert_ne!(
        success, error,
        "error_string(SUCCESS) and error_string(ERROR) must return different strings"
    );
}

/// `error_string` returns a `Result<String, PmixStatus>`, not a raw pointer.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn error_string_returns_result_string() {
    let status = PmixStatus::from_raw(0);
    let _result: Result<String, PmixStatus> = error_string(status);
}
