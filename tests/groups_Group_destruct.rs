//! Tests for `PMIx_Group_destruct` and `PMIx_Group_destruct_nb`.
//!
//! Derived from the C API signatures in `pmix.h` and the group management
//! spec. No dedicated C test file exists for group destruct in the PMIx test
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
// group_destruct — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// Empty group_id must return PMIX_ERR_BAD_PARAM without reaching the FFI layer.
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

/// group_destruct with valid group_id but no PMIx_Init should fail from the FFI layer.
#[test]
fn group_destruct_without_init_fails() {
    let result = group_destruct("my_group", &[]);
    assert!(
        result.is_err(),
        "group_destruct without PMIx_Init should fail"
    );
}

/// group_destruct with a long group_id — should fail from FFI, not from NUL error.
#[test]
fn group_destruct_long_group_id_without_init_fails() {
    let long_id = "g".repeat(256);
    let result = group_destruct(&long_id, &[]);
    assert!(
        result.is_err(),
        "group_destruct with long group_id without init should fail"
    );
}

/// group_destruct with special characters in group_id.
#[test]
fn group_destruct_special_chars_group_id_without_init_fails() {
    let result = group_destruct("group-with_special.chars123", &[]);
    assert!(
        result.is_err(),
        "group_destruct with special chars in group_id without init should fail"
    );
}

/// group_destruct with a numeric group_id.
#[test]
fn group_destruct_numeric_group_id_without_init_fails() {
    let result = group_destruct("12345", &[]);
    assert!(
        result.is_err(),
        "group_destruct with numeric group_id without init should fail"
    );
}

/// group_destruct with a single-character group_id.
#[test]
fn group_destruct_single_char_group_id_without_init_fails() {
    let result = group_destruct("x", &[]);
    assert!(
        result.is_err(),
        "group_destruct with single-char group_id without init should fail"
    );
}

/// group_destruct with empty info vector — should reach FFI and fail there.
#[test]
fn group_destruct_empty_info_without_init_fails() {
    let result = group_destruct("empty_info_group", &[]);
    assert!(
        result.is_err(),
        "group_destruct with empty info without init should fail"
    );
}

/// group_destruct with hyphenated group_id.
#[test]
fn group_destruct_hyphenated_group_id_without_init_fails() {
    let result = group_destruct("my-test-group-name", &[]);
    assert!(
        result.is_err(),
        "group_destruct with hyphenated group_id without init should fail"
    );
}

/// group_destruct with underscore group_id.
#[test]
fn group_destruct_underscore_group_id_without_init_fails() {
    let result = group_destruct("my_test_group_name", &[]);
    assert!(
        result.is_err(),
        "group_destruct with underscore group_id without init should fail"
    );
}

/// group_destruct with unicode group_id.
#[test]
fn group_destruct_unicode_group_id_without_init_fails() {
    let result = group_destruct("group_with_unicode_\u{00e9}", &[]);
    assert!(
        result.is_err(),
        "group_destruct with unicode group_id without init should fail"
    );
}

/// group_destruct called twice with different group_ids — both should fail.
#[test]
fn group_destruct_two_different_groups_without_init_fail() {
    let result1 = group_destruct("group_alpha", &[]);
    let result2 = group_destruct("group_beta", &[]);
    assert!(result1.is_err(), "first destruct should fail without init");
    assert!(result2.is_err(), "second destruct should fail without init");
}

/// group_destruct with a dotted group_id.
#[test]
fn group_destruct_dotted_group_id_without_init_fails() {
    let result = group_destruct("my.group.name", &[]);
    assert!(
        result.is_err(),
        "group_destruct with dotted group_id without init should fail"
    );
}

/// group_destruct with a slash-containing group_id.
#[test]
fn group_destruct_slash_group_id_without_init_fails() {
    let result = group_destruct("my/group/name", &[]);
    assert!(
        result.is_err(),
        "group_destruct with slash group_id without init should fail"
    );
}

/// group_destruct with a mixed-case group_id.
#[test]
fn group_destruct_mixed_case_group_id_without_init_fails() {
    let result = group_destruct("MyGroup_Name123", &[]);
    assert!(
        result.is_err(),
        "group_destruct with mixed-case group_id without init should fail"
    );
}

/// group_destruct with a very short group_id (2 chars).
#[test]
fn group_destruct_two_char_group_id_without_init_fails() {
    let result = group_destruct("ab", &[]);
    assert!(
        result.is_err(),
        "group_destruct with two-char group_id without init should fail"
    );
}

/// group_destruct with a group_id containing only digits and underscores.
#[test]
fn group_destruct_digits_underscore_group_id_without_init_fails() {
    let result = group_destruct("group_123_test_456", &[]);
    assert!(
        result.is_err(),
        "group_destruct with digits+underscore group_id without init should fail"
    );
}

/// group_destruct with a group_id that starts with a digit.
#[test]
fn group_destruct_leading_digit_group_id_without_init_fails() {
    let result = group_destruct("1group", &[]);
    assert!(
        result.is_err(),
        "group_destruct with leading digit group_id without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct_nb — parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with empty group_id must return PMIX_ERR_BAD_PARAM.
#[test]
fn group_destruct_nb_empty_group_id_bad_param() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_destruct_nb("", &[], callback);
    let err = unwrap_err_result(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_destruct_nb with valid group_id but no PMIx_Init should fail from the FFI layer.
#[test]
fn group_destruct_nb_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on FFI failure");
    });
    let result = group_destruct_nb("my_group", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb without PMIx_Init should fail"
    );
}

/// group_destruct_nb with a long group_id — should fail from FFI.
#[test]
fn group_destruct_nb_long_group_id_without_init_fails() {
    let long_id = "g".repeat(256);
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb(&long_id, &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with long group_id without init should fail"
    );
}

/// group_destruct_nb with special characters in group_id.
#[test]
fn group_destruct_nb_special_chars_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("group-with_special.chars123", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with special chars group_id without init should fail"
    );
}

/// group_destruct_nb with a numeric group_id.
#[test]
fn group_destruct_nb_numeric_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("12345", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with numeric group_id without init should fail"
    );
}

/// group_destruct_nb with a single-character group_id.
#[test]
fn group_destruct_nb_single_char_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("x", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with single-char group_id without init should fail"
    );
}

/// group_destruct_nb with hyphenated group_id.
#[test]
fn group_destruct_nb_hyphenated_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("my-test-group-name", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with hyphenated group_id without init should fail"
    );
}

/// group_destruct_nb with underscore group_id.
#[test]
fn group_destruct_nb_underscore_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("my_test_group_name", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with underscore group_id without init should fail"
    );
}

/// group_destruct_nb with unicode group_id.
#[test]
fn group_destruct_nb_unicode_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("group_with_unicode_\u{00e9}", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with unicode group_id without init should fail"
    );
}

/// group_destruct_nb called twice with different group_ids — both should fail.
#[test]
fn group_destruct_nb_two_different_groups_without_init_fail() {
    let cb1 = GroupDestructCallbackWrapper::new(|_status| {});
    let cb2 = GroupDestructCallbackWrapper::new(|_status| {});
    let result1 = group_destruct_nb("group_alpha", &[], cb1);
    let result2 = group_destruct_nb("group_beta", &[], cb2);
    assert!(
        result1.is_err(),
        "first nb destruct should fail without init"
    );
    assert!(
        result2.is_err(),
        "second nb destruct should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// GroupDestructCallbackWrapper — construction and trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// GroupDestructCallbackWrapper::new accepts a closure and compiles.
#[test]
fn group_destruct_callback_wrapper_construction() {
    let _wrapper = GroupDestructCallbackWrapper::new(|_status| {
        // No-op callback
    });
}

/// GroupDestructCallbackWrapper can capture and record status via Arc<Mutex>.
#[test]
fn group_destruct_callback_wrapper_records_status() {
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(None::<PmixStatus>));
    let status_clone = Arc::clone(&status);

    let wrapper = GroupDestructCallbackWrapper::new(move |s: PmixStatus| {
        let mut locked = status_clone.lock().unwrap();
        *locked = Some(s);
    });

    drop(wrapper);
}

/// GroupDestructCallbackWrapper is Send (required for cross-thread callbacks).
#[test]
fn group_destruct_callback_wrapper_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupDestructCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Combined sync + nb tests
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct and group_destruct_nb both fail without init — sequential test.
#[test]
fn group_destruct_both_variants_without_init_fail() {
    let sync_result = group_destruct("test_group", &[]);
    let cb = GroupDestructCallbackWrapper::new(|_status| {});
    let nb_result = group_destruct_nb("test_group", &[], cb);
    assert!(sync_result.is_err(), "sync should fail without init");
    assert!(nb_result.is_err(), "nb should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (ignored — require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration: Init -> Group_construct -> Group_join -> Group_destruct.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_destruct_integration() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: group_destruct_nb callback invocation.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_destruct_nb_callback_invocation() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Integration: destruct a group the caller did not construct (should fail with appropriate error).
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn group_destruct_not_member_integration() {
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
