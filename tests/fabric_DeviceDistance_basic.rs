//! Phase 4 Batch 3: PmixDeviceDistance and DeviceDistances Basic Operations
//!
//! Tests for PmixDeviceDistance and DeviceDistances construction, accessors,
//! Debug, and type traits. Uses test constructors — no FFI required.

use pmix::PmixDeviceType;
use pmix::fabric::{DeviceDistances, PmixDeviceDistance};

// ── PmixDeviceDistance construction tests ──

/// Test creating a PmixDeviceDistance with GPU type.
#[test]
fn test_device_distance_new_gpu() {
    let dist =
        PmixDeviceDistance::test_new("gpu-uuid-123", "/dev/nvidia0", PmixDeviceType::Gpu, 10, 20);
    assert_eq!(dist.uuid(), "gpu-uuid-123");
    assert_eq!(dist.osname(), "/dev/nvidia0");
    assert_eq!(dist.device_type(), PmixDeviceType::Gpu);
    assert_eq!(dist.mindist(), 10);
    assert_eq!(dist.maxdist(), 20);
}

/// Test creating a PmixDeviceDistance with Network type.
#[test]
fn test_device_distance_new_network() {
    let dist = PmixDeviceDistance::test_new("net-uuid-456", "eth0", PmixDeviceType::Network, 5, 15);
    assert_eq!(dist.device_type(), PmixDeviceType::Network);
    assert_eq!(dist.mindist(), 5);
    assert_eq!(dist.maxdist(), 15);
}

/// Test creating a PmixDeviceDistance with UnknownType (0x00).
#[test]
fn test_device_distance_unknown_type() {
    let dist = PmixDeviceDistance::test_new(
        "unknown-uuid",
        "unknown-dev",
        PmixDeviceType::UnknownType,
        0,
        0,
    );
    assert_eq!(dist.device_type(), PmixDeviceType::UnknownType);
    assert_eq!(dist.mindist(), 0);
    assert_eq!(dist.maxdist(), 0);
}

/// Test creating a PmixDeviceDistance with zero uuid and osname.
#[test]
fn test_device_distance_empty_strings() {
    let dist = PmixDeviceDistance::test_new("", "", PmixDeviceType::Block, 100, 200);
    assert_eq!(dist.uuid(), "");
    assert_eq!(dist.osname(), "");
    assert_eq!(dist.device_type(), PmixDeviceType::Block);
}

/// Test creating a PmixDeviceDistance with Unknown(u64) type.
#[test]
fn test_device_distance_unknown_custom() {
    let dist = PmixDeviceDistance::test_new(
        "custom-uuid",
        "custom-dev",
        PmixDeviceType::Unknown(0xFF),
        42,
        84,
    );
    assert_eq!(dist.device_type(), PmixDeviceType::Unknown(0xFF));
}

/// Test that PmixDeviceDistance implements Debug.
#[test]
fn test_device_distance_debug() {
    let dist = PmixDeviceDistance::test_new("debug-uuid", "debug-dev", PmixDeviceType::Gpu, 10, 20);
    let debug_str = format!("{:?}", dist);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixDeviceDistance"));
    assert!(debug_str.contains("debug-uuid"));
}

/// Test that PmixDeviceDistance is Clone.
#[test]
fn test_device_distance_clone() {
    let dist = PmixDeviceDistance::test_new(
        "clone-uuid",
        "clone-dev",
        PmixDeviceType::OpenFabrics,
        7,
        14,
    );
    let cloned = dist.clone();
    assert_eq!(cloned.uuid(), dist.uuid());
    assert_eq!(cloned.osname(), dist.osname());
    assert_eq!(cloned.device_type(), dist.device_type());
    assert_eq!(cloned.mindist(), dist.mindist());
    assert_eq!(cloned.maxdist(), dist.maxdist());
}

// ── Type trait tests ──

/// Test that PmixDeviceDistance is Send and Sync.
#[test]
fn test_device_distance_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<PmixDeviceDistance>();
    assert_sync::<PmixDeviceDistance>();
}

/// Test that PmixDeviceDistance can be placed behind an Arc.
#[test]
fn test_device_distance_arc() {
    use std::sync::Arc;
    let dist = PmixDeviceDistance::test_new("arc-uuid", "arc-dev", PmixDeviceType::Dma, 3, 6);
    let _arc = Arc::new(dist);
}

// ── DeviceDistances tests ──

/// Test creating an empty DeviceDistances.
#[test]
fn test_device_distances_empty() {
    let dd = DeviceDistances::test_new(Vec::new());
    assert!(dd.is_empty());
    assert_eq!(dd.len(), 0);
    assert!(dd.distances().is_empty());
}

/// Test creating DeviceDistances with entries.
#[test]
fn test_device_distances_with_entries() {
    let distances = vec![
        PmixDeviceDistance::test_new("gpu-0", "/dev/nvidia0", PmixDeviceType::Gpu, 10, 20),
        PmixDeviceDistance::test_new("gpu-1", "/dev/nvidia1", PmixDeviceType::Gpu, 15, 25),
        PmixDeviceDistance::test_new("net-0", "eth0", PmixDeviceType::Network, 5, 10),
    ];
    let dd = DeviceDistances::test_new(distances);
    assert!(!dd.is_empty());
    assert_eq!(dd.len(), 3);
    assert_eq!(dd.distances().len(), 3);
}

/// Test DeviceDistances distances() returns correct entries.
#[test]
fn test_device_distances_accessor() {
    let distances = vec![
        PmixDeviceDistance::test_new("uuid-1", "dev-1", PmixDeviceType::Gpu, 1, 2),
        PmixDeviceDistance::test_new("uuid-2", "dev-2", PmixDeviceType::Network, 3, 4),
    ];
    let dd = DeviceDistances::test_new(distances);
    let dist_slice = dd.distances();
    assert_eq!(dist_slice[0].uuid(), "uuid-1");
    assert_eq!(dist_slice[1].osname(), "dev-2");
    assert_eq!(dist_slice[0].mindist(), 1);
    assert_eq!(dist_slice[1].maxdist(), 4);
}

/// Test DeviceDistances Debug implementation.
#[test]
fn test_device_distances_debug() {
    let distances = vec![PmixDeviceDistance::test_new(
        "dbg-uuid",
        "dbg-dev",
        PmixDeviceType::Coproc,
        42,
        84,
    )];
    let dd = DeviceDistances::test_new(distances);
    let debug_str = format!("{:?}", dd);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("DeviceDistances"));
}

/// Test DeviceDistances drop with entries (raw_ptr is null, should be no-op).
#[test]
fn test_device_distances_drop() {
    let distances = vec![PmixDeviceDistance::test_new(
        "drop-uuid",
        "drop-dev",
        PmixDeviceType::Gpu,
        1,
        2,
    )];
    let _dd = DeviceDistances::test_new(distances);
}

/// Test DeviceDistances with all device types.
#[test]
fn test_device_distances_all_types() {
    let distances = vec![
        PmixDeviceDistance::test_new("", "", PmixDeviceType::UnknownType, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Block, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Gpu, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Network, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::OpenFabrics, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Dma, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Coproc, 0, 0),
        PmixDeviceDistance::test_new("", "", PmixDeviceType::Unknown(0xFF), 0, 0),
    ];
    let dd = DeviceDistances::test_new(distances);
    assert_eq!(dd.len(), 8);
}
