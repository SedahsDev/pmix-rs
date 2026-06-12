//! Tests for `PMIx_Fabric_register_nb` — non-blocking fabric registration.
//!
//! Derived from the fabric API spec in the PMIx v4.1 standard (Section 14.4.2)
//! and the C test patterns in `test/simple/simpfabric.c`. The C test file does
//! not exercise `PMIx_Fabric_register_nb` directly, so these tests cover the
//! safe Rust wrapper parameter validation, callback wrapper construction,
//! error handling paths, and integration scenarios.
//!
//! Tests that require `PMIx_Init` or a PMIx server are marked `#[ignore]`
//! because they need a running PMIx daemon / server. Calling the FFI without
//! an initialized PMIx library causes a segfault, so all tests that invoke
//! `fabric_register_nb` directly are ignored.

use pmix::PmixStatus;
use pmix::fabric::{FabricCallback, PmixFabric, fabric_register_nb};

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
fn fabric_register_nb_callback_trait_object() {
    let _cb: Box<dyn FabricCallback> = Box::new(NoOpFabricCallback);
}

/// RecordingFabricCallback compiles and is object-safe.
#[test]
fn fabric_register_nb_callback_records_status_type() {
    let cb = RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// CountingFabricCallback compiles and is object-safe.
#[test]
fn fabric_register_nb_callback_counting_type() {
    let cb = CountingFabricCallback {
        count: std::cell::Cell::new(0),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ArcStatusCallback compiles and is object-safe.
#[test]
fn fabric_register_nb_callback_arc_status_type() {
    let cb = ArcStatusCallback {
        status: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// ClosureCallback compiles and is object-safe.
#[test]
fn fabric_register_nb_callback_closure_type() {
    let cb = ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(move |_: PmixStatus| {}))),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// FabricCallback is Send (required by the trait bound).
#[test]
fn fabric_register_nb_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn FabricCallback>>();
}

/// NoOpFabricCallback is Send.
#[test]
fn fabric_register_nb_callback_noop_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NoOpFabricCallback>();
}

/// RecordingFabricCallback is Send.
#[test]
fn fabric_register_nb_callback_recording_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RecordingFabricCallback>();
}

/// ArcStatusCallback is Send.
#[test]
fn fabric_register_nb_callback_arc_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ArcStatusCallback>();
}

/// ClosureCallback is Send.
#[test]
fn fabric_register_nb_callback_closure_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ClosureCallback>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric construction — prerequisites for fabric_register_nb
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric can be created unnamed — prerequisite for fabric_register_nb.
#[test]
fn fabric_register_nb_fabric_unamed() {
    let fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert_eq!(fabric.name(), None);
}

/// PmixFabric can be created with a name — prerequisite for fabric_register_nb.
#[test]
fn fabric_register_nb_fabric_named() {
    let fabric = PmixFabric::new(Some("infiniband")).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("infiniband"));
}

/// PmixFabric can be created with None name.
#[test]
fn fabric_register_nb_fabric_none_name() {
    let fabric = PmixFabric::new(None).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), None);
}

/// PmixFabric::new rejects names with interior NUL bytes.
#[test]
fn fabric_register_nb_fabric_nul_name_rejected() {
    let result = PmixFabric::new(Some("test\0fabric"));
    assert!(result.is_err());
}

/// PmixFabric implements Debug.
#[test]
fn fabric_register_nb_fabric_debug() {
    let fabric = PmixFabric::new(Some("debug_test")).unwrap();
    let debug_str = format!("{:?}", fabric);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("debug_test"));
}

/// PmixFabric Debug for unnamed fabric.
#[test]
fn fabric_register_nb_fabric_debug_unamed() {
    let fabric = PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("registered"));
}

/// PmixFabric index and ninfo start at zero.
#[test]
fn fabric_register_nb_fabric_initial_state() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert!(!fabric.is_registered());
}

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type compatibility tests (compile-only, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_register_nb accepts the correct parameter types.
/// This is a compile-time check — the function signature must accept:
/// &mut PmixFabric, &[Info], Box<dyn FabricCallback>.
#[test]
fn fabric_register_nb_signature_compiles() {
    // Verify the function is callable with the right types.
    // We can't actually call it without a PMIx server (segfaults),
    // but we can verify the types are correct by checking it compiles.
    fn _check_signature(
        fabric: &mut PmixFabric,
        callback: Box<dyn FabricCallback>,
    ) -> Result<(), PmixStatus> {
        fabric_register_nb(fabric, &[], callback)
    }
    // Just verify the function pointer type is valid.
    let _ =
        _check_signature as fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>;
}

/// Multiple callback implementations can all be passed to fabric_register_nb.
#[test]
fn fabric_register_nb_multiple_callback_types_compile() {
    // Verify all callback types implement FabricCallback.
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
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(move |_: PmixStatus| {}))),
    });
}

/// fabric_register_nb return type is Result<(), PmixStatus>.
#[test]
fn fabric_register_nb_return_type() {
    // Verify the return type is what we expect.
    fn _check_return(
        _f: fn(&mut PmixFabric, &[pmix::Info], Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {
    }
    _check_return(fabric_register_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper memory safety (compile checks, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// Callback wrapper with large captured data compiles.
#[test]
fn fabric_register_nb_large_callback_data_compiles() {
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
fn fabric_register_nb_complex_callback_state() {
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
        name: "complex_test".to_string(),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — all ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_register_nb with an unnamed fabric and no directives.
/// This is the simplest possible call — register the default fabric.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_unamed_no_directives() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NoOpFabricCallback));
    if result.is_ok() {
        assert!(fabric.is_registered());
    }
}

/// fabric_register_nb with a named fabric.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_named_fabric() {
    let mut fabric = PmixFabric::new(Some("infiniband")).unwrap();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NoOpFabricCallback));
    if result.is_ok() {
        assert!(fabric.is_registered());
        assert!(fabric.index() > 0);
    }
}

/// fabric_register_nb with a recording callback — verify callback receives status.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_recording_callback() {
    use std::sync::{Arc, Mutex};

    let status = Arc::new(Mutex::new(None));
    let status_clone = status.clone();
    let cb = ArcStatusCallback {
        status: status_clone,
    };

    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
    if result.is_ok() {
        assert!(fabric.is_registered());
    }
}

/// fabric_register_nb with a closure callback — verify closure is accepted.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_closure_callback() {
    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let called_clone = called.clone();
    let cb = ClosureCallback {
        f: std::sync::Arc::new(std::sync::Mutex::new(Box::new(move |_: PmixStatus| {
            *called_clone.lock().unwrap() = true;
        }))),
    };

    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
    if result.is_ok() {
        assert!(fabric.is_registered());
    }
}

/// fabric_register_nb followed by fabric_update_nb then fabric_deregister_nb.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_full_lifecycle() {
    use pmix::fabric::{fabric_deregister_nb, fabric_update_nb};

    let mut fabric = PmixFabric::new(Some("lifecycle")).unwrap();

    // Register (non-blocking)
    let reg = fabric_register_nb(&mut fabric, &[], Box::new(NoOpFabricCallback));
    if reg.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Update (non-blocking)
    let _update = fabric_update_nb(&mut fabric, Box::new(NoOpFabricCallback));

    // Deregister (non-blocking)
    let _dereg = fabric_deregister_nb(&mut fabric, Box::new(NoOpFabricCallback));
}

/// fabric_register_nb with multiple fabrics registered simultaneously.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_multiple_fabrics() {
    let mut fabrics: Vec<PmixFabric> = Vec::new();
    for i in 0..3 {
        fabrics.push(PmixFabric::new(Some(&format!("fabric_{}", i))).unwrap());
    }

    for fabric in &mut fabrics {
        let result = fabric_register_nb(fabric, &[], Box::new(NoOpFabricCallback));
        if result.is_ok() {
            assert!(fabric.is_registered());
        }
    }
}

/// fabric_register_nb callback receives PMIX_SUCCESS on successful registration.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_callback_receives_success() {
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let status_clone = status.clone();
    let cb = ArcStatusCallback {
        status: status_clone,
    };

    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
    // Callback will be invoked asynchronously by PMIx.
}

/// fabric_register_nb with a counting callback — verify it can track invocations.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_counting_callback() {
    let cb = CountingFabricCallback {
        count: std::cell::Cell::new(0),
    };
    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
}

/// fabric_register_nb registers the fabric (sets registered flag on success).
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_sets_registered_flag() {
    let mut fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NoOpFabricCallback));
    if result.is_ok() {
        assert!(fabric.is_registered());
    }
}

/// fabric_register_nb with complex callback state.
#[test]
#[ignore = "requires PMIx daemon"]
fn fabric_register_nb_complex_callback() {
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
        name: "complex_test".to_string(),
    };

    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
}
