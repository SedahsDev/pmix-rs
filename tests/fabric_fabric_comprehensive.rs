//! Comprehensive tests for fabric.rs — PmixFabric, PmixTopology, PmixCpuset,
//! and all fabric_* public functions.
//!
//! Most fabric tests verify that operations fail gracefully without PMIx init —
//! they do NOT require a running daemon.

use pmix::fabric::{
    compute_distances, compute_distances_nb, fabric_deregister, fabric_deregister_nb,
    fabric_register, fabric_register_nb, fabric_update, fabric_update_nb, load_topology,
    ComputeDistancesCallback, FabricCallback, PmixCpuset, PmixFabric, PmixTopology,
};
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric — construction and traits (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_new_no_name() {
    let fabric = PmixFabric::new(None).expect("fabric new failed");
    assert!(format!("{:?}", fabric).contains("PmixFabric"));
}

#[test]
fn test_fabric_new_with_name() {
    let fabric = PmixFabric::new(Some("test-fabric")).expect("fabric new failed");
    assert!(format!("{:?}", fabric).contains("test-fabric"));
}

#[test]
fn test_fabric_new_nul_name() {
    assert!(PmixFabric::new(Some("test\0fabric")).is_err());
}

#[test]
fn test_fabric_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixFabric>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology — construction and traits (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_topology_new_no_source() {
    let topo = PmixTopology::new(None).expect("topology new failed");
    assert!(format!("{:?}", topo).contains("PmixTopology"));
}

#[test]
fn test_topology_new_with_source() {
    let topo = PmixTopology::new(Some("test-source")).expect("topology new failed");
    assert!(format!("{:?}", topo).contains("test-source"));
}

#[test]
fn test_topology_new_nul_source() {
    assert!(PmixTopology::new(Some("test\0source")).is_err());
}

#[test]
fn test_topology_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixTopology>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCpuset — construction and traits (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_cpuset_new() {
    let cs = PmixCpuset::new();
    assert!(format!("{:?}", cs).contains("PmixCpuset"));
}

#[test]
fn test_cpuset_default() {
    let cs: PmixCpuset = Default::default();
    assert!(format!("{:?}", cs).contains("PmixCpuset"));
}

#[test]
fn test_cpuset_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixCpuset>();
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_register — tests (no daemon needed, tests "not initialized" path)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_register_without_init() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    let info = InfoBuilder::new().build();
    let directives: &[pmix::Info] = std::slice::from_ref(&info);
    assert!(
        fabric_register(&mut fabric, directives).is_err(),
        "fabric_register should fail without init"
    );
}

#[test]
fn test_fabric_register_empty_directives() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    let directives: &[pmix::Info] = &[];
    assert!(
        fabric_register(&mut fabric, directives).is_err(),
        "fabric_register should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_register_nb — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_callback_requires_send() {
    struct TestFabricCallback;
    impl FabricCallback for TestFabricCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    fn assert_send<T: FabricCallback>()
    where
        T: Send,
    {
    }
    assert_send::<TestFabricCallback>();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_nb_without_init() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    let info = InfoBuilder::new().build();
    let directives: &[pmix::Info] = std::slice::from_ref(&info);
    struct NopFabricCb;
    impl FabricCallback for NopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let cb: Box<dyn FabricCallback> = Box::new(NopFabricCb);
    assert!(
        fabric_register_nb(&mut fabric, directives, cb).is_err(),
        "fabric_register_nb should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_update — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_update_without_registration() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    assert!(
        fabric_update(&mut fabric).is_err(),
        "fabric_update should fail without registration"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_update_nb — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_update_nb_without_registration() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    struct NopFabricCb;
    impl FabricCallback for NopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let cb: Box<dyn FabricCallback> = Box::new(NopFabricCb);
    assert!(
        fabric_update_nb(&mut fabric, cb).is_err(),
        "fabric_update_nb should fail without registration"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_deregister — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_deregister_without_registration() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    assert!(
        fabric_deregister(&mut fabric).is_err(),
        "fabric_deregister should fail without registration"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_deregister_nb — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_deregister_nb_without_registration() {
    let mut fabric = PmixFabric::new(Some("test")).expect("fabric new failed");
    struct NopFabricCb;
    impl FabricCallback for NopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let cb: Box<dyn FabricCallback> = Box::new(NopFabricCb);
    assert!(
        fabric_deregister_nb(&mut fabric, cb).is_err(),
        "fabric_deregister_nb should fail without registration"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// load_topology — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_load_topology_without_init() {
    let mut topo = PmixTopology::new(None).expect("topology new failed");
    assert!(
        load_topology(&mut topo).is_err(),
        "load_topology should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_distances — tests (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_compute_distances_callback_requires_send() {
    struct TestDistCb;
    impl ComputeDistancesCallback for TestDistCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: pmix::fabric::DeviceDistances) {}
    }
    fn assert_send<T: ComputeDistancesCallback>()
    where
        T: Send,
    {
    }
    assert_send::<TestDistCb>();
}

#[test]
fn test_compute_distances_without_init() {
    let mut topo = PmixTopology::new(None).expect("topology new failed");
    let mut cpuset = PmixCpuset::new();
    let info = InfoBuilder::new().build();
    let directives: &[pmix::Info] = std::slice::from_ref(&info);
    assert!(
        compute_distances(&mut topo, &mut cpuset, directives).is_err(),
        "compute_distances should fail without init"
    );
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_nb_without_init() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::new(None).expect("topology new failed");
    let mut cpuset = PmixCpuset::new();
    let info = InfoBuilder::new().build();
    let directives: &[pmix::Info] = std::slice::from_ref(&info);
    struct TestDistCb;
    impl ComputeDistancesCallback for TestDistCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: pmix::fabric::DeviceDistances) {}
    }
    let cb: Box<dyn ComputeDistancesCallback> = Box::new(TestDistCb);
    assert!(
        compute_distances_nb(&mut topo, &mut cpuset, directives, cb).is_err(),
        "compute_distances_nb should fail without init"
    );
}
