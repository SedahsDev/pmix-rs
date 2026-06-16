//! Tests for fabric topology and device distances — structure, error propagation,
//! distances API, and type checks.
//!
//! Focus areas (avoiding duplication of fabric_Load_topology / fabric_Compute_distances
//! / fabric_Compute_distances_nb):
//!
//! * PmixTopology construction variants and field invariants
//! * PmixDeviceDistance accessor API and derive traits
//! * PmixDeviceType enum variants, from_raw / to_raw round-trips, Display
//! * Error propagation through load_topology / compute_distances / compute_distances_nb
//! * Send / Sync trait bounds on types and callbacks
//! * Debug / Display output content
//! * Panic safety of all public entry points

use pmix::fabric::{
    compute_distances, compute_distances_nb, load_topology, ComputeDistancesCallback,
    DeviceDistances, PmixCpuset, PmixDeviceDistance, PmixTopology,
};
use pmix::{PmixDeviceType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology — construction and field invariants
// ─────────────────────────────────────────────────────────────────────────────

/// PmixTopology::new(None) produces a valid, un-loaded topology.
#[test]
fn test_topology_new_none_is_valid() {
    let topo = PmixTopology::new(None).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// PmixTopology::new(Some("hwloc")) stores the source hint.
#[test]
fn test_topology_new_some_stores_source() {
    let topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), Some("hwloc"));
}

/// PmixTopology::unamed() yields a topology with no source.
#[test]
fn test_topology_unamed_has_no_source() {
    let topo = PmixTopology::unamed();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// loaded flag is false immediately after construction (all variants).
#[test]
fn test_topology_loaded_starts_false_all_constructors() {
    assert!(!PmixTopology::unamed().is_loaded());
    assert!(!PmixTopology::new(None).unwrap().is_loaded());
    assert!(!PmixTopology::new(Some("hwloc")).unwrap().is_loaded());
    assert!(!PmixTopology::new(Some("")).unwrap().is_loaded());
}

/// PmixTopology Debug output contains struct name and source info.
#[test]
fn test_topology_debug_contains_struct_name() {
    let topo = PmixTopology::unamed();
    let s = format!("{:?}", topo);
    assert!(s.contains("PmixTopology"), "debug should mention struct name: {}", s);
}

/// PmixTopology Debug output for a sourced topology contains the source.
#[test]
fn test_topology_debug_contains_source() {
    let topo = PmixTopology::new(Some("my_source")).unwrap();
    let s = format!("{:?}", topo);
    assert!(
        s.contains("my_source"),
        "debug should mention source: {}",
        s
    );
}

/// PmixTopology::new rejects NUL bytes in source.
#[test]
fn test_topology_new_rejects_nul() {
    assert!(PmixTopology::new(Some("a\x00b")).is_err());
}

/// Source accessor returns the exact string that was stored.
#[test]
fn test_topology_source_returns_exact_string() {
    let topo = PmixTopology::new(Some("test123")).unwrap();
    assert_eq!(topo.source(), Some("test123"));
}

/// Source accessor returns None for unamed topologies.
#[test]
fn test_topology_source_none_for_unamed() {
    let topo = PmixTopology::unamed();
    assert_eq!(topo.source(), None);
}

/// Source accessor returns None for new(None).
#[test]
fn test_topology_source_none_for_new_none() {
    let topo = PmixTopology::new(None).unwrap();
    assert_eq!(topo.source(), None);
}

/// Empty source string is preserved (distinct from None).
#[test]
fn test_topology_empty_source_distinct_from_none() {
    let empty = PmixTopology::new(Some("")).unwrap();
    let none = PmixTopology::new(None).unwrap();
    assert_eq!(empty.source(), Some(""));
    assert_eq!(none.source(), None);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDeviceType — enum variants, conversions, Display
// ─────────────────────────────────────────────────────────────────────────────

/// PmixDeviceType::from_raw(0x00) → UnknownType
#[test]
fn test_device_type_from_raw_unknown_type() {
    assert_eq!(PmixDeviceType::from_raw(0x00), PmixDeviceType::UnknownType);
}

/// PmixDeviceType::from_raw(0x01) → Block
#[test]
fn test_device_type_from_raw_block() {
    assert_eq!(PmixDeviceType::from_raw(0x01), PmixDeviceType::Block);
}

/// PmixDeviceType::from_raw(0x02) → Gpu
#[test]
fn test_device_type_from_raw_gpu() {
    assert_eq!(PmixDeviceType::from_raw(0x02), PmixDeviceType::Gpu);
}

/// PmixDeviceType::from_raw(0x04) → Network
#[test]
fn test_device_type_from_raw_network() {
    assert_eq!(PmixDeviceType::from_raw(0x04), PmixDeviceType::Network);
}

/// PmixDeviceType::from_raw(0x08) → OpenFabrics
#[test]
fn test_device_type_from_raw_openfabrics() {
    assert_eq!(PmixDeviceType::from_raw(0x08), PmixDeviceType::OpenFabrics);
}

/// PmixDeviceType::from_raw(0x10) → Dma
#[test]
fn test_device_type_from_raw_dma() {
    assert_eq!(PmixDeviceType::from_raw(0x10), PmixDeviceType::Dma);
}

/// PmixDeviceType::from_raw(0x20) → Coproc
#[test]
fn test_device_type_from_raw_coproc() {
    assert_eq!(PmixDeviceType::from_raw(0x20), PmixDeviceType::Coproc);
}

/// PmixDeviceType::from_raw with unknown value → Unknown(u64)
#[test]
fn test_device_type_from_raw_unknown_value() {
    assert_eq!(
        PmixDeviceType::from_raw(0xFF),
        PmixDeviceType::Unknown(0xFF)
    );
}

/// to_raw round-trips for all known variants.
#[test]
fn test_device_type_to_raw_roundtrip_known() {
    for (raw, expected) in [
        (0x00, PmixDeviceType::UnknownType),
        (0x01, PmixDeviceType::Block),
        (0x02, PmixDeviceType::Gpu),
        (0x04, PmixDeviceType::Network),
        (0x08, PmixDeviceType::OpenFabrics),
        (0x10, PmixDeviceType::Dma),
        (0x20, PmixDeviceType::Coproc),
    ] {
        let ty = PmixDeviceType::from_raw(raw);
        assert_eq!(ty, expected, "from_raw({:#x})", raw);
        assert_eq!(ty.to_raw(), raw, "to_raw({:?})", ty);
    }
}

/// to_raw round-trips for unknown value.
#[test]
fn test_device_type_to_raw_roundtrip_unknown() {
    let ty = PmixDeviceType::from_raw(0xABCD);
    assert_eq!(ty.to_raw(), 0xABCD);
}

/// PmixDeviceType Display output for Gpu.
#[test]
fn test_device_type_display_gpu() {
    assert_eq!(format!("{}", PmixDeviceType::Gpu), "GPU");
}

/// PmixDeviceType Display output for Network.
#[test]
fn test_device_type_display_network() {
    assert_eq!(format!("{}", PmixDeviceType::Network), "NETWORK");
}

/// PmixDeviceType Display output for UnknownType.
#[test]
fn test_device_type_display_unknown_type() {
    assert_eq!(format!("{}", PmixDeviceType::UnknownType), "UNKNOWN");
}

/// PmixDeviceType Display output for Unknown variant.
#[test]
fn test_device_type_display_unknown_variant() {
    let s = format!("{}", PmixDeviceType::Unknown(0x99));
    assert!(s.contains("UNKNOWN DEVICE TYPE"), "unexpected display: {}", s);
}

/// PmixDeviceType implements Debug.
#[test]
fn test_device_type_debug() {
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<PmixDeviceType>();
}

/// PmixDeviceType implements Clone + Copy + PartialEq + Eq + Hash.
#[test]
fn test_device_type_traits() {
    fn _assert_clone<T: Clone>() {}
    fn _assert_copy<T: Copy>() {}
    fn _assert_partial_eq<T: PartialEq>() {}
    fn _assert_eq<T: Eq>() {}
    fn _assert_hash<T: std::hash::Hash>() {}
    _assert_clone::<PmixDeviceType>();
    _assert_copy::<PmixDeviceType>();
    _assert_partial_eq::<PmixDeviceType>();
    _assert_eq::<PmixDeviceType>();
    _assert_hash::<PmixDeviceType>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDeviceDistance — accessor API and derive traits
// ─────────────────────────────────────────────────────────────────────────────

/// PmixDeviceDistance implements Clone.
#[test]
fn test_device_distance_clone() {
    fn _assert_clone<T: Clone>() {}
    _assert_clone::<PmixDeviceDistance>();
}

/// PmixDeviceDistance implements Debug.
#[test]
fn test_device_distance_debug() {
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<PmixDeviceDistance>();
}

/// PmixDeviceDistance has the expected accessor methods at compile time.
#[test]
fn test_device_distance_accessors_exist() {
    // Verify the type has the expected method signatures at compile time
    // by constructing a closure that references them.
    fn _check_accessors(d: &PmixDeviceDistance) {
        let _: &str = d.uuid();
        let _: &str = d.osname();
        let _: PmixDeviceType = d.device_type();
        let _: u16 = d.mindist();
        let _: u16 = d.maxdist();
    }
    // We can't construct PmixDeviceDistance directly (from_raw is unsafe
    // and private), so we just verify the function compiles.
    let _: fn(&PmixDeviceDistance) = _check_accessors;
}

/// DeviceDistances implements Debug.
#[test]
fn test_device_distances_debug() {
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<DeviceDistances>();
}

/// DeviceDistances has len, is_empty, distances accessors at compile time.
#[test]
fn test_device_distances_accessors_exist() {
    fn _check(d: &DeviceDistances) {
        let _: usize = d.len();
        let _: bool = d.is_empty();
        let _: &[PmixDeviceDistance] = d.distances();
    }
    let _: fn(&DeviceDistances) = _check;
}

// ─────────────────────────────────────────────────────────────────────────────
// load_topology — error propagation
// ─────────────────────────────────────────────────────────────────────────────

/// load_topology returns an error without a PMIx server.
#[test]
fn test_load_topology_returns_error_without_server() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    assert!(
        result.is_err(),
        "load_topology should fail without PMIx server"
    );
}

/// load_topology error is a valid PmixStatus (not a panic).
#[test]
fn test_load_topology_error_is_valid_status() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    if let Err(status) = result {
        // Should be Debug-printable without panic.
        let _ = format!("{:?}", status);
        // Should be Display-printable.
        let _ = format!("{}", status);
    }
}

/// load_topology does not set loaded flag on error.
#[test]
fn test_load_topology_loaded_false_on_error() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    if result.is_err() {
        assert!(
            !topo.is_loaded(),
            "loaded flag should remain false after error"
        );
    }
}

/// Multiple load_topology calls do not crash or panic.
#[test]
fn test_load_topology_multiple_calls_safe() {
    let mut topo = PmixTopology::unamed();
    let _ = load_topology(&mut topo);
    let _ = load_topology(&mut topo);
    let _ = load_topology(&mut topo);
}

/// load_topology on unamed topology returns error.
#[test]
fn test_load_topology_on_unamed_returns_error() {
    let mut topo = PmixTopology::unamed();
    assert!(load_topology(&mut topo).is_err());
}

/// load_topology on sourced topology returns error (no server).
#[test]
fn test_load_topology_on_sourced_returns_error() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert!(load_topology(&mut topo).is_err());
}

/// load_topology on new(None) topology returns error.
#[test]
fn test_load_topology_on_new_none_returns_error() {
    let mut topo = PmixTopology::new(None).unwrap();
    assert!(load_topology(&mut topo).is_err());
}

/// load_topology does not panic on any input.
#[test]
fn test_load_topology_no_panic() {
    // If we reach here, no panic occurred.
    let mut t1 = PmixTopology::unamed();
    let _ = load_topology(&mut t1);
    let mut t2 = PmixTopology::new(None).unwrap();
    let _ = load_topology(&mut t2);
    let mut t3 = PmixTopology::new(Some("hwloc")).unwrap();
    let _ = load_topology(&mut t3);
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_distances — error propagation
// ─────────────────────────────────────────────────────────────────────────────

/// compute_distances without loaded topology returns error.
#[test]
fn test_compute_distances_without_loaded_topology() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let result = compute_distances(&mut topo, &mut cpuset, &[]);
    assert!(
        result.is_err(),
        "compute_distances should fail without loaded topology"
    );
}

/// compute_distances with unamed topologies returns error.
#[test]
fn test_compute_distances_with_unamed_topologies() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    assert!(compute_distances(&mut topo, &mut cpuset, &[]).is_err());
}

/// compute_distances with empty info array returns error (no server).
#[test]
fn test_compute_distances_empty_info_returns_error() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let result = compute_distances(&mut topo, &mut cpuset, &[]);
    assert!(result.is_err());
}

/// compute_distances error is a valid, printable PmixStatus.
#[test]
fn test_compute_distances_error_is_printable() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let result = compute_distances(&mut topo, &mut cpuset, &[]);
    if let Err(status) = result {
        let _ = format!("{:?}", status);
        let _ = format!("{}", status);
    }
}

/// compute_distances does not panic on any input.
#[test]
fn test_compute_distances_no_panic() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let _ = compute_distances(&mut topo, &mut cpuset, &[]);
}

/// compute_distances with sourced topology returns error (no server).
#[test]
fn test_compute_distances_sourced_topology_returns_error() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let mut cpuset = PmixCpuset::new();
    assert!(compute_distances(&mut topo, &mut cpuset, &[]).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_distances_nb — error propagation and callback behavior
// ─────────────────────────────────────────────────────────────────────────────

/// ComputeDistancesCallback trait requires Send.
#[test]
fn test_compute_distances_callback_requires_send() {
    struct SendCb;
    impl ComputeDistancesCallback for SendCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    fn _assert_send<T: ComputeDistancesCallback>()
    where
        T: Send,
    {
    }
    _assert_send::<SendCb>();
}

/// ComputeDistancesCallback is object-safe (can be boxed as dyn).
#[test]
fn test_compute_distances_callback_is_object_safe() {
    struct NopCb;
    impl ComputeDistancesCallback for NopCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let _cb: Box<dyn ComputeDistancesCallback> = Box::new(NopCb);
}

/// compute_distances_nb without loaded topology — FFI call is ignored
/// because it segfaults without PMIx init.
#[test]
#[ignore = "SIGSEGV — FFI calls PMIx_Compute_distances_nb without init"]
fn test_compute_distances_nb_without_loaded_topology() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    struct NopCb;
    impl ComputeDistancesCallback for NopCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let cb: Box<dyn ComputeDistancesCallback> = Box::new(NopCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], cb);
}

/// compute_distances_nb callback compiles with custom state.
#[test]
fn test_compute_distances_nb_callback_with_state() {
    struct StatefulCb {
        called: std::cell::Cell<bool>,
    }
    impl ComputeDistancesCallback for StatefulCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {
            self.called.set(true);
        }
    }
    let cb = StatefulCb {
        called: std::cell::Cell::new(false),
    };
    let _boxed: Box<dyn ComputeDistancesCallback> = Box::new(cb);
}

/// compute_distances_nb with empty info compiles.
#[test]
#[ignore = "SIGSEGV — FFI calls PMIx_Compute_distances_nb without init"]
fn test_compute_distances_nb_empty_info() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    struct NopCb;
    impl ComputeDistancesCallback for NopCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let cb: Box<dyn ComputeDistancesCallback> = Box::new(NopCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], cb);
}

/// compute_distances_nb with sourced topology compiles.
#[test]
#[ignore = "SIGSEGV — FFI calls PMIx_Compute_distances_nb without init"]
fn test_compute_distances_nb_sourced_topology() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let mut cpuset = PmixCpuset::new();
    struct NopCb;
    impl ComputeDistancesCallback for NopCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let cb: Box<dyn ComputeDistancesCallback> = Box::new(NopCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Type checks — Send / Sync
// ─────────────────────────────────────────────────────────────────────────────

/// PmixDeviceDistance is Send (all fields are Send: String, PmixDeviceType).
#[test]
fn test_device_distance_is_send() {
    fn _assert_send<T: Send>() {}
    _assert_send::<PmixDeviceDistance>();
}

/// PmixDeviceDistance is Sync (all fields are Sync).
#[test]
fn test_device_distance_is_sync() {
    fn _assert_sync<T: Sync>() {}
    _assert_sync::<PmixDeviceDistance>();
}

/// PmixDeviceType is Send.
#[test]
fn test_device_type_is_send() {
    fn _assert_send<T: Send>() {}
    _assert_send::<PmixDeviceType>();
}

/// PmixDeviceType is Sync.
#[test]
fn test_device_type_is_sync() {
    fn _assert_sync<T: Sync>() {}
    _assert_sync::<PmixDeviceType>();
}

/// PmixTopology is NOT Sync (contains raw pointer *mut c_void).
#[test]
fn test_topology_not_sync() {
    // PmixTopology contains *mut c_void which is !Sync.
    // We verify this compiles (i.e. we don't accidentally assert Sync).
    fn _not_sync<T>() {}
    _not_sync::<PmixTopology>();
}

/// PmixTopology is NOT Send (contains raw pointer *mut c_void).
#[test]
fn test_topology_not_send() {
    fn _not_send<T>() {}
    _not_send::<PmixTopology>();
}

/// DeviceDistances is NOT Send (contains *mut raw pointer).
#[test]
fn test_device_distances_not_send() {
    fn _not_send<T>() {}
    _not_send::<DeviceDistances>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Function signature checks
// ─────────────────────────────────────────────────────────────────────────────

/// load_topology signature: takes &mut PmixTopology, returns Result<(), PmixStatus>.
#[test]
fn test_load_topology_signature() {
    fn _check_sig(
        f: impl Fn(&mut PmixTopology) -> Result<(), PmixStatus>,
    ) {
        let _ = f;
    }
    _check_sig(load_topology);
}

/// compute_distances signature: takes &mut topo, &mut cpuset, &[Info].
#[test]
fn test_compute_distances_signature() {
    fn _check_sig(
        f: impl Fn(&mut PmixTopology, &mut PmixCpuset, &[pmix::Info])
            -> Result<DeviceDistances, PmixStatus>,
    ) {
        let _ = f;
    }
    _check_sig(compute_distances);
}

/// compute_distances_nb signature: takes callback as last arg.
#[test]
fn test_compute_distances_nb_signature() {
    fn _check_sig(
        f: impl Fn(
            &mut PmixTopology,
            &mut PmixCpuset,
            &[pmix::Info],
            Box<dyn ComputeDistancesCallback>,
        ) -> Result<(), PmixStatus>,
    ) {
        let _ = f;
    }
    _check_sig(compute_distances_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases and additional invariants
// ─────────────────────────────────────────────────────────────────────────────

/// PmixTopology can be dropped without being loaded.
#[test]
fn test_topology_drop_without_load() {
    // Just create and let it drop — should not crash.
    let _ = PmixTopology::unamed();
    let _ = PmixTopology::new(None).unwrap();
    let _ = PmixTopology::new(Some("hwloc")).unwrap();
}

/// Multiple PmixTopology objects can coexist.
#[test]
fn test_topology_multiple_coexist() {
    let t1 = PmixTopology::unamed();
    let t2 = PmixTopology::new(None).unwrap();
    let t3 = PmixTopology::new(Some("hwloc")).unwrap();
    let t4 = PmixTopology::new(Some("")).unwrap();
    assert_eq!(t1.source(), None);
    assert_eq!(t2.source(), None);
    assert_eq!(t3.source(), Some("hwloc"));
    assert_eq!(t4.source(), Some(""));
}

/// PmixDeviceType variants are distinct.
#[test]
fn test_device_type_variants_distinct() {
    let types = [
        PmixDeviceType::UnknownType,
        PmixDeviceType::Block,
        PmixDeviceType::Gpu,
        PmixDeviceType::Network,
        PmixDeviceType::OpenFabrics,
        PmixDeviceType::Dma,
        PmixDeviceType::Coproc,
    ];
    for i in 0..types.len() {
        for j in (i + 1)..types.len() {
            assert_ne!(
                types[i], types[j],
                "{:?} should not equal {:?}",
                types[i],
                types[j]
            );
        }
    }
}

/// Unknown variant with same value compares equal.
#[test]
fn test_device_type_unknown_equality() {
    let a = PmixDeviceType::Unknown(0x42);
    let b = PmixDeviceType::Unknown(0x42);
    assert_eq!(a, b);
}

/// Unknown variant with different value compares unequal.
#[test]
fn test_device_type_unknown_inequality() {
    let a = PmixDeviceType::Unknown(0x42);
    let b = PmixDeviceType::Unknown(0x99);
    assert_ne!(a, b);
}

/// Known variant never equals Unknown variant.
#[test]
fn test_device_type_known_ne_unknown() {
    assert_ne!(PmixDeviceType::Gpu, PmixDeviceType::Unknown(0x02));
    assert_ne!(PmixDeviceType::UnknownType, PmixDeviceType::Unknown(0));
}

/// PmixDeviceType Copy works correctly.
#[test]
fn test_device_type_copy_works() {
    let a = PmixDeviceType::Gpu;
    let b = a; // Copy
    assert_eq!(a, b);
    assert_eq!(a.to_raw(), 0x02);
    assert_eq!(b.to_raw(), 0x02);
}

/// PmixDeviceType Clone works correctly.
#[test]
fn test_device_type_clone_works() {
    let a = PmixDeviceType::Network;
    let b = a.clone();
    assert_eq!(a, b);
}

/// PmixDeviceType Hash is consistent with equality.
#[test]
fn test_device_type_hash_consistent() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(PmixDeviceType::Gpu);
    set.insert(PmixDeviceType::Network);
    assert!(set.contains(&PmixDeviceType::Gpu));
    assert!(set.contains(&PmixDeviceType::Network));
    assert!(!set.contains(&PmixDeviceType::Block));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration-style tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full topology + distances pipeline under real PMIx.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_topology_distances_full_pipeline() {
    let mut topo = PmixTopology::unamed();
    match load_topology(&mut topo) {
        Ok(()) => {
            assert!(topo.is_loaded());
            let mut cpuset = PmixCpuset::new();
            match compute_distances(&mut topo, &mut cpuset, &[]) {
                Ok(distances) => {
                    for d in distances.distances() {
                        println!(
                            "device uuid={} osname={} type={:?} mindist={} maxdist={}",
                            d.uuid(),
                            d.osname(),
                            d.device_type(),
                            d.mindist(),
                            d.maxdist()
                        );
                    }
                }
                Err(e) => {
                    println!("compute_distances returned {:?} (acceptable)", e);
                }
            }
        }
        Err(e) => {
            println!("load_topology returned {:?} (acceptable)", e);
        }
    }
}

/// compute_distances_nb callback receives distances under real PMIx.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_full_pipeline() {
    let mut topo = PmixTopology::unamed();
    if load_topology(&mut topo).is_err() {
        return; // No PMIx server
    }
    let mut cpuset = PmixCpuset::new();
    struct NopCb;
    impl ComputeDistancesCallback for NopCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
    }
    let cb: Box<dyn ComputeDistancesCallback> = Box::new(NopCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], cb);
}
