//! Wrapper tests for groups.rs — Group Leave Operation.
//!
//! Tests exercise `group_leave`, `group_leave_nb` wrapper logic without PMIx_Init.
//! Input validation (empty group_id) returns PMIX_ERR_BAD_PARAM synchronously
//! without hitting FFI. FFI calls return errors gracefully.

use pmix::groups::*;
use pmix::{InfoBuilder, PmixError, PmixStatus};

/// Helper to extract the error from a Result<_, PmixStatus>.
fn extract_err<T>(result: Result<T, PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Helper to extract the error from a Result<(), PmixStatus>.
fn unwrap_err_result(result: Result<(), PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave — synchronous group leave
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_leave_empty_group_id() {
    let err = extract_err(group_leave("", &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_leave with valid inputs returns error (not initialized).
#[test]
fn test_group_leave_no_init() {
    let err = extract_err(group_leave("test_group", &[]));
    assert!(!err.is_success());
}

/// group_leave with info returns error (not initialized).
#[test]
fn test_group_leave_with_info() {
    let info = InfoBuilder::new().build();
    let err = extract_err(group_leave("test_group", &[info]));
    assert!(!err.is_success());
}

/// group_leave is deterministic.
#[test]
fn test_group_leave_deterministic() {
    let err1 = extract_err(group_leave("test_group", &[]));
    let err2 = extract_err(group_leave("test_group", &[]));
    assert_eq!(err1, err2);
}

/// group_leave repeated calls are idempotent.
#[test]
fn test_group_leave_idempotent() {
    let err1 = extract_err(group_leave("test_group", &[]));
    let err2 = extract_err(group_leave("test_group", &[]));
    let err3 = extract_err(group_leave("test_group", &[]));
    assert_eq!(err1, err2);
    assert_eq!(err2, err3);
}

/// group_leave with multiple info entries returns error.
#[test]
fn test_group_leave_multiple_info() {
    let i1 = InfoBuilder::new().build();
    let i2 = InfoBuilder::new().build();
    let err = extract_err(group_leave("test_group", &[i1, i2]));
    assert!(!err.is_success());
}

/// group_leave error type is consistent across calls.
#[test]
fn test_group_leave_error_consistent() {
    let err = extract_err(group_leave("test_group", &[]));
    // Should be a non-success error, not a panic
    assert!(!err.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave_nb — non-blocking group leave
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave_nb with empty group_id returns error, callback not invoked.
#[test]
fn test_group_leave_nb_empty_group_id() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let err = unwrap_err_result(group_leave_nb("", &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_leave_nb with valid inputs returns error (not initialized).
#[test]
fn test_group_leave_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let err = unwrap_err_result(group_leave_nb("test_group", &[], cb));
    assert!(!err.is_success());
}

/// group_leave_nb with info returns error (not initialized).
#[test]
fn test_group_leave_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let info = InfoBuilder::new().build();
    let err = unwrap_err_result(group_leave_nb("test_group", &[info], cb));
    assert!(!err.is_success());
}

/// group_leave_nb is deterministic for validation errors.
#[test]
fn test_group_leave_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        c1.store(true, Ordering::SeqCst);
    });
    let err1 = unwrap_err_result(group_leave_nb("", &[], cb1));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        c2.store(true, Ordering::SeqCst);
    });
    let err2 = unwrap_err_result(group_leave_nb("", &[], cb2));

    assert_eq!(err1, err2);
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// GroupLeaveCallbackWrapper::new works.
#[test]
fn test_group_leave_callback_wrapper_new() {
    let _cb = GroupLeaveCallbackWrapper::new(|_status: PmixStatus| {});
}

/// group_leave_nb with multiple info entries returns error.
#[test]
fn test_group_leave_nb_multiple_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let i1 = InfoBuilder::new().build();
    let i2 = InfoBuilder::new().build();
    let err = unwrap_err_result(group_leave_nb("test_group", &[i1, i2], cb));
    assert!(!err.is_success());
}

/// group_leave_nb repeated calls are idempotent.
#[test]
fn test_group_leave_nb_idempotent() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        c1.store(true, Ordering::SeqCst);
    });
    let err1 = unwrap_err_result(group_leave_nb("test_group", &[], cb1));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 = GroupLeaveCallbackWrapper::new(move |_status: PmixStatus| {
        c2.store(true, Ordering::SeqCst);
    });
    let err2 = unwrap_err_result(group_leave_nb("test_group", &[], cb2));

    assert_eq!(err1, err2);
}

// ─────────────────────────────────────────────────────────────────────────────
// #[ignore] tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave success path returns Ok(()).
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_group_leave_success() {
    // Requires PMIx_Init + server. See lines 642-668 in groups.rs.
}

/// group_leave_nb success path invokes callback.
#[test]
#[ignore = "requires PMIx_Init — callback bridge only invoked by C library"]
fn test_group_leave_nb_success() {
    // Requires PMIx_Init + server. See lines 698-751 in groups.rs.
}
