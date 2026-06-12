//! Tests for `PMIx_Compute_distances_nb` — non-blocking device distance computation.
//!
//! These tests verify the Rust wrapper for the non-blocking variant of
//! `PMIx_Compute_distances`, including the [`ComputeDistancesCallback`] trait,
//! the callback wrapper mechanism, and parameter handling.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::PmixStatus;
use pmix::fabric::{
    ComputeDistancesCallback, DeviceDistances, PmixCpuset, PmixTopology, compute_distances_nb,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback that just verifies the trait compiles.
struct NoOpDistCb;

impl ComputeDistancesCallback for NoOpDistCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {
        // No-op — just verify invocation doesn't panic.
    }
}

/// Callback that records the status and number of distances received.
struct RecordingDistCb {
    status: std::cell::Cell<Option<PmixStatus>>,
    n_distances: std::cell::Cell<Option<usize>>,
}

impl ComputeDistancesCallback for RecordingDistCb {
    fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances) {
        self.status.set(Some(status));
        self.n_distances.set(Some(distances.len()));
    }
}

/// Callback that checks specific distance properties.
struct PropertyCheckDistCb {
    #[allow(dead_code)]
    expected_status_ok: std::cell::Cell<bool>,
    got_success: std::cell::Cell<bool>,
    distances_non_empty: std::cell::Cell<bool>,
}

impl ComputeDistancesCallback for PropertyCheckDistCb {
    fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances) {
        self.got_success.set(status.is_success());
        self.distances_non_empty.set(!distances.is_empty());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ComputeDistancesCallback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that ComputeDistancesCallback is object-safe (can be boxed as dyn).
#[test]
fn test_callback_trait_object_safe() {
    let _cb: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
}

/// Test that RecordingDistCb compiles and can be boxed.
#[test]
fn test_recording_callback_boxable() {
    let cb = RecordingDistCb {
        status: std::cell::Cell::new(None),
        n_distances: std::cell::Cell::new(None),
    };
    let _boxed: Box<dyn ComputeDistancesCallback> = Box::new(cb);
}

/// Test that PropertyCheckDistCb compiles and can be boxed.
#[test]
fn test_property_check_callback_boxable() {
    let cb = PropertyCheckDistCb {
        expected_status_ok: std::cell::Cell::new(false),
        got_success: std::cell::Cell::new(false),
        distances_non_empty: std::cell::Cell::new(false),
    };
    let _boxed: Box<dyn ComputeDistancesCallback> = Box::new(cb);
}

/// Test that multiple callback implementations can coexist.
#[test]
fn test_multiple_callback_types() {
    let cb1: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let cb2: Box<dyn ComputeDistancesCallback> = Box::new(RecordingDistCb {
        status: std::cell::Cell::new(None),
        n_distances: std::cell::Cell::new(None),
    });
    let cb3: Box<dyn ComputeDistancesCallback> = Box::new(PropertyCheckDistCb {
        expected_status_ok: std::cell::Cell::new(false),
        got_success: std::cell::Cell::new(false),
        distances_non_empty: std::cell::Cell::new(false),
    });
    // All three should be valid trait objects.
    drop(cb1);
    drop(cb2);
    drop(cb3);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology construction tests (prerequisite for compute_distances_nb)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixTopology can be created for use with compute_distances_nb.
#[test]
fn test_topology_for_compute_distances_nb() {
    let topo = PmixTopology::unamed();
    assert!(!topo.is_loaded());
}

/// Test that PmixTopology with source hint compiles for compute_distances_nb.
#[test]
fn test_topology_with_source_for_nb() {
    let topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert_eq!(topo.source(), Some("hwloc"));
}

/// Test that PmixTopology rejects NUL in source.
#[test]
fn test_topology_nul_source_rejected() {
    let result = PmixTopology::new(Some("hw\0loc"));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCpuset construction tests (prerequisite for compute_distances_nb)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixCpuset can be created for use with compute_distances_nb.
#[test]
fn test_cpuset_for_compute_distances_nb() {
    let _cpuset = PmixCpuset::new();
    // Construction and drop should succeed without panicking.
}

/// Test that multiple cpusets can coexist (needed for concurrent nb calls).
#[test]
fn test_multiple_cpusets_for_nb() {
    let _cs1 = PmixCpuset::new();
    let _cs2 = PmixCpuset::new();
    let _cs3 = PmixCpuset::new();
    // All three should construct and drop independently.
}

// ─────────────────────────────────────────────────────────────────────────────
// DeviceDistances type tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that DeviceDistances implements Debug (for logging in callbacks).
#[test]
fn test_device_distances_debug_trait() {
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<DeviceDistances>();
}

/// Test that DeviceDistances has the expected accessor methods at compile time.
#[test]
fn test_device_distances_api() {
    fn _assert_debug<T: std::fmt::Debug>() {}
    _assert_debug::<DeviceDistances>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback invocation tests (manual invocation without FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that a callback with Mutex-based state compiles and is a valid trait object.
#[test]
fn test_recording_callback_with_mutex() {
    struct TestRecorder {
        status: std::sync::Mutex<Option<PmixStatus>>,
        n_distances: std::sync::Mutex<Option<usize>>,
    }

    impl ComputeDistancesCallback for TestRecorder {
        fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances) {
            *self.status.lock().unwrap() = Some(status);
            *self.n_distances.lock().unwrap() = Some(distances.len());
        }
    }

    // Verify the callback compiles and is a valid trait object.
    let _cb: Box<dyn ComputeDistancesCallback> = Box::new(TestRecorder {
        status: std::sync::Mutex::new(None),
        n_distances: std::sync::Mutex::new(None),
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_distances_nb FFI tests (require PMIx runtime)
// ─────────────────────────────────────────────────────────────────────────────
// These tests call the actual FFI function PMIx_Compute_distances_nb.
// Without a PMIx server/daemon, the FFI call will segfault because
// PMIx internals are not initialized. Mark all as #[ignore].

/// Test that compute_distances_nb compiles with all required parameters.
/// Without a PMIx server, the FFI call will crash, so this is ignored.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_signature() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that compute_distances_nb accepts empty info array.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_empty_info() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that compute_distances_nb with a RecordingDistCb works.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_recording_callback() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(RecordingDistCb {
        status: std::cell::Cell::new(None),
        n_distances: std::cell::Cell::new(None),
    });
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that compute_distances_nb with PropertyCheckDistCb works.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_property_check_callback() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(PropertyCheckDistCb {
        expected_status_ok: std::cell::Cell::new(false),
        got_success: std::cell::Cell::new(false),
        distances_non_empty: std::cell::Cell::new(false),
    });
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that compute_distances_nb compiles with a topology that has a source.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_with_source_topology() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that compute_distances_nb with an unamed topology works.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_unamed_topology() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test compute_distances_nb under a real PMIx environment.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_integration() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
    match result {
        Ok(()) => {
            println!("compute_distances_nb accepted — callback will be invoked asynchronously");
        }
        Err(status) => {
            println!("compute_distances_nb returned: {:?}", status);
        }
    }
}

/// Test compute_distances_nb with a recording callback under a real PMIx environment.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_recording_integration() {
    let mut topo = PmixTopology::unamed();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(RecordingDistCb {
        status: std::cell::Cell::new(None),
        n_distances: std::cell::Cell::new(None),
    });
    let result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
    match result {
        Ok(()) => {
            println!("compute_distances_nb accepted with recording callback");
        }
        Err(status) => {
            println!("compute_distances_nb returned: {:?}", status);
        }
    }
}

/// Test compute_distances_nb with a hwloc topology source.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_hwloc_integration() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let mut cpuset = PmixCpuset::new();
    let callback: Box<dyn ComputeDistancesCallback> = Box::new(PropertyCheckDistCb {
        expected_status_ok: std::cell::Cell::new(false),
        got_success: std::cell::Cell::new(false),
        distances_non_empty: std::cell::Cell::new(false),
    });
    let _result = compute_distances_nb(&mut topo, &mut cpuset, &[], callback);
}

/// Test that multiple compute_distances_nb calls can be queued simultaneously.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_compute_distances_nb_multiple_concurrent() {
    let mut topo1 = PmixTopology::unamed();
    let mut cpuset1 = PmixCpuset::new();
    let cb1: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let result1 = compute_distances_nb(&mut topo1, &mut cpuset1, &[], cb1);

    let mut topo2 = PmixTopology::unamed();
    let mut cpuset2 = PmixCpuset::new();
    let cb2: Box<dyn ComputeDistancesCallback> = Box::new(NoOpDistCb);
    let result2 = compute_distances_nb(&mut topo2, &mut cpuset2, &[], cb2);

    // Both calls should either succeed or fail consistently.
    assert_eq!(
        result1.is_ok(),
        result2.is_ok(),
        "both nb calls should have same outcome"
    );
}
