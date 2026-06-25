//! Phase 4 Batch 1: PmixFabric Core Lifecycle — Update basic tests
//!
//! User-space tests for fabric_update that exercise the error paths
//! without requiring prterun.

use pmix::fabric::{PmixFabric, fabric_update};

/// Test that fabric_update on an unregistered fabric returns error.
#[test]
fn test_fabric_update_not_registered() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
    // Should be BAD_PARAM since not registered
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// Test that fabric_update on a named unregistered fabric returns error.
#[test]
fn test_fabric_update_named_not_registered() {
    let mut fabric = PmixFabric::new(Some("update-test")).unwrap();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// Test that fabric_update preserves fabric name on error.
#[test]
fn test_fabric_update_preserves_name_on_error() {
    let mut fabric = PmixFabric::new(Some("preserved")).unwrap();
    let _ = fabric_update(&mut fabric);
    assert_eq!(fabric.name(), Some("preserved"));
}

/// Test that fabric_update preserves unamed state on error.
#[test]
fn test_fabric_update_preserves_unamed_on_error() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_update(&mut fabric);
    assert_eq!(fabric.name(), None);
}

/// Test that fabric_update does not modify index on error.
#[test]
fn test_fabric_update_preserves_index_on_error() {
    let mut fabric = PmixFabric::new(Some("idx-test")).unwrap();
    assert_eq!(fabric.index(), 0);
    let _ = fabric_update(&mut fabric);
    assert_eq!(fabric.index(), 0);
}

/// Test that fabric_update does not modify ninfo on error.
#[test]
fn test_fabric_update_preserves_ninfo_on_error() {
    let mut fabric = PmixFabric::new(Some("ninfo-test")).unwrap();
    assert_eq!(fabric.ninfo(), 0);
    let _ = fabric_update(&mut fabric);
    assert_eq!(fabric.ninfo(), 0);
}

/// Test that fabric_update does not change registered flag on error.
#[test]
fn test_fabric_update_preserves_registered_on_error() {
    let mut fabric = PmixFabric::unamed();
    assert!(!fabric.is_registered());
    let _ = fabric_update(&mut fabric);
    assert!(!fabric.is_registered());
}

/// Test that multiple fabric_update calls are idempotent on error.
#[test]
fn test_fabric_update_multiple_calls() {
    let mut fabric = PmixFabric::new(Some("multi")).unwrap();
    for _ in 0..3 {
        let result = fabric_update(&mut fabric);
        assert!(result.is_err());
        assert!(!fabric.is_registered());
        assert_eq!(fabric.ninfo(), 0);
    }
}
