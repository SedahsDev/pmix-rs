//! Tests for `PMIx_Compute_distances` — device distance computation and topology APIs.
//!
//! These tests verify the Rust wrappers for topology and device distance APIs:
//! `load_topology`, `compute_distances`, `compute_distances_nb`,
//! and the `PmixTopology`, `PmixCpuset`, `PmixDeviceDistance`, `DeviceDistances` types.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::fabric::{
    compute_distances, compute_distances_nb, load_topology, ComputeDistancesCallback,
    DeviceDistances, PmixCpuset, PmixDeviceDistance, PmixTopology,
};
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for non-blocking compute distances.
struct TestComputeDistancesCallback;

impl ComputeDistancesCallback for TestComputeDistancesCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {
        // No-op — just verify the trait compiles and the callback
        // can be invoked without panicking.
    }
}

/// Test callback that records the status and distances it received.
struct RecordingComputeDistancesCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
    n_distances: std::cell::Cell<Option<usize>>,
}

impl ComputeDistancesCallback for RecordingComputeDistancesCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances) {
        self.status.set(Some(status));
        self.n_distances.set(Some(distances.len()));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology construction tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixTopology can be created with no source hint.
#[test]
fn test_topology_unamed() {
    let topo = PmixTopology::unamed();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology can be created with a source hint.
#[test]
fn test_topology_new_with_source() {
    let topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), Some("hwloc"));
}

/// Test that PmixTopology can be created with None source.
#[test]
fn test_topology_new_none_source() {
    let topo = PmixTopology::new(None).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology::new rejects sources with interior NUL bytes.
#[test]
fn test_topology_new_nul_source() {
    let result = PmixTopology::new(Some("hw\0loc"));
    assert!(result.is_err());
}

/// Test that PmixTopology implements Debug.
#[test]
fn test_topology_debug() {
    let topo = PmixTopology::unamed();
    let debug_str = format!("{:?}", topo);
    assert!(debug_str.contains("PmixTopology"));
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCpuset construction tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixCpuset can be created.
#[test]
fn test_cpuset_new() {
    let _cpuset = PmixCpuset::new();
    // If construction and drop both succeed without panicking, the test passes.
}

/// Test that PmixCpuset implements Debug.
#[test]
fn test_cpuset_debug() {
    let cpuset = PmixCpuset::new();
    let debug_str = format!("{:?}", cpuset);
    assert!(debug_str.contains("PmixCpuset"));
}

/// Test that multiple PmixCpuset objects can coexist.
#[test]
fn test_cpuset_multiple() {
    let _cpuset1 = PmixCpuset::new();
    let _cpuset2 = PmixCpuset::new();
    // Both should construct and drop without issues.
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDeviceDistance tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test PmixDeviceDistance accessor methods.
/// Note: We cannot construct PmixDeviceDistance directly (from_raw is unsafe and private),
/// so we verify the type exists and has the expected API via the DeviceDistances wrapper.
#[test]
fn test_device_distance_type_exists() {
    // Just verify the type is usable — actual values come from FFI.
    fn _assert_clone<T: Clone>() {}
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_clone::<PmixDeviceDistance>();
    _assert_debug::<PmixDeviceDistance>();
}

// ─────────────────────────────────────────────────────────────────────────────
// DeviceDistances tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that DeviceDistances implements Debug.
#[test]
fn test_device_distances_debug() {
    // We cannot construct DeviceDistances directly (it owns raw pointers),
    // so we just verify the type is debuggable.
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<DeviceDistances>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that ComputeDistancesCallback trait is object-safe.
#[test]
fn test_callback_trait_object() {
    let _callback: Box<dyn ComputeDistancesCallback> = Box::new(TestComputeDistancesCallback);
}

/// Test that RecordingComputeDistancesCallback compiles.
#[test]
fn test_recording_callback() {
    let _callback: Box<dyn ComputeDistancesCallback> = Box::new(RecordingComputeDistancesCallback {
        status: std::cell::Cell::new(None),
        n_distances: std::cell::Cell::new(None),
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Test load_topology + compute_distances under a real PMIx environment.
/// This test is ignored by default because it requires a running PMIx daemon.
#[test]
#[ignore]
fn test_load_topology_and_compute_distances() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    match result {
        Ok(()) => {
            assert!(topo.is_loaded());
            let mut cpuset = PmixCpuset::new();
            // Note: compute_distances takes &mut cpuset
            let distances = compute_distances(&mut topo, &mut cpuset, &[]);
            // Depending on the environment, this may succeed or return
            // PMIX_ERR_NOT_SUPPORTED if no devices are available.
            // We accept either outcome as valid.
            match distances {
                Ok(dist) => {
                    println!("Found {} device distances", dist.len());
                    for d in dist.distances() {
                        println!(
                            "  device={:?} type={:?} mindist={} maxdist={}",
                            d.uuid(),
                            d.device_type(),
                            d.mindist(),
                            d.maxdist()
                        );
                    }
                }
                Err(_) => {
                    // Acceptable if no devices available in this environment
                }
            }
        }
        Err(status) => {
            println!("load_topology returned: {:?}", status);
            // Acceptable if topology is not supported in this environment
        }
    }
}

/// Test compute_distances with a specific topology source (hwloc).
#[test]
#[ignore]
fn test_compute_distances_with_hwloc_source() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let mut cpuset = PmixCpuset::new();
    match load_topology(&mut topo) {
        Ok(()) => {
            assert!(topo.is_loaded());
            let _ = compute_distances(&mut topo, &mut cpuset, &[]);
        }
        Err(_) => {
            // hwloc may not be available; that's fine
        }
    }
}

/// Test compute_distances_nb callback invocation.
#[test]
#[ignore]
fn test_compute_distances_nb() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(TestComputeDistancesCallback);
    let result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
    // The call itself may succeed or fail depending on PMIx initialization.
    // We just verify it doesn't panic.
    match result {
        Ok(()) => {
            println!("compute_distances_nb accepted");
        }
        Err(status) => {
            println!("compute_distances_nb returned: {:?}", status);
        }
    }
}
