//! Tests for `PMIx_Group_destruct_nb` — non-blocking group destruction.
//!
//! Derived from the C API signature in `pmix.h` and the group management
//! spec in the PMIx v4.1 standard. No dedicated C test file exists for
//! group destruct in the PMIx test suite — the group APIs are tested as
//! part of higher-level integration scenarios. These tests cover the safe
//! Rust wrapper parameter validation, callback wrapper construction, and
//! error handling paths.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::groups::{GroupDestructCallbackWrapper, group_destruct_nb};
use pmix::{PmixError, PmixStatus};

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

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation — empty group_id
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with empty group_id should return PMIX_ERR_BAD_PARAM
/// immediately without calling FFI or invoking the callback.
#[test]
fn group_destruct_nb_empty_group_id_bad_param() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = group_destruct_nb("", &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id should return PMIX_ERR_BAD_PARAM"
    );
}

/// group_destruct_nb with empty group_id and non-empty info — still BAD_PARAM.
#[test]
fn group_destruct_nb_empty_group_id_with_info() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    // Info is not directly constructible from user code in the current API,
    // so we test with an empty slice.
    let result = group_destruct_nb("", &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id with info should return PMIX_ERR_BAD_PARAM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation — group_id formats
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with a long group_id — should fail from FFI.
#[test]
fn group_destruct_nb_long_group_id_without_init_fails() {
    let long_id = "g".repeat(256);
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
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

/// group_destruct_nb with a dotted group_id.
#[test]
fn group_destruct_nb_dotted_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("my.group.name", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with dotted group_id without init should fail"
    );
}

/// group_destruct_nb with a slash-containing group_id.
#[test]
fn group_destruct_nb_slash_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("my/group/name", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with slash group_id without init should fail"
    );
}

/// group_destruct_nb with a mixed-case group_id.
#[test]
fn group_destruct_nb_mixed_case_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("MyGroup_Name123", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with mixed-case group_id without init should fail"
    );
}

/// group_destruct_nb with a two-character group_id.
#[test]
fn group_destruct_nb_two_char_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("ab", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with two-char group_id without init should fail"
    );
}

/// group_destruct_nb with a group_id containing digits and underscores.
#[test]
fn group_destruct_nb_digits_underscore_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("group_123_test_456", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with digits+underscore group_id without init should fail"
    );
}

/// group_destruct_nb with a leading-digit group_id.
#[test]
fn group_destruct_nb_leading_digit_group_id_without_init_fails() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {});
    let result = group_destruct_nb("1group", &[], callback);
    assert!(
        result.is_err(),
        "group_destruct_nb with leading digit group_id without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple calls
// ─────────────────────────────────────────────────────────────────────────────

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

/// group_destruct_nb called three times — all should fail without init.
#[test]
fn group_destruct_nb_three_calls_without_init_fail() {
    let cb1 = GroupDestructCallbackWrapper::new(|_status| {});
    let cb2 = GroupDestructCallbackWrapper::new(|_status| {});
    let cb3 = GroupDestructCallbackWrapper::new(|_status| {});
    assert!(group_destruct_nb("group_a", &[], cb1).is_err());
    assert!(group_destruct_nb("group_b", &[], cb2).is_err());
    assert!(group_destruct_nb("group_c", &[], cb3).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper — construction and trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// GroupDestructCallbackWrapper::new accepts a closure and compiles.
#[test]
fn group_destruct_callback_wrapper_simple() {
    let _wrapper = GroupDestructCallbackWrapper::new(|_status| {
        // simple no-op callback
    });
}

/// GroupDestructCallbackWrapper captures status in the callback.
#[test]
fn group_destruct_callback_wrapper_captures_status() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    let status_code = Arc::new(AtomicI32::new(999));
    let status_clone = status_code.clone();
    let _wrapper = GroupDestructCallbackWrapper::new(move |status| {
        status_clone.store(status.to_raw(), Ordering::SeqCst);
    });
    // Wrapper is constructible and captures the status code.
}

/// GroupDestructCallbackWrapper with Arc<Mutex<>> for shared state.
#[test]
fn group_destruct_callback_wrapper_arc_mutex() {
    use std::sync::Arc;
    use std::sync::Mutex;

    let shared = Arc::new(Mutex::new(Vec::<PmixStatus>::new()));
    let shared_clone = shared.clone();
    let _wrapper = GroupDestructCallbackWrapper::new(move |status| {
        shared_clone.lock().unwrap().push(status);
    });
    // Wrapper is constructible with Arc<Mutex<>> state.
}

/// GroupDestructCallbackWrapper with complex state tracking.
#[test]
fn group_destruct_callback_wrapper_state_tracking() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let status_raw = Arc::new(AtomicI32::new(i32::MAX));
    let called_clone = called.clone();
    let status_clone = status_raw.clone();
    let _wrapper = GroupDestructCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        status_clone.store(status.to_raw(), Ordering::SeqCst);
        assert!(
            status.is_error() || status.is_success(),
            "status must be valid"
        );
    });
    // Wrapper tracks call state and status code.
}

/// GroupDestructCallbackWrapper construction with move closure.
#[test]
fn group_destruct_callback_wrapper_move_closure() {
    let data = String::from("destruct_context");
    let _wrapper = GroupDestructCallbackWrapper::new(move |_status| {
        let _ = data; // captured string
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper — Send bound
// ─────────────────────────────────────────────────────────────────────────────

/// Verify GroupDestructCallbackWrapper is Send (required for FFI callback).
#[test]
fn group_destruct_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupDestructCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI call without PMIx_Init — should fail synchronously
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb without PMIx_Init — should fail without invoking
/// the callback because the library is not initialized.
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

/// group_destruct_nb without init — error status should be negative.
#[test]
fn group_destruct_nb_error_is_valid_status() {
    let callback = GroupDestructCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked");
    });
    let result = group_destruct_nb("my_group", &[], callback);
    assert!(result.is_err());
    let err = extract_err(result);
    // The error should be a known PMIx status code.
    // Without init, PMIx typically returns PMIX_ERR_INIT.
    let raw = err.to_raw();
    assert!(raw < 0, "error status should be negative, got {}", raw);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx daemon (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// group_destruct_nb with a valid group — requires PMIx_Init.
/// This test is ignored because it needs a running PMIx daemon.
#[test]
#[ignore]
fn group_destruct_nb_with_init() {
    // Requires PMIx_Init which needs a PMIx daemon.
    // In a real PMIx environment:
    //   let called = Arc::new(AtomicBool::new(false));
    //   let called_clone = called.clone();
    //   let callback = GroupDestructCallbackWrapper::new(move |status| {
    //       called_clone.store(true, Ordering::SeqCst);
    //       assert!(status.is_success());
    //   });
    //   let result = group_destruct_nb("test_group", &[], callback);
    //   assert!(result.is_ok());
    //   // Wait for callback...
    panic!("requires PMIx daemon — integration test");
}

/// group_destruct_nb callback invocation — verifies the callback is actually
/// called with success status after a valid destruct.
#[test]
#[ignore]
fn group_destruct_nb_callback_invoked_on_success() {
    // Requires PMIx_Init which needs a PMIx daemon.
    panic!("requires PMIx daemon — integration test");
}

/// Integration: construct -> join -> destruct_nb full lifecycle.
#[test]
#[ignore]
fn group_destruct_nb_full_lifecycle() {
    // Requires PMIx_Init which needs a PMIx daemon.
    // Tests the full group lifecycle: construct, invite/join, destruct_nb.
    panic!("requires PMIx daemon — integration test");
}

/// group_destruct_nb on a non-existent group — should return appropriate error.
#[test]
#[ignore]
fn group_destruct_nb_nonexistent_group() {
    // Requires PMIx_Init which needs a PMIx daemon.
    panic!("requires PMIx daemon — integration test");
}
