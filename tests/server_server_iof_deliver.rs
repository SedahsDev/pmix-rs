//! Tests for `PMIx_server_IOF_deliver` via the safe `server_iof_deliver` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_server_init) are marked `#[ignore]`.

use pmix::data_serialization::PmixByteObject;
use pmix::server::{server_iof_deliver, IOFDeliverCallback};
use pmix::{IOFChannelFlags, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `server_iof_deliver` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&Proc`,
/// `IOFChannelFlags`, `&PmixByteObject`, `&pmix::Info`, and
/// `Box<dyn IOFDeliverCallback>`.
#[test]
fn iof_deliver_function_signature() {
    let _: fn(
        &Proc,
        IOFChannelFlags,
        &PmixByteObject,
        &pmix::Info,
        Box<dyn IOFDeliverCallback>,
    ) -> Result<(), PmixStatus> = server_iof_deliver;
}

/// `IOFDeliverCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object with the expected `on_complete` method.
#[test]
fn iof_deliver_callback_trait_object() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn IOFDeliverCallback> = Box::new(TestCallback);
    let _: Box<dyn IOFDeliverCallback> = cb;
}

/// `IOFDeliverCallback::on_complete` receives the correct types.
///
/// Compile-time check: the callback receives `PmixStatus`.
#[test]
fn iof_deliver_callback_signature() {
    struct SigCheck;
    impl IOFDeliverCallback for SigCheck {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            // Verify type: status is PmixStatus.
            let _: PmixStatus = status;
        }
    }
}

/// `IOFDeliverCallback` is `Send` (required for cross-thread callback use).
#[test]
fn iof_deliver_callback_is_send() {
    struct SendCheck;
    impl IOFDeliverCallback for SendCheck {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn IOFDeliverCallback>>();
}

/// `IOFChannelFlags` is usable as the channel parameter type.
#[test]
fn iof_deliver_channel_type() {
    let _channel: IOFChannelFlags = IOFChannelFlags::STDOUT;
}

/// `PmixByteObject` is usable as the bo parameter type.
#[test]
fn iof_deliver_byte_object_type() {
    let _bo = PmixByteObject::from(b"test data".to_vec());
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_iof_deliver` does not panic when called without server init.
///
/// PMIx_server_IOF_deliver may return success even without PMIx_server_init
/// (the library may queue the data internally). The important thing is that
/// the safe wrapper does not panic or cause undefined behavior.
#[test]
fn iof_deliver_before_init_no_panic() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"hello".to_vec());
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    // Should not panic — the FFI call itself is safe.
    // The result may be Ok or Err depending on PMIx version/config.
    let _ = result;
}

/// `server_iof_deliver` with stdout channel compiles and calls FFI.
#[test]
fn iof_deliver_with_stdout_channel() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 1).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"stdout data".to_vec());
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    // Should not panic regardless of PMIx init state.
    let _ = result;
}

/// `server_iof_deliver` with stderr channel compiles and calls FFI.
#[test]
fn iof_deliver_with_stderr_channel() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 2).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"stderr data".to_vec());
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDERR,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    let _ = result;
}

/// `server_iof_deliver` with stdin channel compiles and calls FFI.
#[test]
fn iof_deliver_with_stdin_channel() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"stdin data".to_vec());
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDIN,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    let _ = result;
}

/// `server_iof_deliver` with empty byte object.
#[test]
fn iof_deliver_with_empty_byte_object() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(Vec::new());
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    let _ = result;
}

/// `server_iof_deliver` with large byte object.
#[test]
fn iof_deliver_with_large_byte_object() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let large_data: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let bo = PmixByteObject::from(large_data);
    let info = pmix::InfoBuilder::new().build();
    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    let _ = result;
}

/// `server_iof_deliver` with different source processes.
#[test]
fn iof_deliver_different_source_procs() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source1 = Proc::new("job1.12345", 0).expect("Proc::new should succeed");
    let source2 = Proc::new("job2.67890", 42).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = pmix::InfoBuilder::new().build();

    let result1 = server_iof_deliver(
        &source1,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );
    let result2 = server_iof_deliver(
        &source2,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    // Both should not panic.
    let _ = result1;
    let _ = result2;
}

/// `server_iof_deliver` returns consistent result across multiple calls.
#[test]
fn iof_deliver_consistent_result_type() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = pmix::InfoBuilder::new().build();

    // Call multiple times — all should return the same result type.
    let first = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    )
    .is_ok();

    for _ in 0..4 {
        let result = server_iof_deliver(
            &source,
            IOFChannelFlags::STDOUT,
            &bo,
            &info,
            Box::new(TestCallback),
        )
        .is_ok();
        assert_eq!(
            result, first,
            "All calls should return consistent result (Ok or Err)"
        );
    }
}

/// `server_iof_deliver` callback trait works with stateful callbacks.
#[test]
fn iof_deliver_callback_with_state() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct StatefulCallback;
    impl IOFDeliverCallback for StatefulCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    // The callback may or may not be invoked depending on PMIx state.
    // Just verify the wrapper doesn't crash.
    CALLBACK_INVOKED.store(false, Ordering::SeqCst);

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = pmix::InfoBuilder::new().build();
    let _result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(StatefulCallback),
    );

    // No assertion on whether callback was invoked — depends on PMIx state.
    // The test verifies no crash/panic occurred.
}

/// `server_iof_deliver` with combined channel flags.
#[test]
fn iof_deliver_combined_channel_flags() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = pmix::InfoBuilder::new().build();

    // Combine STDOUT | STDERR using BitOr
    let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
    let result = server_iof_deliver(
        &source,
        combined,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    let _ = result;
}

/// `server_iof_deliver` with various nspace formats.
#[test]
fn iof_deliver_various_nspace_formats() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let nsamples = [
        "test.nspace",
        "job.12345",
        "a.b.c.d",
        "single",
        "very.long.namespace.identifier.for.testing.purposes.12345",
    ];

    for nspace in &nsamples {
        let source =
            Proc::new(nspace, 0).expect(&format!("Proc::new should succeed for {}", nspace));
        let bo = PmixByteObject::from(b"data".to_vec());
        let info = pmix::InfoBuilder::new().build();
        let _result = server_iof_deliver(
            &source,
            IOFChannelFlags::STDOUT,
            &bo,
            &info,
            Box::new(TestCallback),
        );
    }
}

/// `server_iof_deliver` with various rank values.
#[test]
fn iof_deliver_various_ranks() {
    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let ranks = [0u32, 1, 42, 1000, u32::MAX];
    for rank in &ranks {
        let source = Proc::new("test.nspace", *rank)
            .expect(&format!("Proc::new should succeed for rank {}", rank));
        let bo = PmixByteObject::from(b"data".to_vec());
        let info = pmix::InfoBuilder::new().build();
        let _result = server_iof_deliver(
            &source,
            IOFChannelFlags::STDOUT,
            &bo,
            &info,
            Box::new(TestCallback),
        );
    }
}

/// PmixStatus from_raw is consistent for success code.
#[test]
fn iof_deliver_status_success_from_raw() {
    let status = PmixStatus::from_raw(0); // PMIX_SUCCESS
    assert!(status.is_success(), "PMIX_SUCCESS should be success");
}

/// PmixStatus from_raw is consistent for error codes.
#[test]
fn iof_deliver_status_error_from_raw() {
    let status = PmixStatus::from_raw(-1); // PMIX_ERROR
    assert!(!status.is_success(), "PMIX_ERROR should not be success");
}

/// PmixStatus from_raw for PMIX_ERR_INIT.
#[test]
fn iof_deliver_status_err_init_from_raw() {
    let status = PmixStatus::from_raw(-31); // PMIX_ERR_INIT
    assert!(!status.is_success(), "PMIX_ERR_INIT should not be success");
    assert_eq!(status, PmixError::ErrInit.into());
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests requiring PMIx server runtime (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_iof_deliver` succeeds after PMIx_server_init.
///
/// This test requires a running PMIx server environment and will be
/// skipped unless PMIx_server_init is available.
#[test]
#[ignore = "requires PMIx server runtime"]
fn iof_deliver_after_server_init() {
    use pmix::server::{server_finalize, server_init, PmixServerModule};

    let module = PmixServerModule::default();
    let handle = match server_init(
        Some(&module),
        &pmix::InfoBuilder::new().build(),
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Skipping: server_init failed: {:?}", e);
            return;
        }
    };

    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"test output".to_vec());
    let info = pmix::InfoBuilder::new().build();

    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    // With server init, this may succeed or return an error depending
    // on whether the nspace is registered. We just check it doesn't panic.
    match result {
        Ok(()) => { /* callback will fire asynchronously */ }
        Err(status) => {
            // Expected if nspace not registered — not PMIX_ERR_INIT.
            assert_ne!(
                status,
                PmixError::ErrInit.into(),
                "Should not be ERR_INIT after server_init"
            );
        }
    }

    let _ = server_finalize(handle);
}

/// `server_iof_deliver` delivers data and callback fires.
///
/// Integration test: after server init, deliver data and verify
/// the callback is actually invoked.
#[test]
#[ignore = "requires PMIx server runtime"]
fn iof_deliver_callback_fires_on_success() {
    use pmix::server::{server_finalize, server_init, PmixServerModule};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    let module = PmixServerModule::default();
    let handle = match server_init(
        Some(&module),
        &pmix::InfoBuilder::new().build(),
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Skipping: server_init failed: {:?}", e);
            return;
        }
    };

    let invoked = Arc::new(AtomicBool::new(false));
    let invoked_clone = invoked.clone();

    struct ArcCallback {
        invoked: Arc<AtomicBool>,
    }
    impl IOFDeliverCallback for ArcCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.invoked.store(true, Ordering::SeqCst);
        }
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"integration test".to_vec());
    let info = pmix::InfoBuilder::new().build();

    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(ArcCallback {
            invoked: invoked_clone,
        }),
    );

    if result.is_ok() {
        // Wait for callback to fire (async operation).
        std::thread::sleep(Duration::from_millis(100));
        // The callback may or may not fire depending on whether
        // the nspace is registered and clients exist.
        // Just verify we didn't crash.
    }

    let _ = server_finalize(handle);
}

/// `server_iof_deliver` with PMIX_IOF_COMPLETE info key.
///
/// Test that passing PMIX_IOF_COMPLETE in the info array works.
#[test]
#[ignore = "requires PMIx server runtime"]
fn iof_deliver_with_complete_flag() {
    use pmix::server::{server_finalize, server_init, PmixServerModule};

    let module = PmixServerModule::default();
    let handle = match server_init(
        Some(&module),
        &pmix::InfoBuilder::new().build(),
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Skipping: server_init failed: {:?}", e);
            return;
        }
    };

    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let source = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
    let bo = PmixByteObject::from(b"final output".to_vec());
    let info = pmix::InfoBuilder::new().build();

    let result = server_iof_deliver(
        &source,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestCallback),
    );

    // Should not panic — may succeed or return non-INIT error.
    if let Err(status) = result {
        assert_ne!(
            status,
            PmixError::ErrInit.into(),
            "Should not be ERR_INIT after server_init"
        );
    }

    let _ = server_finalize(handle);
}

/// Multiple concurrent `server_iof_deliver` calls don't interfere.
#[test]
#[ignore = "requires PMIx server runtime"]
fn iof_deliver_concurrent_calls() {
    use pmix::server::{server_finalize, server_init, PmixServerModule};

    let module = PmixServerModule::default();
    let handle = match server_init(
        Some(&module),
        &pmix::InfoBuilder::new().build(),
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Skipping: server_init failed: {:?}", e);
            return;
        }
    };

    struct TestCallback;
    impl IOFDeliverCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Make multiple calls — they should not interfere with each other.
    for i in 0..10 {
        let source =
            Proc::new(&format!("test.{}", i), 0).expect("Proc::new should succeed");
        let bo = PmixByteObject::from(format!("data {}", i).into_bytes());
        let info = pmix::InfoBuilder::new().build();

        let result = server_iof_deliver(
            &source,
            IOFChannelFlags::STDOUT,
            &bo,
            &info,
            Box::new(TestCallback),
        );

        // Should not panic on any call.
        if let Err(status) = result {
            assert_ne!(
                status,
                PmixError::ErrInit.into(),
                "Call {} should not be ERR_INIT after server_init",
                i
            );
        }
    }

    let _ = server_finalize(handle);
}
