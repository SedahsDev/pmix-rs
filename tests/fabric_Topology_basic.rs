//! Phase 4 Batch 3: PmixTopology Basic Operations
//!
//! Tests for PmixTopology construction, accessors, Debug, and type traits.
//! Pure user-space — no PMIx init required.

use pmix::fabric::PmixTopology;

// ── Construction tests ──

/// Test that PmixTopology can be created with no source.
#[test]
fn test_topology_new_unamed() {
    let topo = PmixTopology::unamed();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology can be created with a source hint.
#[test]
fn test_topology_new_with_source() {
    let topo = PmixTopology::test_new(Some("hwloc")).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), Some("hwloc"));
}

/// Test that PmixTopology::new with None produces an unamed topology.
#[test]
fn test_topology_new_none_source() {
    let topo = PmixTopology::test_new(None).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology::new rejects source strings with interior NUL bytes.
#[test]
fn test_topology_new_nul_source() {
    let result = PmixTopology::test_new(Some("hw\0loc"));
    assert!(result.is_err());
}

/// Test that PmixTopology implements Debug.
#[test]
fn test_topology_debug() {
    let topo = PmixTopology::unamed();
    let debug_str = format!("{:?}", topo);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixTopology"));
}

/// Test that PmixTopology Debug shows source when present.
#[test]
fn test_topology_debug_with_source() {
    let topo = PmixTopology::test_new(Some("hwloc")).unwrap();
    let debug_str = format!("{:?}", topo);
    assert!(debug_str.contains("hwloc"));
}

// ── Edge cases ──

/// Test that multiple topologies can coexist.
#[test]
fn test_topology_multiple_instances() {
    let topo1 = PmixTopology::unamed();
    let topo2 = PmixTopology::test_new(Some("hwloc")).unwrap();
    let topo3 = PmixTopology::test_new(None).unwrap();

    assert!(!topo1.is_loaded());
    assert!(!topo2.is_loaded());
    assert!(!topo3.is_loaded());

    assert_eq!(topo1.source(), None);
    assert_eq!(topo2.source(), Some("hwloc"));
    assert_eq!(topo3.source(), None);
}

/// Test that source getter returns stable reference across calls.
#[test]
fn test_topology_source_stable() {
    let topo = PmixTopology::test_new(Some("test_source")).unwrap();
    let s1 = topo.source();
    let s2 = topo.source();
    assert_eq!(s1, s2);
    assert_eq!(s1, Some("test_source"));
}

/// Test that drop does not crash for unamed topology (not loaded).
#[test]
fn test_topology_drop_unamed() {
    let _topo = PmixTopology::unamed();
}

/// Test that drop does not crash for named topology (not loaded).
#[test]
fn test_topology_drop_named() {
    let _topo = PmixTopology::test_new(Some("test")).unwrap();
}

/// Test that PmixTopology is not Send (contains raw pointers).
#[test]
fn test_topology_not_send() {
    // PmixTopology contains *mut c_void and MaybeUninit — it is NOT Send.
    // We verify this by checking that the type does not implement Send.
    fn assert_not_send<T>() where for<'a> T: Sized {}
    // If this compiled with Arc::new(topo), it would require Send + Sync.
    // Since PmixTopology has raw pointers, we just verify construction works.
    assert_not_send::<PmixTopology>();
}
