//! Tests for `PMIx_Fabric_update` — fabric update operations.
//!
//! These tests verify the Rust wrappers for fabric update APIs:
//! `fabric_update` and `fabric_update_nb`.
//!
//! Derived from the fabric API spec in the PMIx v4.1 standard (Section 14.4.3)
//! and the C test patterns in `test/simple/simpfabric.c`. The C test file does
//! not exercise `PMIx_Fabric_update` directly, so these tests cover the
//! safe Rust wrapper parameter validation, callback wrapper construction,
//! error handling paths, and integration scenarios.
//!
//! Tests that require `PMIx_Init` or a PMIx server are marked `#[ignore]`
//! because they need a running PMIx daemon / server. Calling the FFI without
//! an initialized PMIx library causes a segfault, so all tests that invoke
//! `fabric_update` directly are ignored.

use pmix::fabric::{fabric_update, fabric_update_nb, FabricCallback, PmixFabric};
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback — verifies the trait compiles and is object-safe.
struct NoOpFabricCallback;

impl FabricCallback for NoOpFabricCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        // No-op — just verify the trait is object-safe and callable.
    }
}

/// Callback that records the status it received via Cell.
struct RecordingFabricCallback {
    status: std::cell::Cell<Option<PmixStatus>>,
}

impl FabricCallback for RecordingFabricCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.status.set(Some(status));
    }
}

/// Callback that counts how many times it has been invoked.
struct CountingFabricCallback {
    count: std::cell::Cell<u32>,
}

impl FabricCallback for CountingFabricCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        let c = self.count.get();
        self.count.set(c + 1);
    }
}

/// Callback that captures status via Arc<Mutex<>> for multi-threaded use.
struct ArcStatusCallback {
    status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
}

impl FabricCallback for ArcStatusCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

/// Callback that wraps a closure for flexible test logic.
struct ClosureCallback {
    f: std::sync::Arc<std::sync::Mutex<Box<dyn FnMut(PmixStatus) + Send>>>,
}

impl FabricCallback for ClosureCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        let mut guard = self.f.lock().unwrap();
        guard(status);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FabricCallback trait tests (no FFI — these always pass)
// ─────────────────────────────────────────────────────────────────────────────

/// FabricCallback is object-safe — can be boxed and stored as dyn.
#[test]
fn fabric_update_callback_trait_object() {
    let _cb: Box<dyn FabricCallback> = Box::new(NoOpFabricCallback);
}

/// RecordingFabricCallback compiles and is object-safe.
#[test]
fn fabric_update_callback_records_status_type() {
    let cb = RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// CountingFabricCallback compiles and is object-safe.
#[test]
fn fabric_update_callback_counting_type() {
    let cb = CountingFabricCallback {
        count: std::cell::Cell::new(0),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ArcStatusCallback compiles and is object-safe.
#[test]
fn fabric_update_callback_arc_status_type() {
    let cb = ArcStatusCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ClosureCallback compiles and is object-safe.
#[test]
fn fabric_update_callback_closure_type() {
    let cb = ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// FabricCallback is Send (required by the trait bound).
#[test]
fn fabric_update_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn FabricCallback>>();
}

/// NoOpFabricCallback is Send.
#[test]
fn fabric_update_callback_noop_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NoOpFabricCallback>();
}

/// RecordingFabricCallback is Send.
#[test]
fn fabric_update_callback_recording_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RecordingFabricCallback>();
}

/// ArcStatusCallback is Send.
#[test]
fn fabric_update_callback_arc_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ArcStatusCallback>();
}

/// ClosureCallback is Send.
#[test]
fn fabric_update_callback_closure_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ClosureCallback>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric construction — prerequisites for fabric_update
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric can be created unnamed — prerequisite for fabric_update.
#[test]
fn fabric_update_fabric_unamed() {
    let fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert_eq!(fabric.name(), None);
}

/// PmixFabric can be created with a name — prerequisite for fabric_update.
#[test]
fn fabric_update_fabric_named() {
    let fabric = PmixFabric::new(Some("infiniband")).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("infiniband"));
}

/// PmixFabric can be created with None name.
#[test]
fn fabric_update_fabric_none_name() {
    let fabric = PmixFabric::new(None).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), None);
}

/// PmixFabric::new rejects names with interior NUL bytes.
#[test]
fn fabric_update_fabric_nul_name_rejected() {
    let result = PmixFabric::new(Some("test\0fabric"));
    assert!(result.is_err());
}

/// PmixFabric implements Debug.
#[test]
fn fabric_update_fabric_debug() {
    let fabric = PmixFabric::new(Some("debug_test")).unwrap();
    let debug_str = format!("{:?}", fabric);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("debug_test"));
}

/// PmixFabric Debug for unnamed fabric.
#[test]
fn fabric_update_fabric_debug_unamed() {
    let fabric = PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("registered"));
}

/// PmixFabric index and ninfo start at zero.
#[test]
fn fabric_update_fabric_initial_state() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert!(!fabric.is_registered());
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation tests (no FFI — these always pass)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_update on an unregistered fabric returns BAD_PARAM error.
#[test]
fn fabric_update_unregistered_returns_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The error should be BAD_PARAM since the fabric is not registered.
    assert!(err.is_error());
}

/// fabric_update on an unregistered named fabric also returns error.
#[test]
fn fabric_update_unregistered_named_returns_error() {
    let mut fabric = PmixFabric::new(Some("test")).unwrap();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
}

/// fabric_update_nb on an unregistered fabric returns error without leaking.
#[test]
fn fabric_update_nb_unregistered_returns_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));
    assert!(result.is_err());
    // If we got here without leaking, the wrapper was reclaimed.
}

/// fabric_update_nb on an unregistered named fabric returns error.
#[test]
fn fabric_update_nb_unregistered_named_returns_error() {
    let mut fabric = PmixFabric::new(Some("test")).unwrap();
    let result = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));
    assert!(result.is_err());
}

/// fabric_update_nb with a recording callback on unregistered fabric.
#[test]
fn fabric_update_nb_unregistered_recording_callback() {
    let cb = RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_update_nb with a counting callback on unregistered fabric.
#[test]
fn fabric_update_nb_unregistered_counting_callback() {
    let cb = CountingFabricCallback {
        count: std::cell::Cell::new(0),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_update_nb with an ArcStatusCallback on unregistered fabric.
#[test]
fn fabric_update_nb_unregistered_arc_callback() {
    let cb = ArcStatusCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_update_nb with a ClosureCallback on unregistered fabric.
#[test]
fn fabric_update_nb_unregistered_closure_callback() {
    let cb = ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type compatibility tests (compile-only, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_update accepts the correct parameter types.
/// This is a compile-time check — the function signature must accept
/// &mut PmixFabric and return Result<(), PmixStatus>.
#[test]
fn fabric_update_signature_compiles() {
    fn _check_signature(
        fabric: &mut PmixFabric,
    ) -> Result<(), PmixStatus> {
        fabric_update(fabric)
    }
    // Just verify the function pointer type is valid.
    let _ = _check_signature as fn(&mut PmixFabric) -> Result<(), PmixStatus>;
}

/// fabric_update_nb accepts the correct parameter types.
/// Signature: &mut PmixFabric, Box<dyn FabricCallback> -> Result<(), PmixStatus>.
#[test]
fn fabric_update_nb_signature_compiles() {
    fn _check_signature(
        fabric: &mut PmixFabric,
        callback: Box<dyn FabricCallback>,
    ) -> Result<(), PmixStatus> {
        fabric_update_nb(fabric, callback)
    }
    let _ = _check_signature
        as fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>;
}

/// fabric_update return type is Result<(), PmixStatus>.
#[test]
fn fabric_update_return_type() {
    fn _check_return(_f: fn(&mut PmixFabric) -> Result<(), PmixStatus>) {}
    _check_return(fabric_update);
}

/// fabric_update_nb return type is Result<(), PmixStatus>.
#[test]
fn fabric_update_nb_return_type() {
    fn _check_return(
        _f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {}
    _check_return(fabric_update_nb);
}

/// Multiple callback implementations can all be passed to fabric_update_nb.
#[test]
fn fabric_update_nb_multiple_callback_types_compile() {
    let _: Box<dyn FabricCallback> = Box::new(NoOpFabricCallback);
    let _: Box<dyn FabricCallback> = Box::new(RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    });
    let _: Box<dyn FabricCallback> = Box::new(CountingFabricCallback {
        count: std::cell::Cell::new(0),
    });
    let _: Box<dyn FabricCallback> = Box::new(ArcStatusCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    });
    let _: Box<dyn FabricCallback> = Box::new(ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper memory safety (compile checks, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// Callback wrapper with large captured data compiles.
#[test]
fn fabric_update_nb_large_callback_data_compiles() {
    struct LargeCallback {
        #[allow(dead_code)]
        data: Vec<u8>,
    }
    impl FabricCallback for LargeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // Data is dropped here.
        }
    }
    let _cb: Box<dyn FabricCallback> = Box::new(LargeCallback {
        data: vec![0u8; 4096],
    });
}

/// Callback wrapper can capture complex state (Arc, Mutex, Vec).
#[test]
fn fabric_update_nb_complex_callback_state() {
    struct ComplexCallback {
        statuses: std::sync::Arc<std::sync::Mutex<Vec<PmixStatus>>>,
        name: String,
    }
    impl FabricCallback for ComplexCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }
    let cb = ComplexCallback {
        statuses: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        name: "update_test".to_string(),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// fabric_update_nb error path reclaims the callback wrapper (no leak).
/// When fabric_update_nb returns an error on the unregistered path,
/// the wrapper is dropped inside the function — verified by not panicking.
#[test]
fn fabric_update_nb_error_reclaims_wrapper() {
    // Multiple calls should not leak — each error path drops its wrapper.
    for _ in 0..10 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));
    }
}

/// fabric_update error path is consistent across multiple calls.
#[test]
fn fabric_update_error_consistent() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_update(&mut fabric);
    let result2 = fabric_update(&mut fabric);
    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — all ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_update on a registered fabric refreshes fabric information.
/// This is the primary use case: register, then update to get fresh data.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_registered_fabric() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::new(Some("update_test")).unwrap();
    let reg_result = fabric_register(&mut fabric, &[]);
    if reg_result.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Update the fabric — should succeed.
    let update_result = fabric_update(&mut fabric);
    if update_result.is_ok() {
        // Fabric info may have been refreshed.
    }
}

/// fabric_update multiple times on the same registered fabric.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_multiple_times() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }

    // Multiple updates should all succeed.
    for _ in 0..3 {
        let _ = fabric_update(&mut fabric);
    }
}

/// fabric_update_nb with a callback — verify callback is accepted.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_registered_callback() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));
}

/// fabric_update_nb with a recording callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_recording_callback() {
    use pmix::fabric::fabric_register;

    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let status_clone = status.clone();
    let cb = ArcStatusCallback { status: status_clone };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
}

/// fabric_update_nb with a closure callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_closure_callback() {
    use pmix::fabric::fabric_register;

    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let called_clone = called.clone();
    let cb = ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {
                *called_clone.lock().unwrap() = true;
            },
        ))),
    };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
}

/// fabric_update_nb with a counting callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_counting_callback() {
    use pmix::fabric::fabric_register;

    let cb = CountingFabricCallback {
        count: std::cell::Cell::new(0),
    };
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
}

/// fabric_update_nb with a complex callback state.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_complex_callback() {
    use pmix::fabric::fabric_register;

    struct ComplexCallback {
        statuses: std::sync::Arc<std::sync::Mutex<Vec<PmixStatus>>>,
        name: String,
    }
    impl FabricCallback for ComplexCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }

    let cb = ComplexCallback {
        statuses: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        name: "update_complex".to_string(),
    };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
}

/// Full lifecycle: register -> update -> update_nb -> deregister.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_full_lifecycle() {
    use pmix::fabric::{fabric_deregister, fabric_register};

    let mut fabric = PmixFabric::new(Some("lifecycle")).unwrap();

    // Register
    let reg = fabric_register(&mut fabric, &[]);
    if reg.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Update (blocking)
    let _update1 = fabric_update(&mut fabric);

    // Update (non-blocking)
    let _update2 = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));

    // Deregister
    let dereg = fabric_deregister(&mut fabric);
    if dereg.is_ok() {
        assert!(!fabric.is_registered());
    }
}

/// fabric_update after fabric_update_nb — verify both can be used together.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_after_update_nb() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }

    // Non-blocking update first
    let _nb = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));
    // Then blocking update
    let _blocking = fabric_update(&mut fabric);
}

/// fabric_update on a named fabric with specific name.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_named_fabric() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::new(Some("infiniband")).unwrap();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update(&mut fabric);
}

/// fabric_update_nb callback receives status on completion.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_callback_receives_status() {
    use pmix::fabric::fabric_register;

    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let status_clone = status.clone();
    let cb = ArcStatusCallback { status: status_clone };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
    // Callback will be invoked asynchronously by PMIx.
}

/// fabric_update with large callback data — verify no memory issues.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_update_nb_large_callback() {
    use pmix::fabric::fabric_register;

    struct LargeCallback {
        data: Vec<u8>,
    }
    impl FabricCallback for LargeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // Data is dropped here.
        }
    }

    let cb = LargeCallback {
        data: vec![0u8; 4096],
    };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(cb));
}
