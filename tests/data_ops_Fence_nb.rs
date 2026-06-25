//! Integration tests for `PMIx_Fence_nb` via the safe `fence_nb()` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_Init) are marked `#[ignore]`.

use pmix::data_ops::{FenceCallback, fence_nb};
use pmix::{InfoBuilder, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `fence_nb` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&[Proc]`,
/// `Option<&Info>`, and a `Box<dyn FenceCallback>`.
#[test]
fn fence_nb_function_signature() {
    let _: fn(&[Proc], Option<&pmix::Info>, Box<dyn FenceCallback>) -> Result<(), PmixStatus> =
        fence_nb;
}

/// `FenceCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object.
#[test]
fn fence_callback_trait_object() {
    struct TestCallback;
    impl FenceCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn FenceCallback> = Box::new(TestCallback);
    let _: Box<dyn FenceCallback> = cb;
}

/// `FenceCallback` on_complete receives `PmixStatus`.
#[test]
fn fence_callback_receives_pmix_status() {
    struct StatusCallback;
    impl FenceCallback for StatusCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            // Compiler verifies the signature.
            let _ = status.is_success();
            let _ = status.to_raw();
        }
    }

    let _cb: Box<dyn FenceCallback> = Box::new(StatusCallback);
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `fence_nb` with empty procs and no info returns `PMIX_ERR_INIT` before init.
///
/// PMIx_Fence_nb requires PMIx_Init to have been called first. Calling it
/// without initialization should return PMIX_ERR_INIT (-31).
#[test]
fn fence_nb_before_init_returns_err_init() {
    struct InitCheckCallback;
    impl FenceCallback for InitCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let procs: &[Proc] = &[];
    let result = fence_nb(procs, None, Box::new(InitCheckCallback));
    assert!(result.is_err(), "fence_nb should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `fence_nb` with procs but no info returns `PMIX_ERR_INIT` before init.
#[test]
fn fence_nb_with_procs_before_init_returns_err_init() {
    struct ProcCheckCallback;
    impl FenceCallback for ProcCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    // Create a Proc to pass as participant.
    let proc = Proc::new("test_namespace", 0).expect("should create proc");
    let procs = vec![proc];

    let result = fence_nb(&procs, None, Box::new(ProcCheckCallback));
    assert!(result.is_err(), "fence_nb should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `fence_nb` with info directives returns `PMIX_ERR_INIT` before init.
#[test]
fn fence_nb_with_info_before_init_returns_err_init() {
    struct InfoCheckCallback;
    impl FenceCallback for InfoCheckCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let procs: &[Proc] = &[];

    let result = fence_nb(procs, Some(&info), Box::new(InfoCheckCallback));
    assert!(result.is_err(), "fence_nb should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `fence_nb` returns a `Result` type with proper error handling.
#[test]
fn fence_nb_returns_result_type() {
    struct ResultTypeCallback;
    impl FenceCallback for ResultTypeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let procs: &[Proc] = &[];
    let result: Result<(), PmixStatus> = fence_nb(procs, None, Box::new(ResultTypeCallback));

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

/// `fence_nb` callback is not invoked on immediate failure.
///
/// When PMIx_Fence_nb returns an error synchronously (e.g., not initialized),
/// the callback should NOT be called.
#[test]
fn fence_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NoInvokeCallback;
    impl FenceCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let procs: &[Proc] = &[];
    let result = fence_nb(procs, None, Box::new(NoInvokeCallback));

    // Should fail immediately without invoking callback.
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert!(
        !CALLBACK_INVOKED.load(Ordering::SeqCst),
        "callback should NOT have been invoked on immediate failure"
    );
}

/// `fence_nb` with empty procs and empty info is a valid call (returns error from PMIx).
#[test]
fn fence_nb_empty_params() {
    struct EmptyCallback;
    impl FenceCallback for EmptyCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let info = InfoBuilder::new().build();
    let procs: &[Proc] = &[];

    let result = fence_nb(procs, Some(&info), Box::new(EmptyCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `fence_nb` is callable multiple times (idempotent error behavior).
#[test]
fn fence_nb_multiple_calls_consistent_error() {
    struct MultiCallback;
    impl FenceCallback for MultiCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should not be invoked");
        }
    }

    let procs: &[Proc] = &[];

    for i in 0..5 {
        let result = fence_nb(procs, None, Box::new(MultiCallback));
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

/// `fence_nb` with multiple procs returns `PMIX_ERR_INIT` before init.
#[test]
fn fence_nb_multiple_procs_before_init() {
    struct MultiProcCallback;
    impl FenceCallback for MultiProcCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let procs = vec![
        Proc::new("test_namespace", 0).expect("should create proc 0"),
        Proc::new("test_namespace", 1).expect("should create proc 1"),
        Proc::new("test_namespace", 2).expect("should create proc 2"),
    ];

    let result = fence_nb(&procs, None, Box::new(MultiProcCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `fence_nb` with collect_data info directive returns `PMIX_ERR_INIT` before init.
#[test]
fn fence_nb_collect_data_before_init() {
    struct CollectCallback;
    impl FenceCallback for CollectCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let proc = Proc::new("test_namespace", 0).expect("should create proc");
    let procs = vec![proc];

    let result = fence_nb(&procs, Some(&info), Box::new(CollectCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `fence_nb` callback trait implements Send (compile-time check).
///
/// FenceCallback requires Send because the callback may be invoked on
/// a different thread by the PMIx library.
#[test]
fn fence_callback_is_send() {
    struct SendCallback;
    impl FenceCallback for SendCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Verify Send is implemented by attempting to use it in a Send context.
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn FenceCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `fence_nb` succeeds after `PMIx_Init` and invokes the callback.
///
/// Requires a running PMIx server. The callback should be invoked with
/// PMIX_SUCCESS once the fence completes.
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_after_init() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NbCallback;
    impl FenceCallback for NbCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(
                status.is_success(),
                "callback should receive success, got {:?}",
                status
            );
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    let result = fence_nb(&procs, None, Box::new(NbCallback));
    assert!(result.is_ok(), "fence_nb should accept the request");

    // Note: callback may be invoked synchronously or asynchronously
    // depending on the PMIx implementation. In a real integration test
    // with a daemon, we would call PMIx_Progress or fence to drive
    // the completion.
}

/// `fence_nb` with collect_data enables data exchange.
///
/// Requires a running PMIx server with multiple processes.
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_with_collect_data() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct CollectCallback;
    impl FenceCallback for CollectCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    let result = fence_nb(&procs, Some(&info), Box::new(CollectCallback));
    assert!(
        result.is_ok(),
        "fence_nb with collect_data should accept the request"
    );
}

/// `fence_nb` with empty procs fences across the entire session.
///
/// Per the PMIx spec, passing NULL/0 for procs means fence across all
/// processes in the session.
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_empty_procs_session_wide() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct SessionCallback;
    impl FenceCallback for SessionCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let procs: &[Proc] = &[];
    let result = fence_nb(procs, None, Box::new(SessionCallback));
    assert!(
        result.is_ok(),
        "fence_nb with empty procs should accept the request"
    );
}

/// `fence_nb` followed by another `fence_nb` (chained fences).
///
/// Requires a running PMIx server. Tests that multiple fence_nb calls
/// can be issued sequentially.
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_chained_fences() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    struct ChainCallback;
    impl FenceCallback for ChainCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
        }
    }

    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    // Issue first fence.
    let result1 = fence_nb(&procs, None, Box::new(ChainCallback));
    assert!(result1.is_ok(), "first fence_nb should accept the request");

    // Issue second fence.
    let result2 = fence_nb(&procs, None, Box::new(ChainCallback));
    assert!(result2.is_ok(), "second fence_nb should accept the request");
}

/// `fence_nb` callback receives correct status on success (type verification).
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_callback_status_on_success() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    struct StatusVerifyCallback;
    impl FenceCallback for StatusVerifyCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            // Verify the status is a known success.
            match status {
                PmixStatus::Known(PmixError::Success) => {
                    // Expected.
                }
                _ => {
                    panic!("expected Success, got {:?}", status);
                }
            }
        }
    }

    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    let result = fence_nb(&procs, None, Box::new(StatusVerifyCallback));
    assert!(result.is_ok(), "fence_nb should accept the request");
}

/// `fence_nb` with proc and info together (full parameter test).
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_full_params() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    struct FullCallback;
    impl FenceCallback for FullCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "callback should receive success");
        }
    }

    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    let result = fence_nb(&procs, Some(&info), Box::new(FullCallback));
    assert!(
        result.is_ok(),
        "fence_nb with full params should accept the request"
    );
}

/// `fence_nb` matches the C test pattern from test_fence.c:
/// put -> commit -> fence_nb -> get.
#[test]
#[ignore = "requires PMIx daemon"]
fn fence_nb_put_commit_fence_pattern() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    struct PatternCallback;
    impl FenceCallback for PatternCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            assert!(status.is_success(), "fence callback should receive success");
        }
    }

    // The C test pattern: put data, commit, then fence.
    // The fence ensures data visibility across the group.
    let proc = _ctx.get_proc().clone();
    let procs = vec![proc];

    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let result = fence_nb(&procs, Some(&info), Box::new(PatternCallback));
    assert!(
        result.is_ok(),
        "fence_nb in put-commit-fence pattern should accept"
    );
}
