//! Lifecycle tests for all 10 group management functions:
//! group_construct, group_construct_nb, group_destruct, group_destruct_nb,
//! group_invite, group_invite_nb, group_join, group_join_nb,
//! group_leave, group_leave_nb.
//!
//! Focus: compile-time type checks, panic safety, callback traits, and error codes.
//!
//! Tests that call server_init_minimal corrupt C-level PMIx state and are
//! marked #[ignore] with reason.

use pmix::groups::*;
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the error from a Result<_, PmixStatus>, panicking on Ok.
fn extract_err<T>(result: Result<T, PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Extract the error from a Result<(), PmixStatus>.
fn unwrap_err_result(result: Result<(), PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err(PmixStatus), got Ok"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compile-time type checks — ensure callback wrappers accept the right traits
// ─────────────────────────────────────────────────────────────────────────────

/// GroupConstructCallbackWrapper accepts Fn(PmixStatus, Vec<Info>) + Send + 'static.
#[test]
fn compile_check_group_construct_callback_trait() {
    let _wrapper = GroupConstructCallbackWrapper::new(|_status, _info| {});
}

/// GroupDestructCallbackWrapper accepts Fn(PmixStatus) + Send + 'static.
#[test]
fn compile_check_group_destruct_callback_trait() {
    let _wrapper = GroupDestructCallbackWrapper::new(|_status| {});
}

/// GroupInviteCallbackWrapper accepts Fn(PmixStatus, Vec<Info>) + Send + 'static.
#[test]
fn compile_check_group_invite_callback_trait() {
    let _wrapper = GroupInviteCallbackWrapper::new(|_status, _info| {});
}

/// GroupJoinCallbackWrapper accepts Fn(PmixStatus, Vec<Info>) + Send + 'static.
#[test]
fn compile_check_group_join_callback_trait() {
    let _wrapper = GroupJoinCallbackWrapper::new(|_status, _info| {});
}

/// GroupLeaveCallbackWrapper accepts Fn(PmixStatus) + Send + 'static.
#[test]
fn compile_check_group_leave_callback_trait() {
    let _wrapper = GroupLeaveCallbackWrapper::new(|_status| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper Send bounds — required for FFI callback safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn group_construct_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupConstructCallbackWrapper>();
}

#[test]
fn group_destruct_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupDestructCallbackWrapper>();
}

#[test]
fn group_invite_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupInviteCallbackWrapper>();
}

#[test]
fn group_join_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupJoinCallbackWrapper>();
}

#[test]
fn group_leave_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupLeaveCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper construction — Arc, Mutex, atomic state capture
// ─────────────────────────────────────────────────────────────────────────────

/// GroupConstructCallbackWrapper captures Arc<AtomicBool> for call tracking.
#[test]
fn group_construct_callback_wrapper_arc_tracking() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = GroupConstructCallbackWrapper::new(move |status, info| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
        let _ = info;
    });
}

/// GroupDestructCallbackWrapper captures Arc<Mutex<Vec>> for shared state.
#[test]
fn group_destruct_callback_wrapper_arc_mutex() {
    use std::sync::Arc;
    use std::sync::Mutex;

    let shared = Arc::new(Mutex::new(Vec::<PmixStatus>::new()));
    let shared_clone = shared.clone();
    let _wrapper = GroupDestructCallbackWrapper::new(move |status| {
        shared_clone.lock().unwrap().push(status);
    });
}

/// GroupInviteCallbackWrapper captures multiple atomic state variables.
#[test]
fn group_invite_callback_wrapper_multi_state() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let info_count = Arc::new(AtomicUsize::new(0));
    let called_clone = called.clone();
    let info_clone = info_count.clone();
    let _wrapper = GroupInviteCallbackWrapper::new(move |status, info| {
        called_clone.store(true, Ordering::SeqCst);
        info_clone.store(info.len(), Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
    });
}

/// GroupJoinCallbackWrapper captures status code in AtomicI32.
#[test]
fn group_join_callback_wrapper_status_capture() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    let status_code = Arc::new(AtomicI32::new(999));
    let status_clone = status_code.clone();
    let _wrapper = GroupJoinCallbackWrapper::new(move |status, _info| {
        status_clone.store(status.to_raw(), Ordering::SeqCst);
    });
}

/// GroupLeaveCallbackWrapper captures raw status code.
#[test]
fn group_leave_callback_wrapper_status_capture() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    let status_code = Arc::new(AtomicI32::new(999));
    let status_clone = status_code.clone();
    let _wrapper = GroupLeaveCallbackWrapper::new(move |status| {
        status_clone.store(status.to_raw(), Ordering::SeqCst);
    });
}

/// All callback wrappers accept move closures capturing owned data.
#[test]
fn all_callback_wrappers_accept_move_closures() {
    let data = String::from("test_data");
    let _construct = GroupConstructCallbackWrapper::new(move |_s, _i| {
        let _ = &data;
    });

    let data2 = String::from("test_data2");
    let _destruct = GroupDestructCallbackWrapper::new(move |_s| {
        let _ = &data2;
    });

    let data3 = String::from("test_data3");
    let _invite = GroupInviteCallbackWrapper::new(move |_s, _i| {
        let _ = &data3;
    });

    let data4 = String::from("test_data4");
    let _join = GroupJoinCallbackWrapper::new(move |_s, _i| {
        let _ = &data4;
    });

    let data5 = String::from("test_data5");
    let _leave = GroupLeaveCallbackWrapper::new(move |_s| {
        let _ = &data5;
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_empty_group_id() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_construct("", &[proc], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_empty_procs() {
    let result = group_construct("my_group", &[], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_construct without PMIx_Init fails (FFI error).
#[test]
fn group_construct_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_construct("my_group", &[proc], &[]);
    assert!(result.is_err(), "group_construct without init should fail");
}

/// group_construct with multiple procs without init fails.
#[test]
fn group_construct_multi_procs_without_init() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
        Proc::new("ns_b", 0).expect("proc c"),
    ];
    let result = group_construct("multi_group", &procs, &[]);
    assert!(result.is_err(), "group_construct with multiple procs without init should fail");
}

/// group_construct with single proc at max rank without init fails.
#[test]
fn group_construct_max_rank_without_init() {
    let proc = Proc::new("test_ns", u32::MAX).expect("create proc");
    let result = group_construct("solo_group", &[proc], &[]);
    assert!(result.is_err(), "group_construct with max rank without init should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct_nb with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_nb_empty_group_id() {
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

/// group_construct_nb with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_construct_nb_empty_procs() {
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

/// group_construct_nb without PMIx_Init fails without invoking callback.
#[test]
fn group_construct_nb_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_construct_nb("my_group", &[proc], &[], callback);
    assert!(result.is_err(), "group_construct_nb without init should fail");
}

/// group_construct_nb error is a valid negative status code.
#[test]
fn group_construct_nb_error_is_valid_status() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_construct_nb("my_group", &[proc], &[], callback);
    let err = extract_err(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_destruct_empty_group_id() {
    let result = group_destruct("", &[]);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_destruct without PMIx_Init fails.
#[test]
fn group_destruct_without_init_fails() {
    let result = group_destruct("my_group", &[]);
    assert!(result.is_err(), "group_destruct without init should fail");
}

/// group_destruct with valid group_id returns a negative error code.
#[test]
fn group_destruct_error_is_valid_status() {
    let result = group_destruct("my_group", &[]);
    let err = unwrap_err_result(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_destruct_nb_empty_group_id() {
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

/// group_destruct_nb without PMIx_Init fails without invoking callback.
#[test]
fn group_destruct_nb_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_destruct_nb("my_group", &[], callback);
    assert!(result.is_err(), "group_destruct_nb without init should fail");
}

/// group_destruct_nb error is a valid negative status code.
#[test]
fn group_destruct_nb_error_is_valid_status() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_destruct_nb("my_group", &[], callback);
    let err = unwrap_err_result(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_empty_group_id() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("", &[proc], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_empty_procs() {
    let result = group_invite("my_group", &[], &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_invite without PMIx_Init fails.
#[test]
fn group_invite_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = group_invite("my_group", &[proc], &[]);
    assert!(result.is_err(), "group_invite without init should fail");
}

/// group_invite with multiple invitees without init fails.
#[test]
fn group_invite_multi_procs_without_init() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_b", 1).expect("proc b"),
    ];
    let result = group_invite("invite_group", &procs, &[]);
    assert!(result.is_err(), "group_invite with multiple procs without init should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_nb_empty_group_id() {
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

/// group_invite_nb with empty procs returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_invite_nb_empty_procs() {
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

/// group_invite_nb without PMIx_Init fails without invoking callback.
#[test]
fn group_invite_nb_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_invite_nb("my_group", &[proc], &[], callback);
    assert!(result.is_err(), "group_invite_nb without init should fail");
}

/// group_invite_nb error is a valid negative status code.
#[test]
fn group_invite_nb_error_is_valid_status() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("my_group", &[proc], &[], callback);
    let err = extract_err(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_join with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_join_empty_group_id() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join("", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join with ACCEPT option without init fails.
#[test]
fn group_join_accept_without_init() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    assert!(result.is_err(), "group_join accept without init should fail");
}

/// group_join with DECLINE option without init fails.
#[test]
fn group_join_decline_without_init() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    assert!(result.is_err(), "group_join decline without init should fail");
}

/// group_join error is a valid negative status code.
#[test]
fn group_join_error_is_valid_status() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let result = group_join("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
    let err = extract_err(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_join_nb with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_join_nb_empty_group_id() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_join_nb("", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_join_nb with ACCEPT option without init fails.
#[test]
fn group_join_nb_accept_without_init() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_join_nb("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[], callback);
    assert!(result.is_err(), "group_join_nb accept without init should fail");
}

/// group_join_nb with DECLINE option without init fails.
#[test]
fn group_join_nb_decline_without_init() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_join_nb("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[], callback);
    assert!(result.is_err(), "group_join_nb decline without init should fail");
}

/// group_join_nb error is a valid negative status code.
#[test]
fn group_join_nb_error_is_valid_status() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_join_nb("my_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[], callback);
    let err = extract_err(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_leave_empty_group_id() {
    let result = group_leave("", &[]);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_leave without PMIx_Init fails.
#[test]
fn group_leave_without_init_fails() {
    let result = group_leave("my_group", &[]);
    assert!(result.is_err(), "group_leave without init should fail");
}

/// group_leave error is a valid negative status code.
#[test]
fn group_leave_error_is_valid_status() {
    let result = group_leave("my_group", &[]);
    let err = unwrap_err_result(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave_nb with empty group_id returns PMIX_ERR_BAD_PARAM.
#[test]
fn group_leave_nb_empty_group_id() {
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

/// group_leave_nb without PMIx_Init fails without invoking callback.
#[test]
fn group_leave_nb_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_leave_nb("my_group", &[], callback);
    assert!(result.is_err(), "group_leave_nb without init should fail");
}

/// group_leave_nb error is a valid negative status code.
#[test]
fn group_leave_nb_error_is_valid_status() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("my_group", &[], callback);
    let err = unwrap_err_result(result);
    assert!(err.to_raw() < 0, "error status should be negative");
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety — functions should not panic on valid inputs
// ─────────────────────────────────────────────────────────────────────────────

/// group_construct should not panic with valid parameters (just fail from FFI).
#[test]
fn group_construct_no_panic_on_valid_params() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    // Should not panic — just return an error.
    let _result = group_construct("valid_group", &[proc], &[]);
}

/// group_destruct should not panic with valid parameters.
#[test]
fn group_destruct_no_panic_on_valid_params() {
    let _result = group_destruct("valid_group", &[]);
}

/// group_invite should not panic with valid parameters.
#[test]
fn group_invite_no_panic_on_valid_params() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let _result = group_invite("valid_group", &[proc], &[]);
}

/// group_join should not panic with valid parameters.
#[test]
fn group_join_no_panic_on_valid_params() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let _result = group_join("valid_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
}

/// group_leave should not panic with valid parameters.
#[test]
fn group_leave_no_panic_on_valid_params() {
    let _result = group_leave("valid_group", &[]);
}

/// group_construct_nb should not panic with valid parameters.
#[test]
fn group_construct_nb_no_panic_on_valid_params() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupConstructCallbackWrapper::new(|_status, _info| {});
    let _result = group_construct_nb("valid_group", &[proc], &[], callback);
}

/// group_destruct_nb should not panic with valid parameters.
#[test]
fn group_destruct_nb_no_panic_on_valid_params() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let _result = group_destruct_nb("valid_group", &[], callback);
}

/// group_invite_nb should not panic with valid parameters.
#[test]
fn group_invite_nb_no_panic_on_valid_params() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {});
    let _result = group_invite_nb("valid_group", &[proc], &[], callback);
}

/// group_join_nb should not panic with valid parameters.
#[test]
fn group_join_nb_no_panic_on_valid_params() {
    let leader = Proc::new("test_ns", 0).expect("create leader");
    let callback = GroupJoinCallbackWrapper::new(|_status, _info| {});
    let _result = group_join_nb("valid_group", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[], callback);
}

/// group_leave_nb should not panic with valid parameters.
#[test]
fn group_leave_nb_no_panic_on_valid_params() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {});
    let _result = group_leave_nb("valid_group", &[], callback);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code consistency — all functions return consistent error types
// ─────────────────────────────────────────────────────────────────────────────

/// All blocking group functions return the same error for empty group_id.
#[test]
fn all_blocking_functions_reject_empty_group_id() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let proc2 = proc.clone();
    let leader = Proc::new("test_ns", 0).expect("create leader");

    let construct_err = extract_err(group_construct("", &[proc], &[]));
    let destruct_err = unwrap_err_result(group_destruct("", &[]));
    let invite_err = extract_err(group_invite("", &[proc2], &[]));
    let join_err = extract_err(group_join(
        "",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &[],
    ));
    let leave_err = unwrap_err_result(group_leave("", &[]));

    let expected = PmixStatus::Known(PmixError::ErrBadParam);
    assert_eq!(construct_err, expected, "group_construct empty group_id");
    assert_eq!(destruct_err, expected, "group_destruct empty group_id");
    assert_eq!(invite_err, expected, "group_invite empty group_id");
    assert_eq!(join_err, expected, "group_join empty group_id");
    assert_eq!(leave_err, expected, "group_leave empty group_id");
}

/// All _nb group functions return the same error for empty group_id.
#[test]
fn all_nb_functions_reject_empty_group_id() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let proc2 = proc.clone();
    let leader = Proc::new("test_ns", 0).expect("create leader");

    let construct_nb_err = extract_err(group_construct_nb(
        "", &[proc], &[],
        GroupConstructCallbackWrapper::new(|_, _| {}),
    ));
    let destruct_nb_err = unwrap_err_result(group_destruct_nb(
        "", &[],
        GroupDestructCallbackWrapper::new(|_| {}),
    ));
    let invite_nb_err = extract_err(group_invite_nb(
        "", &[proc2], &[],
        GroupInviteCallbackWrapper::new(|_, _| {}),
    ));
    let join_nb_err = extract_err(group_join_nb(
        "", &leader, pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[],
        GroupJoinCallbackWrapper::new(|_, _| {}),
    ));
    let leave_nb_err = unwrap_err_result(group_leave_nb(
        "", &[],
        GroupLeaveCallbackWrapper::new(|_| {}),
    ));

    let expected = PmixStatus::Known(PmixError::ErrBadParam);
    assert_eq!(construct_nb_err, expected, "group_construct_nb empty group_id");
    assert_eq!(destruct_nb_err, expected, "group_destruct_nb empty group_id");
    assert_eq!(invite_nb_err, expected, "group_invite_nb empty group_id");
    assert_eq!(join_nb_err, expected, "group_join_nb empty group_id");
    assert_eq!(leave_nb_err, expected, "group_leave_nb empty group_id");
}

/// PmixStatus equality checks across all error results.
#[test]
fn error_status_equality_checks() {
    let proc = Proc::new("test_ns", 0).expect("create proc");

    let err1 = extract_err(group_construct("", &[proc], &[]));
    let err2 = PmixStatus::Known(PmixError::ErrBadParam);
    assert_eq!(err1, err2, "PmixStatus equality should work for ErrBadParam");
    assert_eq!(err1.to_raw(), -27, "ErrBadParam raw value should be -27");
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper — both info-carrying and status-only variants
// ─────────────────────────────────────────────────────────────────────────────

/// Construct and invite callbacks carry (PmixStatus, Vec<Info>).
#[test]
fn construct_and_invite_callbacks_carry_info() {
    // These compile because the callback signature includes Vec<Info>.
    let _c = GroupConstructCallbackWrapper::new(|s, i| {
        let _status = s;
        let _info = i;
    });
    let _i = GroupInviteCallbackWrapper::new(|s, i| {
        let _status = s;
        let _info = i;
    });
    let _j = GroupJoinCallbackWrapper::new(|s, i| {
        let _status = s;
        let _info = i;
    });
}

/// Destruct and leave callbacks carry only PmixStatus.
#[test]
fn destruct_and_leave_callbacks_status_only() {
    // These compile because the callback signature is Fn(PmixStatus) only.
    let _d = GroupDestructCallbackWrapper::new(|s| {
        let _status = s;
    });
    let _l = GroupLeaveCallbackWrapper::new(|s| {
        let _status = s;
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// pmix_group_opt_t enum values
// ─────────────────────────────────────────────────────────────────────────────

/// pmix_group_opt_t has PMIX_GROUP_ACCEPT variant.
#[test]
fn pmix_group_opt_accept_exists() {
    let _opt = pmix_group_opt_t::PMIX_GROUP_ACCEPT;
}

/// pmix_group_opt_t has PMIX_GROUP_DECLINE variant.
#[test]
fn pmix_group_opt_decline_exists() {
    let _opt = pmix_group_opt_t::PMIX_GROUP_DECLINE;
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-function lifecycle ordering tests (without daemon — ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: construct -> invite -> join -> destruct — requires daemon.
#[test]
#[ignore = "requires PMIx daemon — server_init_minimal corrupts C-level PMIx state"]
fn group_lifecycle_construct_invite_join_destruct() {
    // In a real environment with a PMIx daemon:
    //   1. group_construct("group1", &[proc1, proc2], &[])
    //   2. group_invite("group1", &[proc3], &[])
    //   3. group_join("group1", &leader, PMIX_GROUP_ACCEPT, &[])
    //   4. group_destruct("group1", &[])
    panic!("requires PMIx daemon — integration test");
}

/// Non-blocking lifecycle: construct_nb -> invite_nb -> join_nb -> destruct_nb.
#[test]
#[ignore = "requires PMIx daemon — server_init_minimal corrupts C-level PMIx state"]
fn group_lifecycle_nb_full_cycle() {
    // In a real environment with a PMIx daemon:
    //   1. group_construct_nb("group1", &[proc1, proc2], &[], cb1)
    //   2. group_invite_nb("group1", &[proc3], &[], cb2)
    //   3. group_join_nb("group1", &leader, PMIX_GROUP_ACCEPT, &[], cb3)
    //   4. group_destruct_nb("group1", &[], cb4)
    panic!("requires PMIx daemon — integration test");
}

/// Leave lifecycle: construct -> join -> leave — requires daemon.
#[test]
#[ignore = "requires PMIx daemon — server_init_minimal corrupts C-level PMIx state"]
fn group_lifecycle_construct_join_leave() {
    // In a real environment with a PMIx daemon:
    //   1. group_construct("group1", &[proc1, proc2], &[])
    //   2. group_join("group1", &leader, PMIX_GROUP_ACCEPT, &[])
    //   3. group_leave("group1", &[])
    panic!("requires PMIx daemon — integration test");
}

/// Non-blocking leave lifecycle — requires daemon.
#[test]
#[ignore = "requires PMIx daemon — server_init_minimal corrupts C-level PMIx state"]
fn group_lifecycle_nb_construct_join_leave() {
    // In a real environment with a PMIx daemon:
    //   1. group_construct_nb("group1", &[proc1], &[], cb1)
    //   2. group_join_nb("group1", &leader, PMIX_GROUP_ACCEPT, &[], cb2)
    //   3. group_leave_nb("group1", &[], cb3)
    panic!("requires PMIx daemon — integration test");
}

/// Decline invitation lifecycle — requires daemon.
#[test]
#[ignore = "requires PMIx daemon — server_init_minimal corrupts C-level PMIx state"]
fn group_lifecycle_decline_invitation() {
    // In a real environment with a PMIx daemon:
    //   1. group_construct("group1", &[proc1], &[])
    //   2. group_invite("group1", &[proc2], &[])
    //   3. group_join("group1", &leader, PMIX_GROUP_DECLINE, &[])
    panic!("requires PMIx daemon — integration test");
}
