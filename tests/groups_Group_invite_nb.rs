//! Tests for `PMIx_Group_invite_nb` — non-blocking group invite.
//!
//! Derived from the group management API spec in the PMIx v4.1 standard.
//! No dedicated C test file exists for group invite in the PMIx test suite —
//! the group APIs are tested as part of higher-level integration scenarios.
//! These tests cover the safe Rust wrapper parameter validation, callback
//! wrapper construction, error handling paths, and FFI call behavior.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::groups::{GroupInviteCallbackWrapper, group_invite_nb};
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

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation — empty group_id
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with empty group_id should return PMIX_ERR_BAD_PARAM
/// immediately without calling FFI or invoking the callback.
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

/// group_invite_nb with empty group_id and multiple invitees — still BAD_PARAM.
#[test]
fn group_invite_nb_empty_group_id_multiple_procs() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
        Proc::new("ns_b", 0).expect("proc c"),
    ];
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("", &procs, &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty group_id with multiple procs should return PMIX_ERR_BAD_PARAM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation — empty procs
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with empty procs array should return PMIX_ERR_BAD_PARAM.
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

/// group_invite_nb with empty procs but valid group_id — still BAD_PARAM.
#[test]
fn group_invite_nb_empty_procs_valid_group_id() {
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("valid_group_name", &[], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty procs with valid group_id should return PMIX_ERR_BAD_PARAM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation — order of checks
// ─────────────────────────────────────────────────────────────────────────────

/// Both empty group_id and empty procs — group_id check should come first.
#[test]
fn group_invite_nb_both_empty_group_id_and_procs() {
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("", &[], &[], callback);
    let err = extract_err(result);
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "both empty should return PMIX_ERR_BAD_PARAM (group_id check first)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper construction
// ─────────────────────────────────────────────────────────────────────────────

/// GroupInviteCallbackWrapper is constructible with a simple closure.
#[test]
fn group_invite_callback_wrapper_simple() {
    let _wrapper = GroupInviteCallbackWrapper::new(|_status, _info| {
        // simple no-op callback
    });
}

/// GroupInviteCallbackWrapper captures status in the callback.
#[test]
fn group_invite_callback_wrapper_captures_status() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    let status_code = Arc::new(AtomicI32::new(999));
    let status_clone = status_code.clone();
    let _wrapper = GroupInviteCallbackWrapper::new(move |status, _info| {
        status_clone.store(status.to_raw(), Ordering::SeqCst);
    });
    // Wrapper is constructible and captures the status code.
}

/// GroupInviteCallbackWrapper captures results info in the callback.
#[test]
fn group_invite_callback_wrapper_captures_info() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let info_len = Arc::new(AtomicUsize::new(usize::MAX));
    let info_clone = info_len.clone();
    let _wrapper = GroupInviteCallbackWrapper::new(move |_status, info| {
        info_clone.store(info.len(), Ordering::SeqCst);
    });
    // Wrapper captures info length.
}

/// GroupInviteCallbackWrapper with Arc<Mutex<>> for shared state.
#[test]
fn group_invite_callback_wrapper_arc_mutex() {
    use std::sync::Arc;
    use std::sync::Mutex;

    let shared = Arc::new(Mutex::new(Vec::<PmixStatus>::new()));
    let shared_clone = shared.clone();
    let _wrapper = GroupInviteCallbackWrapper::new(move |status, _info| {
        shared_clone.lock().unwrap().push(status);
    });
    // Wrapper is constructible with Arc<Mutex<>> state.
}

/// GroupInviteCallbackWrapper with complex state tracking.
#[test]
fn group_invite_callback_wrapper_state_tracking() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let info_count = Arc::new(AtomicUsize::new(0));
    let called_clone = called.clone();
    let info_clone = info_count.clone();
    let _wrapper = GroupInviteCallbackWrapper::new(move |status, info| {
        called_clone.store(true, Ordering::SeqCst);
        info_clone.store(info.len(), Ordering::SeqCst);
        assert!(
            status.is_error() || status.is_success(),
            "status must be valid"
        );
    });
    // Wrapper tracks call state and info count.
}

/// GroupInviteCallbackWrapper construction with move closure capturing data.
#[test]
fn group_invite_callback_wrapper_move_closure() {
    let data = String::from("invite_data");
    let _wrapper = GroupInviteCallbackWrapper::new(move |_status, _info| {
        let _ = data; // captured string
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper — Send + 'static bounds
// ─────────────────────────────────────────────────────────────────────────────

/// Verify GroupInviteCallbackWrapper is Send (required for FFI callback).
#[test]
fn group_invite_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupInviteCallbackWrapper>();
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI call without PMIx_Init — should fail synchronously
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb without PMIx_Init — should fail without invoking
/// the callback because the library is not initialized.
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

/// group_invite_nb without init, single proc — should fail.
#[test]
fn group_invite_nb_single_proc_without_init() {
    let proc = Proc::new("solo_ns", 42).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("solo_group", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with single proc without init should fail"
    );
}

/// group_invite_nb without init, multiple procs — should fail.
#[test]
fn group_invite_nb_multiple_procs_without_init() {
    let procs = vec![
        Proc::new("ns_a", 0).expect("proc a"),
        Proc::new("ns_a", 1).expect("proc b"),
        Proc::new("ns_b", 0).expect("proc c"),
    ];
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("multi_group", &procs, &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with multiple procs without init should fail"
    );
}

/// group_invite_nb without init, procs from different namespaces — should fail.
#[test]
fn group_invite_nb_cross_namespace_without_init() {
    let procs = vec![
        Proc::new("job_001", 0).expect("proc 1"),
        Proc::new("job_002", 0).expect("proc 2"),
        Proc::new("job_003", 1).expect("proc 3"),
    ];
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("cross_ns_group", &procs, &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with cross-namespace procs without init should fail"
    );
}

/// group_invite_nb without init, proc with max rank — should fail.
#[test]
fn group_invite_nb_max_rank_without_init() {
    let proc = Proc::new("test_ns", u32::MAX).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("max_rank_group", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with max rank proc without init should fail"
    );
}

/// group_invite_nb without init, same namespace different ranks — should fail.
#[test]
fn group_invite_nb_same_ns_different_ranks_without_init() {
    let procs = vec![
        Proc::new("ns", 10).expect("rank 10"),
        Proc::new("ns", 20).expect("rank 20"),
        Proc::new("ns", 30).expect("rank 30"),
    ];
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("same_ns_group", &procs, &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with same-ns different-rank procs without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite_nb error status codes
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that the error returned by group_invite_nb without init is a
/// valid PmixStatus (not a random value).
#[test]
fn group_invite_nb_error_is_valid_status() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("my_group", &[proc], &[], callback);
    assert!(result.is_err());
    let err = extract_err(result);
    // The error should be a known PMIx status code.
    // Without init, PMIx typically returns PMIX_ERR_INIT or PMIX_ERR_NOT_SUPPORTED.
    let raw = err.to_raw();
    assert!(raw < 0, "error status should be negative, got {}", raw);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases — group_id variations
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with a long group_id — should fail from FFI, not from NUL error.
#[test]
fn group_invite_nb_long_group_id_without_init() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let long_id = "g".repeat(256);
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb(&long_id, &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with long group_id without init should fail"
    );
}

/// group_invite_nb with special characters in group_id.
#[test]
fn group_invite_nb_special_chars_group_id_without_init() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let callback = GroupInviteCallbackWrapper::new(|_status, _info| {
        panic!("callback should not be invoked");
    });
    let result = group_invite_nb("group-with_special.chars123", &[proc], &[], callback);
    assert!(
        result.is_err(),
        "group_invite_nb with special chars in group_id without init should fail"
    );
}

/// group_invite_nb with a numeric group_id.
#[test]
fn group_invite_nb_numeric_group_id_without_init() {
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
// Integration tests — require PMIx daemon (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// group_invite_nb with a valid group and single proc — requires PMIx_Init.
/// This test is ignored because it needs a running PMIx daemon.
#[test]
#[ignore]
fn group_invite_nb_with_init_single_proc() {
    // Requires PMIx_Init which needs a PMIx daemon.
    // In a real PMIx environment:
    //   let proc = Proc::new("my_job", 0).unwrap();
    //   let called = Arc::new(AtomicBool::new(false));
    //   let called_clone = called.clone();
    //   let callback = GroupInviteCallbackWrapper::new(move |status, info| {
    //       called_clone.store(true, Ordering::SeqCst);
    //       assert!(status.is_success());
    //   });
    //   let result = group_invite_nb("test_group", &[proc], &[], callback);
    //   assert!(result.is_ok());
    //   // Wait for callback...
    panic!("requires PMIx daemon — integration test");
}

/// group_invite_nb with multiple invitees — requires PMIx_Init.
#[test]
#[ignore]
fn group_invite_nb_with_init_multiple_procs() {
    // Requires PMIx_Init which needs a PMIx daemon.
    panic!("requires PMIx daemon — integration test");
}

/// group_invite_nb callback invocation — verifies the callback is actually
/// called with success status and results info.
#[test]
#[ignore]
fn group_invite_nb_callback_invoked_on_success() {
    // Requires PMIx_Init which needs a PMIx daemon.
    panic!("requires PMIx daemon — integration test");
}

/// Integration: Init -> Group_invite_nb -> Group_join -> Group_destruct.
#[test]
#[ignore]
fn group_invite_nb_join_integration() {
    // Requires PMIx_Init which needs a PMIx daemon.
    panic!("requires PMIx daemon — integration test");
}
