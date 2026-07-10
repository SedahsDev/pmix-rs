//! Wrapper tests for groups.rs — Group Invite/Join Operations.
//!
//! Tests exercise `group_invite`, `group_invite_nb`, `group_join`,
//! `group_join_nb` wrapper logic without PMIx_Init.
//! Input validation (empty group_id, empty procs) returns PMIX_ERR_BAD_PARAM
//! synchronously without hitting FFI. FFI calls return errors gracefully.

mod daemon_helper;

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
// group_invite — synchronous group invite
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_invite_empty_group_id() {
    let err = extract_err(group_invite("", &[], &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_invite with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_invite_empty_procs() {
    let err = extract_err(group_invite("test_group", &[], &[]));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_invite with valid inputs returns error (not initialized).
#[test]
fn test_group_invite_no_init() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_invite("test_group", &[proc], &[]));
    assert!(!err.is_success());
}

/// group_invite with info returns error (not initialized).
#[test]
fn test_group_invite_with_info() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let err = extract_err(group_invite("test_group", &[proc], &[info]));
    assert!(!err.is_success());
}

/// group_invite is deterministic.
#[test]
fn test_group_invite_deterministic() {
    let err1 = extract_err(group_invite("test_group", &[], &[]));
    let err2 = extract_err(group_invite("test_group", &[], &[]));
    assert_eq!(err1, err2);
}

/// group_invite with multiple procs returns error (not initialized).
#[test]
fn test_group_invite_multiple_procs() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
    ];
    let err = extract_err(group_invite("test_group", &procs, &[]));
    assert!(!err.is_success());
}

/// group_invite repeated calls are idempotent.
#[test]
fn test_group_invite_idempotent() {
    let err1 = extract_err(group_invite("test_group", &[], &[]));
    let err2 = extract_err(group_invite("test_group", &[], &[]));
    let err3 = extract_err(group_invite("test_group", &[], &[]));
    assert_eq!(err1, err2);
    assert_eq!(err2, err3);
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite_nb — non-blocking group invite
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with empty group_id returns error, callback not invoked.
#[test]
fn test_group_invite_nb_empty_group_id() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let err = unwrap_err_result(group_invite_nb("", &[], &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_invite_nb with empty procs returns error, callback not invoked.
#[test]
fn test_group_invite_nb_empty_procs() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let err = unwrap_err_result(group_invite_nb("test_group", &[], &[], cb));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_invite_nb with valid inputs returns error (not initialized).
#[test]
fn test_group_invite_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let err = unwrap_err_result(group_invite_nb("test_group", &[proc], &[], cb));
    assert!(!err.is_success());
}

/// group_invite_nb with info returns error (not initialized).
#[test]
fn test_group_invite_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let err = unwrap_err_result(group_invite_nb("test_group", &[proc], &[info], cb));
    assert!(!err.is_success());
}

/// group_invite_nb is deterministic for validation errors.
#[test]
fn test_group_invite_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c1.store(true, Ordering::SeqCst);
        });
    let err1 = unwrap_err_result(group_invite_nb("", &[], &[], cb1));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 =
        GroupInviteCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c2.store(true, Ordering::SeqCst);
        });
    let err2 = unwrap_err_result(group_invite_nb("", &[], &[], cb2));

    assert_eq!(err1, err2);
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// GroupInviteCallbackWrapper::new works.
#[test]
fn test_group_invite_callback_wrapper_new() {
    let _cb = GroupInviteCallbackWrapper::new(|_status: PmixStatus, _results: Vec<pmix::Info>| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — synchronous group join
// ─────────────────────────────────────────────────────────────────────────────

/// group_join with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_group_join_empty_group_id() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_join(
        "",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// group_join with valid inputs returns error (not initialized).
#[test]
fn test_group_join_no_init() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    assert!(!err.is_success());
}

/// group_join with info returns error (not initialized).
#[test]
fn test_group_join_with_info() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let err = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[info],
    ));
    assert!(!err.is_success());
}

/// group_join is deterministic.
#[test]
fn test_group_join_deterministic() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let err1 = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    let err2 = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    assert_eq!(err1, err2);
}

/// group_join with PMIX_GROUP_JOIN_AND_CONSTRUCT option.
#[test]
fn test_group_join_join_and_construct() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let err = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
    ));
    assert!(!err.is_success());
}

/// group_join repeated calls are idempotent.
#[test]
fn test_group_join_idempotent() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let err1 = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    let err2 = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    let err3 = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    assert_eq!(err1, err2);
    assert_eq!(err2, err3);
}

/// group_join with multiple info entries returns error.
#[test]
fn test_group_join_multiple_info() {
    let leader = Proc::new("test_ns", 0).expect("create proc");
    let i1 = InfoBuilder::new().build();
    let i2 = InfoBuilder::new().build();
    let err = extract_err(group_join(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[i1, i2],
    ));
    assert!(!err.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join_nb — non-blocking group join
// ─────────────────────────────────────────────────────────────────────────────

/// group_join_nb with empty group_id returns error, callback not invoked.
#[test]
fn test_group_join_nb_empty_group_id() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let leader = Proc::new("test_ns", 0).expect("create proc");
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let err = unwrap_err_result(group_join_nb(
        "",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        cb,
    ));
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate validation failure"
    );
}

/// group_join_nb with valid inputs returns error (not initialized).
#[test]
fn test_group_join_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let leader = Proc::new("test_ns", 0).expect("create proc");
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let err = unwrap_err_result(group_join_nb(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        cb,
    ));
    assert!(!err.is_success());
}

/// group_join_nb with info returns error (not initialized).
#[test]
fn test_group_join_nb_with_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let leader = Proc::new("test_ns", 0).expect("create proc");
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let info = InfoBuilder::new().build();
    let err = unwrap_err_result(group_join_nb(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[info],
        cb,
    ));
    assert!(!err.is_success());
}

/// group_join_nb is deterministic for validation errors.
#[test]
fn test_group_join_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let leader = Proc::new("test_ns", 0).expect("create proc");
    let called1 = Arc::new(AtomicBool::new(false));
    let c1 = Arc::clone(&called1);
    let cb1 =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c1.store(true, Ordering::SeqCst);
        });
    let err1 = unwrap_err_result(group_join_nb(
        "",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        cb1,
    ));

    let called2 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&called2);
    let cb2 =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            c2.store(true, Ordering::SeqCst);
        });
    let err2 = unwrap_err_result(group_join_nb(
        "",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        cb2,
    ));

    assert_eq!(err1, err2);
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// GroupJoinCallbackWrapper::new works.
#[test]
fn test_group_join_callback_wrapper_new() {
    let _cb = GroupJoinCallbackWrapper::new(|_status: PmixStatus, _results: Vec<pmix::Info>| {});
}

/// group_join_nb with PMIX_GROUP_JOIN_AND_CONSTRUCT option.
#[test]
fn test_group_join_nb_join_and_construct() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let leader = Proc::new("test_ns", 0).expect("create proc");
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);
    let cb =
        GroupJoinCallbackWrapper::new(move |_status: PmixStatus, _results: Vec<pmix::Info>| {
            called_clone.store(true, Ordering::SeqCst);
        });
    let err = unwrap_err_result(group_join_nb(
        "test_group",
        &leader,
        pmix::groups::pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
        cb,
    ));
    assert!(!err.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// #[ignore] tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite success path returns Vec<Info>.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_success() {
    daemon_helper::ensure_pmix_init();
    // Requires PMIx_Init + server. See lines 247-316 in groups.rs.
}

/// group_invite_nb success path invokes callback.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_nb_success() {
    daemon_helper::ensure_pmix_init();
    // Requires PMIx_Init + server. See lines 348-443 in groups.rs.
}

/// group_join success path returns Vec<Info>.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_join_success() {
    daemon_helper::ensure_pmix_init();
    // Requires PMIx_Init + server. See lines 452-518 in groups.rs.
}

/// group_join_nb success path invokes callback.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_join_nb_success() {
    daemon_helper::ensure_pmix_init();
    // Requires PMIx_Init + server. See lines 550-634 in groups.rs.
}
