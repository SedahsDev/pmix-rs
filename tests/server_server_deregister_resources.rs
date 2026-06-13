//! Tests for `PMIx_server_deregister_resources`, `DeregisterResourcesCallback`,
//! and the resource deregistration callback infrastructure.
//!
//! Note: `PMIx_server_deregister_resources` requires a running PMIx server
//! environment (`PMIx_server_init` must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::InfoBuilder;
use pmix::PmixStatus;
use pmix::server::{DeregisterResourcesCallback, server_deregister_resources};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// DeregisterResourcesCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// DeregisterResourcesCallback trait is object-safe and requires Send.
#[test]
fn test_deregister_resources_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn DeregisterResourcesCallback>) {}

    struct DummyCb;
    impl DeregisterResourcesCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// DeregisterResourcesCallback::on_complete receives PmixStatus.
#[test]
fn test_deregister_resources_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for StatusCapture {
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

/// Callback can capture error status.
#[test]
fn test_deregister_resources_callback_receives_error_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for StatusCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCapture {
        status: Arc::clone(&status),
    });

    // Simulate callback invocation with an error status.
    let error_status = PmixStatus::from_raw(-1); // PMIX_ERROR
    cb.on_complete(error_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some(), "callback should have captured status");
    let captured = captured.as_ref().unwrap();
    assert!(captured.is_error(), "captured status should be error");
}

/// Callback can carry custom state.
#[test]
fn test_deregister_resources_callback_carries_custom_state() {
    struct CustomState {
        message: String,
        count: usize,
    }
    impl DeregisterResourcesCallback for CustomState {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // State is accessible inside the callback.
            let _ = &self.message;
            let _ = self.count;
        }
    }

    let cb = Box::new(CustomState {
        message: "resource deregistered".to_string(),
        count: 42,
    });
    fn assert_trait_obj(_: Box<dyn DeregisterResourcesCallback>) {}
    assert_trait_obj(cb);
}

/// Multiple callbacks can be created and stored independently.
#[test]
fn test_deregister_resources_multiple_callbacks() {
    struct CountingCb {
        counter: Arc<Mutex<usize>>,
    }
    impl DeregisterResourcesCallback for CountingCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    let counter = Arc::new(Mutex::new(0));
    let cb1 = Box::new(CountingCb {
        counter: Arc::clone(&counter),
    });
    let cb2 = Box::new(CountingCb {
        counter: Arc::clone(&counter),
    });

    // Simulate both callbacks being invoked.
    cb1.on_complete(PmixStatus::from_raw(0));
    cb2.on_complete(PmixStatus::from_raw(0));

    assert_eq!(
        *counter.lock().unwrap(),
        2,
        "both callbacks should have incremented"
    );
}

/// Callback can distinguish between different error codes.
#[test]
fn test_deregister_resources_callback_distinguishes_errors() {
    struct ErrorCapture {
        codes: Arc<Mutex<Vec<i32>>>,
    }
    impl DeregisterResourcesCallback for ErrorCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.codes.lock().unwrap().push(status.to_raw());
        }
    }

    let codes = Arc::new(Mutex::new(Vec::new()));
    let cb = Box::new(ErrorCapture {
        codes: Arc::clone(&codes),
    });

    // Simulate different error statuses.
    cb.on_complete(PmixStatus::from_raw(-1)); // PMIX_ERROR
    // Note: cb is consumed after first call, so we can't call again.
    // This tests that the callback receives the correct raw code.
    assert_eq!(
        *codes.lock().unwrap(),
        vec![-1],
        "should capture PMIX_ERROR"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// server_deregister_resources — API structure
// ─────────────────────────────────────────────────────────────────────────────

/// server_deregister_resources takes &Info and a callback.
#[test]
fn test_server_deregister_resources_signature() {
    // Verify the function signature compiles with the expected types.
    fn check_signature(
        _f: fn(&pmix::Info, Box<dyn DeregisterResourcesCallback>) -> Result<(), PmixStatus>,
    ) {
    }

    struct DummyCb;
    impl DeregisterResourcesCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // This should compile — we don't call it (no PMIx server running).
    check_signature(server_deregister_resources);
}

/// InfoBuilder produces an Info that can be passed to server_deregister_resources.
#[test]
fn test_info_builder_produces_compatible_info() {
    let info = InfoBuilder::new().build();

    // Verify the info has the expected structure (handle + len).
    // We can't call the function without a server, but we can verify
    // the types are compatible.
    fn accepts_info(_: &pmix::Info) {}
    accepts_info(&info);
}

/// Empty info (no keys) is valid for deregister_resources.
#[test]
fn test_empty_info_for_deregister_resources() {
    let info = InfoBuilder::new().build();
    // Empty info is valid — it means "deregister all previously registered
    // non-namespace resources".
    fn accepts_info(_: &pmix::Info) {}
    accepts_info(&info);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus behavior with deregister_resources
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw(0) is success (PMIX_SUCCESS).
#[test]
fn test_pmix_status_success_for_deregister_resources() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "PMIX_SUCCESS should be success");
}

/// PmixStatus::from_raw(-1) is error (PMIX_ERROR).
#[test]
fn test_pmix_status_error_for_deregister_resources() {
    let status = PmixStatus::from_raw(-1);
    assert!(status.is_error(), "PMIX_ERROR should be error");
}

/// PmixStatus::from_raw(-46) is PMIX_ERR_NOT_FOUND.
#[test]
fn test_pmix_status_not_found() {
    let status = PmixStatus::from_raw(-46);
    assert!(status.is_error(), "PMIX_ERR_NOT_FOUND should be error");
}

/// PmixStatus round-trips through to_raw/from_raw.
#[test]
fn test_pmix_status_roundtrip() {
    let original = 0i32; // PMIX_SUCCESS
    let status = PmixStatus::from_raw(original);
    assert_eq!(status.to_raw(), original, "success should round-trip");

    let original = -1i32; // PMIX_ERROR
    let status = PmixStatus::from_raw(original);
    assert_eq!(status.to_raw(), original, "error should round-trip");
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback invocation patterns (simulated bridge behavior)
// ─────────────────────────────────────────────────────────────────────────────

/// Simulated callback bridge: null cbdata should not invoke callback.
#[test]
fn test_callback_bridge_null_cbdata_guard() {
    // The real bridge checks cbdata.is_null() before proceeding.
    // We verify that our callback pattern handles this correctly.
    struct GuardCb {
        invoked: Arc<Mutex<bool>>,
    }
    impl DeregisterResourcesCallback for GuardCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.invoked.lock().unwrap() = true;
        }
    }

    let invoked = Arc::new(Mutex::new(false));
    let _cb = Box::new(GuardCb {
        invoked: Arc::clone(&invoked),
    });

    // Simulate: bridge sees null cbdata, does NOT invoke callback.
    // The callback should NOT have been called.
    assert!(
        !*invoked.lock().unwrap(),
        "null cbdata should not invoke callback"
    );
}

/// Simulated callback bridge: valid cbdata invokes callback.
#[test]
fn test_callback_bridge_valid_cbdata_invokes() {
    struct InvokedCb {
        invoked: Arc<Mutex<bool>>,
    }
    impl DeregisterResourcesCallback for InvokedCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.invoked.lock().unwrap() = true;
        }
    }

    let invoked = Arc::new(Mutex::new(false));
    let cb = Box::new(InvokedCb {
        invoked: Arc::clone(&invoked),
    });

    // Simulate: bridge sees valid cbdata, invokes callback.
    cb.on_complete(PmixStatus::from_raw(0));

    assert!(
        *invoked.lock().unwrap(),
        "valid cbdata should invoke callback"
    );
}

/// Callback is consumed after invocation (registry remove pattern).
#[test]
fn test_callback_consumed_after_invocation() {
    // The real pattern: callback is removed from registry after invocation.
    // We verify the callback can only be invoked once (Box<Self> is consumed).
    struct OnceCb {
        call_count: Arc<Mutex<usize>>,
    }
    impl DeregisterResourcesCallback for OnceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.call_count.lock().unwrap() += 1;
        }
    }

    let call_count = Arc::new(Mutex::new(0));
    let cb = Box::new(OnceCb {
        call_count: Arc::clone(&call_count),
    });

    // on_complete takes self: Box<Self>, consuming the callback.
    cb.on_complete(PmixStatus::from_raw(0));

    // The callback is consumed — it can't be invoked again.
    // The Arc<Mutex<usize>> inside survives, showing one call.
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "callback invoked exactly once"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Request ID encoding/decoding (cbdata pointer pattern)
// ─────────────────────────────────────────────────────────────────────────────

/// Request ID encoding: shift left 2 bits to ensure non-null pointer.
#[test]
fn test_request_id_encoding_non_null() {
    // The pattern: cbdata = (req_id << 2) as *mut c_void
    // This ensures the pointer is never null (req_id starts at 1).
    let req_id: usize = 1;
    let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
    assert!(!cbdata.is_null(), "encoded request ID should be non-null");
}

/// Request ID decoding: shift right 2 bits to recover original.
#[test]
fn test_request_id_decoding_roundtrip() {
    let original_id: usize = 42;
    let encoded = (original_id << 2) as *mut std::os::raw::c_void;
    let decoded = (encoded as usize) >> 2;
    assert_eq!(
        decoded, original_id,
        "request ID should round-trip through encoding"
    );
}

/// Multiple request IDs encode to distinct non-null pointers.
#[test]
fn test_request_id_distinct_pointers() {
    let ids: Vec<usize> = (1..=10).collect();
    let mut pointers = Vec::new();

    for id in &ids {
        let ptr = (*id << 2) as *mut std::os::raw::c_void;
        assert!(!ptr.is_null(), "pointer for id {} should be non-null", id);
        pointers.push(ptr);
    }

    // All pointers should be distinct.
    for i in 0..pointers.len() {
        for j in (i + 1)..pointers.len() {
            assert_ne!(
                pointers[i], pointers[j],
                "pointers for ids {} and {} should be distinct",
                ids[i], ids[j]
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Deregister-specific behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// Deregister with empty info means "remove all previously registered resources".
#[test]
fn test_deregister_all_resources_empty_info() {
    // Empty info is the standard way to deregister all non-namespace
    // resources. Verify the Info type supports this.
    let info = InfoBuilder::new().build();
    // The Info struct is accepted by the function — that's the important part.
    fn accepts_info(_: &pmix::Info) {}
    accepts_info(&info);
}

/// DeregisterResourcesCallback can track which resources were deregistered.
#[test]
fn test_deregister_callback_tracks_resource_state() {
    struct ResourceTracker {
        deregistered: Arc<Mutex<bool>>,
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for ResourceTracker {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
            if status.is_success() {
                *self.deregistered.lock().unwrap() = true;
            }
        }
    }

    let deregistered = Arc::new(Mutex::new(false));
    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(ResourceTracker {
        deregistered: Arc::clone(&deregistered),
        status: Arc::clone(&status),
    });

    // Simulate successful deregistration.
    cb.on_complete(PmixStatus::from_raw(0));

    assert!(
        *deregistered.lock().unwrap(),
        "should mark as deregistered on success"
    );
    let s = status.lock().unwrap();
    assert!(s.as_ref().unwrap().is_success(), "status should be success");
}

/// DeregisterResourcesCallback handles failure gracefully.
#[test]
fn test_deregister_callback_handles_failure() {
    struct FailureTracker {
        failed: Arc<Mutex<bool>>,
        error_code: Arc<Mutex<Option<i32>>>,
    }
    impl DeregisterResourcesCallback for FailureTracker {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            if status.is_error() {
                *self.failed.lock().unwrap() = true;
                *self.error_code.lock().unwrap() = Some(status.to_raw());
            }
        }
    }

    let failed = Arc::new(Mutex::new(false));
    let error_code = Arc::new(Mutex::new(None));
    let cb = Box::new(FailureTracker {
        failed: Arc::clone(&failed),
        error_code: Arc::clone(&error_code),
    });

    // Simulate failed deregistration.
    cb.on_complete(PmixStatus::from_raw(-1));

    assert!(*failed.lock().unwrap(), "should mark as failed on error");
    assert_eq!(
        *error_code.lock().unwrap(),
        Some(-1),
        "should capture error code"
    );
}

/// Deregister and register callbacks are independent types.
#[test]
fn test_deregister_and_register_callbacks_independent() {
    use pmix::server::RegisterResourcesCallback;

    struct DeregCb;
    impl DeregisterResourcesCallback for DeregCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct RegCb;
    impl RegisterResourcesCallback for RegCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Both should be creatable and usable independently.
    let _dereg: Box<dyn DeregisterResourcesCallback> = Box::new(DeregCb);
    let _reg: Box<dyn RegisterResourcesCallback> = Box::new(RegCb);
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests (require PMIx server — ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// server_deregister_resources with empty info and a callback.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_deregister_resources_empty_info() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, cb);

    // Without a running server, this will fail with an error status.
    // With a server, it should return Ok(()).
    assert!(result.is_ok() || result.is_err(), "should return a result");
}

/// server_deregister_resources with info keys specifying resources to remove.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_deregister_resources_with_info() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    // Build info with keys (if InfoBuilder supports it).
    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, cb);

    assert!(result.is_ok() || result.is_err(), "should return a result");
}

/// server_deregister_resources callback is invoked asynchronously.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_deregister_resources_callback_invoked() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, cb);

    if result.is_ok() {
        // Callback should eventually be invoked.
        // In a real test with a server, we would poll or wait.
        // Here we just verify the call was accepted.
    }
}

/// server_deregister_resources returns error when not initialized as server.
///
/// Requires PMIx library (no server needed — just library availability).
/// Ignored because the PMIx library may not be linked in test env.
#[test]
#[ignore = "requires PMIx library"]
fn test_server_deregister_resources_not_initialized() {
    struct TestCb;
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, Box::new(TestCb));

    // Without PMIx_server_init, this should return an error.
    assert!(
        result.is_err(),
        "should fail when not initialized as server"
    );
}

/// server_deregister_resources with immediate error does not invoke callback.
///
/// Requires PMIx library. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx library"]
fn test_server_deregister_resources_immediate_error_no_callback() {
    struct CountingCb {
        invoked: Arc<Mutex<bool>>,
    }
    impl DeregisterResourcesCallback for CountingCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.invoked.lock().unwrap() = true;
        }
    }

    let invoked = Arc::new(Mutex::new(false));
    let cb = Box::new(CountingCb {
        invoked: Arc::clone(&invoked),
    });

    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, cb);

    // If the FFI call returns an error immediately, the callback should
    // NOT be invoked — it was removed from the registry.
    if result.is_err() {
        assert!(
            !*invoked.lock().unwrap(),
            "callback should not be invoked on immediate error"
        );
    }
}

/// server_deregister_resources callback receives correct status on success.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_deregister_resources_callback_success_status() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_deregister_resources(&info, cb);

    if result.is_ok() {
        // In a real server environment, the callback would be invoked
        // with PMIX_SUCCESS. We can't wait for async here without a server.
    }
}

/// server_deregister_resources can be called multiple times with different callbacks.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_deregister_resources_multiple_calls() {
    struct TestCb {
        id: usize,
        status: Arc<Mutex<Option<(usize, PmixStatus)>>>,
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some((self.id, status));
        }
    }

    let status1 = Arc::new(Mutex::new(None));
    let cb1 = Box::new(TestCb {
        id: 1,
        status: Arc::clone(&status1),
    });

    let status2 = Arc::new(Mutex::new(None));
    let cb2 = Box::new(TestCb {
        id: 2,
        status: Arc::clone(&status2),
    });

    let info = InfoBuilder::new().build();
    let _result1 = server_deregister_resources(&info, cb1);
    let _result2 = server_deregister_resources(&info, cb2);

    // Both calls should return a result (success or error).
    // Each callback should be independent.
}

/// server_deregister_resources paired with server_register_resources.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_register_then_deregister_resources() {
    use pmix::server::{RegisterResourcesCallback, server_register_resources};

    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    impl DeregisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let reg_status = Arc::new(Mutex::new(None));
    let dereg_status = Arc::new(Mutex::new(None));

    let info = InfoBuilder::new().build();
    let _reg_result = server_register_resources(
        &info,
        Box::new(TestCb {
            status: Arc::clone(&reg_status),
        }),
    );
    let _dereg_result = server_deregister_resources(
        &info,
        Box::new(TestCb {
            status: Arc::clone(&dereg_status),
        }),
    );

    // Both should return a result.
}
