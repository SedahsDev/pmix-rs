//! Wrapper tests for groups.rs — Group Construct/Destruct Core.
//!
//! Tests exercise `group_construct`, `group_construct_nb`, `group_destruct`,
//! `group_destruct_nb` wrapper logic without PMIx_Init.
//! Input validation (empty group_id, empty procs) returns PMIX_ERR_BAD_PARAM
//! synchronously without hitting FFI. FFI calls return errors gracefully.

use pmix::groups::*;
use pmix::{InfoBuilder, PmixError, PmixStatus, Proc};

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
// group_construct — synchronous group construction
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_construct_empty_group_id() {
    let err = extract_err(group_construct("", &[], &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_construct with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_construct_empty_procs() {
    let err = extract_err(group_construct("test_group", &[], &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_construct with valid group_id and procs returns error (not initialized).
#[test]
fn test_group_construct_no_init() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_construct("test_group", &[proc], &[]));
    assert!(!err.is_success());
}

/// group_construct with directives returns error (not initialized).
#[test]
fn test_group_construct_with_directives() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let directive = InfoBuilder::new().build();
    let err = extract_err(group_construct("test_group", &[proc], &[directive]));
    assert!(!err.is_success());
}

/// group_construct is deterministic.
#[test]
fn test_group_construct_deterministic() {
    let err1 = extract_err(group_construct("test_group", &[], &[]));
    let err2 = extract_err(group_construct("test_group", &[], &[]));
    assert_eq!(err1, err2);
}

/// group_construct with both empty group_id and empty procs returns error.
#[test]
fn test_group_construct_both_empty() {
    let err = extract_err(group_construct("", &[], &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_construct repeated calls are idempotent.
#[test]
fn test_group_construct_idempotent() {
    let err1 = extract_err(group_construct("test_group", &[], &[]));
    let err2 = extract_err(group_construct("test_group", &[], &[]));
    let err3 = extract_err(group_construct("test_group", &[], &[]));
    assert_eq!(err1, err2);
    assert_eq!(err2, err3);
}

/// group_construct with multiple procs returns error (not initialized).
#[test]
fn test_group_construct_multiple_procs() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
        Proc::new("ns_b", 0).expect("proc c"),
    ];
    let err = extract_err(group_construct("test_group", &procs, &[]));
    assert!(!err.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct_nb — non-blocking group construction
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct_nb with empty group_id returns error, callback not invoked.
#[test]
fn test_group_construct_nb_empty_group_id() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        },
    );
    let err = extract_err(group_construct_nb("", &[], &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_construct_nb with empty procs returns error, callback not invoked.
#[test]
fn test_group_construct_nb_empty_procs() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        },
    );
    let err = extract_err(group_construct_nb("test_group", &[], &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_construct_nb with valid inputs returns error (not initialized).
#[test]
fn test_group_construct_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        },
    );
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_construct_nb("test_group", &[proc], &[], cb));
    assert!(!err.is_success());
}

/// group_construct_nb with info returns error (not initialized).
#[test]
fn test_group_construct_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        },
    );
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let err = extract_err(group_construct_nb("test_group", &[proc], &[info], cb));
    assert!(!err.is_success());
}

/// group_construct_nb is deterministic for validation errors.
#[test]
fn test_group_construct_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c1.store(true, Ordering::SeqCst);
        },
    );
    let err1 = extract_err(group_construct_nb("", &[], &[], cb1));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 = GroupConstructCallbackWrapper::new(
        move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c2.store(true, Ordering::SeqCst);
        },
    );
    let err2 = extract_err(group_construct_nb("", &[], &[], cb2));

    assert_eq!(err1, err2);
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// GroupConstructCallbackWrapper::new works.
#[test]
fn test_group_construct_callback_wrapper_new() {
    let _cb =
        GroupConstructCallbackWrapper::new(|_status: PmixStatus, _results: Vec<pmix::Info>| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct — synchronous group destruction
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_destruct_empty_group_id() {
    let err = unwrap_err_result(group_destruct("", &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_destruct with valid group_id returns error (not initialized).
#[test]
fn test_group_destruct_no_init() {
    let err = unwrap_err_result(group_destruct("test_group", &[]));
    assert!(!err.is_success());
}

/// group_destruct with info returns error (not initialized).
#[test]
fn test_group_destruct_with_info() {
    let info = InfoBuilder::new().build();
    let err = unwrap_err_result(group_destruct("test_group", &[info]));
    assert!(!err.is_success());
}

/// group_destruct is deterministic.
#[test]
fn test_group_destruct_deterministic() {
    let err1 = unwrap_err_result(group_destruct("test_group", &[]));
    let err2 = unwrap_err_result(group_destruct("test_group", &[]));
    assert_eq!(err1, err2);
}

/// group_destruct repeated calls are idempotent.
#[test]
fn test_group_destruct_idempotent() {
    let err1 = unwrap_err_result(group_destruct("test_group", &[]));
    let err2 = unwrap_err_result(group_destruct("test_group", &[]));
    let err3 = unwrap_err_result(group_destruct("test_group", &[]));
    assert_eq!(err1, err2);
    assert_eq!(err2, err3);
}

/// group_destruct with multiple info entries returns error.
#[test]
fn test_group_destruct_multiple_info() {
    let i1 = InfoBuilder::new().build();
    let i2 = InfoBuilder::new().build();
    let err = unwrap_err_result(group_destruct("test_group", &[i1, i2]));
    assert!(!err.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct_nb — non-blocking group destruction
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with empty group_id returns error, callback not invoked.
#[test]
fn test_group_destruct_nb_empty_group_id() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupDestructCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let err = unwrap_err_result(group_destruct_nb("", &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_destruct_nb with valid group_id returns error (not initialized).
#[test]
fn test_group_destruct_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupDestructCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let err = unwrap_err_result(group_destruct_nb("test_group", &[], cb));
    assert!(!err.is_success());
}

/// group_destruct_nb with info returns error (not initialized).
#[test]
fn test_group_destruct_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb = GroupDestructCallbackWrapper::new(move |_status: PmixStatus| {
        called_clone.store(true, Ordering::SeqCst);
    });
    let info = InfoBuilder::new().build();
    let err = unwrap_err_result(group_destruct_nb("test_group", &[info], cb));
    assert!(!err.is_success());
}

/// group_destruct_nb is deterministic for validation errors.
#[test]
fn test_group_destruct_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 = GroupDestructCallbackWrapper::new(move |_status: PmixStatus| {
        c1.store(true, Ordering::SeqCst);
    });
    let err1 = unwrap_err_result(group_destruct_nb("", &[], cb1));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 = GroupDestructCallbackWrapper::new(move |_status: PmixStatus| {
        c2.store(true, Ordering::SeqCst);
    });
    let err2 = unwrap_err_result(group_destruct_nb("", &[], cb2));

    assert_eq!(err1, err2);
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// GroupDestructCallbackWrapper::new works.
#[test]
fn test_group_destruct_callback_wrapper_new() {
    let _cb = GroupDestructCallbackWrapper::new(|_status: PmixStatus| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// #[ignore] tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct success path returns Vec<Info>.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_group_construct_success() {
    // Requires PMIx_Init + server. See lines 63-112 in groups.rs.
}

/// group_construct_nb success path invokes callback.
#[test]
#[ignore = "requires PMIx_Init — callback bridge only invoked by C library"]
fn test_group_construct_nb_success() {
    // Requires PMIx_Init + server. See lines 165-229 in groups.rs.
}

/// group_destruct success path returns Ok(()).
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_group_destruct_success() {
    // Requires PMIx_Init + server. See lines 767-779 in groups.rs.
}

/// group_destruct_nb success path invokes callback.
#[test]
#[ignore = "requires PMIx_Init — callback bridge only invoked by C library"]
fn test_group_destruct_nb_success() {
    // Requires PMIx_Init + server. See lines 825-859 in groups.rs.
}
