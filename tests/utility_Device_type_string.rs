//! Integration tests for `PMIx_Device_type_string` via the safe `device_type_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Device_type_string` only looks up a static string table
//! inside the library.

use pmix::{utility::device_type_string, PmixDeviceType};

/// `device_type_string` returns "UNKNOWN" for PMIX_DEVTYPE_UNKNOWN (0x00).
#[test]
fn device_type_string_unknown_returns_ok() {
    let ty = PmixDeviceType::UnknownType;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(UNKNOWN) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "UNKNOWN");
}

/// `device_type_string` returns "BLOCK" for PMIX_DEVTYPE_BLOCK (0x01).
#[test]
fn device_type_string_block_returns_ok() {
    let ty = PmixDeviceType::Block;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(BLOCK) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "BLOCK");
}

/// `device_type_string` returns "GPU" for PMIX_DEVTYPE_GPU (0x02).
#[test]
fn device_type_string_gpu_returns_ok() {
    let ty = PmixDeviceType::Gpu;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(GPU) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "GPU");
}

/// `device_type_string` returns "NETWORK" for PMIX_DEVTYPE_NETWORK (0x04).
#[test]
fn device_type_string_network_returns_ok() {
    let ty = PmixDeviceType::Network;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(NETWORK) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "NETWORK");
}

/// `device_type_string` returns "OPENFABRICS" for PMIX_DEVTYPE_OPENFABRICS (0x08).
#[test]
fn device_type_string_openfabrics_returns_ok() {
    let ty = PmixDeviceType::OpenFabrics;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(OPENFABRICS) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "OPENFABRICS");
}

/// `device_type_string` returns "DMA" for PMIX_DEVTYPE_DMA (0x10).
#[test]
fn device_type_string_dma_returns_ok() {
    let ty = PmixDeviceType::Dma;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(DMA) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "DMA");
}

/// `device_type_string` returns "COPROCESSOR" for PMIX_DEVTYPE_COPROC (0x20).
#[test]
fn device_type_string_coproc_returns_ok() {
    let ty = PmixDeviceType::Coproc;
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string(COPROC) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "COPROCESSOR");
}

/// `device_type_string` handles all known device types without error.
#[test]
fn device_type_string_all_known() {
    let types = [
        (PmixDeviceType::UnknownType, "UNKNOWN"),
        (PmixDeviceType::Block, "BLOCK"),
        (PmixDeviceType::Gpu, "GPU"),
        (PmixDeviceType::Network, "NETWORK"),
        (PmixDeviceType::OpenFabrics, "OPENFABRICS"),
        (PmixDeviceType::Dma, "DMA"),
        (PmixDeviceType::Coproc, "COPROCESSOR"),
    ];
    for (ty, expected) in types {
        let result = device_type_string(ty);
        assert!(
            result.is_ok(),
            "device_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "device_type_string({:?}) should not return empty string",
            ty
        );
        assert_eq!(
            desc, expected,
            "device_type_string({:?}) should return '{}'",
            ty, expected
        );
    }
}

/// `device_type_string` is deterministic — the same type always
/// produces the same string description.
#[test]
fn device_type_string_is_deterministic() {
    let ty = PmixDeviceType::Gpu;
    let first = device_type_string(ty).unwrap();
    let second = device_type_string(ty).unwrap();
    assert_eq!(
        first, second,
        "device_type_string must be deterministic for the same input"
    );
}

/// `device_type_string` returns different strings for different types.
#[test]
fn device_type_string_distinct_for_different_types() {
    let gpu = device_type_string(PmixDeviceType::Gpu).unwrap();
    let network = device_type_string(PmixDeviceType::Network).unwrap();
    assert_ne!(
        gpu, network,
        "device_type_string(GPU) and device_type_string(NETWORK) must return different strings"
    );
}

/// `device_type_string` returns a `Result<String, pmix::PmixStatus>`, not a raw pointer.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn device_type_string_returns_result_string() {
    let ty = PmixDeviceType::Gpu;
    let _result: Result<String, pmix::PmixStatus> = device_type_string(ty);
}

/// `PmixDeviceType::from_raw` maps known values correctly.
#[test]
fn device_type_from_raw() {
    assert_eq!(PmixDeviceType::from_raw(0x00), PmixDeviceType::UnknownType);
    assert_eq!(PmixDeviceType::from_raw(0x01), PmixDeviceType::Block);
    assert_eq!(PmixDeviceType::from_raw(0x02), PmixDeviceType::Gpu);
    assert_eq!(PmixDeviceType::from_raw(0x04), PmixDeviceType::Network);
    assert_eq!(PmixDeviceType::from_raw(0x08), PmixDeviceType::OpenFabrics);
    assert_eq!(PmixDeviceType::from_raw(0x10), PmixDeviceType::Dma);
    assert_eq!(PmixDeviceType::from_raw(0x20), PmixDeviceType::Coproc);
}

/// `PmixDeviceType::to_raw` returns the correct C values.
#[test]
fn device_type_to_raw() {
    assert_eq!(PmixDeviceType::UnknownType.to_raw(), 0x00);
    assert_eq!(PmixDeviceType::Block.to_raw(), 0x01);
    assert_eq!(PmixDeviceType::Gpu.to_raw(), 0x02);
    assert_eq!(PmixDeviceType::Network.to_raw(), 0x04);
    assert_eq!(PmixDeviceType::OpenFabrics.to_raw(), 0x08);
    assert_eq!(PmixDeviceType::Dma.to_raw(), 0x10);
    assert_eq!(PmixDeviceType::Coproc.to_raw(), 0x20);
}

/// `PmixDeviceType::from_raw` / `to_raw` roundtrip for all values.
#[test]
fn device_type_roundtrip() {
    for raw in [0u64, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0xFF, 0xDEAD] {
        let ty = PmixDeviceType::from_raw(raw);
        assert_eq!(ty.to_raw(), raw);
    }
}

/// `PmixDeviceType::from_raw` falls back to Unknown for unrecognized values.
#[test]
fn device_type_from_raw_unknown() {
    let ty = PmixDeviceType::from_raw(0xFF);
    assert!(matches!(ty, PmixDeviceType::Unknown(0xFF)));
}

/// `device_type_string` handles unknown device type values gracefully.
#[test]
fn device_type_string_unknown_value() {
    let ty = PmixDeviceType::Unknown(0xFF);
    let result = device_type_string(ty);
    assert!(
        result.is_ok(),
        "device_type_string should handle unknown values gracefully"
    );
}
