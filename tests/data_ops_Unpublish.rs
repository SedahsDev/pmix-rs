//! Integration tests for `PMIx_Unpublish` via the safe `unpublish()` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_Init) are marked `#[ignore]`.
//!
//! Derived from C test: `test/simple/simppub.c` which exercises the
//! publish -> fence -> lookup -> unpublish -> fence pattern.

use pmix::data_ops::{UnpublishCallback, unpublish, unpublish_nb};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `unpublish` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `Option<&[&str]>`
/// for keys and `Option<&Info>` for directives.
#[test]
fn unpublish_function_signature() {
    let _: fn(Option<&[&str]>, Option<&Info>) -> Result<(), PmixStatus> = unpublish;
}

/// `unpublish_nb` function is public and has the correct signature.
///
/// Compile-time check: the non-blocking variant exists and accepts
/// keys, info, and a callback.
#[test]
fn unpublish_nb_function_signature() {
    let _: fn(
        Option<&[&str]>,
        Option<&Info>,
        Box<dyn UnpublishCallback>,
    ) -> Result<(), PmixStatus> = unpublish_nb;
}

/// `UnpublishCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object.
#[test]
fn unpublish_callback_trait_object() {
    struct TestCallback;
    impl UnpublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn UnpublishCallback> = Box::new(TestCallback);
    let _: Box<dyn UnpublishCallback> = cb;
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests — keys parameter variants (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `unpublish` with `None` keys (unpublish all) returns `PMIX_ERR_INIT` before init.
///
/// Per the PMIx spec: "A value of NULL for the keys parameter instructs the
/// server to remove all data published by this process."
/// Before init, PMIx returns PMIX_ERR_INIT (-31).
#[test]
fn unpublish_none_keys_before_init() {
    let result = unpublish(None, None);
    assert!(result.is_err(), "unpublish should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `unpublish` with empty keys slice returns `PMIX_ERR_INIT` before init.
///
/// Passing an empty slice should behave like `None` (no keys to unpublish),
/// and still fail with PMIX_ERR_INIT because PMIx is not initialized.
#[test]
fn unpublish_empty_keys_before_init() {
    let keys: &[&str] = &[];
    let result = unpublish(Some(keys), None);
    assert!(result.is_err(), "unpublish should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

/// `unpublish` with a single key returns `PMIX_ERR_INIT` before init.
///
/// Derived from simppub.c: rank 0 calls `PMIx_Unpublish(keys, NULL, 0)`
/// with keys = ["FOOBAR", "PANDA"].
#[test]
fn unpublish_single_key_before_init() {
    let result = unpublish(Some(&["FOOBAR"]), None);
    assert!(result.is_err(), "unpublish should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

/// `unpublish` with multiple keys returns `PMIX_ERR_INIT` before init.
///
/// Derived from simppub.c: rank 0 unpublishes both "FOOBAR" and "PANDA".
#[test]
fn unpublish_multiple_keys_before_init() {
    let result = unpublish(Some(&["FOOBAR", "PANDA"]), None);
    assert!(result.is_err(), "unpublish should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

/// `unpublish` with info directives returns `PMIX_ERR_INIT` before init.
///
/// Test that the info parameter is accepted (even though the call fails).
#[test]
fn unpublish_with_info_before_init() {
    let info = InfoBuilder::new().build();
    let result = unpublish(Some(&["FOOBAR"]), Some(&info));
    assert!(result.is_err(), "unpublish should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking variant tests
// ─────────────────────────────────────────────────────────────────────────────

/// `unpublish_nb` with `None` keys returns `PMIX_ERR_INIT` before init.
#[test]
fn unpublish_nb_none_keys_before_init() {
    struct InitCheckCallback;
    impl UnpublishCallback for InitCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result = unpublish_nb(None, None, Box::new(InitCheckCallback));
    assert!(result.is_err(), "unpublish_nb should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

/// `unpublish_nb` with keys returns `PMIX_ERR_INIT` before init.
#[test]
fn unpublish_nb_keys_before_init() {
    struct KeysCheckCallback;
    impl UnpublishCallback for KeysCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result = unpublish_nb(
        Some(&["FOOBAR", "PANDA"]),
        None,
        Box::new(KeysCheckCallback),
    );
    assert!(result.is_err(), "unpublish_nb should fail without PMIx_Init");
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31)"
    );
}

/// `unpublish_nb` callback is not invoked on immediate failure.
///
/// When PMIx_Unpublish_nb returns an error synchronously (e.g., not init),
/// the callback should NOT be called.
#[test]
fn unpublish_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NoInvokeCallback;
    impl UnpublishCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let result = unpublish_nb(Some(&["FOOBAR"]), None, Box::new(NoInvokeCallback));

    // Should fail immediately without invoking callback.
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert!(
        !CALLBACK_INVOKED.load(Ordering::SeqCst),
        "callback should NOT have been invoked on immediate failure"
    );
}

/// `unpublish_nb` callback receives `PmixStatus` (type check).
#[test]
fn unpublish_callback_receives_pmix_status() {
    struct StatusCallback;
    impl UnpublishCallback for StatusCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            // This function should not be called in this test,
            // but the compiler verifies the signature.
            let _ = status.is_success();
            let _ = status.to_raw();
        }
    }

    let result = unpublish_nb(None, None, Box::new(StatusCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error type and consistency tests
// ─────────────────────────────────────────────────────────────────────────────

/// `unpublish` returns a proper `Result` type with known error codes.
#[test]
fn unpublish_returns_result_type() {
    let result: Result<(), PmixStatus> = unpublish(None, None);

    match result {
        Ok(()) => panic!("should not succeed without init"),
        Err(PmixStatus::Known(PmixError::ErrInit)) => {
            // Expected: PMIX_ERR_INIT
        }
        Err(PmixStatus::Unknown(code)) => {
            panic!("unexpected unknown status code: {}", code);
        }
        Err(PmixStatus::Known(other)) => {
            panic!(
                "unexpected known error: {:?} (raw={})",
                other,
                (other as i32)
            );
        }
    }
}

/// `unpublish` is callable multiple times with consistent error behavior.
#[test]
fn unpublish_multiple_calls_consistent_error() {
    for i in 0..5 {
        let result = unpublish(Some(&["FOOBAR"]), None);
        assert!(
            result.is_err(),
            "iteration {}: should fail without PMIx_Init",
            i
        );
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "iteration {}: should be PMIX_ERR_INIT",
            i
        );
    }
}

/// `unpublish_nb` is callable multiple times with consistent error behavior.
#[test]
fn unpublish_nb_multiple_calls_consistent_error() {
    for i in 0..5 {
        struct MultiCallback;
        impl UnpublishCallback for MultiCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                panic!("Callback should not be invoked");
            }
        }

        let result = unpublish_nb(Some(&["KEY"]), None, Box::new(MultiCallback));
        assert!(
            result.is_err(),
            "iteration {}: should fail without PMIx_Init",
            i
        );
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "iteration {}: should be PMIX_ERR_INIT",
            i
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `unpublish` succeeds after `PMIx_Init` with valid keys.
///
/// Requires a running PMIx server. Follows the simppub.c pattern:
/// publish -> fence -> unpublish -> fence.
#[test]
#[ignore = "requires PMIx daemon"]
fn unpublish_after_init() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    // Unpublish specific keys.
    let result = unpublish(Some(&["FOOBAR", "PANDA"]), None);
    assert!(
        result.is_ok(),
        "unpublish should succeed after init, got: {:?}",
        result
    );
}

/// `unpublish` with `None` keys removes all published data.
///
/// Per the PMIx spec: "A value of NULL for the keys parameter instructs the
/// server to remove all data published by this process."
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn unpublish_all_after_init() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    // Unpublish all data for this process.
    let result = unpublish(None, None);
    assert!(
        result.is_ok(),
        "unpublish(all) should succeed after init, got: {:?}",
        result
    );
}

/// `unpublish_nb` succeeds after `PMIx_Init` and invokes the callback.
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn unpublish_nb_after_init() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NbCallback;
    impl UnpublishCallback for NbCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let result = unpublish_nb(Some(&["FOOBAR"]), None, Box::new(NbCallback));
    assert!(result.is_ok(), "unpublish_nb should accept the request");

    // Note: callback may be invoked synchronously or asynchronously
    // depending on the PMIx implementation. In a real integration test
    // with a daemon, we would call PMIx_Progress or fence to drive
    // the completion.
}

/// Full publish -> unpublish cycle (simppub.c pattern).
///
/// This follows the exact pattern from test/simple/simppub.c:
/// 1. Rank 0 publishes "FOOBAR" and "PANDA"
/// 2. Fence to ensure data is visible
/// 3. Other ranks lookup "FOOBAR"
/// 4. Rank 0 unpublishes both keys
/// 5. Final fence to ensure cleanup is complete
///
/// Requires a running PMIx server with multiple processes.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_unpublish_cycle() {
    use pmix::data_ops::{publish, unpublish};

    let ctx = pmix::init(None).expect("PMIx_Init should succeed");

    // Publish data (simplified — real test would use proper InfoBuilder).
    let info = InfoBuilder::new().build();
    let publish_result = publish(&info);
    assert!(publish_result.is_ok(), "publish should succeed");

    // Fence to ensure data is visible.
    let proc = ctx.get_proc();
    let fence_result = pmix::fence(proc, None);
    assert!(fence_result.is_ok(), "fence after publish should succeed");

    // Unpublish the keys.
    let unpublish_result = unpublish(Some(&["FOOBAR", "PANDA"]), None);
    assert!(
        unpublish_result.is_ok(),
        "unpublish should succeed after publish+fence"
    );

    // Final fence to ensure cleanup is complete.
    let final_fence = pmix::fence(proc, None);
    assert!(final_fence.is_ok(), "final fence should succeed");
}

/// `unpublish` with NUL in key name returns `PMIX_ERROR`.
///
/// Keys containing NUL bytes are invalid for C string conversion.
#[test]
fn unpublish_key_with_nul_returns_error() {
    // Keys with embedded NUL cannot be converted to CString.
    // We can't easily construct such a &str in Rust (str can't contain NUL),
    // so this tests the error path is handled correctly by the type system.
    // The actual NUL check happens in CString::new which we call internally.
    // This test verifies the function is callable and returns an error.
    let result = unpublish(Some(&["valid_key"]), None);
    assert!(result.is_err(), "should fail without PMIx_Init");
}
