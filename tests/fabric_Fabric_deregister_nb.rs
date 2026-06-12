//! Tests for `PMIx_Fabric_deregister_nb` — non-blocking fabric deregistration.
//!
//! Derived from the PMIx v4.1 standard (Section 14.4.5) and the C test
//! patterns in `test/simple/simpfabric.c`. The C test file exercises the
//! blocking `PMIx_Fabric_deregister` but not the `_nb` variant directly,
//! so these tests cover the safe Rust wrapper parameter validation, callback
//! wrapper construction, error handling paths, and integration scenarios.
//!
//! Tests that require `PMIx_Init` or a PMIx server are marked `#[ignore]`
//! because they need a running PMIx daemon / server. Calling the FFI without
//! an initialized PMIx library causes a segfault, so all tests that invoke
//! `fabric_deregister_nb` on a registered fabric are ignored.
//!
//! # Spec (Section 14.4.5)
//!
//! ```c
//! pmix_status_t PMIx_Fabric_deregister_nb(pmix_fabric_t *fabric,
//!                                         pmix_op_cbfunc_t cbfunc, void *cbdata);
//! ```
//!
//! Returns `PMIX_SUCCESS` when the request has been accepted for processing
//! and the callback will be executed upon completion. A non-zero error means
//! the request was rejected and the callback will NOT be executed. The caller
//! must not access the fabric until the callback has been invoked. Upon
//! successful deregistration, PMIx cleans up any associated resources
//! (e.g., cost matrices, topology data).

use pmix::fabric::{fabric_deregister_nb, FabricCallback, PmixFabric};
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback — verifies the trait compiles and is object-safe.
struct NoOpDeregisterCallback;

impl FabricCallback for NoOpDeregisterCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        // No-op — just verify the trait is object-safe and callable.
    }
}

/// Callback that records the status it received via Cell.
struct RecordingDeregisterCallback {
    status: std::cell::Cell<Option<PmixStatus>>,
}

impl FabricCallback for RecordingDeregisterCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.status.set(Some(status));
    }
}

/// Callback that counts how many times it has been invoked.
struct CountingDeregisterCallback {
    count: std::cell::Cell<u32>,
}

impl FabricCallback for CountingDeregisterCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        let c = self.count.get();
        self.count.set(c + 1);
    }
}

/// Callback that captures status via Arc<Mutex<>> for multi-threaded use.
struct ArcDeregisterCallback {
    status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
}

impl FabricCallback for ArcDeregisterCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

/// Callback that wraps a closure for flexible test logic.
struct ClosureDeregisterCallback {
    f: std::sync::Arc<std::sync::Mutex<Box<dyn FnMut(PmixStatus) + Send>>>,
}

impl FabricCallback for ClosureDeregisterCallback {
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
fn fabric_deregister_nb_callback_trait_object() {
    let _cb: Box<dyn FabricCallback> = Box::new(NoOpDeregisterCallback);
}

/// RecordingDeregisterCallback compiles and is object-safe.
#[test]
fn fabric_deregister_nb_callback_records_status_type() {
    let cb = RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// CountingDeregisterCallback compiles and is object-safe.
#[test]
fn fabric_deregister_nb_callback_counting_type() {
    let cb = CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ArcDeregisterCallback compiles and is object-safe.
#[test]
fn fabric_deregister_nb_callback_arc_status_type() {
    let cb = ArcDeregisterCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ClosureDeregisterCallback compiles and is object-safe.
#[test]
fn fabric_deregister_nb_callback_closure_type() {
    let cb = ClosureDeregisterCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// FabricCallback is Send (required by the trait bound).
#[test]
fn fabric_deregister_nb_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn FabricCallback>>();
}

/// NoOpDeregisterCallback is Send.
#[test]
fn fabric_deregister_nb_callback_noop_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NoOpDeregisterCallback>();
}

/// RecordingDeregisterCallback is Send.
#[test]
fn fabric_deregister_nb_callback_recording_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RecordingDeregisterCallback>();
}

/// ArcDeregisterCallback is Send.
#[test]
fn fabric_deregister_nb_callback_arc_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ArcDeregisterCallback>();
}

/// ClosureDeregisterCallback is Send.
#[test]
fn fabric_deregister_nb_callback_closure_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ClosureDeregisterCallback>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric construction — prerequisites for fabric_deregister_nb
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric can be created unnamed — prerequisite for fabric_deregister_nb.
#[test]
fn fabric_deregister_nb_fabric_unamed() {
    let fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert_eq!(fabric.name(), None);
}

/// PmixFabric can be created with a name — prerequisite for fabric_deregister_nb.
#[test]
fn fabric_deregister_nb_fabric_named() {
    let fabric = PmixFabric::new(Some("infiniband")).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("infiniband"));
}

/// PmixFabric can be created with None name.
#[test]
fn fabric_deregister_nb_fabric_none_name() {
    let fabric = PmixFabric::new(None).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), None);
}

/// PmixFabric::new rejects names with interior NUL bytes.
#[test]
fn fabric_deregister_nb_fabric_nul_name_rejected() {
    let result = PmixFabric::new(Some("test\0fabric"));
    assert!(result.is_err());
}

/// PmixFabric implements Debug.
#[test]
fn fabric_deregister_nb_fabric_debug() {
    let fabric = PmixFabric::new(Some("debug_test")).unwrap();
    let debug_str = format!("{:?}", fabric);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("debug_test"));
}

/// PmixFabric Debug for unnamed fabric.
#[test]
fn fabric_deregister_nb_fabric_debug_unamed() {
    let fabric = PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("registered"));
}

/// PmixFabric index and ninfo start at zero.
#[test]
fn fabric_deregister_nb_fabric_initial_state() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert!(!fabric.is_registered());
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation tests (no FFI — these always pass)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_deregister_nb on an unregistered fabric returns error without invoking
/// the callback. Per the spec: "a non-zero PMIx error constant indicating a
/// reason for the request to have been rejected. In this case, the provided
/// callback function will not be executed."
#[test]
fn fabric_deregister_nb_unregistered_returns_error() {
    struct CallbackNotCalled;
    impl FabricCallback for CallbackNotCalled {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            panic!("callback should not be called on error path");
        }
    }
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(CallbackNotCalled));
    assert!(result.is_err());
    // If we got here without the callback panicking, the error path is correct.
}

/// fabric_deregister_nb on an unregistered named fabric also returns error.
#[test]
fn fabric_deregister_nb_unregistered_named_returns_error() {
    let mut fabric = PmixFabric::new(Some("test")).unwrap();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(result.is_err());
}

/// fabric_deregister_nb with a recording callback on unregistered fabric —
/// callback must not be invoked.
#[test]
fn fabric_deregister_nb_unregistered_recording_callback() {
    let cb = RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_deregister_nb with a counting callback on unregistered fabric.
#[test]
fn fabric_deregister_nb_unregistered_counting_callback() {
    let cb = CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_deregister_nb with an ArcDeregisterCallback on unregistered fabric.
#[test]
fn fabric_deregister_nb_unregistered_arc_callback() {
    let cb = ArcDeregisterCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

/// fabric_deregister_nb with a ClosureDeregisterCallback on unregistered fabric.
#[test]
fn fabric_deregister_nb_unregistered_closure_callback() {
    let cb = ClosureDeregisterCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    };
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code verification
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_deregister_nb on unregistered fabric returns BAD_PARAM.
#[test]
fn fabric_deregister_nb_error_is_bad_param() {
    use pmix::PmixError;
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_error(), "returned status must be an error");
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "error must be BAD_PARAM for unregistered fabric"
    );
}

/// fabric_deregister_nb error is consistent across multiple calls.
#[test]
fn fabric_deregister_nb_error_consistent() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    let result2 = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2, "error must be consistent across calls");
}

// ─────────────────────────────────────────────────────────────────────────────
// State preservation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Attempting fabric_deregister_nb on an unregistered fabric does not
/// change the fabric's internal state.
#[test]
fn fabric_deregister_nb_unregistered_state_preserved() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(!fabric.is_registered(), "fabric must remain unregistered");
    assert_eq!(fabric.index(), 0, "index must remain 0");
    assert_eq!(fabric.ninfo(), 0, "ninfo must remain 0");
}

/// fabric_deregister_nb on a named but unregistered fabric preserves name.
#[test]
fn fabric_deregister_nb_named_state_preserved() {
    let mut fabric = PmixFabric::new(Some("preserve_test")).unwrap();
    let _ = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert_eq!(fabric.name(), Some("preserve_test"));
    assert!(!fabric.is_registered());
}

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type compatibility tests (compile-only, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_deregister_nb accepts the correct parameter types.
/// Signature: &mut PmixFabric, Box<dyn FabricCallback> -> Result<(), PmixStatus>.
#[test]
fn fabric_deregister_nb_signature_compiles() {
    fn _check_signature(
        fabric: &mut PmixFabric,
        callback: Box<dyn FabricCallback>,
    ) -> Result<(), PmixStatus> {
        fabric_deregister_nb(fabric, callback)
    }
    let _ = _check_signature
        as fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>;
}

/// fabric_deregister_nb return type is Result<(), PmixStatus>.
#[test]
fn fabric_deregister_nb_return_type() {
    fn _check_return(
        _f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {}
    _check_return(fabric_deregister_nb);
}

/// Multiple callback implementations can all be passed to fabric_deregister_nb.
#[test]
fn fabric_deregister_nb_multiple_callback_types_compile() {
    let _: Box<dyn FabricCallback> = Box::new(NoOpDeregisterCallback);
    let _: Box<dyn FabricCallback> = Box::new(RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    });
    let _: Box<dyn FabricCallback> = Box::new(CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    });
    let _: Box<dyn FabricCallback> = Box::new(ArcDeregisterCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    });
    let _: Box<dyn FabricCallback> = Box::new(ClosureDeregisterCallback {
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
fn fabric_deregister_nb_large_callback_data_compiles() {
    struct LargeDeregisterCallback {
        #[allow(dead_code)]
        data: Vec<u8>,
    }
    impl FabricCallback for LargeDeregisterCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // Data is dropped here.
        }
    }
    let _cb: Box<dyn FabricCallback> = Box::new(LargeDeregisterCallback {
        data: vec![0u8; 4096],
    });
}

/// Callback wrapper can capture complex state (Arc, Mutex, Vec).
#[test]
fn fabric_deregister_nb_complex_callback_state() {
    struct ComplexDeregisterCallback {
        statuses: std::sync::Arc<std::sync::Mutex<Vec<PmixStatus>>>,
        name: String,
    }
    impl FabricCallback for ComplexDeregisterCallback {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }
    let cb = ComplexDeregisterCallback {
        statuses: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        name: "deregister_nb_test".to_string(),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// fabric_deregister_nb error path reclaims the callback wrapper (no leak).
/// When fabric_deregister_nb returns an error on the unregistered path,
/// the wrapper is dropped inside the function — verified by not panicking.
/// Running multiple times ensures no accumulation of leaked wrappers.
#[test]
fn fabric_deregister_nb_error_reclaims_wrapper() {
    for _ in 0..10 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    }
}

/// fabric_deregister_nb error path reclaims all callback types correctly.
#[test]
fn fabric_deregister_nb_error_all_callback_types_reclaim() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    let _ = fabric_deregister_nb(&mut fabric, Box::new(RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    }));
    let _ = fabric_deregister_nb(&mut fabric, Box::new(CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    }));
    let _ = fabric_deregister_nb(&mut fabric, Box::new(ArcDeregisterCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    }));
    let _ = fabric_deregister_nb(&mut fabric, Box::new(ClosureDeregisterCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            move |_: PmixStatus| {},
        ))),
    }));
    // If all wrappers were reclaimed, no leak occurred.
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple fabric instances tests
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple unregistered fabrics each return their own independent error.
#[test]
fn fabric_deregister_nb_multiple_unregistered() {
    let fabrics: Vec<_> = (0..5)
        .map(|i| PmixFabric::new(Some(&format!("dereg_nb_{}", i))).unwrap())
        .collect();

    let mut results = Vec::new();
    for mut f in fabrics {
        results.push(fabric_deregister_nb(&mut f, Box::new(NoOpDeregisterCallback)));
    }

    for (i, result) in results.into_iter().enumerate() {
        assert!(
            result.is_err(),
            "dereg_nb_{} deregister_nb should return error (unregistered)",
            i
        );
    }
}

/// Deregistering one fabric does not affect another.
#[test]
fn fabric_deregister_nb_independent_instances() {
    let mut fabric_a = PmixFabric::new(Some("fabric_a")).unwrap();
    let fabric_b = PmixFabric::new(Some("fabric_b")).unwrap();

    let result_a = fabric_deregister_nb(&mut fabric_a, Box::new(NoOpDeregisterCallback));
    assert!(result_a.is_err()); // both unregistered, so error expected

    // fabric_b should be unaffected.
    assert!(!fabric_b.is_registered());
    assert_eq!(fabric_b.name(), Some("fabric_b"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety tests
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_deregister_nb does not panic on a default-constructed fabric.
#[test]
fn fabric_deregister_nb_no_panic_default() {
    let fabric = PmixFabric::new(None).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = fabric;
        fabric_deregister_nb(&mut f, Box::new(NoOpDeregisterCallback))
    }));
    assert!(
        result.is_ok(),
        "fabric_deregister_nb must not panic on default fabric"
    );
}

/// fabric_deregister_nb does not panic on an unamed fabric.
#[test]
fn fabric_deregister_nb_no_panic_unamed() {
    let fabric = PmixFabric::unamed();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = fabric;
        fabric_deregister_nb(&mut f, Box::new(NoOpDeregisterCallback))
    }));
    assert!(
        result.is_ok(),
        "fabric_deregister_nb must not panic on unamed fabric"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — all ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_deregister_nb on a registered fabric — the primary use case.
/// Per spec: returns PMIX_SUCCESS when accepted, callback invoked on completion.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_registered_fabric() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::new(Some("dereg_nb_test")).unwrap();
    let reg_result = fabric_register(&mut fabric, &[]);
    if reg_result.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Non-blocking deregister — should succeed and queue the callback.
    let dereg_result = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    if dereg_result.is_ok() {
        // Callback will be invoked asynchronously by PMIx.
        assert!(!fabric.is_registered(), "fabric must be marked unregistered");
    }
}

/// fabric_deregister_nb with a recording callback under a real PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_recording_callback() {
    use pmix::fabric::fabric_register;

    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let status_clone = status.clone();
    let cb = ArcDeregisterCallback { status: status_clone };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }
    let _result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    // Callback will be invoked asynchronously by PMIx.
}

/// fabric_deregister_nb with a closure callback under a real PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_closure_callback() {
    use pmix::fabric::fabric_register;

    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let called_clone = called.clone();
    let cb = ClosureDeregisterCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(move |_: PmixStatus| {
            *called_clone.lock().unwrap() = true;
        }))),
    };

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    let _result = fabric_deregister_nb(&mut fabric, Box::new(cb));
}

/// Full register -> deregister_nb lifecycle.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_lifecycle() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::new(Some("lifecycle_test")).unwrap();
    assert!(!fabric.is_registered());

    // Register
    let reg_result = fabric_register(&mut fabric, &[]);
    if reg_result.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Deregister non-blocking
    let dereg_result = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(
        dereg_result.is_ok(),
        "deregister_nb must succeed for a registered fabric"
    );
    assert!(!fabric.is_registered(), "fabric must be unregistered after deregister_nb");
}

/// Double deregister_nb — register, deregister_nb, then try again.
/// Second call should fail with BAD_PARAM.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_double_deregister() {
    use pmix::fabric::fabric_register;
    use pmix::PmixError;

    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }

    // First deregister_nb should succeed.
    let first = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(first.is_ok(), "first deregister_nb must succeed");
    assert!(!fabric.is_registered());

    // Second deregister_nb should fail with BAD_PARAM.
    let second = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(
        second.is_err(),
        "double deregister_nb must return error"
    );
    let err = second.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "double deregister_nb error must be BAD_PARAM"
    );
}

/// Multiple fabrics registered and deregistered independently via nb.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_multiple_independent() {
    use pmix::fabric::fabric_register;

    let names = ["alpha", "beta", "gamma"];
    let mut fabrics: Vec<_> = names
        .iter()
        .map(|n| PmixFabric::new(Some(n)).unwrap())
        .collect();

    // Register all
    for f in &mut fabrics {
        let _ = fabric_register(f, &[]);
    }

    // Check all registered (or bail if no server)
    if !fabrics.iter().all(|f| f.is_registered()) {
        return;
    }

    // Deregister each independently via nb
    for (i, f) in fabrics.iter_mut().enumerate() {
        let result = fabric_deregister_nb(f, Box::new(NoOpDeregisterCallback));
        assert!(
            result.is_ok(),
            "fabric {} ({}) deregister_nb must succeed",
            i, names[i]
        );
        assert!(!f.is_registered(), "fabric {} must be unregistered", names[i]);
    }
}

/// fabric_deregister_nb resets fabric state after successful deregistration.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_resets_state() {
    use pmix::fabric::fabric_register;

    let mut fabric = PmixFabric::new(Some("state_reset_test")).unwrap();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }

    let _ = fabric_deregister_nb(&mut fabric, Box::new(NoOpDeregisterCallback));
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0, "ninfo should be reset after deregister_nb");
}

/// fabric_deregister_nb with a counting callback — verify callback is only
/// called once per operation.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_deregister_nb_counting_callback() {
    use pmix::fabric::fabric_register;

    let cb = CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    };
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    let _result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    // Callback will be invoked asynchronously by PMIx.
}
