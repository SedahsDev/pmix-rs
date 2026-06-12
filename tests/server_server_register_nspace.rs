//! Tests for `PMIx_server_register_nspace`, `RegisterNspaceCallback`,
//! and the nspace registration callback infrastructure.
//!
//! Note: `PMIx_server_register_nspace` requires a running PMIx server
//! environment (PMIx_server_init must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{RegisterNspaceCallback, server_register_nspace};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// RegisterNspaceCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// RegisterNspaceCallback trait is object-safe and requires Send.
#[test]
fn test_register_nspace_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn RegisterNspaceCallback>) {}

    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// RegisterNspaceCallback::on_complete receives PmixStatus.
#[test]
fn test_register_nspace_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterNspaceCallback for StatusCapture {
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
    let captured = captured.as_ref().unwrap();
    assert!(captured.is_success(), "captured status should be success");
}

/// Multiple callback implementations can coexist.
#[test]
fn test_multiple_callback_implementations() {
    struct Cb1;
    impl RegisterNspaceCallback for Cb1 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct Cb2;
    impl RegisterNspaceCallback for Cb2 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let _cb1: Box<dyn RegisterNspaceCallback> = Box::new(Cb1);
    let _cb2: Box<dyn RegisterNspaceCallback> = Box::new(Cb2);
}

/// Callback can capture state via Arc<Mutex<>>.
#[test]
fn test_callback_captures_state() {
    struct StatefulCb {
        counter: Arc<Mutex<usize>>,
    }
    impl RegisterNspaceCallback for StatefulCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    let counter = Arc::new(Mutex::new(0usize));
    let cb1 = Box::new(StatefulCb {
        counter: Arc::clone(&counter),
    });
    let cb2 = Box::new(StatefulCb {
        counter: Arc::clone(&counter),
    });

    cb1.on_complete(PmixStatus::from_raw(0));
    cb2.on_complete(PmixStatus::from_raw(0));

    assert_eq!(
        *counter.lock().unwrap(),
        2,
        "counter should be incremented twice"
    );
}

/// Callback receives error status correctly.
#[test]
fn test_callback_receives_error_status() {
    struct ErrorCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterNspaceCallback for ErrorCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(ErrorCapture {
        status: Arc::clone(&status),
    });

    // Simulate error status (-1 = PMIX_ERROR).
    let error_status = PmixStatus::from_raw(-1);
    cb.on_complete(error_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some());
    assert!(
        !captured.as_ref().unwrap().is_success(),
        "captured status should be an error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// server_register_nspace — API signature and validation
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_nspace has the expected signature.
#[test]
fn test_server_register_nspace_signature() {
    // Verify the function compiles with the expected parameter types
    // by assigning it to a typed variable.
    fn _check_signature() {
        struct DummyCb;
        impl RegisterNspaceCallback for DummyCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = pmix::InfoBuilder::new().build();
        let _f: fn(
            &str,
            i32,
            &pmix::Info,
            Box<dyn RegisterNspaceCallback>,
        ) -> Result<(), PmixStatus> = server_register_nspace;
        let _ = (_f, &info); // suppress unused warnings
    }
}

/// server_register_nspace with NUL byte in nspace returns error immediately.
#[test]
fn test_register_nspace_nul_in_nspace() {
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let result = server_register_nspace(
        "job\0name", // Contains NUL byte
        4,
        &info,
        Box::new(DummyCb),
    );

    // Should return Err because CString::new fails on NUL bytes.
    assert!(
        result.is_err(),
        "register_nspace should reject nspace containing NUL byte"
    );
    let err = result.unwrap_err();
    assert!(!err.is_success(), "error status should not be success");
}

/// server_register_nspace compiles with valid nspace (will fail without PMIx daemon).
#[test]
fn test_register_nspace_valid_nspace_signature() {
    // Just verify the function compiles with correct types.
    // The actual call will fail without a PMIx server environment,
    // but that's expected and tested in the ignored integration tests.
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_register_nspace("myjob.12345", 4, &info, Box::new(DummyCb));
    // We don't assert the result because it depends on PMIx server state.
}

/// server_register_nspace with zero local procs compiles.
#[test]
fn test_register_nspace_zero_local_procs() {
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_register_nspace("myjob.67890", 0, &info, Box::new(DummyCb));
}

/// server_register_nspace with large nlocalprocs compiles.
#[test]
fn test_register_nspace_large_nlocalprocs() {
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_register_nspace("bigjob.99999", 1024, &info, Box::new(DummyCb));
}

/// server_register_nspace with empty info array compiles.
#[test]
fn test_register_nspace_empty_info() {
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_register_nspace("test_nspace", 2, &info, Box::new(DummyCb));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server environment
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_nspace with a running PMIx server should succeed.
///
/// This test is ignored by default because it requires a PMIx server
/// environment. Run with: `cargo test -- --ignored --test-threads=1`
/// in an environment where PMIx_server_init has been called.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_nspace_with_server() {
    use std::sync::Arc;
    use std::sync::Mutex;

    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterNspaceCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = pmix::InfoBuilder::new().build();
    let result = server_register_nspace("test_nspace", 4, &info, cb);

    // The initial call should succeed (request accepted).
    assert!(result.is_ok(), "register_nspace request should be accepted");

    // The callback status should eventually be set.
    // In a real test environment, we would wait for the callback.
    // Here we just verify the initial call works.
}

/// server_register_nspace with multiple nspaces should work sequentially.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_nspace_multiple_nspaces() {
    use std::sync::Arc;
    use std::sync::Mutex;

    struct CountCb {
        count: Arc<Mutex<usize>>,
    }
    impl RegisterNspaceCallback for CountCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let count = Arc::new(Mutex::new(0usize));
    let info = pmix::InfoBuilder::new().build();

    for i in 0..3 {
        let cb = Box::new(CountCb {
            count: Arc::clone(&count),
        });
        let nspace = format!("job_{}", i);
        let result = server_register_nspace(&nspace, 4, &info, cb);
        assert!(
            result.is_ok(),
            "register_nspace for {} should be accepted",
            nspace
        );
    }
}

/// server_register_nspace with invalid nspace (empty string) should fail.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_nspace_empty_nspace() {
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    let result = server_register_nspace("", 4, &info, Box::new(DummyCb));

    // Empty nspace should be rejected by the PMIx library.
    // The callback should NOT be called on immediate rejection.
    assert!(
        result.is_err(),
        "register_nspace with empty nspace should be rejected"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback registry behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// Callback registry assigns unique request IDs.
#[test]
fn test_callback_registry_unique_ids() {
    // Verify that multiple callback registrations don't conflict.
    // We can't directly test the internal registry, but we can verify
    // that multiple callback creations work without panicking.
    struct DummyCb;
    impl RegisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = pmix::InfoBuilder::new().build();
    // Create multiple callbacks — each should get a unique ID internally.
    for _ in 0..10 {
        let _result = server_register_nspace("test", 1, &info, Box::new(DummyCb));
    }
}

/// Callback trait is Send — can be used across threads.
#[test]
fn test_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn RegisterNspaceCallback>>();
}
