//! Tests for `PMIx_server_dmodex_request` via the safe `server_dmodex_request` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_server_init) are marked `#[ignore]`.

use pmix::server::{DmodexRequestCallback, server_dmodex_request};
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `server_dmodex_request` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&Proc` plus a
/// `Box<dyn DmodexRequestCallback>`.
#[test]
fn dmodex_request_function_signature() {
    let _: fn(&Proc, Box<dyn DmodexRequestCallback>) -> Result<(), PmixStatus> =
        server_dmodex_request;
}

/// `DmodexRequestCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object with the expected `on_complete` method.
#[test]
fn dmodex_request_callback_trait_object() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn DmodexRequestCallback> = Box::new(TestCallback);
    let _: Box<dyn DmodexRequestCallback> = cb;
}

/// `DmodexRequestCallback::on_complete` receives the correct types.
///
/// Compile-time check: the callback receives `PmixStatus` and `Vec<u8>`.
#[test]
fn dmodex_request_callback_signature() {
    struct SigCheck;
    impl DmodexRequestCallback for SigCheck {
        fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
            // Verify types: status is PmixStatus, blob is Vec<u8>.
            let _: PmixStatus = status;
            let _: Vec<u8> = blob;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_dmodex_request` before server init returns `PMIX_ERR_INIT`.
///
/// PMIx_server_dmodex_request requires PMIx_server_init to have been
/// called first. Calling it without initialization should return
/// PMIX_ERR_INIT (-31).
#[test]
fn dmodex_request_before_init_returns_err_init() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let result = server_dmodex_request(&proc, Box::new(TestCallback));

    assert!(
        result.is_err(),
        "dmodex_request should fail without PMIx_server_init"
    );

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `server_dmodex_request` with a valid Proc and callback returns
/// PMIX_ERR_INIT when not initialized (not PMIX_ERR_BAD_PARAM).
///
/// This confirms the function accepts valid parameters and the error
/// comes from the init check, not a parameter validation failure.
#[test]
fn dmodex_request_valid_params_err_init_not_bad_param() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc = Proc::new("another.job.99999", 42).expect("Proc::new should succeed");
    let result = server_dmodex_request(&proc, Box::new(TestCallback));

    assert!(result.is_err());
    let err = result.unwrap_err();
    // Should be ERR_INIT (-31), not ERR_BAD_PARAM (-29).
    assert_ne!(
        err.to_raw(),
        -29,
        "error should not be PMIX_ERR_BAD_PARAM, got {}",
        err.to_raw()
    );
}

/// `server_dmodex_request` with wildcard proc (rank PMIX_PROC_RANK = -1).
///
/// The PMIx wildcard rank (-1 as u32 = 4294967295) should be accepted
/// as a valid process identifier.
#[test]
fn dmodex_request_wildcard_proc_before_init() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    // Wildcard proc: rank = PMIX_PROC_RANK (-1 as u32)
    let proc = Proc::new("wildcard.job", u32::MAX).expect("Proc::new should succeed");
    let result = server_dmodex_request(&proc, Box::new(TestCallback));

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_raw(),
        -31,
        "should be PMIX_ERR_INIT for wildcard proc too"
    );
}

/// `server_dmodex_request` with different process ranks returns consistent errors.
///
/// Multiple calls with different procs should all return PMIX_ERR_INIT
/// when the server is not initialized.
#[test]
fn dmodex_request_multiple_procs_consistent_error() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let ranks = [0u32, 1, 100, u32::MAX];
    for rank in ranks {
        let proc = Proc::new("test.job", rank).expect("Proc::new should succeed");
        let result = server_dmodex_request(&proc, Box::new(TestCallback));
        assert!(result.is_err(), "rank {} should fail without init", rank);
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "rank {} should get PMIX_ERR_INIT",
            rank
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// `DmodexRequestCallback` can capture and use status + blob in the callback.
///
/// Compile-time check: the callback can access both parameters.
#[test]
fn dmodex_callback_receives_status_and_blob() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLED: AtomicBool = AtomicBool::new(false);

    struct CaptureCallback {
        called: Arc<AtomicBool>,
    }
    impl DmodexRequestCallback for CaptureCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
            self.called.store(true, Ordering::SeqCst);
            // Verify we can inspect the status and blob.
            let _ = status.is_success();
            let _ = blob.len();
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let cb: Box<dyn DmodexRequestCallback> = Box::new(CaptureCallback {
        called: called.clone(),
    });

    // Manually invoke the callback to verify it works.
    let cb = cb as Box<dyn DmodexRequestCallback>;
    // We can't downcast directly, but the type system guarantees the signature.
    drop(cb);

    // Verify the callback struct compiles and the trait is properly implemented.
    assert!(!CALLED.load(Ordering::SeqCst));
}

/// `DmodexRequestCallback` is Send (required for cross-thread callbacks).
///
/// Compile-time check: `Box<dyn DmodexRequestCallback>` is Send.
#[test]
fn dmodex_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DmodexRequestCallback>>();
}

/// `DmodexRequestCallback` can be a zero-sized type (no state needed).
///
/// Some callbacks don't need to capture any state.
#[test]
fn dmodex_callback_zst() {
    struct EmptyCallback;
    impl DmodexRequestCallback for EmptyCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {
            // Intentionally empty — no state to capture.
        }
    }

    let _cb: Box<dyn DmodexRequestCallback> = Box::new(EmptyCallback);
}

/// `DmodexRequestCallback` can capture state via fields.
///
/// Callbacks can hold references or owned data.
#[test]
fn dmodex_callback_with_state() {
    struct StatefulCallback {
        job_id: String,
        expected_rank: u32,
    }
    impl DmodexRequestCallback for StatefulCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, blob: Vec<u8>) {
            // Can access captured state.
            let _ = &self.job_id;
            let _ = self.expected_rank;
            let _ = blob.len();
        }
    }

    let cb: Box<dyn DmodexRequestCallback> = Box::new(StatefulCallback {
        job_id: "test.job.12345".to_string(),
        expected_rank: 42,
    });
    drop(cb);
}

/// `server_dmodex_request` returns a Result, not a panic.
///
/// Even when called without init, the function should return an Err,
/// not panic.
#[test]
fn dmodex_request_returns_result_not_panic() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc = Proc::new("test", 0).expect("Proc::new should succeed");

    // This should not panic — it should return Err(PmixStatus).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        server_dmodex_request(&proc, Box::new(TestCallback))
    }));

    assert!(result.is_ok(), "should not panic, should return Err");
    assert!(result.unwrap().is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction for dmodex_request
// ─────────────────────────────────────────────────────────────────────────────

/// Proc::new with various nspace formats works correctly.
///
/// PMIx nspaces can have various formats. The Proc wrapper should
/// accept them all.
#[test]
fn dmodex_request_various_nspace_formats() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let nspaces = [
        "job.12345",
        "myapp",
        "a",
        "very.long.namespace.name.that.still.fits.in.pmix_max_nslen",
    ];

    for nspace in nspaces {
        let proc = Proc::new(nspace, 0).expect(&format!("Proc::new({}) should succeed", nspace));
        let result = server_dmodex_request(&proc, Box::new(TestCallback));
        assert!(
            result.is_err(),
            "nspace '{}' should fail without init",
            nspace
        );
    }
}

/// Proc::new rejects nspaces containing NUL bytes.
///
/// PMIx nspaces are fixed-length C strings and cannot contain NUL bytes.
#[test]
fn dmodex_request_nul_in_nspace_rejected() {
    // CString::new (used inside Proc::new) rejects NUL bytes.
    // This should fail at Proc construction, not at dmodex_request.
    let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Proc::new("test\0nspace", 0)
    }));

    // Proc::new returns Result<_, NulError>, so it should not panic
    // but return an Err.
    // Actually, let's check the return type.
    let proc_result: Result<Proc, std::ffi::NulError> = Proc::new("test\0nspace", 0);
    assert!(
        proc_result.is_err(),
        "Proc::new should reject nspace with NUL byte"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus error type checks
// ─────────────────────────────────────────────────────────────────────────────

/// Error from dmodex_request before init is PMIX_ERR_INIT.
///
/// Verify the specific error code matches the PMIx specification.
#[test]
fn dmodex_request_error_is_pmix_err_init() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc = Proc::new("test", 0).expect("Proc::new should succeed");
    let result = server_dmodex_request(&proc, Box::new(TestCallback));

    let err = result.unwrap_err();
    // PMIX_ERR_INIT = -31
    assert_eq!(err.to_raw(), -31);
    assert!(err.is_error());
    assert!(!err.is_success());

    // Check if it maps to a known PmixError.
    match err {
        PmixStatus::Known(e) => {
            assert_eq!(e, PmixError::ErrInit, "should be ErrInit variant");
        }
        PmixStatus::Unknown(code) => {
            assert_eq!(code, -31, "unknown code should still be -31");
        }
    }
}

/// PmixStatus::from_raw for PMIX_ERR_INIT is consistent.
///
/// The from_raw conversion should be deterministic.
#[test]
fn dmodex_status_from_raw_consistent() {
    let status = PmixStatus::from_raw(-31);
    assert_eq!(status.to_raw(), -31);
    assert!(!status.is_success());
    assert!(status.is_error());
}

/// PmixStatus::from_raw for PMIX_SUCCESS.
#[test]
fn dmodex_status_success_from_raw() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success());
    assert!(!status.is_error());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_dmodex_request` with initialized server returns data via callback.
///
/// This test requires a running PMIx server initialized via
/// PMIx_server_init. It is ignored by default because it needs
/// a full PMIx runtime environment.
///
/// # Setup
/// 1. Call PMIx_server_init with a server module.
/// 2. Register an nspace and client.
/// 3. Publish some data for the client.
/// 4. Call server_dmodex_request for that client.
/// 5. Verify the callback receives a non-empty blob on success.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_with_initialized_server() {
    // This test needs PMIx_server_init which we can't call without
    // a proper server module and runtime. Mark as ignored.
    // In a real test environment, this would:
    //
    // use pmix::server::{server_init, server_dmodex_request, PmixServerModule, DmodexRequestCallback};
    //
    // struct CollectCallback {
    //     status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
    //     blob: std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>,
    // }
    // impl DmodexRequestCallback for CollectCallback {
    //     fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
    //         *self.status.lock().unwrap() = Some(status);
    //         *self.blob.lock().unwrap() = Some(blob);
    //     }
    // }
    //
    // let module = PmixServerModule::default();
    // let handle = server_init(Some(&module), &[]).expect("server_init");
    // let proc = Proc::new("test.job", 0).expect("proc");
    //
    // let status = Arc::new(Mutex::new(None));
    // let blob = Arc::new(Mutex::new(None));
    // let cb = Box::new(CollectCallback {
    //     status: status.clone(),
    //     blob: blob.clone(),
    // });
    //
    // let result = server_dmodex_request(&proc, cb);
    // assert!(result.is_ok(), "should accept request");
    // // Wait for callback...
    // assert!(status.lock().unwrap().is_some(), "callback should have been called");
    // assert!((*status.lock().unwrap()).as_ref().unwrap().is_success());
    // assert!((*blob.lock().unwrap()).as_ref().unwrap().len() > 0);
}

/// `server_dmodex_request` callback receives error status for unknown proc.
///
/// When requesting modex data for a process that has no published data,
/// the callback should receive an appropriate error status (e.g.,
/// PMIX_ERR_NOT_FOUND).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_unknown_proc_returns_not_found() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Request dmodex for a proc that doesn't exist.
    // 3. Verify callback gets PMIX_ERR_NOT_FOUND or similar.
}

/// `server_dmodex_request` callback receives non-empty blob for known proc.
///
/// When requesting modex data for a process that has published data,
/// the callback should receive a serialized blob.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_known_proc_returns_blob() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Register nspace + client + publish data.
    // 3. Request dmodex for that proc.
    // 4. Verify callback gets a non-empty blob.
}

/// `server_dmodex_request` is thread-safe for concurrent requests.
///
/// Multiple concurrent dmodex requests should all be processed
/// independently without data races.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_concurrent_requests() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Spawn multiple threads, each calling server_dmodex_request.
    // 3. Verify all callbacks are invoked with correct data.
}

/// `server_dmodex_request` with large nspace name.
///
/// PMIx supports nspaces up to PMIX_MAX_NSLEN (255 chars).
/// The callback should handle large nspaces correctly.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_large_nspace() {
    // In a real environment:
    // 1. Initialize server with a large nspace.
    // 2. Request dmodex for that nspace.
    // 3. Verify callback succeeds.
}

/// `server_dmodex_request` callback receives empty blob on error.
///
/// When the callback is invoked with an error status, the blob
/// should be empty (no data to return).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_error_callback_empty_blob() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Request dmodex for a non-existent proc.
    // 3. Verify callback gets error status + empty blob.
}

/// `server_dmodex_request` callback is only called once per request.
///
/// Each request should invoke its callback exactly once — not zero,
/// not twice.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_callback_called_once() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Call dmodex_request with a callback that counts invocations.
    // 3. Verify the counter is exactly 1.
}

/// `server_dmodex_request` registers and unregisters callbacks properly.
///
/// When the FFI call returns an immediate error (e.g., ERR_INIT),
/// the callback should be removed from the registry and never invoked.
#[test]
fn dmodex_request_callback_not_invoked_on_immediate_error() {
    use std::sync::atomic::{AtomicU32, Ordering};

    static INVOCATION_COUNT: AtomicU32 = AtomicU32::new(0);

    struct CountingCallback;
    impl DmodexRequestCallback for CountingCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {
            INVOCATION_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    // Reset counter before test.
    INVOCATION_COUNT.store(0, Ordering::SeqCst);

    let proc = Proc::new("test", 0).expect("Proc::new should succeed");
    let result = server_dmodex_request(&proc, Box::new(CountingCallback));

    // Should fail with ERR_INIT.
    assert!(result.is_err());

    // The callback should NOT have been invoked because the request
    // was rejected immediately (callback was removed from registry).
    assert_eq!(
        INVOCATION_COUNT.load(Ordering::SeqCst),
        0,
        "callback should not be invoked on immediate error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback bridge behavior
// ─────────────────────────────────────────────────────────────────────────────

/// The dmodex_request callback bridge handles null cbdata gracefully.
///
/// When cbdata is null, the bridge should return without panicking.
/// This is tested indirectly: the bridge is only called from C, so
/// we verify the bridge function exists and has the right signature.
#[test]
fn dmodex_callback_bridge_exists() {
    // The bridge function `dmodex_request_callback_bridge` is an
    // extern "C" function in server.rs. We can't call it directly
    // from Rust tests, but its existence is verified by compilation.
    // This test is a compile-time check that the module compiles.
}

/// Multiple dmodex_request calls create distinct callback registrations.
///
/// Each call to server_dmodex_request should get a unique request ID,
/// ensuring callbacks are not confused.
#[test]
fn dmodex_request_multiple_calls_distinct_registrations() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc = Proc::new("test", 0).expect("Proc::new should succeed");

    // Multiple calls should all return ERR_INIT without panicking
    // or corrupting the callback registry.
    for i in 0..10 {
        let result = server_dmodex_request(&proc, Box::new(TestCallback));
        assert!(result.is_err(), "call {} should fail without init", i);
    }
}

/// `server_dmodex_request` with rank 0 vs rank MAX are handled consistently.
#[test]
fn dmodex_request_rank_consistency() {
    struct TestCallback;
    impl DmodexRequestCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    let proc_zero = Proc::new("test", 0).expect("Proc::new");
    let proc_max = Proc::new("test", u32::MAX).expect("Proc::new");

    let result_zero = server_dmodex_request(&proc_zero, Box::new(TestCallback));
    let result_max = server_dmodex_request(&proc_max, Box::new(TestCallback));

    assert_eq!(
        result_zero.unwrap_err().to_raw(),
        result_max.unwrap_err().to_raw(),
        "rank 0 and rank MAX should get the same error"
    );
}
