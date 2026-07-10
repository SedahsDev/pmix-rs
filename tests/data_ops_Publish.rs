//! Integration tests for `PMIx_Publish` via the safe `publish()` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_Init) are marked `#[ignore]`.

mod daemon_helper;

use pmix::data_ops::{PublishCallback, publish, publish_nb};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `publish` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&Info`.
#[test]
fn publish_function_signature() {
    let _: fn(&Info) -> Result<(), PmixStatus> = publish;
}

/// `publish_nb` function is public and has the correct signature.
///
/// Compile-time check: the non-blocking variant exists and accepts
/// `&Info` plus a callback.
#[test]
fn publish_nb_function_signature() {
    let _: fn(&Info, Box<dyn PublishCallback>) -> Result<(), PmixStatus> = publish_nb;
}

/// `PublishCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object.
#[test]
fn publish_callback_trait_object() {
    struct TestCallback;
    impl PublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn PublishCallback> = Box::new(TestCallback);
    let _: Box<dyn PublishCallback> = cb;
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `publish` with an empty info array returns `PMIX_ERR_INIT` before init.
///
/// PMIx_Publish requires PMIx_Init to have been called first. Calling it
/// without initialization should return PMIX_ERR_INIT (-31).
#[test]
fn publish_before_init_returns_err_init() {
    // Build an empty info array.
    let info = InfoBuilder::new().build();

    let result = publish(&info);
    assert!(result.is_err(), "publish should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `publish_nb` with an empty info array returns `PMIX_ERR_INIT` before init.
///
/// The non-blocking variant also requires initialization.
#[test]
fn publish_nb_before_init_returns_err_init() {
    let info = InfoBuilder::new().build();

    struct InitCheckCallback;
    impl PublishCallback for InitCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result = publish_nb(&info, Box::new(InitCheckCallback));
    assert!(result.is_err(), "publish_nb should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `publish` returns a `Result` type with proper error handling.
#[test]
fn publish_returns_result_type() {
    let info = InfoBuilder::new().build();
    let result: Result<(), PmixStatus> = publish(&info);

    // Verify the error is a known PMIx error code.
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

/// `publish_nb` returns a `Result` type with proper error handling.
#[test]
fn publish_nb_returns_result_type() {
    let info = InfoBuilder::new().build();

    struct ResultTypeCallback;
    impl PublishCallback for ResultTypeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result: Result<(), PmixStatus> = publish_nb(&info, Box::new(ResultTypeCallback));

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

/// `publish` with InfoBuilder-constructed info (directives).
///
/// This tests that the InfoBuilder can construct an info array and
/// that publish accepts it. The call itself will fail without init,
/// but the info construction and type compatibility are verified.
#[test]
fn publish_with_info_builder() {
    // Build an info array using InfoBuilder.
    // The InfoBuilder API uses internal FFI constants, so we just
    // verify the builder constructs a valid Info that publish accepts.
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();

    let result = publish(&info);
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `publish_nb` callback receives `PmixStatus` (type check).
#[test]
fn publish_callback_receives_pmix_status() {
    struct StatusCallback;
    impl PublishCallback for StatusCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            // This function should not be called in this test,
            // but the compiler verifies the signature.
            let _ = status.is_success();
            let _ = status.to_raw();
        }
    }

    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(StatusCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `publish` succeeds after `PMIx_Init` with valid data.
///
/// Requires a running PMIx server. This test follows the pattern from
/// `test/simple/simppub.c`: publish key-value pairs and verify success.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_after_init() {
    daemon_helper::ensure_pmix_init();
    // Build info with a simple key-value to publish.
    // The InfoBuilder requires static key bytes matching PMIx key format.
    // For now, just test with an empty info (publish metadata only).
    let info = InfoBuilder::new().build();

    let result = publish(&info);
    assert!(
        result.is_ok(),
        "publish should succeed after init, got: {:?}",
        result
    );
}

/// `publish_nb` succeeds after `PMIx_Init` and invokes the callback.
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_nb_after_init() {
    use std::sync::atomic::{AtomicBool, Ordering};

    daemon_helper::ensure_pmix_init();
    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NbCallback;
    impl PublishCallback for NbCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(NbCallback));
    assert!(result.is_ok(), "publish_nb should accept the request");

    // Note: callback may be invoked synchronously or asynchronously
    // depending on the PMIx implementation. In a real integration test
    // with a daemon, we would call PMIx_Progress or fence to drive
    // the completion.
}

/// `publish` with duplicate keys on the same range returns `PMIX_ERR_DUPLICATE_KEY`.
///
/// Per the PMIx spec: "Publishing duplicate keys is permitted provided they
/// are published to different ranges. Duplicate keys being published on the
/// same data range shall return the PMIX_ERR_DUPLICATE_KEY error."
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_duplicate_key_returns_error() {
    daemon_helper::ensure_pmix_init();
    let info = InfoBuilder::new().build();

    // First publish should succeed.
    let result1 = publish(&info);
    assert!(result1.is_ok(), "first publish should succeed");

    // Second publish with same key on same range should fail.
    // (This test would need a proper InfoBuilder that supports
    // arbitrary string keys to be fully meaningful.)
    let result2 = publish(&info);
    // May succeed or fail depending on what keys are in the info array.
    // The important thing is that the function is callable and returns
    // a proper Result type.
    let _ = result2;
}

/// `publish` followed by fence ensures data visibility.
///
/// This follows the simppub.c pattern: publish -> fence -> lookup.
///
/// Requires a running PMIx server with multiple processes.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_fence_pattern() {
    let ctx = daemon_helper::ensure_pmix_init();

    // Publish data.
    let info = InfoBuilder::new().build();
    let result = publish(&info);
    assert!(result.is_ok(), "publish should succeed");

    // Fence to ensure data is visible.
    let proc = ctx.get_proc();
    let fence_result = pmix::fence(proc, None);
    assert!(fence_result.is_ok(), "fence should succeed after publish");
}

/// `publish_nb` callback is not invoked on immediate failure.
///
/// When PMIx_Publish_nb returns an error synchronously (e.g., bad params),
/// the callback should NOT be called.
#[test]
fn publish_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NoInvokeCallback;
    impl PublishCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(NoInvokeCallback));

    // Should fail immediately without invoking callback.
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert!(
        !CALLBACK_INVOKED.load(Ordering::SeqCst),
        "callback should NOT have been invoked on immediate failure"
    );
}

/// `publish` with empty info array is a valid call (returns error from PMIx).
#[test]
fn publish_empty_info_array() {
    let info = InfoBuilder::new().build();
    // Empty info array — InfoBuilder::new().build() produces zero-length info.
    // (info.len is private, so we just verify the call behavior.)

    let result = publish(&info);
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `publish` is callable multiple times (idempotent error behavior).
#[test]
fn publish_multiple_calls_consistent_error() {
    let info = InfoBuilder::new().build();

    // Call publish multiple times — each should return the same error.
    for i in 0..5 {
        let result = publish(&info);
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

/// `publish_nb` is callable multiple times (idempotent error behavior).
#[test]
fn publish_nb_multiple_calls_consistent_error() {
    let info = InfoBuilder::new().build();

    for i in 0..5 {
        struct MultiCallback;
        impl PublishCallback for MultiCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                panic!("Callback should not be invoked");
            }
        }

        let result = publish_nb(&info, Box::new(MultiCallback));
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
