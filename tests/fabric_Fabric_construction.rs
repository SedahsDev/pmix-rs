//! Phase 4 Batch 1: PmixFabric Core Lifecycle — Construction tests
//!
//! User-space tests for PmixFabric::new, unamed, name, index,
//! is_registered, ninfo that do not require prterun.

use pmix::fabric::PmixFabric;

/// Test that PmixFabric::new with Some(name) creates a named fabric.
#[test]
fn test_fabric_new_named() {
    let fabric = PmixFabric::new(Some("test-fabric")).unwrap();
    assert_eq!(fabric.name(), Some("test-fabric"));
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

/// Test that PmixFabric::new with None creates an unamed fabric.
#[test]
fn test_fabric_new_none() {
    let fabric = PmixFabric::new(None).unwrap();
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

/// Test PmixFabric::unamed creates an unamed fabric.
#[test]
fn test_fabric_unamed() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

/// Test that PmixFabric::new with empty string name works.
#[test]
fn test_fabric_new_empty_name() {
    let fabric = PmixFabric::new(Some("")).unwrap();
    assert_eq!(fabric.name(), Some(""));
    assert!(!fabric.is_registered());
}

/// Test that PmixFabric::new with NUL byte returns error.
#[test]
fn test_fabric_new_nul_name() {
    let result = PmixFabric::new(Some("test\0fabric"));
    assert!(result.is_err());
}

/// Test that a named fabric has the correct index (0 for unregistered).
#[test]
fn test_fabric_index_unregistered() {
    let fabric = PmixFabric::new(Some("indexed")).unwrap();
    assert_eq!(fabric.index(), 0);
}

/// Test that an unamed fabric has index 0.
#[test]
fn test_fabric_unamed_index() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.index(), 0);
}

/// Test that is_registered is false for newly created fabrics.
#[test]
fn test_fabric_not_registered_on_creation() {
    let fabric1 = PmixFabric::new(Some("reg-test")).unwrap();
    let fabric2 = PmixFabric::unamed();
    assert!(!fabric1.is_registered());
    assert!(!fabric2.is_registered());
}

/// Test that ninfo is 0 for newly created fabrics.
#[test]
fn test_fabric_ninfo_zero_on_creation() {
    let fabric1 = PmixFabric::new(Some("info-test")).unwrap();
    let fabric2 = PmixFabric::unamed();
    assert_eq!(fabric1.ninfo(), 0);
    assert_eq!(fabric2.ninfo(), 0);
}

/// Test Debug trait implementation for named fabric.
#[test]
fn test_fabric_debug_named() {
    let fabric = PmixFabric::new(Some("debug-me")).unwrap();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("PmixFabric"));
    assert!(debug_str.contains("debug-me"));
}

/// Test Debug trait implementation for unamed fabric.
#[test]
fn test_fabric_debug_unamed() {
    let fabric = PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("PmixFabric"));
}

/// Test that multiple fabrics can coexist independently.
#[test]
fn test_fabric_multiple_instances() {
    let fabric1 = PmixFabric::new(Some("first")).unwrap();
    let fabric2 = PmixFabric::new(Some("second")).unwrap();
    let fabric3 = PmixFabric::unamed();

    assert_eq!(fabric1.name(), Some("first"));
    assert_eq!(fabric2.name(), Some("second"));
    assert_eq!(fabric3.name(), None);

    assert!(!fabric1.is_registered());
    assert!(!fabric2.is_registered());
    assert!(!fabric3.is_registered());
}
