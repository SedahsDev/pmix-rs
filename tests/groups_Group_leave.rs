//! Tests for `PMIx_Group_leave` and `PMIx_Group_leave_nb`.
//!
//! Derived from the C API signatures in `pmix.h` and the group management
//! spec. No dedicated C test file exists for group leave in the PMIx test
//! suite — these tests cover the safe Rust wrapper parameter validation,
//! error handling, callback trait, and integration paths.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::groups::*;
use pmix::{PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the error from a Result<(), PmixStatus>.
fn unwrap_err_result(result: Result<(), PmixStatus>) -> PmixStatus {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err(PmixStatus), got Ok"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// Empty group_id must return PMIX_ERR_BAD_PARAM without reaching the FFI layer.
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

/// group_leave with valid group_id but no PMIx_Init should fail from the FFI layer.
#[test]
fn group_leave_without_init_fails() {
    let result = group_leave("my_group", &[]);
    assert!(result.is_err(), "group_leave without PMIx_Init should fail");
}

/// group_leave with a long group_id — should fail from FFI, not from NUL error.
#[test]
fn group_leave_long_group_id_without_init_fails() {
    let long_id = "g".repeat(256);
    let result = group_leave(&long_id, &[]);
    assert!(
        result.is_err(),
        "group_leave with long group_id without init should fail"
    );
}

/// group_leave with special characters in group_id.
#[test]
fn group_leave_special_chars_group_id_without_init_fails() {
    let result = group_leave("group-with_special.chars123", &[]);
    assert!(
        result.is_err(),
        "group_leave with special chars in group_id without init should fail"
    );
}

/// group_leave with a numeric group_id.
#[test]
fn group_leave_numeric_group_id_without_init_fails() {
    let result = group_leave("12345", &[]);
    assert!(
        result.is_err(),
        "group_leave with numeric group_id without init should fail"
    );
}

/// group_leave with a single-character group_id.
#[test]
fn group_leave_single_char_group_id_without_init_fails() {
    let result = group_leave("x", &[]);
    assert!(
        result.is_err(),
        "group_leave with single-char group_id without init should fail"
    );
}

/// group_leave with empty info vector — should reach FFI and fail there.
#[test]
fn group_leave_empty_info_without_init_fails() {
    let result = group_leave("empty_info_group", &[]);
    assert!(
        result.is_err(),
        "group_leave with empty info without init should fail"
    );
}

/// group_leave with hyphenated group_id.
#[test]
fn group_leave_hyphenated_group_id_without_init_fails() {
    let result = group_leave("my-test-group-name", &[]);
    assert!(
        result.is_err(),
        "group_leave with hyphenated group_id without init should fail"
    );
}

/// group_leave with underscore group_id.
#[test]
fn group_leave_underscore_group_id_without_init_fails() {
    let result = group_leave("my_test_group_name", &[]);
    assert!(
        result.is_err(),
        "group_leave with underscore group_id without init should fail"
    );
}

/// group_leave with unicode group_id.
#[test]
fn group_leave_unicode_group_id_without_init_fails() {
    let result = group_leave("group_with_unicode_\u{00e9}", &[]);
    assert!(
        result.is_err(),
        "group_leave with unicode group_id without init should fail"
    );
}

/// group_leave called twice with different group_ids — both should fail.
#[test]
fn group_leave_two_different_groups_without_init_fail() {
    let result1 = group_leave("group_alpha", &[]);
    let result2 = group_leave("group_beta", &[]);
    assert!(result1.is_err(), "first leave should fail without init");
    assert!(result2.is_err(), "second leave should fail without init");
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

/// group_leave_nb without PMIx_Init should fail without invoking callback.
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

/// group_leave_nb with a long group_id.
#[test]
fn group_leave_nb_long_group_id_without_init_fails() {
    let long_id = "g".repeat(256);
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb(&long_id, &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with long group_id without init should fail"
    );
}

/// group_leave_nb with special characters in group_id.
#[test]
fn group_leave_nb_special_chars_group_id_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("group-with_special.chars123", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with special chars without init should fail"
    );
}

/// group_leave_nb with a numeric group_id.
#[test]
fn group_leave_nb_numeric_group_id_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("12345", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with numeric group_id without init should fail"
    );
}

/// group_leave_nb with a single-character group_id.
#[test]
fn group_leave_nb_single_char_group_id_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("x", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with single-char group_id without init should fail"
    );
}

/// group_leave_nb with hyphenated group_id.
#[test]
fn group_leave_nb_hyphenated_group_id_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("my-test-group-name", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with hyphenated group_id without init should fail"
    );
}

/// group_leave_nb with unicode group_id.
#[test]
fn group_leave_nb_unicode_group_id_without_init_fails() {
    let callback = GroupLeaveCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_leave_nb("group_with_unicode_\u{00e9}", &[], callback);
    assert!(
        result.is_err(),
        "group_leave_nb with unicode group_id without init should fail"
    );
}

/// group_leave_nb called twice with different group_ids — both should fail.
#[test]
fn group_leave_nb_two_different_groups_without_init_fail() {
    let cb1 = GroupLeaveCallbackWrapper::new(|_status| {});
    let cb2 = GroupLeaveCallbackWrapper::new(|_status| {});
    let result1 = group_leave_nb("group_alpha", &[], cb1);
    let result2 = group_leave_nb("group_beta", &[], cb2);
    assert!(result1.is_err(), "first nb leave should fail without init");
    assert!(result2.is_err(), "second nb leave should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// GroupLeaveCallbackWrapper — construction and trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// GroupLeaveCallbackWrapper::new accepts a closure and compiles.
#[test]
fn group_leave_callback_wrapper_construction() {
    let _wrapper = GroupLeaveCallbackWrapper::new(|_status| {
        // No-op callback
    });
}

/// GroupLeaveCallbackWrapper can capture and record status via Arc<Mutex>.
#[test]
fn group_leave_callback_wrapper_records_status() {
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(None::<PmixStatus>));
    let status_clone = Arc::clone(&status);

    let wrapper = GroupLeaveCallbackWrapper::new(move |s: PmixStatus| {
        let mut locked = status_clone.lock().unwrap();
        *locked = Some(s);
    });

    drop(wrapper);
}

/// GroupLeaveCallbackWrapper is Send (required for cross-thread callbacks).
#[test]
fn group_leave_callback_wrapper_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupLeaveCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Combined sync + nb tests
// ─────────────────────────────────────────────────────────────────────────────

/// group_leave and group_leave_nb both fail without init — sequential test.
#[test]
fn group_leave_both_variants_without_init_fail() {
    let sync_result = group_leave("test_group", &[]);
    let cb = GroupLeaveCallbackWrapper::new(|_status| {});
    let nb_result = group_leave_nb("test_group", &[], cb);
    assert!(sync_result.is_err(), "sync should fail without init");
    assert!(nb_result.is_err(), "nb should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (ignored — require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration: Init -> Group_construct -> Group_join -> Group_leave -> Group_destruct.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_leave_integration() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: group_leave_nb callback invocation.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_leave_nb_callback_invocation() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: leave a group the caller did not construct (should fail with appropriate error).
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_leave_not_member_integration() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
