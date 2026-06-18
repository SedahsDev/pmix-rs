//! Fabric tests that work both standalone and under prterun.
//!
//! Standalone tests (no PMIx init required) run in normal cargo test.
//! DVM tests have been moved to fabric_dvm_via_prterun.rs and
//! fabric_directives_via_prterun.rs to avoid PMIx state corruption.
//!
//! Run standalone tests:
//! ```bash
//! cargo test --test fabric_ffi_via_prterun -- --test-threads=1
//! ```
//!
//! Run DVM tests (separate files):
//! ```bash
//! prterun -np 1 cargo test --test fabric_dvm_via_prterun -- --include-ignored --test-threads=1
//! prterun -np 1 cargo test --test fabric_directives_via_prterun -- --include-ignored --test-threads=1
//! ```

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests (no PMIx init required)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric::new creates a fabric with the given name.
#[test]
fn test_fabric_new_named() {
    let fabric = pmix::fabric::PmixFabric::new(Some("test-fabric")).expect("new failed");
    assert_eq!(fabric.name(), Some("test-fabric"));
    assert!(!fabric.is_registered());
}

/// PmixFabric::new with None creates an unnamed fabric.
#[test]
fn test_fabric_new_unnamed() {
    let fabric = pmix::fabric::PmixFabric::new(None).expect("new failed");
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
}

/// PmixFabric::new rejects names containing NUL bytes.
#[test]
fn test_fabric_new_rejects_nul() {
    let result = pmix::fabric::PmixFabric::new(Some("bad\x00name"));
    assert!(result.is_err());
}

/// PmixFabric::unamed creates an unnamed fabric.
#[test]
fn test_fabric_unamed() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
}

/// PmixFabric Debug impl works.
#[test]
fn test_fabric_debug() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    let debug_str = format!("{:?}", fabric);
    assert!(!debug_str.is_empty());
}

/// PmixFabric Debug impl for named fabric.
#[test]
fn test_fabric_debug_named() {
    let fabric = pmix::fabric::PmixFabric::new(Some("debug-test")).expect("new failed");
    let debug_str = format!("{:?}", fabric);
    assert!(debug_str.contains("debug-test"));
}

/// PmixFabric accessor: name().
#[test]
fn test_fabric_name_accessor_some() {
    let fabric = pmix::fabric::PmixFabric::new(Some("accessor-test")).expect("new failed");
    assert_eq!(fabric.name(), Some("accessor-test"));
}

/// PmixFabric accessor: name() for None.
#[test]
fn test_fabric_name_accessor_none() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
}

/// PmixFabric accessor: is_registered().
#[test]
fn test_fabric_is_registered_accessor() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert!(!fabric.is_registered());
}

/// PmixFabric accessor: index().
#[test]
fn test_fabric_index_accessor() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.index(), 0);
}

/// PmixFabric accessor: ninfo().
#[test]
fn test_fabric_ninfo_accessor() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.ninfo(), 0);
}

/// PmixTopology::new creates a topology.
#[test]
fn test_topology_new() {
    let topo = pmix::fabric::PmixTopology::new(None).expect("new failed");
    assert_eq!(topo.source(), None);
    assert!(!topo.is_loaded());
}

/// PmixTopology::unamed creates an unnamed topology.
#[test]
fn test_topology_unamed() {
    let topo = pmix::fabric::PmixTopology::unamed();
    assert_eq!(topo.source(), None);
    assert!(!topo.is_loaded());
}

/// PmixCpuset::new creates a cpuset.
#[test]
fn test_cpuset_new() {
    let _cpuset = pmix::fabric::PmixCpuset::new();
}

/// PmixCpuset Debug impl works.
#[test]
fn test_cpuset_debug() {
    let cpuset = pmix::fabric::PmixCpuset::new();
    let debug_str = format!("{:?}", cpuset);
    assert!(!debug_str.is_empty());
}

/// fabric_register fails gracefully without PMIx init (only when NOT under DVM).
#[test]
fn test_fabric_register_fails_without_init() {
    if std::env::var("PMIX_RANK").is_err() {
        let mut fabric = pmix::fabric::PmixFabric::unamed();
        let result = pmix::fabric::fabric_register(&mut fabric, &[]);
        assert!(result.is_err());
    }
}

/// fabric_update fails gracefully without PMIx init (only when NOT under DVM).
#[test]
fn test_fabric_update_fails_without_init() {
    if std::env::var("PMIX_RANK").is_err() {
        let mut fabric = pmix::fabric::PmixFabric::unamed();
        let result = pmix::fabric::fabric_update(&mut fabric);
        assert!(result.is_err());
    }
}

/// fabric_deregister on unregistered fabric returns appropriate error.
#[test]
fn test_fabric_deregister_unregistered() {
    let mut fabric = pmix::fabric::PmixFabric::unamed();
    let result = pmix::fabric::fabric_deregister(&mut fabric);
    assert!(result.is_err());
}

/// compute_distances without init fails gracefully (only when NOT under DVM).
#[test]
fn test_compute_distances_fails_without_init() {
    if std::env::var("PMIX_RANK").is_err() {
        let mut topo = pmix::fabric::PmixTopology::unamed();
        let mut cpuset = pmix::fabric::PmixCpuset::new();
        let result = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
        assert!(result.is_err());
    }
}

/// load_topology without init fails gracefully (only when NOT under DVM).
#[test]
fn test_load_topology_fails_without_init() {
    if std::env::var("PMIX_RANK").is_err() {
        let mut topo = pmix::fabric::PmixTopology::unamed();
        let result = pmix::fabric::load_topology(&mut topo);
        assert!(result.is_err());
    }
}

/// fabric_drop_unregistered - dropping unregistered fabric is safe.
#[test]
fn test_fabric_drop_unregistered() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    drop(fabric);
}
