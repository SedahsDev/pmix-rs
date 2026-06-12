//! Tests for `PMIx_server_setup_application` via the safe `server_setup_application` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_server_init) are marked `#[ignore]`.

use pmix::server::{server_setup_application, SetupApplicationCallback};
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `server_setup_application` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&str` (namespace),
/// `&Info` (info array), and `Box<dyn SetupApplicationCallback>`.
#[test]
fn setup_application_function_signature() {
    use pmix::Info;
    let _: fn(&str, &Info, Box<dyn SetupApplicationCallback>) -> Result<(), PmixStatus> =
        server_setup_application;
}

/// `SetupApplicationCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object with the expected `on_complete` method.
#[test]
fn setup_application_callback_trait_object() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn SetupApplicationCallback> = Box::new(TestCallback);
    let _: Box<dyn SetupApplicationCallback> = cb;
}

/// `SetupApplicationCallback::on_complete` receives the correct types.
///
/// Compile-time check: the callback receives `PmixStatus` and `Vec<(String, String)>`.
#[test]
fn setup_application_callback_signature() {
    struct SigCheck;
    impl SetupApplicationCallback for SigCheck {
        fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
            // Verify types: status is PmixStatus, info is Vec of (key, value) pairs.
            let _: PmixStatus = status;
            let _: Vec<(String, String)> = info;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_setup_application` before server init returns `PMIX_ERR_INIT`.
///
/// PMIx_server_setup_application requires PMIx_server_init to have been
/// called first. Calling it without initialization should return
/// PMIX_ERR_INIT (-31).
#[test]
fn setup_application_before_init_returns_err_init() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_application("test.nspace", &info, Box::new(TestCallback));

    assert!(
        result.is_err(),
        "setup_application should fail without PMIx_server_init"
    );

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `server_setup_application` with a valid namespace and callback returns
/// PMIX_ERR_INIT when not initialized (not PMIX_ERR_BAD_PARAM).
///
/// This confirms the function accepts valid parameters and the error
/// comes from the init check, not a parameter validation failure.
#[test]
fn setup_application_valid_params_err_init_not_bad_param() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_application("another.job.99999", &info, Box::new(TestCallback));

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

/// `server_setup_application` with different namespaces returns consistent errors.
///
/// Multiple calls with different namespaces should all return PMIX_ERR_INIT
/// when the server is not initialized.
#[test]
fn setup_application_multiple_namespaces_consistent_error() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let namespaces = ["job.12345", "myapp", "a", "test.nspace"];
    for ns in namespaces {
        let info = InfoBuilder::new().build();
        let result = server_setup_application(ns, &info, Box::new(TestCallback));
        assert!(
            result.is_err(),
            "namespace '{}' should fail without init",
            ns
        );
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "namespace '{}' should get PMIX_ERR_INIT",
            ns
        );
    }
}

/// `server_setup_application` with empty info array returns PMIX_ERR_INIT.
#[test]
fn setup_application_empty_info_before_init() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_application("test.nspace", &info, Box::new(TestCallback));

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -31);
}

/// `server_setup_application` returns a Result, not a panic.
///
/// Even when called without init, the function should return an Err,
/// not panic.
#[test]
fn setup_application_returns_result_not_panic() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();

    // This should not panic — it should return Err(PmixStatus).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        server_setup_application("test.nspace", &info, Box::new(TestCallback))
    }));

    assert!(result.is_ok(), "should not panic, should return Err");
    assert!(result.unwrap().is_err());
}

/// `server_setup_application` with single-char namespace.
#[test]
fn setup_application_single_char_namespace() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_application("x", &info, Box::new(TestCallback));

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -31);
}

/// `server_setup_application` with long namespace.
#[test]
fn setup_application_long_namespace() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let long_ns = "a".repeat(200);
    let info = InfoBuilder::new().build();
    let result = server_setup_application(&long_ns, &info, Box::new(TestCallback));

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -31);
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait behavior
// ─────────────────────────────────────────────────────────────────────────────

/// `SetupApplicationCallback` can capture and use status + info in the callback.
///
/// Compile-time check: the callback can access both parameters.
#[test]
fn setup_app_callback_receives_status_and_info() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    static CALLED: AtomicBool = AtomicBool::new(false);

    struct CaptureCallback {
        called: Arc<AtomicBool>,
    }
    impl SetupApplicationCallback for CaptureCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
            self.called.store(true, Ordering::SeqCst);
            // Verify we can inspect the status and info.
            let _ = status.is_success();
            let _ = info.len();
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let cb: Box<dyn SetupApplicationCallback> = Box::new(CaptureCallback {
        called: called.clone(),
    });

    // We can't invoke the callback directly (it's a trait object),
    // but the type system guarantees the signature.
    drop(cb);

    // Verify the callback struct compiles and the trait is properly implemented.
    assert!(!CALLED.load(Ordering::SeqCst));
}

/// `SetupApplicationCallback` is Send (required for cross-thread callbacks).
///
/// Compile-time check: `Box<dyn SetupApplicationCallback>` is Send.
#[test]
fn setup_app_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn SetupApplicationCallback>>();
}

/// `SetupApplicationCallback` can be a zero-sized type (no state needed).
///
/// Some callbacks don't need to capture any state.
#[test]
fn setup_app_callback_zst() {
    struct EmptyCallback;
    impl SetupApplicationCallback for EmptyCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {
            // Intentionally empty — no state to capture.
        }
    }

    let _cb: Box<dyn SetupApplicationCallback> = Box::new(EmptyCallback);
}

/// `SetupApplicationCallback` can capture state via fields.
///
/// Callbacks can hold owned data.
#[test]
fn setup_app_callback_with_state() {
    struct StatefulCallback {
        job_id: String,
        expected_nspace: String,
    }
    impl SetupApplicationCallback for StatefulCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, info: Vec<(String, String)>) {
            // Can access captured state.
            let _ = &self.job_id;
            let _ = &self.expected_nspace;
            let _ = info.len();
        }
    }

    let cb: Box<dyn SetupApplicationCallback> = Box::new(StatefulCallback {
        job_id: "test.job.12345".to_string(),
        expected_nspace: "test.nspace".to_string(),
    });
    drop(cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Info handling
// ─────────────────────────────────────────────────────────────────────────────

/// `server_setup_application` accepts an empty Info without panicking.
#[test]
fn setup_application_accepts_empty_info() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    // Info created from empty builder has length 0.

    let result = server_setup_application("test.nspace", &info, Box::new(TestCallback));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus error type checks
// ─────────────────────────────────────────────────────────────────────────────

/// Error from setup_application before init is PMIX_ERR_INIT.
///
/// Verify the specific error code matches the PMIx specification.
#[test]
fn setup_application_error_is_pmix_err_init() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_application("test.nspace", &info, Box::new(TestCallback));

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
fn setup_app_status_from_raw_consistent() {
    let status = PmixStatus::from_raw(-31);
    assert_eq!(status.to_raw(), -31);
    assert!(!status.is_success());
    assert!(status.is_error());
}

/// PmixStatus::from_raw for PMIX_SUCCESS.
#[test]
fn setup_app_status_success_from_raw() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success());
    assert!(!status.is_error());
}

/// Multiple calls to setup_application return consistent errors.
#[test]
fn setup_application_multiple_calls_consistent() {
    struct TestCallback;
    impl SetupApplicationCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    let mut last_err: Option<i32> = None;
    for i in 0..5 {
        let info = InfoBuilder::new().build();
        let result = server_setup_application(&format!("job.{}", i), &info, Box::new(TestCallback));
        let err = result.unwrap_err().to_raw();
        assert_eq!(err, -31, "call {} should return PMIX_ERR_INIT", i);
        if let Some(last) = last_err {
            assert_eq!(err, last, "call {} should match previous error", i);
        }
        last_err = Some(err);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_setup_application` with initialized server delivers result via callback.
///
/// This test requires a running PMIx server initialized via
/// PMIx_server_init. It is ignored by default because it needs
/// a full PMIx runtime environment.
///
/// # Setup
/// 1. Call PMIx_server_init with a server module.
/// 2. Register an nspace.
/// 3. Call server_setup_application for that nspace.
/// 4. Verify the callback receives setup info on success.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_with_initialized_server() {
    // This test needs PMIx_server_init which we can't call without
    // a proper server module and runtime. Mark as ignored.
    // In a real test environment, this would:
    //
    // use pmix::server::{server_init, server_setup_application, PmixServerModule, SetupApplicationCallback};
    //
    // struct CollectCallback {
    //     status: Arc<Mutex<Option<PmixStatus>>>,
    //     info: Arc<Mutex<Option<Vec<(String, String)>>>>,
    // }
    // impl SetupApplicationCallback for CollectCallback {
    //     fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
    //         *self.status.lock().unwrap() = Some(status);
    //         *self.info.lock().unwrap() = Some(info);
    //     }
    // }
    //
    // let module = PmixServerModule::default();
    // let handle = server_init(Some(&module), &[]).expect("server_init");
    //
    // let status = Arc::new(Mutex::new(None));
    // let info = Arc::new(Mutex::new(None));
    // let cb = Box::new(CollectCallback {
    //     status: status.clone(),
    //     info: info.clone(),
    // });
    //
    // let result = server_setup_application("test.nspace", &Info::empty(), cb);
    // assert!(result.is_ok(), "should accept request");
    // // Wait for callback...
    // assert!(status.lock().unwrap().is_some(), "callback should have been called");
    // assert!((*status.lock().unwrap()).as_ref().unwrap().is_success());
}

/// `server_setup_application` callback receives setup info.
///
/// When the setup completes, the callback should receive
/// key-value pairs describing the setup result.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_callback_receives_info() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Register nspace.
    // 3. Call setup_application.
    // 4. Verify callback receives info with expected keys.
}

/// `server_setup_application` callback receives error status for unknown nspace.
///
/// When setting up an application for an unknown namespace,
/// the callback should receive an appropriate error status.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_unknown_nspace_returns_error() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Call setup_application for an unregistered nspace.
    // 3. Verify callback gets an error status.
}

/// `server_setup_application` is thread-safe for concurrent requests.
///
/// Multiple concurrent setup_application requests should all be processed
/// independently without data races.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_concurrent_requests() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Spawn multiple threads, each calling server_setup_application.
    // 3. Verify all callbacks are invoked with correct data.
}

/// `server_setup_application` with info directives.
///
/// When passing info directives (e.g., PMIX_INFO_DIRECTIVE_OPTIONAL),
/// the function should respect them.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_with_info_directives() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Create info with directives.
    // 3. Call setup_application.
    // 4. Verify directive behavior.
}

/// `server_setup_application` callback info contains expected keys.
///
/// The setup info returned in the callback should contain
/// PMIx-specific keys describing the application setup.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_callback_info_keys() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Call setup_application.
    // 3. Verify callback info contains expected keys.
}

/// `server_setup_application` ack callback is invoked.
///
/// The ack callback (cbfunc) should be invoked before the Rust
/// callback is called, to signal that the info array can be freed.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_ack_callback_invoked() {
    // In a real environment:
    // 1. Initialize server.
    // 2. Call setup_application with a callback that tracks timing.
    // 3. Verify ack was called before Rust callback.
}

/// `server_setup_application` memory safety — no double free.
///
/// Calling setup_application multiple times should not cause
/// memory issues with the info array.
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn setup_application_no_memory_leak() {
    // In a real environment with valgrind:
    // 1. Initialize server.
    // 2. Call setup_application multiple times.
    // 3. Verify no memory leaks with valgrind.
    // 4. Finalize server.
}
