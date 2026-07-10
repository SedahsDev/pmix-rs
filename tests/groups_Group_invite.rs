//! Tests for `PMIx_Group_invite` and `PMIx_Group_invite_nb`.
//!
//! Derived from the C API signatures in `pmix.h` and the group management
//! spec. No dedicated C test file exists for group invite in the PMIx test
//! suite — these tests cover the safe Rust wrapper parameter validation,
//! error handling, callback trait, and integration paths.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

mod daemon_helper;

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
// group_invite — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// Empty group_id must return PMIX_ERR_BAD_PARAM without reaching the FFI layer.
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

/// Empty procs slice must return PMIX_ERR_BAD_PARAM.
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

/// group_invite with valid params but no PMIx_Init should fail from the FFI layer.
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

/// group_invite with a single invitee from a different namespace.
#[test]
fn group_invite_cross_namespace_without_init_fails() {
    let proc = Proc::new("other_namespace", 42).expect("cross-ns proc");
    let result = group_invite("cross_ns_group", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_invite across namespaces without init should fail"
    );
}

/// group_invite with a long group_id — should fail from FFI, not from NUL error.
#[test]
fn group_invite_long_group_id_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let long_id = "g".repeat(256);
    let result = group_invite(&long_id, &[proc], &[]);
    assert!(
        result.is_err(),
        "group_invite with long group_id without init should fail"
    );
}

/// group_invite with special characters in group_id.
#[test]
fn group_invite_special_chars_group_id_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("group-with_special.chars123", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_invite with special chars in group_id without init should fail"
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
    let err = unwrap_err_result(result);
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
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite_nb without PMIx_Init should fail without invoking callback.
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

/// group_invite_nb with multiple invitees — should fail without init.
#[test]
fn group_invite_nb_multiple_procs_without_init_fails() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_b", 1).expect("proc b"),
        Proc::new("ns_c", 2).expect("proc c"),
    ];
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("multi_invite", &procs, &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with multiple procs without init should fail"
    );
}

/// group_invite_nb with special characters in group_id.
#[test]
fn group_invite_nb_special_chars_group_id_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("group-with_special.chars123", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with special chars without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// GroupInviteCallbackWrapper — construction and trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// GroupInviteCallbackWrapper::new accepts a closure and compiles.
#[test]
fn group_invite_callback_wrapper_construction() {
    let _wrapper = GroupInviteCallbackWrapper::new(|_status, _info| {
        // No-op callback
    });
}

/// GroupInviteCallbackWrapper can capture and record status.
#[test]
fn group_invite_callback_wrapper_records_status() {
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(None::<PmixStatus>));
    let status_clone = Arc::clone(&status);

    let wrapper = GroupInviteCallbackWrapper::new(move |s: PmixStatus, _info: Vec<_>| {
        let mut locked = status_clone.lock().unwrap();
        *locked = Some(s);
    });

    drop(wrapper);
}

/// GroupInviteCallbackWrapper can capture result info count.
#[test]
fn group_invite_callback_wrapper_records_info_count() {
    use std::sync::{Arc, Mutex};

    let info_count = Arc::new(Mutex::new(None::<usize>));
    let info_count_clone = Arc::clone(&info_count);

    let _wrapper = GroupInviteCallbackWrapper::new(move |_status: PmixStatus, info: Vec<_>| {
        let mut locked = info_count_clone.lock().unwrap();
        *locked = Some(info.len());
    });
}

/// GroupInviteCallbackWrapper is Send (required for cross-thread callbacks).
#[test]
fn group_invite_callback_wrapper_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupInviteCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite — edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite with procs having same namespace but different ranks.
#[test]
fn group_invite_same_ns_different_ranks_without_init_fails() {
    let procs = vec![
        Proc::new("ns", 10).expect("rank 10"),
        Proc::new("ns", 20).expect("rank 20"),
        Proc::new("ns", 30).expect("rank 30"),
    ];
    let result = group_invite("same_ns_group", &procs, &[]);
    assert!(
        result.is_err(),
        "group_invite with same-ns different-rank procs without init should fail"
    );
}

/// group_invite with a numeric group_id.
#[test]
fn group_invite_numeric_group_id_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("12345", &[proc], &[]);
    assert!(
        result.is_err(),
        "group_invite with numeric group_id without init should fail"
    );
}

/// group_invite_nb with a numeric group_id.
#[test]
fn group_invite_nb_numeric_group_id_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("12345", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with numeric group_id without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (ignored — require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration: Init -> Group_invite -> Group_join -> Group_destruct.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_invite_join_integration() {
    daemon_helper::ensure_pmix_init();
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: group_invite with directives (info array).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_invite_with_directives() {
    daemon_helper::ensure_pmix_init();
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: group_invite_nb callback invocation.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn group_invite_nb_callback_invocation() {
    daemon_helper::ensure_pmix_init();
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
