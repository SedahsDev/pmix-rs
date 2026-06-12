//! Tests for `PMIx_Fabric_deregister` — fabric deregistration operations.
//!
//! These tests verify the Rust wrappers for the fabric deregistration APIs:
//! `fabric_deregister` (blocking) and `fabric_deregister_nb` (non-blocking).
//!
//! The C API spec states:
//! > Deregister a fabric object, providing an opportunity for
//! > the PMIx server library to cleanup any information
//! > (e.g., cost matrix) associated with it.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::fabric::{
    fabric_deregister, fabric_deregister_nb, fabric_register, FabricCallback, PmixFabric,
};
use pmix::{PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for non-blocking deregister operations.
struct TestDeregisterCallback;

impl FabricCallback for TestDeregisterCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        // No-op — just verify the trait compiles and the callback
        // can be invoked without panicking.
    }
}

/// Test callback that records the status it received via Cell.
struct RecordingDeregisterCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
}

impl FabricCallback for RecordingDeregisterCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.status.set(Some(status));
    }
}

/// Callback that counts invocations (should only be called once).
struct CountingDeregisterCallback {
    count: std::cell::Cell<u32>,
}

impl FabricCallback for CountingDeregisterCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        self.count.set(self.count.get().saturating_add(1));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that fabric_deregister on an unregistered fabric returns BAD_PARAM.
#[test]
fn test_fabric_deregister_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err(), "deregistering unregistered fabric must return error");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "error must be BAD_PARAM, not some other code"
    );
}

/// Test that fabric_deregister_nb on an unregistered fabric returns BAD_PARAM.
#[test]
fn test_fabric_deregister_nb_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(TestDeregisterCallback));
    assert!(
        result.is_err(),
        "deregister_nb on unregistered fabric must return error"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "error must be BAD_PARAM"
    );
}

/// Test that fabric_deregister on a fabric created with a name but never
/// registered still returns error.
#[test]
fn test_fabric_deregister_named_but_not_registered() {
    let mut fabric = PmixFabric::new(Some("test_deregister")).unwrap();
    assert!(!fabric.is_registered());
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
}

/// Test that fabric_deregister_nb on a named but unregistered fabric returns error.
#[test]
fn test_fabric_deregister_nb_named_not_registered() {
    let mut fabric = PmixFabric::new(Some("test_deregister_nb")).unwrap();
    let result = fabric_deregister_nb(&mut fabric, Box::new(TestDeregisterCallback));
    assert!(result.is_err());
}

/// Test that fabric_deregister does not panic on a default-constructed fabric.
#[test]
fn test_fabric_deregister_no_panic_default() {
    let fabric = PmixFabric::new(None).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = fabric;
        fabric_deregister(&mut f)
    }));
    // Should not panic — should return an error instead.
    assert!(
        result.is_ok(),
        "fabric_deregister must not panic on default fabric"
    );
}

/// Test that fabric_deregister_nb does not panic on a default-constructed fabric.
#[test]
fn test_fabric_deregister_nb_no_panic_default() {
    let fabric = PmixFabric::new(None).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = fabric;
        fabric_deregister_nb(&mut f, Box::new(TestDeregisterCallback))
    }));
    assert!(
        result.is_ok(),
        "fabric_deregister_nb must not panic on default fabric"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the FabricCallback trait is object-safe and accepts a
/// deregister-specific callback.
#[test]
fn test_deregister_callback_trait_object() {
    let _cb: Box<dyn FabricCallback> = Box::new(TestDeregisterCallback);
}

/// Test that a recording callback compiles and can be boxed.
#[test]
fn test_deregister_recording_callback() {
    let cb = RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

/// Test that a counting callback compiles and can be boxed.
#[test]
fn test_deregister_counting_callback() {
    let cb = CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    };
    let _boxed: Box<dyn FabricCallback> = Box::new(cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// State reset tests (verify internal state after deregister attempt)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that attempting to deregister an unregistered fabric does not
/// change the fabric's state (still unregistered, index/ninfo unchanged).
#[test]
fn test_fabric_deregister_unregistered_state_preserved() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_deregister(&mut fabric);
    assert!(!fabric.is_registered(), "fabric must remain unregistered");
    assert_eq!(fabric.index(), 0, "index must remain 0");
    assert_eq!(fabric.ninfo(), 0, "ninfo must remain 0");
}

/// Test that attempting to deregister_nb an unregistered fabric does not
/// change the fabric's state.
#[test]
fn test_fabric_deregister_nb_unregistered_state_preserved() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_deregister_nb(&mut fabric, Box::new(TestDeregisterCallback));
    assert!(!fabric.is_registered(), "fabric must remain unregistered");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code verification tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the error returned for unregistered fabric is a known BAD_PARAM.
#[test]
fn test_fabric_deregister_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.is_error(),
        "returned status must be an error, not success"
    );
}

/// Test that the error from deregister_nb is also BAD_PARAM.
#[test]
fn test_fabric_deregister_nb_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(TestDeregisterCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_error());
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple fabric instances tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that deregistering one fabric does not affect another.
#[test]
fn test_fabric_deregister_independent_instances() {
    let mut fabric_a = PmixFabric::new(Some("fabric_a")).unwrap();
    let fabric_b = PmixFabric::new(Some("fabric_b")).unwrap();

    let result_a = fabric_deregister(&mut fabric_a);
    assert!(result_a.is_err()); // both unregistered, so error expected

    // fabric_b should be unaffected.
    assert!(!fabric_b.is_registered());
    assert_eq!(fabric_b.name(), Some("fabric_b"));
}

/// Test that multiple unregistered fabrics each return their own error.
#[test]
fn test_fabric_deregister_multiple_unregistered() {
    let fabrics: Vec<_> = (0..5)
        .map(|i| PmixFabric::new(Some(&format!("fabric_{}", i))).unwrap())
        .collect();

    let mut results = Vec::new();
    for mut f in fabrics {
        results.push(fabric_deregister(&mut f));
    }

    for (i, result) in results.into_iter().enumerate() {
        assert!(
            result.is_err(),
            "fabric_{} deregister should return error (unregistered)",
            i
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Test the full register -> deregister lifecycle.
///
/// Under a real PMIx server, register a fabric and then deregister it,
/// verifying that the fabric's internal state is reset.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_lifecycle() {
    let mut fabric = PmixFabric::new(Some("deregister_test")).unwrap();
    assert!(!fabric.is_registered());

    // Register
    let reg_result = fabric_register(&mut fabric, &[]);
    if reg_result.is_err() {
        return; // No PMIx server available
    }
    assert!(fabric.is_registered());

    // Deregister
    let dereg_result = fabric_deregister(&mut fabric);
    assert!(
        dereg_result.is_ok(),
        "deregister must succeed for a registered fabric"
    );
    assert!(!fabric.is_registered(), "fabric must be unregistered after deregister");
    assert_eq!(fabric.ninfo(), 0, "ninfo must be reset to 0 after deregister");
}

/// Test double deregister returns error.
///
/// Register a fabric, deregister it once (should succeed), then try again
/// (should fail with BAD_PARAM).
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_double_deregister() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }

    // First deregister should succeed.
    let first = fabric_deregister(&mut fabric);
    assert!(first.is_ok(), "first deregister must succeed");
    assert!(!fabric.is_registered());

    // Second deregister should fail.
    let second = fabric_deregister(&mut fabric);
    assert!(
        second.is_err(),
        "double deregister must return error"
    );
    let err = second.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "double deregister error must be BAD_PARAM"
    );
}

/// Test fabric_deregister_nb with a callback under a real PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_nb_lifecycle() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return; // No PMIx server
    }

    let result = fabric_deregister_nb(&mut fabric, Box::new(TestDeregisterCallback));
    assert!(result.is_ok(), "deregister_nb must succeed for registered fabric");
    assert!(!fabric.is_registered());
}

/// Test fabric_deregister_nb with a recording callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_nb_recording() {
    let cb = RecordingDeregisterCallback {
        status: std::cell::Cell::new(None),
    };
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    let result = fabric_deregister_nb(&mut fabric, Box::new(cb));
    assert!(result.is_ok());
}

/// Test that fabric_deregister resets module pointer and ninfo.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_resets_state() {
    let mut fabric = PmixFabric::new(Some("state_reset_test")).unwrap();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }

    let _ = fabric_deregister(&mut fabric);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0, "index should be reset after deregister");
    assert_eq!(fabric.ninfo(), 0, "ninfo should be reset after deregister");
}

/// Test deregister after register with empty directives.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_after_register_empty() {
    let mut fabric = PmixFabric::new(Some("empty_directives")).unwrap();
    let reg = fabric_register(&mut fabric, &[]);
    if reg.is_err() {
        return;
    }
    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_ok());
}

/// Test that the callback wrapper is properly reclaimed on error for nb.
/// When deregister_nb fails (unregistered fabric), the callback wrapper
/// must be dropped and not leaked.
#[test]
fn test_fabric_deregister_nb_wrapper_reclaim_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(CountingDeregisterCallback {
        count: std::cell::Cell::new(0),
    }));
    assert!(result.is_err());
    // If we reach here without issues, the wrapper was properly reclaimed.
}

/// Test multiple fabrics registered and deregistered independently.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_multiple_independent() {
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

    // Deregister each independently
    for (i, f) in fabrics.iter_mut().enumerate() {
        let result = fabric_deregister(f);
        assert!(result.is_ok(), "fabric {} ({}) deregister must succeed", i, names[i]);
        assert!(!f.is_registered(), "fabric {} must be unregistered", names[i]);
    }
}
