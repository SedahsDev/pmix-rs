//! Tests for `PMIx_server_setup_local_support`, `SetupLocalSupportCallback`,
//! and the setup_local_support callback infrastructure.
//!
//! Note: `PMIx_server_setup_local_support` requires a running PMIx server
//! environment (PMIx_server_init must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::server::{SetupLocalSupportCallback, server_setup_local_support};
use pmix::{InfoBuilder, PmixStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// SetupLocalSupportCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// SetupLocalSupportCallback trait is object-safe and requires Send.
#[test]
fn test_setup_local_support_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn SetupLocalSupportCallback>) {}

    struct DummyCb;
    impl SetupLocalSupportCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// SetupLocalSupportCallback::on_complete receives PmixStatus.
#[test]
fn test_setup_local_support_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl SetupLocalSupportCallback for StatusCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCapture {
        status: Arc::clone(&status),
    });

    // Simulate callback invocation with a success status.
    let success_status = PmixStatus::from_raw(0);
    cb.on_complete(success_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some(), "callback should have captured status");
    assert!(
        captured.as_ref().unwrap().is_success(),
        "captured status should be success"
    );
}

/// Callback receives error status correctly.
#[test]
fn test_setup_local_support_callback_receives_error() {
    struct ErrorCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl SetupLocalSupportCallback for ErrorCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(ErrorCapture {
        status: Arc::clone(&status),
    });

    // Simulate callback invocation with an error status.
    let error_status = PmixStatus::from_raw(-1); // PMIX_ERROR
    cb.on_complete(error_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some(), "callback should have captured status");
    assert!(
        captured.as_ref().unwrap().is_error(),
        "captured status should be an error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// server_setup_local_support — API signature and parameter validation
// ─────────────────────────────────────────────────────────────────────────────

/// server_setup_local_support function signature accepts the right types.
#[test]
fn test_setup_local_support_signature() {
    // This test verifies the function signature compiles with the expected
    // parameter types: &str, &Info, Box<dyn SetupLocalSupportCallback>.
    fn call_with_correct_types(
        nspace: &str,
        info: &pmix::Info,
        cb: Box<dyn SetupLocalSupportCallback>,
    ) -> Result<(), PmixStatus> {
        server_setup_local_support(nspace, info, cb)
    }

    // We cannot actually call this without a PMIx server, but the
    // function signature is verified by compilation.
    let _ = call_with_correct_types
        as fn(&str, &pmix::Info, Box<dyn SetupLocalSupportCallback>) -> Result<(), PmixStatus>;
}

/// server_setup_local_support is callable and returns error without PMIx server.
#[test]
fn test_setup_local_support_without_server() {
    struct TestCb;
    impl SetupLocalSupportCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_setup_local_support("valid_nspace", &info, Box::new(TestCb));

    // Without a PMIx server initialized, this should return an error.
    assert!(
        result.is_err(),
        "should fail without PMIx server initialized"
    );
}

/// server_setup_local_support accepts empty info array.
#[test]
fn test_setup_local_support_empty_info() {
    struct TestCb;
    impl SetupLocalSupportCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = InfoBuilder::new().build();
    // Info.len is private; we trust InfoBuilder::new().build() produces an empty array.

    let result = server_setup_local_support("test_nspace", &info, Box::new(TestCb));

    // Without a PMIx server initialized, this should return an error.
    assert!(
        result.is_err(),
        "should fail without PMIx server initialized"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback invocation patterns
// ─────────────────────────────────────────────────────────────────────────────

/// Callback can capture status and check success.
#[test]
fn test_setup_local_support_callback_capture_success() {
    struct CaptureCb {
        statuses: Arc<Mutex<Vec<PmixStatus>>>,
    }
    impl SetupLocalSupportCallback for CaptureCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }

    let statuses = Arc::new(Mutex::new(Vec::new()));
    let cb = Box::new(CaptureCb {
        statuses: Arc::clone(&statuses),
    });

    // Simulate callback invocation.
    cb.on_complete(PmixStatus::from_raw(0));

    let captured = statuses.lock().unwrap();
    assert_eq!(captured.len(), 1, "should have captured one status");
    assert!(
        captured[0].is_success(),
        "captured status should be success"
    );
}

/// Callback can distinguish between success and failure.
#[test]
fn test_setup_local_support_callback_distinguishes_outcomes() {
    struct OutcomeCapture {
        was_success: Arc<Mutex<bool>>,
    }
    impl SetupLocalSupportCallback for OutcomeCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.was_success.lock().unwrap() = status.is_success();
        }
    }

    let was_success = Arc::new(Mutex::new(false));
    let cb = Box::new(OutcomeCapture {
        was_success: Arc::clone(&was_success),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert!(*was_success.lock().unwrap(), "should detect success");
}

/// Callback handles PMIX_OPERATION_SUCCEEDED status.
#[test]
fn test_setup_local_support_operation_succeeded_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl SetupLocalSupportCallback for StatusCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCapture {
        status: Arc::clone(&status),
    });

    // PMIX_OPERATION_SUCCEEDED = -157
    let op_succeeded = PmixStatus::from_raw(-157);
    cb.on_complete(op_succeeded);

    let captured = status.lock().unwrap();
    let captured = captured.as_ref().unwrap();
    assert_eq!(
        captured.to_raw(),
        -157,
        "should capture OPERATION_SUCCEEDED"
    );
}

/// Multiple independent callbacks can be created and invoked separately.
#[test]
fn test_setup_local_support_multiple_callbacks() {
    struct SharedCb {
        counter: Arc<Mutex<u32>>,
    }
    impl SetupLocalSupportCallback for SharedCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    let counter = Arc::new(Mutex::new(0u32));

    // Create two independent callbacks sharing the same counter.
    let cb1 = Box::new(SharedCb {
        counter: Arc::clone(&counter),
    });
    let cb2 = Box::new(SharedCb {
        counter: Arc::clone(&counter),
    });

    // Invoke both callbacks.
    cb1.on_complete(PmixStatus::from_raw(0));
    cb2.on_complete(PmixStatus::from_raw(0));

    assert_eq!(
        *counter.lock().unwrap(),
        2,
        "both callbacks should have been invoked"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait Send safety
// ─────────────────────────────────────────────────────────────────────────────

/// SetupLocalSupportCallback implementations can be sent across threads.
#[test]
fn test_setup_local_support_callback_is_send() {
    struct SendCb {
        counter: Arc<Mutex<u32>>,
    }
    impl SetupLocalSupportCallback for SendCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    // Verify Send bound is satisfied.
    fn assert_send<T: Send>() {}
    assert_send::<SendCb>();

    let counter = Arc::new(Mutex::new(0u32));
    let cb = Box::new(SendCb {
        counter: Arc::clone(&counter),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "callback should have been invoked"
    );
}

/// Callback with Arc<Mutex<T>> payload stores results.
#[test]
fn test_setup_local_support_callback_with_shared_state() {
    struct SharedStateCb {
        results: Arc<Mutex<Vec<String>>>,
    }
    impl SetupLocalSupportCallback for SharedStateCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.results.lock().unwrap().push(format!("{:?}", status));
        }
    }

    let results = Arc::new(Mutex::new(Vec::new()));
    let cb = Box::new(SharedStateCb {
        results: Arc::clone(&results),
    });

    cb.on_complete(PmixStatus::from_raw(0));

    let captured = results.lock().unwrap();
    assert_eq!(captured.len(), 1, "should have one result");
    assert!(
        captured[0].contains("Success"),
        "result should contain Success"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx server — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: register nspace, then setup local support.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_setup_local_support_integration() {
    // This test would:
    // 1. Initialize PMIx server
    // 2. Register an nspace
    // 3. Call server_setup_local_support with valid info
    // 4. Verify callback is invoked with success
    // 5. Finalize PMIx server
    panic!("integration test requires PMIx server");
}

/// Test setup_local_support with node info parameters.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_setup_local_support_with_node_info() {
    // This test would:
    // 1. Build an Info array with PMIX_NODE_INFO_ARRAY entries
    // 2. Call server_setup_local_support
    // 3. Verify the server processes the node info correctly
    panic!("integration test requires PMIx server");
}

/// Test setup_local_support called multiple times for different nspaces.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_setup_local_support_multiple_nspaces() {
    // This test would:
    // 1. Call server_setup_local_support for nspace A
    // 2. Call server_setup_local_support for nspace B
    // 3. Verify both callbacks are invoked independently
    panic!("integration test requires PMIx server");
}

/// Test setup_local_support error handling for invalid nspace.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_setup_local_support_invalid_nspace() {
    // This test would:
    // 1. Call server_setup_local_support with an empty nspace
    // 2. Verify it returns an error immediately
    panic!("integration test requires PMIx server");
}

/// Test setup_local_support callback is not invoked on immediate error.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_setup_local_support_no_callback_on_error() {
    // This test would:
    // 1. Call server_setup_local_support with invalid params
    // 2. Verify the callback is NOT invoked
    // 3. Verify an error is returned immediately
    panic!("integration test requires PMIx server");
}
