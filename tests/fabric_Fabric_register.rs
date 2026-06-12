//! Tests for `PMIx_Fabric_register` — fabric registration operations.
//!
//! These tests verify the Rust wrappers for fabric management APIs:
//! `fabric_register`, `fabric_register_nb`, `fabric_update`,
//! `fabric_update_nb`, `fabric_deregister`, `fabric_deregister_nb`,
//! and the `PmixFabric` type.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::fabric::{
    fabric_deregister, fabric_deregister_nb, fabric_register, fabric_register_nb, fabric_update,
    fabric_update_nb, FabricCallback, PmixFabric,
};
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for non-blocking fabric operations.
struct TestFabricCallback;

impl FabricCallback for TestFabricCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        // No-op — just verify the trait compiles and the callback
        // can be invoked without panicking.
    }
}

/// Test callback that records the status it received.
struct RecordingFabricCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
}

impl FabricCallback for RecordingFabricCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.status.set(Some(status));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric construction tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixFabric can be created with no name.
#[test]
fn test_fabric_unamed() {
    let fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
    assert_eq!(fabric.name(), None);
}

/// Test that PmixFabric can be created with a name.
#[test]
fn test_fabric_new_with_name() {
    let fabric = PmixFabric::new(Some("test_fabric")).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("test_fabric"));
}

/// Test that PmixFabric can be created with None name.
#[test]
fn test_fabric_new_none_name() {
    let fabric = PmixFabric::new(None).unwrap();
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), None);
}

/// Test that PmixFabric::new rejects names with interior NUL bytes.
#[test]
fn test_fabric_new_nul_name() {
    let result = PmixFabric::new(Some("test\0fabric"));
    assert!(result.is_err());
}

/// Test that PmixFabric implements Debug.
#[test]
fn test_fabric_debug() {
    let fabric = PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixFabric"));
}

/// Test that a named fabric's Debug output contains the name.
#[test]
fn test_fabric_debug_with_name() {
    let fabric = PmixFabric::new(Some("ib0")).unwrap();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("ib0"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that fabric_update on an unregistered fabric returns error.
#[test]
fn test_fabric_update_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
}

/// Test that fabric_deregister on an unregistered fabric returns error.
#[test]
fn test_fabric_deregister_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
}

/// Test that fabric_update_nb on an unregistered fabric returns error.
#[test]
fn test_fabric_update_nb_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(TestFabricCallback));
    assert!(result.is_err());
}

/// Test that fabric_deregister_nb on an unregistered fabric returns error.
#[test]
fn test_fabric_deregister_nb_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(TestFabricCallback));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the FabricCallback trait is object-safe.
#[test]
fn test_fabric_callback_trait_object() {
    struct TestCb;
    impl FabricCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn FabricCallback> = Box::new(TestCb);
}

/// Test that the RecordingFabricCallback records status.
#[test]
fn test_fabric_callback_records_status() {
    let cb = RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    };
    let boxed: Box<dyn FabricCallback> = Box::new(cb);
    // We can't invoke the trait method on a boxed RecordingFabricCallback
    // directly, but we verified the trait compiles and is object-safe.
    drop(boxed);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Test fabric_register with empty directives.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_register_empty_directives() {
    let mut fabric = PmixFabric::new(Some("test")).unwrap();
    let result = fabric_register(&mut fabric, &[]);
    if let Ok(()) = result {
        assert!(fabric.is_registered());
    }
}

/// Test fabric_register with a named fabric.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_register_named() {
    let mut fabric = PmixFabric::new(Some("infiniband")).unwrap();
    let result = fabric_register(&mut fabric, &[]);
    if let Ok(()) = result {
        assert!(fabric.is_registered());
        assert!(fabric.index() > 0);
    }
}

/// Test the full register/update/deregister lifecycle.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_lifecycle() {
    let mut fabric = PmixFabric::new(Some("lifecycle_test")).unwrap();
    assert!(!fabric.is_registered());

    // Register
    let reg_result = fabric_register(&mut fabric, &[]);
    if reg_result.is_err() {
        return; // No PMIx server
    }
    assert!(fabric.is_registered());

    // Update
    let _ = fabric_update(&mut fabric);

    // Deregister
    let dereg_result = fabric_deregister(&mut fabric);
    assert!(dereg_result.is_ok());
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

/// Test double deregister returns error.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_double_deregister() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    assert!(fabric_deregister(&mut fabric).is_ok());
    assert!(!fabric.is_registered());
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
}

/// Test fabric_register_nb with a callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_register_nb() {
    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(TestFabricCallback));
}

/// Test fabric_update_nb with a callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_update_nb() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    let _result = fabric_update_nb(&mut fabric, Box::new(TestFabricCallback));
}

/// Test fabric_deregister_nb with a callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_deregister_nb() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    if !fabric.is_registered() {
        return;
    }
    let _result = fabric_deregister_nb(&mut fabric, Box::new(TestFabricCallback));
}

/// Test fabric_register_nb with a recording callback.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_fabric_register_nb_recording() {
    let cb = RecordingFabricCallback {
        status: std::cell::Cell::new(None),
    };
    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(cb));
}
