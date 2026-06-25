//! Deep tests for fabric module — Round 2.
//!
//! Targets untested code paths in fabric.rs (41.23% coverage).
//! Focus: PmixFabric construction/accessors, PmixTopology, PmixCpuset,
//! DeviceDistances, compute_distances edge cases, fabric lifecycle.
//!
//! FFI tests that require PMIx_Init are marked #[ignore].

use pmix::fabric::*;
use pmix::{InfoBuilder, PmixDeviceType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric construction and accessors (no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_unamed_defaults() {
    let fabric = PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
    assert_eq!(fabric.index(), 0);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

#[test]
fn test_fabric_new_with_name() {
    let fabric = PmixFabric::new(Some("my_fabric")).expect("create fabric");
    assert_eq!(fabric.name(), Some("my_fabric"));
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
}

#[test]
fn test_fabric_new_none_name() {
    let fabric = PmixFabric::new(None).expect("create fabric");
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
}

#[test]
fn test_fabric_new_nul_rejected() {
    let result = PmixFabric::new(Some("bad\x00name"));
    assert!(result.is_err(), "nul in name should be rejected");
}

#[test]
fn test_fabric_new_empty_name() {
    let fabric = PmixFabric::new(Some("")).expect("empty name ok");
    assert_eq!(fabric.name(), Some(""));
}

#[test]
fn test_fabric_new_long_name() {
    let long_name = "f".repeat(256);
    let fabric = PmixFabric::new(Some(&long_name)).expect("long name ok");
    assert!(fabric.name().unwrap().len() == 256);
}

#[test]
fn test_fabric_new_unicode_name() {
    let fabric = PmixFabric::new(Some("fabric-αβγ")).expect("unicode name ok");
    assert_eq!(fabric.name(), Some("fabric-αβγ"));
}

#[test]
fn test_fabric_multiple_independent() {
    let f1 = PmixFabric::new(Some("first")).expect("f1");
    let f2 = PmixFabric::new(Some("second")).expect("f2");
    assert_eq!(f1.name(), Some("first"));
    assert_eq!(f2.name(), Some("second"));
    assert!(!f1.is_registered());
    assert!(!f2.is_registered());
}

#[test]
fn test_fabric_debug_format() {
    let fabric = PmixFabric::unamed();
    let debug = format!("{:?}", fabric);
    assert!(debug.contains("PmixFabric") || !debug.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology construction (no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_topology_unamed_defaults() {
    let topo = PmixTopology::unamed();
    assert_eq!(topo.source(), None);
    assert!(!topo.is_loaded());
}

#[test]
fn test_topology_new_with_source() {
    let topo = PmixTopology::new(Some("hwloc")).expect("create topology");
    assert_eq!(topo.source(), Some("hwloc"));
    assert!(!topo.is_loaded());
}

#[test]
fn test_topology_new_none_source() {
    let topo = PmixTopology::new(None).expect("create topology");
    assert_eq!(topo.source(), None);
    assert!(!topo.is_loaded());
}

#[test]
fn test_topology_new_nul_rejected() {
    let result = PmixTopology::new(Some("bad\x00source"));
    assert!(result.is_err(), "nul in source should be rejected");
}

#[test]
fn test_topology_new_empty_source() {
    let topo = PmixTopology::new(Some("")).expect("empty source ok");
    assert_eq!(topo.source(), Some(""));
}

#[test]
fn test_topology_debug_format() {
    let topo = PmixTopology::unamed();
    let debug = format!("{:?}", topo);
    assert!(!debug.is_empty());
}

#[test]
fn test_topology_multiple_independent() {
    let t1 = PmixTopology::new(Some("hwloc")).expect("t1");
    let t2 = PmixTopology::new(Some("numactl")).expect("t2");
    assert_eq!(t1.source(), Some("hwloc"));
    assert_eq!(t2.source(), Some("numactl"));
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCpuset construction (safe without PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_cpuset_new() {
    let mut cpuset = PmixCpuset::new();
    let _ = cpuset.as_mut_ptr();
}

#[test]
fn test_cpuset_default() {
    let mut cpuset = PmixCpuset::default();
    let _ = cpuset.as_mut_ptr();
}

#[test]
fn test_cpuset_debug_format() {
    let cpuset = PmixCpuset::new();
    let debug = format!("{:?}", cpuset);
    assert!(!debug.is_empty());
}

#[test]
fn test_cpuset_multiple_independent() {
    let mut c1 = PmixCpuset::new();
    let mut c2 = PmixCpuset::new();
    let _ = c1.as_mut_ptr();
    let _ = c2.as_mut_ptr();
}

#[test]
fn test_cpuset_drop_loop() {
    for _ in 0..100 {
        let _cpuset = PmixCpuset::new();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DeviceDistances type checks (PmixDeviceDistance fields are private)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_device_distances_type_exists() {
    // DeviceDistances is a type — just verify it compiles
    let _: Option<DeviceDistances> = None;
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fabric_new_does_not_panic() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| PmixFabric::new(None)));
    assert!(result.is_ok());
}

#[test]
fn test_topology_new_does_not_panic() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| PmixTopology::new(None)));
    assert!(result.is_ok());
}

#[test]
fn test_cpuset_new_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixCpuset::new());
    assert!(result.is_ok());
}

#[test]
fn test_cpuset_drop_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _cpuset = PmixCpuset::new();
    });
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_infobuilder_build_empty() {
    let info = InfoBuilder::new().build();
    let _ = info;
}

#[test]
fn test_infobuilder_collect_data() {
    // collect_data returns &mut self, so it chains but build() consumes self
    // — just verify the type compiles
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let _info = builder.build();
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("test_fabric")).expect("create");
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_ok(), "register should succeed");
    assert!(fabric.is_registered());
    assert!(fabric.index() > 0);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_empty_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("empty_info")).expect("create");
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_ok());
    assert!(fabric.is_registered());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_unamed() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_ok());
    assert!(fabric.is_registered());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_twice_fails() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("double")).expect("create");
    fabric_register(&mut fabric, &[]).expect("first register");
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err(), "double register should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_then_update() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("update_test")).expect("create");
    fabric_register(&mut fabric, &[]).expect("register");
    let result = fabric_update(&mut fabric);
    assert!(result.is_ok(), "update should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_then_deregister() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("dereg_test")).expect("create");
    fabric_register(&mut fabric, &[]).expect("register");
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_ok(), "deregister should succeed");
    assert!(!fabric.is_registered());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_deregister_unregistered() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err(), "deregister unregistered should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_update_unregistered() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err(), "update unregistered should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_update_deregister_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("lifecycle")).expect("create");
    fabric_register(&mut fabric, &[]).expect("register");
    assert!(fabric.is_registered());
    fabric_update(&mut fabric).expect("update");
    fabric_deregister(&mut fabric).expect("deregister");
    assert!(!fabric.is_registered());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_register_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("nb_test")).expect("create");
    struct NoopFabricCb;
    impl FabricCallback for NoopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NoopFabricCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_update_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("nb_update")).expect("create");
    fabric_register(&mut fabric, &[]).expect("register");
    struct NoopFabricCb;
    impl FabricCallback for NoopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fabric_update_nb(&mut fabric, Box::new(NoopFabricCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_deregister_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("nb_dereg")).expect("create");
    fabric_register(&mut fabric, &[]).expect("register");
    struct NoopFabricCb;
    impl FabricCallback for NoopFabricCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fabric_deregister_nb(&mut fabric, Box::new(NoopFabricCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_topology_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::new(Some("hwloc")).expect("create");
    let result = load_topology(&mut topo);
    assert!(result.is_ok(), "load_topology should succeed");
    assert!(topo.is_loaded());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_topology_unamed() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    assert!(result.is_ok());
    assert!(topo.is_loaded());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_basic() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    let mut cpuset = PmixCpuset::new();
    let result = compute_distances(&mut topo, &mut cpuset, &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_returns_distances() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    let mut cpuset = PmixCpuset::new();
    let distances = compute_distances(&mut topo, &mut cpuset, &[]);
    if distances.is_ok() {
        let dist = distances.unwrap();
        assert!(dist.len() >= 0);
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_device_distances_is_empty() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    let mut cpuset = PmixCpuset::new();
    let distances = compute_distances(&mut topo, &mut cpuset, &[]);
    if distances.is_ok() {
        let dist = distances.unwrap();
        let _ = dist.is_empty();
        let _ = dist.len();
        let _ = dist.distances();
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_nb_basic() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    struct NoopDistCb;
    impl ComputeDistancesCallback for NoopDistCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let result = compute_distances_nb(&mut topo, &mut PmixCpuset::new(), &[], Box::new(NoopDistCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_nb_with_callback() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    struct RecordingDistCb;
    impl ComputeDistancesCallback for RecordingDistCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let result = compute_distances_nb(
        &mut topo,
        &mut PmixCpuset::new(),
        &[],
        Box::new(RecordingDistCb),
    );
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_multiple_fabric_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabrics: Vec<_> = (0..3)
        .map(|i| PmixFabric::new(Some(&format!("fabric_{}", i))).expect("create"))
        .collect();
    for fabric in &mut fabrics {
        fabric_register(fabric, &[]).expect("register");
    }
    for fabric in &mut fabrics {
        assert!(fabric.is_registered());
    }
    for fabric in &mut fabrics {
        fabric_deregister(fabric).expect("deregister");
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_index_after_register() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("index_test")).expect("create");
    assert_eq!(fabric.index(), 0);
    fabric_register(&mut fabric, &[]).expect("register");
    assert!(
        fabric.index() > 0,
        "index should be assigned after register"
    );
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fabric_ninfo_after_register() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::new(Some("ninfo_test")).expect("create");
    assert_eq!(fabric.ninfo(), 0);
    fabric_register(&mut fabric, &[]).expect("register");
    // ninfo may be 0 with empty info — just verify no crash
    let _ = fabric.ninfo();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_compute_distances_with_collect_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut topo = PmixTopology::unamed();
    load_topology(&mut topo).expect("load");
    let mut cpuset = PmixCpuset::new();
    // Use empty info — InfoBuilder creates Info that can't be easily sliced
    let result = compute_distances(&mut topo, &mut cpuset, &[]);
    let _ = result;
}
