//! Tests for `PMIx_Group_join` and `PMIx_Group_join_nb`.
//!
//! Derived from the C API signatures in `pmix.h` and the group management
//! spec. No dedicated C test file exists for group join in the PMIx test
//! suite — these tests cover the safe Rust wrapper parameter validation,
//! error handling, callback trait, and integration paths.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::groups::*;
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the error from a Result<Vec<Info>, PmixStatus>.
fn extract_err<T>(result: Result<T, PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err(PmixStatus), got Ok"),
    }
}

/// Extract the error from a Result<(), PmixStatus> (used by _nb variants).
fn unwrap_err_result(result: Result<(), PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err(PmixStatus), got Ok"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// Empty group_id must return PMIX_ERR_BAD_PARAM without reaching the FFI layer.
#[test]
fn group_join_empty_group_id_bad_param() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join("", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join with valid group_id but no PMIx_Init should fail from the FFI layer.
#[test]
fn group_join_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(result.is_err(), "group_join without PMIx_Init should fail");
}

/// group_join with PMIX_GROUP_DECLINE option — should fail without init.
#[test]
fn group_join_decline_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with decline option without init should fail"
    );
}

/// group_join with a leader from a different namespace.
#[test]
fn group_join_cross_namespace_leader_without_init_fails() {
    let leader = Proc::new("other_namespace", 42).expect("cross-ns leader");
    let result = group_join(
        "cross_ns_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with cross-namespace leader without init should fail"
    );
}

/// group_join with a long group_id — should fail from FFI, not from NUL error.
#[test]
fn group_join_long_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let long_id = "g".repeat(256);
    let result = group_join(&long_id, &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    assert!(
        result.is_err(),
        "group_join with long group_id without init should fail"
    );
}

/// group_join with special characters in group_id.
#[test]
fn group_join_special_chars_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join(
        "group-with_special.chars123",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with special chars in group_id without init should fail"
    );
}

/// group_join with a leader having rank 0 in the same namespace.
#[test]
fn group_join_leader_rank_zero_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("rank 0 leader");
    let result = group_join(
        "rank_zero_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with rank-0 leader without init should fail"
    );
}

/// group_join with a leader having a high rank value.
#[test]
fn group_join_leader_high_rank_without_init_fails() {
    let leader = Proc::new("test_ns", 9999).expect("high rank leader");
    let result = group_join(
        "high_rank_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with high-rank leader without init should fail"
    );
}

/// group_join with a numeric group_id.
#[test]
fn group_join_numeric_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join("12345", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    assert!(
        result.is_err(),
        "group_join with numeric group_id without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_join_nb with empty group_id should return PMIX_ERR_BAD_PARAM.
#[test]
fn group_join_nb_empty_group_id_bad_param() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
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
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join_nb without PMIx_Init should fail without invoking callback.
#[test]
fn group_join_nb_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
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

/// group_join_nb with PMIX_GROUP_DECLINE option — should fail without init.
#[test]
fn group_join_nb_decline_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb(
        "my_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with decline option without init should fail"
    );
}

/// group_join_nb with a cross-namespace leader — should fail without init.
#[test]
fn group_join_nb_cross_namespace_leader_without_init_fails() {
    let leader = Proc::new("other_namespace", 42).expect("cross-ns leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb(
        "cross_ns_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with cross-ns leader without init should fail"
    );
}

/// group_join_nb with special characters in group_id.
#[test]
fn group_join_nb_special_chars_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb(
        "group-with_special.chars123",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with special chars without init should fail"
    );
}

/// group_join_nb with a numeric group_id.
#[test]
fn group_join_nb_numeric_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb(
        "12345",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with numeric group_id without init should fail"
    );
}

/// group_join_nb with a long group_id.
#[test]
fn group_join_nb_long_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let long_id = "g".repeat(256);
    let result = group_join_nb(
        &long_id,
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with long group_id without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// GroupJoinCallbackWrapper — construction and trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// GroupJoinCallbackWrapper::new accepts a closure and compiles.
#[test]
fn group_join_callback_wrapper_construction() {
    let _wrapper = GroupJoinCallbackWrapper::new(|_status, _info| {
        // No-op callback
    });
}

/// GroupJoinCallbackWrapper can capture and record status.
#[test]
fn group_join_callback_wrapper_records_status() {
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(None::<PmixStatus>));
    let status_clone = Arc::clone(&status);

    let wrapper = GroupJoinCallbackWrapper::new(move |s: PmixStatus, _info: Vec<_>| {
        let mut locked = status_clone.lock().unwrap();
        *locked = Some(s);
    });

    drop(wrapper);
}

/// GroupJoinCallbackWrapper can capture result info count.
#[test]
fn group_join_callback_wrapper_records_info_count() {
    use std::sync::{Arc, Mutex};

    let info_count = Arc::new(Mutex::new(None::<usize>));
    let info_count_clone = Arc::clone(&info_count);

    let _wrapper = GroupJoinCallbackWrapper::new(move |_status: PmixStatus, info: Vec<_>| {
        let mut locked = info_count_clone.lock().unwrap();
        *locked = Some(info.len());
    });
}

/// GroupJoinCallbackWrapper is Send (required for cross-thread callbacks).
#[test]
fn group_join_callback_wrapper_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupJoinCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// group_join with a leader having rank MAX_U32.
#[test]
fn group_join_leader_max_rank_without_init_fails() {
    let leader = Proc::new("test_ns", u32::MAX).expect("max rank leader");
    let result = group_join(
        "max_rank_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with max-rank leader without init should fail"
    );
}

/// group_join with empty namespace leader.
#[test]
fn group_join_leader_empty_namespace_without_init_fails() {
    let leader = Proc::new("", 0).expect("empty ns leader");
    let result = group_join(
        "empty_ns_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    assert!(
        result.is_err(),
        "group_join with empty-ns leader without init should fail"
    );
}

/// group_join with a single-character group_id.
#[test]
fn group_join_single_char_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let result = group_join("x", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    assert!(
        result.is_err(),
        "group_join with single-char group_id without init should fail"
    );
}

/// group_join_nb with a single-character group_id.
#[test]
fn group_join_nb_single_char_group_id_without_init_fails() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb(
        "x",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        callback,
    );
    assert!(
        result.is_err(),
        "group_join_nb with single-char group_id without init should fail"
    );
}

/// group_join with both accept and decline options tested sequentially.
#[test]
fn group_join_both_options_without_init_fail() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let accept_result = group_join(
        "test_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    );
    let decline_result = group_join(
        "test_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
    );
    assert!(accept_result.is_err(), "accept should fail without init");
    assert!(decline_result.is_err(), "decline should fail without init");
}

/// group_join_nb with both accept and decline options tested sequentially.
#[test]
fn group_join_nb_both_options_without_init_fail() {
    let leader = Proc::new("test_ns", 0).expect("create leader proc");
    let accept_cb = GroupJoinCallbackWrapper::new(|_, _| {});
    let decline_cb = GroupJoinCallbackWrapper::new(|_, _| {});
    let accept_result = group_join_nb(
        "test_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
        accept_cb,
    );
    let decline_result = group_join_nb(
        "test_group",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        &[],
        decline_cb,
    );
    assert!(accept_result.is_err(), "accept should fail without init");
    assert!(decline_result.is_err(), "decline should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (ignored — require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration: Init -> Group_construct -> Group_join (accept) -> Group_destruct.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_join_accept_integration() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: Init -> Group_invite -> Group_join (decline) -> Group_destruct.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_join_decline_integration() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: group_join_nb callback invocation.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_join_nb_callback_invocation() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
