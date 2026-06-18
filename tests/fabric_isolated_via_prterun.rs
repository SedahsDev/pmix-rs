//! Fabric tests that MUST be run individually under prterun.
//!
//! These tests involve NB callbacks or topology operations that corrupt
//! PMIx library state when run in batch. Each test must be invoked
//! separately to avoid state corruption from preceding tests.
//!
//! Run each test individually:
//! ```bash
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_fabric_register_nb_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_fabric_update_nb_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_fabric_deregister_nb_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_load_topology_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_compute_distances_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_compute_distances_nb_via_dvm -- --include-ignored
//! prterun -np 1 cargo test --test fabric_isolated_via_prterun test_topology_compute_distances_lifecycle_via_dvm -- --include-ignored
//! ```

use std::sync::OnceLock;

static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

fn ensure_pmix_init() -> bool {
    if std::env::var("PMIX_RANK").is_err() {
        return false;
    }
    PMIX_CONTEXT.set(pmix::init(None).ok()).is_ok() && PMIX_CONTEXT.get().unwrap().is_some()
}

// ─── NB callback tests (async callback corrupts PMIx state) ───

/// fabric_register_nb via DVM.
#[test]
#[ignore = "run-individually under prterun"]
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
#[test]
#[ignore = "run-individually under prterun"]
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
#[test]
#[ignore = "run-individually under prterun"]
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

// ─── Topology tests (PMIx state corruption in batch) ───

/// load_topology via DVM.
#[test]
#[ignore = "run-individually under prterun"]
fn test_load_topology_via_dvm() {
    assert!(ensure_pmix_init());
    let mut topo = pmix::fabric::PmixTopology::unamed();
    let _ = pmix::fabric::load_topology(&mut topo);
}

/// compute_distances via DVM.
#[test]
#[ignore = "run-individually under prterun"]
fn test_compute_distances_via_dvm() {
    assert!(ensure_pmix_init());
    let mut topo = pmix::fabric::PmixTopology::unamed();
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    if pmix::fabric::load_topology(&mut topo).is_ok() {
        let _ = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
    }
}

/// compute_distances_nb via DVM.
#[test]
#[ignore = "run-individually under prterun"]
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

/// Full topology + compute distances lifecycle via DVM.
#[test]
#[ignore = "run-individually under prterun"]
fn test_topology_compute_distances_lifecycle_via_dvm() {
    assert!(ensure_pmix_init());
    let mut topo = pmix::fabric::PmixTopology::unamed();
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    if pmix::fabric::load_topology(&mut topo).is_ok() {
        assert!(topo.is_loaded());
    }
    let _ = pmix::fabric::compute_distances(&mut topo, &mut cpuset, &[]);
}
