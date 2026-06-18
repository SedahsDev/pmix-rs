//! Phase 4 Batch 1: PmixFabric Core Lifecycle — Register basic tests
//!
//! User-space tests for fabric_register/fabric_deregister that exercise
//! the error paths without requiring prterun.

use pmix::fabric::{fabric_deregister, fabric_register, PmixFabric};

/// Test that fabric_register on a named fabric returns error without PMIx server.
#[test]
fn test_fabric_register_no_server_named() {
    let mut fabric = PmixFabric::new(Some("test-fabric")).unwrap();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err());
    // Should still be unregistered
    assert!(!fabric.is_registered());
}

/// Test that fabric_register on an unamed fabric returns error without PMIx server.
#[test]
fn test_fabric_register_no_server_unamed() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err());
    assert!(!fabric.is_registered());
}

/// Test that fabric_deregister on an unregistered fabric returns error.
#[test]
fn test_fabric_deregister_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    // Verify the error code is BAD_PARAM (-27)
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// Test that fabric_deregister on a named unregistered fabric returns error.
#[test]
fn test_fabric_deregister_named_not_registered() {
    let mut fabric = PmixFabric::new(Some("test")).unwrap();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// Test that fabric_register with empty directives works (error path).
#[test]
fn test_fabric_register_empty_directives() {
    let mut fabric = PmixFabric::new(Some("empty-dir")).unwrap();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err());
    assert!(!fabric.is_registered());
}

/// Test that fabric_register preserves fabric name on error.
#[test]
fn test_fabric_register_preserves_name_on_error() {
    let mut fabric = PmixFabric::new(Some("preserved")).unwrap();
    let _ = fabric_register(&mut fabric, &[]);
    assert_eq!(fabric.name(), Some("preserved"));
}

/// Test that fabric_register preserves unamed state on error.
#[test]
fn test_fabric_register_preserves_unamed_on_error() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    assert_eq!(fabric.name(), None);
}

/// Test that fabric_register does not modify index on error.
#[test]
fn test_fabric_register_preserves_index_on_error() {
    let mut fabric = PmixFabric::new(Some("idx-test")).unwrap();
    assert_eq!(fabric.index(), 0);
    let _ = fabric_register(&mut fabric, &[]);
    assert_eq!(fabric.index(), 0);
}
