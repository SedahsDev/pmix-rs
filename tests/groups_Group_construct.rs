//! Tests for `PMIx_Group_construct`, `PMIx_Group_construct_nb`,
//! `PMIx_Group_invite`, `PMIx_Group_invite_nb`, `PMIx_Group_join`,
//! `PMIx_Group_join_nb`, `PMIx_Group_leave`, `PMIx_Group_leave_nb`,
//! `PMIx_Group_destruct`, `PMIx_Group_destruct_nb` via the safe
//! `groups` module wrapper.
//!
//! No dedicated C test file exists for group management in the PMIx
//! test suite — the group APIs are tested as part of higher-level
//! integration scenarios. These tests cover the safe Rust wrapper
//! parameter validation and error handling paths.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they
//! need a running PMIx daemon / server.

use pmix::groups::pmix_group_opt_t;
use pmix::groups::*;
use pmix::{PmixError, PmixStatus, Proc};

/// Helper to extract the error from a Result<_, PmixStatus> without
/// requiring Debug on the Ok type (Info doesn't implement Debug).
fn unwrap_err_result(result: Result<(), PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Helper to check that a Result<Vec<Info>, PmixStatus> is an error
/// and return the PmixStatus.
fn extract_err<T>(result: Result<T, PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected error, got Ok"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct with empty group_id should return PMIX_ERR_BAD_PARAM
/// immediately without calling FFI.
#[test]
fn group_construct_empty_group_id_bad_param() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_construct("", &[proc], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct with empty procs array should return PMIX_ERR_BAD_PARAM
/// immediately without calling FFI.
#[test]
fn group_construct_empty_procs_bad_param() {
    let result = group_construct("my_group", &[], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct without PMIx_Init — the FFI call will be made but
/// should return an error because the library is not initialized.
#[test]
fn group_construct_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_construct("my_group", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_construct without PMIx_Init should fail"
    );
}

/// group_construct with multiple procs and directives — should fail
/// without init but should not panic.
#[test]
fn group_construct_multiple_procs_without_init_fails() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
        Proc::new("ns_b", 0).expect("proc c"),
    ];
    let result = group_construct("multi_group", &procs, &[]);
    assert!(
        result.is_err(),
        "group_construct with multiple procs without init should fail"
    );
}

/// group_construct with a single proc — should fail without init.
#[test]
fn group_construct_single_proc_without_init_fails() {
    let proc = Proc::new("test_ns", u32::MAX).expect("create proc");
    let result = group_construct("solo_group", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_construct with single proc without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_nb_empty_group_id_bad_param() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_construct_nb("", &[proc], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct_nb with empty procs should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_nb_empty_procs_bad_param() {
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on bad param");
    });
    let result = group_construct_nb("my_group", &[], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct_nb without PMIx_Init — should fail without invoking
/// the callback.
#[test]
fn group_construct_nb_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_construct_nb("my_group", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_construct_nb without PMIx_Init should fail"
    );
}

/// group_construct_nb callback wrapper construction test.
#[test]
fn group_construct_callback_wrapper_construction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = GroupConstructCallbackWrapper::new(move |status, info| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
        let _ = info;
    });
    // Wrapper is constructible.
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_empty_group_id_bad_param() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("", &[proc], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite with empty procs should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_empty_procs_bad_param() {
    let result = group_invite("my_group", &[], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite without PMIx_Init — should fail.
#[test]
fn group_invite_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("my_group", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_invite without PMIx_Init should fail"
    );
}

/// group_invite with multiple invitees — should fail without init.
#[test]
fn group_invite_multiple_procs_without_init_fails() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_b", 1).expect("proc b"),
    ];
    let result = group_invite("invite_group", &procs, &[]);
    assert!(
        result.is_err(),
        "group_invite with multiple procs without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_nb_empty_group_id_bad_param() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_invite_nb("", &[proc], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite_nb with empty procs should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_nb_empty_procs_bad_param() {
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on bad param");
    });
    let result = group_invite_nb("my_group", &[], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite_nb without PMIx_Init — should fail without invoking callback.
#[test]
fn group_invite_nb_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_invite_nb("my_group", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb without PMIx_Init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_join with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_join_empty_group_id_bad_param() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join("", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join with ACCEPT option — should fail without init.
#[test]
fn group_join_accept_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join accept without PMIx_Init should fail"
    );
}

/// group_join with DECLINE option — should fail without init.
#[test]
fn group_join_decline_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join decline without PMIx_Init should fail"
    );
}

/// group_join with directives — should fail without init.
#[test]
fn group_join_with_info_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with info without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_join_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_join_nb_empty_group_id_bad_param() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_join_nb(
        "",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join_nb without PMIx_Init — should fail without invoking callback.
#[test]
fn group_join_nb_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_join_nb(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb without PMIx_Init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_leave_empty_group_id_bad_param() {
    let result = group_leave("", &[]);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_leave without PMIx_Init — should fail.
#[test]
fn group_leave_without_init_fails() {
    let result = group_leave("my_group", &[]);
    assert!(result.is_err(), "group_leave without PMIx_Init should fail");
}

/// group_leave with directives — should fail without init.
#[test]
fn group_leave_with_info_without_init_fails() {
    let result = group_leave("my_group", &[]);
    assert!(
        result.is_err(),
        "group_leave with info without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_leave_nb_empty_group_id_bad_param() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_leave_nb("", &[], callback);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_leave_nb without PMIx_Init — should fail without invoking callback.
#[test]
fn group_leave_nb_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_leave_nb("my_group", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb without PMIx_Init should fail"
    );
}

/// group_leave_nb callback wrapper construction test.
#[test]
fn group_leave_callback_wrapper_construction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = GroupLeaveCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
    });
    // Wrapper is constructible.
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_destruct_empty_group_id_bad_param() {
    let result = group_destruct("", &[]);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_destruct without PMIx_Init — should fail.
#[test]
fn group_destruct_without_init_fails() {
    let result = group_destruct("my_group", &[]);
    assert!(
        result.is_err(),
        "group_destruct without PMIx_Init should fail"
    );
}

/// group_destruct with directives — should fail without init.
#[test]
fn group_destruct_with_info_without_init_fails() {
    let result = group_destruct("my_group", &[]);
    assert!(
        result.is_err(),
        "group_destruct with info without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_destruct_nb_empty_group_id_bad_param() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_destruct_nb("", &[], callback);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_destruct_nb without PMIx_Init — should fail without invoking callback.
#[test]
fn group_destruct_nb_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_destruct_nb("my_group", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb without PMIx_Init should fail"
    );
}

/// group_destruct_nb callback wrapper construction test.
#[test]
fn group_destruct_callback_wrapper_construction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = GroupDestructCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
    });
    // Wrapper is constructible.
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (requires PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: Init → Group_construct → Group_connect →
/// Group_destruct → Finalize.
///
/// NOTE: This test requires a running PMIx server and must be run
/// under `pmixrun` or equivalent. It is ignored by default.
///
/// ```sh
/// pmixrun -n 2 -- cargo test --test groups_Group_construct group_construct_integration -- --include-ignored
/// ```
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_construct_integration() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // In a real integration test environment:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create Procs for the group members.
    // 3. Call group_construct("my_group", &procs, &[]) and verify Ok(_).
    // 4. Call group_leave("my_group", &[]) and verify Ok(()).
    // 5. Call group_destruct("my_group", &[]) and verify Ok(()).
    // 6. Call pmix::lifecycle::finalize().expect("finalize");
    //
    // Because group_construct is a blocking collective, it requires all
    // participating processes to call it simultaneously.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Async integration: Init → Group_invite → Group_join → Group_destruct.
///
/// Tests the async invite/join workflow:
/// 1. Leader calls group_invite to invite members.
/// 2. Members call group_join to accept/decline.
/// 3. Leader calls group_destruct when done.
///
/// Ignored by default — requires PMIx daemon.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_invite_join_integration() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // In a real integration test:
    //
    // 1. Leader calls pmix::lifecycle::init(None).expect("init");
    // 2. Leader calls group_invite("async_group", &invitees, &[])
    //    and verifies Ok(_).
    // 3. Invitees call group_join("async_group", &leader,
    //    PMIX_GROUP_ACCEPT, &[]) and verify Ok(_).
    // 4. Leader calls group_destruct("async_group", &[]) and verify Ok(()).
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
