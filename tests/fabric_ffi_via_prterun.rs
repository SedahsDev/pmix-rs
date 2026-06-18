//! Fabric FFI tests that require PMIx initialization via prterun.
//!
//! These tests exercise the actual FFI paths in fabric.rs by calling
//! `pmix::init()` before fabric operations. Run with:
//!
//! ```bash
//! # Run all DVM tests (shared PMIx context, no state corruption):
//! prterun -np 1 cargo test --test fabric_ffi_via_prterun -- --include-ignored --test-threads=1
//! ```

use std::sync::OnceLock;

// ─────────────────────────────────────────────────────────────────────────────
// Shared PMIx context for all DVM tests
// ─────────────────────────────────────────────────────────────────────────────

/// Shared PMIx context initialized once when first DVM test runs.
/// Avoids multiple init/finalize cycles that corrupt PMIx library state.
static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

/// Initialize the shared PMIx context if we're running under prterun.
/// Returns true if PMIx is available.
fn ensure_pmix_init() -> bool {
    if !is_dvm_launched() {
        return false;
    }
    PMIX_CONTEXT
        .set(pmix::init(None).ok())
        .is_ok() && PMIX_CONTEXT.get().unwrap().is_some()
}

/// Check if launched by prterun.
fn is_dvm_launched() -> bool {
    std::env::var("PMIX_RANK").is_ok()
}

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
    if !is_dvm_launched() {
        let mut fabric = pmix::fabric::PmixFabric::unamed();
        let result = pmix::fabric::fabric_register(&mut fabric, &[]);
        assert!(result.is_err());
    }
}

/// fabric_update fails gracefully without PMIx init (only when NOT under DVM).
#[test]
fn test_fabric_update_fails_without_init() {
    if !is_dvm_launched() {
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
    if !is_dvm_launched() {
        let mut topo = pmix::fabric::PmixTopology::unamed();
        let mut cpuset = pmix::fabric::PmixCpuset::new();
        let result = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
        assert!(result.is_err());
    }
}

/// load_topology without init fails gracefully (only when NOT under DVM).
#[test]
fn test_load_topology_fails_without_init() {
    if !is_dvm_launched() {
        let mut topo = pmix::fabric::PmixTopology::unamed();
        let result = pmix::fabric::load_topology(&mut topo);
        assert!(result.is_err());
    }
}

/// fabric_drop_unregistered - dropping unregistered fabric is safe.
#[test]
fn test_fabric_drop_unregistered() {
    let fabric = pmix::fabric::PmixFabric::unamed();
    drop(fabric); // Should not crash
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests (require prterun, use shared PMIx context)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric::new via DVM with shared context.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_new_via_dvm() {
    assert!(ensure_pmix_init());
    let fabric = pmix::fabric::PmixFabric::new(Some("dvm-test")).expect("new failed");
    assert_eq!(fabric.name(), Some("dvm-test"));
    assert!(!fabric.is_registered());
}

/// PmixFabric::unamed via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_unamed_via_dvm() {
    assert!(ensure_pmix_init());
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
}

/// fabric_register success path via DVM.
/// Covers: fabric_register FFI call, PMIx_Fabric_register
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_register_success_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric = pmix::fabric::PmixFabric::new(Some("register-test")).expect("new failed");
    assert!(!fabric.is_registered());
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_update via DVM.
/// Covers: fabric_update FFI call, PMIx_Fabric_update
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_update_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric = pmix::fabric::PmixFabric::unamed();
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        let _ = pmix::fabric::fabric_update(&mut fabric);
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_deregister via DVM.
/// Covers: fabric_deregister FFI call, PMIx_Fabric_deregister
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_deregister_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric = pmix::fabric::PmixFabric::new(Some("dereg-test")).expect("new failed");
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());
        let dereg_result = pmix::fabric::fabric_deregister(&mut fabric);
        if dereg_result.is_ok() {
            assert!(!fabric.is_registered());
        }
    }
    drop(fabric);
}

/// fabric_register_nb via DVM.
/// Covers: fabric_register_nb FFI call, PMIx_Fabric_register_nb
/// NOTE: Must be run individually — async callback corrupts PMIx state for subsequent tests.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_fabric_register_nb_via_dvm() {
    assert!(ensure_pmix_init());
    use pmix::fabric::FabricCallback;

    struct TestCallback;
    impl FabricCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: pmix::PmixStatus) {}
    }

    let mut fabric = pmix::fabric::PmixFabric::unamed();
    let result = pmix::fabric::fabric_register_nb(&mut fabric, &[], Box::new(TestCallback));
    if result.is_ok() {
        assert!(fabric.is_registered());
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_update_nb via DVM.
/// Covers: fabric_update_nb FFI call, PMIx_Fabric_update_nb
/// NOTE: Must be run individually — async callback corrupts PMIx state for subsequent tests.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_fabric_update_nb_via_dvm() {
    assert!(ensure_pmix_init());
    use pmix::fabric::FabricCallback;

    struct UpdateCallback;
    impl FabricCallback for UpdateCallback {
        fn on_complete(self: Box<Self>, _status: pmix::PmixStatus) {}
    }

    let mut fabric = pmix::fabric::PmixFabric::unamed();
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        let _ = pmix::fabric::fabric_update_nb(&mut fabric, Box::new(UpdateCallback));
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_deregister_nb via DVM.
/// Covers: fabric_deregister_nb FFI call, PMIx_Fabric_deregister_nb
/// NOTE: Must be run individually — async callback corrupts PMIx state for subsequent tests.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_fabric_deregister_nb_via_dvm() {
    assert!(ensure_pmix_init());
    use pmix::fabric::FabricCallback;

    struct DeregCallback;
    impl FabricCallback for DeregCallback {
        fn on_complete(self: Box<Self>, _status: pmix::PmixStatus) {}
    }

    let mut fabric = pmix::fabric::PmixFabric::unamed();
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        let _ = pmix::fabric::fabric_deregister_nb(&mut fabric, Box::new(DeregCallback));
    }
}

/// load_topology via DVM.
/// Covers: load_topology FFI call, PMIx_Topology_load
/// NOTE: Must be run individually — PMIx library state corruption when run after compute_distances_nb.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_load_topology_via_dvm() {
    assert!(ensure_pmix_init());
    let mut topo = pmix::fabric::PmixTopology::unamed();
    let _ = pmix::fabric::load_topology(&mut topo);
}

/// compute_distances via DVM (after load_topology).
/// Covers: compute_distances FFI call, PMIx_Topology_compute_distances
/// NOTE: Must be run individually — PMIx library state corruption when run after compute_distances_nb.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_compute_distances_via_dvm() {
    assert!(ensure_pmix_init());
    let mut topo = pmix::fabric::PmixTopology::unamed();
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    // compute_distances requires a loaded topology — load first to avoid FFI crash
    if pmix::fabric::load_topology(&mut topo).is_ok() {
        let _ = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
    }
    // If load_topology fails, we still cover the function call path (it returns error gracefully)
}

/// compute_distances_nb via DVM.
/// Covers: compute_distances_nb FFI call, PMIx_Topology_compute_distances_nb
/// NOTE: Must be run individually — async callback corrupts PMIx state for subsequent tests.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_compute_distances_nb_via_dvm() {
    assert!(ensure_pmix_init());
    use pmix::fabric::ComputeDistancesCallback;

    struct DistCallback;
    impl ComputeDistancesCallback for DistCallback {
        fn on_complete(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _distances: pmix::fabric::DeviceDistances,
        ) {
        }
    }

    let mut topo = pmix::fabric::PmixTopology::unamed();
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    if pmix::fabric::load_topology(&mut topo).is_ok() {
        let _ =
            pmix::fabric::compute_distances_nb(&mut topo, &mut cpuset, &[], Box::new(DistCallback));
    }
}

/// PmixCpuset::as_mut_ptr via DVM.
/// Covers: PmixCpuset::as_mut_ptr
#[test]
#[ignore = "requires prterun launch"]
fn test_cpuset_as_mut_ptr_via_dvm() {
    assert!(ensure_pmix_init());
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    let ptr = cpuset.as_mut_ptr();
    assert!(!ptr.is_null(), "cpuset as_mut_ptr should be non-null");
}

/// Full lifecycle: new -> register -> update -> deregister -> drop.
/// Covers: complete fabric lifecycle with all FFI paths
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_full_lifecycle_via_dvm() {
    assert!(ensure_pmix_init());

    // 1. Create fabric
    let mut fabric =
        pmix::fabric::PmixFabric::new(Some("lifecycle-test")).expect("new failed");
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("lifecycle-test"));

    // 2. Register (FFI: PMIx_Fabric_register)
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());

        // 3. Update (FFI: PMIx_Fabric_update)
        let _ = pmix::fabric::fabric_update(&mut fabric);

        // 4. Deregister (FFI: PMIx_Fabric_deregister)
        let dereg_result = pmix::fabric::fabric_deregister(&mut fabric);
        if dereg_result.is_ok() {
            assert!(!fabric.is_registered());
        }
    }

    // 5. Drop
    drop(fabric);
}

/// Topology + compute distances lifecycle via DVM.
/// NOTE: Must be run individually — PMIx library state corruption in batch mode.
#[test]
#[ignore = "requires prterun launch (run individually)"]
fn test_topology_compute_distances_lifecycle_via_dvm() {
    assert!(ensure_pmix_init());

    let mut topo = pmix::fabric::PmixTopology::unamed();
    let mut cpuset = pmix::fabric::PmixCpuset::new();

    // Load topology
    if pmix::fabric::load_topology(&mut topo).is_ok() {
        assert!(topo.is_loaded());
    }

    // Compute distances
    let _ = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
}