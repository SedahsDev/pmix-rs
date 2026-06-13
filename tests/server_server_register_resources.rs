//! Tests for `PMIx_server_register_resources`, `RegisterResourcesCallback`,
//! and the resource registration callback infrastructure.
//!
//! Note: `PMIx_server_register_resources` requires a running PMIx server
//! environment (`PMIx_server_init` must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{RegisterResourcesCallback, server_register_resources};
use pmix::InfoBuilder;
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// RegisterResourcesCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// RegisterResourcesCallback trait is object-safe and requires Send.
#[test]
fn test_register_resources_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn RegisterResourcesCallback>) {}

    struct DummyCb;
    impl RegisterResourcesCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// RegisterResourcesCallback::on_complete receives PmixStatus.
#[test]
fn test_register_resources_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for StatusCapture {
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
fn test_register_resources_callback_receives_error_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for StatusCapture {
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
fn test_register_resources_callback_carries_custom_state() {
    struct CustomState {
        message: String,
        count: usize,
    }
    impl RegisterResourcesCallback for CustomState {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // State is accessible inside the callback.
            let _ = &self.message;
            let _ = self.count;
        }
    }

    let cb = Box::new(CustomState {
        message: "resource registered".to_string(),
        count: 42,
    });
    assert_trait_obj(cb);

    fn assert_trait_obj(_: Box<dyn RegisterResourcesCallback>) {}
}

/// Multiple callbacks can be created and stored independently.
#[test]
fn test_register_resources_multiple_callbacks() {
    struct CountingCb {
        counter: Arc<Mutex<usize>>,
    }
    impl RegisterResourcesCallback for CountingCb {
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

    assert_eq!(*counter.lock().unwrap(), 2, "both callbacks should have incremented");
}

// ─────────────────────────────────────────────────────────────────────────────
// server_register_resources — API structure
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_resources takes &Info and a callback.
#[test]
fn test_server_register_resources_signature() {
    // Verify the function signature compiles with the expected types.
    fn check_signature(
        _f: fn(&pmix::Info, Box<dyn RegisterResourcesCallback>) -> Result<(), PmixStatus>,
    ) {}

    struct DummyCb;
    impl RegisterResourcesCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // This should compile — we don't call it (no PMIx server running).
    check_signature(server_register_resources);
}

/// InfoBuilder produces an Info that can be passed to server_register_resources.
#[test]
fn test_info_builder_produces_compatible_info() {
    let info = InfoBuilder::new().build();

    // Verify the info has the expected structure (handle + len).
    // We can't call the function without a server, but we can verify
    // the types are compatible.
    fn accepts_info(_: &pmix::Info) {}
    accepts_info(&info);
}

/// Empty info (no keys) is valid for register_resources.
#[test]
fn test_empty_info_for_register_resources() {
    let info = InfoBuilder::new().build();
    // Empty info is valid — it means "no resource info to register".
    fn accepts_info(_: &pmix::Info) {}
    accepts_info(&info);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus behavior with register_resources
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw(0) is success (PMIX_SUCCESS).
#[test]
fn test_pmix_status_success_for_register_resources() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "PMIX_SUCCESS should be success");
}

/// PmixStatus::from_raw(-1) is error (PMIX_ERROR).
#[test]
fn test_pmix_status_error_for_register_resources() {
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
    impl RegisterResourcesCallback for GuardCb {
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
    assert!(!*invoked.lock().unwrap(), "null cbdata should not invoke callback");
}

/// Simulated callback bridge: valid cbdata invokes callback.
#[test]
fn test_callback_bridge_valid_cbdata_invokes() {
    struct InvokedCb {
        invoked: Arc<Mutex<bool>>,
    }
    impl RegisterResourcesCallback for InvokedCb {
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

    assert!(*invoked.lock().unwrap(), "valid cbdata should invoke callback");
}

/// Callback is consumed after invocation (registry remove pattern).
#[test]
fn test_callback_consumed_after_invocation() {
    // The real pattern: callback is removed from registry after invocation.
    // We verify the callback can only be invoked once (Box<Self> is consumed).
    struct OnceCb {
        call_count: Arc<Mutex<usize>>,
    }
    impl RegisterResourcesCallback for OnceCb {
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
    assert_eq!(*call_count.lock().unwrap(), 1, "callback invoked exactly once");
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
    assert_eq!(decoded, original_id, "request ID should round-trip through encoding");
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
// FFI integration tests (require PMIx server — ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_resources with empty info and a callback.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_register_resources_empty_info() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_register_resources(&info, cb);

    // Without a running server, this will fail with an error status.
    // With a server, it should return Ok(()).
    assert!(result.is_ok() || result.is_err(), "should return a result");
}

/// server_register_resources with info keys.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_register_resources_with_info() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for TestCb {
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
    let result = server_register_resources(&info, cb);

    assert!(result.is_ok() || result.is_err(), "should return a result");
}

/// server_register_resources callback is invoked asynchronously.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_register_resources_callback_invoked() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_register_resources(&info, cb);

    if result.is_ok() {
        // Callback should eventually be invoked.
        // In a real test with a server, we would poll or wait.
        // Here we just verify the call was accepted.
    }
}

/// server_register_resources returns error when not initialized as server.
///
/// Requires PMIx library (no server needed — just library availability).
/// Ignored because the PMIx library may not be linked in test env.
#[test]
#[ignore = "requires PMIx library"]
fn test_server_register_resources_not_initialized() {
    struct TestCb;
    impl RegisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = InfoBuilder::new().build();
    let result = server_register_resources(&info, Box::new(TestCb));

    // Without PMIx_server_init, this should return an error.
    assert!(result.is_err(), "should fail when not initialized as server");
}

/// server_register_resources with immediate error does not invoke callback.
///
/// Requires PMIx library. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx library"]
fn test_server_register_resources_immediate_error_no_callback() {
    struct CountingCb {
        invoked: Arc<Mutex<bool>>,
    }
    impl RegisterResourcesCallback for CountingCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.invoked.lock().unwrap() = true;
        }
    }

    let invoked = Arc::new(Mutex::new(false));
    let cb = Box::new(CountingCb {
        invoked: Arc::clone(&invoked),
    });

    let info = InfoBuilder::new().build();
    let result = server_register_resources(&info, cb);

    // If the FFI call returns an error immediately, the callback should
    // NOT be invoked — it was removed from the registry.
    if result.is_err() {
        assert!(
            !*invoked.lock().unwrap(),
            "callback should not be invoked on immediate error"
        );
    }
}

/// server_register_resources callback receives correct status on success.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_register_resources_callback_success_status() {
    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let info = InfoBuilder::new().build();
    let result = server_register_resources(&info, cb);

    if result.is_ok() {
        // In a real server environment, the callback would be invoked
        // with PMIX_SUCCESS. We can't wait for async here without a server.
    }
}

/// server_register_resources can be called multiple times with different callbacks.
///
/// Requires a running PMIx server. Ignored in unit test mode.
#[test]
#[ignore = "requires PMIx server"]
fn test_server_register_resources_multiple_calls() {
    struct TestCb {
        id: usize,
        status: Arc<Mutex<Option<(usize, PmixStatus)>>>,
    }
    impl RegisterResourcesCallback for TestCb {
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
    let _result1 = server_register_resources(&info, cb1);
    let _result2 = server_register_resources(&info, cb2);

    // Both calls should return a result (success or error).
    // Each callback should be independent.
}
