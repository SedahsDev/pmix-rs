//! Integration tests for `PMIx_Data_type_string` via the safe `data_type_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Data_type_string` only looks up a static string table
//! inside the library.

use pmix::{utility::data_type_string, PmixDataType, PmixStatus};

/// `data_type_string` returns `Ok(String)` for basic scalar types.
#[test]
fn data_type_string_scalar_types_returns_ok() {
    let types: Vec<PmixDataType> = vec![
        PmixDataType::Undef,
        PmixDataType::Bool,
        PmixDataType::Byte,
        PmixDataType::String,
        PmixDataType::Int,
        PmixDataType::Int8,
        PmixDataType::Int16,
        PmixDataType::Int32,
        PmixDataType::Int64,
        PmixDataType::Uint,
        PmixDataType::Uint8,
        PmixDataType::Uint16,
        PmixDataType::Uint32,
        PmixDataType::Uint64,
        PmixDataType::Float,
        PmixDataType::Double,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_type_string({:?}) should not return an empty string",
            ty
        );
    }
}

/// `data_type_string` returns `Ok(String)` for composite and complex types.
#[test]
fn data_type_string_composite_types_returns_ok() {
    let types: Vec<PmixDataType> = vec![
        PmixDataType::Value,
        PmixDataType::Proc,
        PmixDataType::App,
        PmixDataType::Info,
        PmixDataType::Pdata,
        PmixDataType::ByteObject,
        PmixDataType::Kval,
        PmixDataType::DataArray,
        PmixDataType::CompressedString,
        PmixDataType::CompressedByteObject,
        PmixDataType::Envar,
        PmixDataType::Coord,
        PmixDataType::Geometry,
        PmixDataType::Topo,
        PmixDataType::Endpoint,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_type_string({:?}) should not return an empty string",
            ty
        );
    }
}

/// `data_type_string` returns `Ok(String)` for enum-type data types.
#[test]
fn data_type_string_enum_types_returns_ok() {
    let types: Vec<PmixDataType> = vec![
        PmixDataType::Status,
        PmixDataType::Persist,
        PmixDataType::Scope,
        PmixDataType::DataRange,
        PmixDataType::Command,
        PmixDataType::DataType,
        PmixDataType::ProcState,
        PmixDataType::JobState,
        PmixDataType::LinkState,
        PmixDataType::IofChannel,
        PmixDataType::AllocDirective,
        PmixDataType::Devtype,
        PmixDataType::LocType,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_type_string({:?}) should not return an empty string",
            ty
        );
    }
}

/// `data_type_string` returns `Ok(String)` for statistics and storage types.
#[test]
fn data_type_string_stats_types_returns_ok() {
    let types: Vec<PmixDataType> = vec![
        PmixDataType::ProcStats,
        PmixDataType::DiskStats,
        PmixDataType::NetStats,
        PmixDataType::NodeStats,
        PmixDataType::StorMedium,
        PmixDataType::StorAccess,
        PmixDataType::StorPersist,
        PmixDataType::StorAccessType,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_type_string({:?}) should not return an empty string",
            ty
        );
    }
}

/// `data_type_string` is deterministic — the same type always
/// produces the same string description.
#[test]
fn data_type_string_is_deterministic() {
    let first = data_type_string(PmixDataType::Int64).unwrap();
    let second = data_type_string(PmixDataType::Int64).unwrap();
    assert_eq!(
        first, second,
        "data_type_string must be deterministic for the same input"
    );
}

/// `data_type_string` returns different strings for different types.
///
/// PMIX_STRING (3) and PMIX_INT (6) must produce distinct descriptions.
#[test]
fn data_type_string_distinct_for_different_types() {
    let string_desc = data_type_string(PmixDataType::String).unwrap();
    let int_desc = data_type_string(PmixDataType::Int).unwrap();
    assert_ne!(
        string_desc, int_desc,
        "data_type_string(STRING) and data_type_string(INT) must return different strings"
    );
}

/// `data_type_string` returns a `Result<String, PmixStatus>`, not a raw pointer.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn data_type_string_returns_result_string() {
    let _result: Result<String, PmixStatus> = data_type_string(PmixDataType::Bool);
}

/// `data_type_string` handles the `Unknown` variant (raw value 70) gracefully.
///
/// Values between 70 and 499 are reserved for implementation extensions.
/// The C library should return a valid string (typically something like
/// "unknown" or a generic description).
#[test]
fn data_type_string_unknown_type_returns_ok() {
    let ty = PmixDataType::Unknown;
    let result = data_type_string(ty);
    assert!(
        result.is_ok(),
        "data_type_string(Unknown) should handle gracefully, got {:?}",
        result
    );
}

/// Verify `PmixDataType::from_raw` round-trips correctly for known types.
#[test]
fn data_type_from_raw_roundtrip() {
    let types: Vec<(u16, PmixDataType)> = vec![
        (0, PmixDataType::Undef),
        (1, PmixDataType::Bool),
        (3, PmixDataType::String),
        (6, PmixDataType::Int),
        (15, PmixDataType::Uint64),
        (17, PmixDataType::Double),
        (20, PmixDataType::Status),
        (24, PmixDataType::Info),
        (36, PmixDataType::DataType),
        (69, PmixDataType::StorAccessType),
    ];
    for (raw, expected) in &types {
        let ty = PmixDataType::from_raw(*raw);
        assert_eq!(ty, *expected, "from_raw({}) should be {:?}", raw, expected);
        assert_eq!(ty.to_raw(), *raw, "to_raw({:?}) should be {}", expected, raw);
    }
}

/// Verify that `PmixDataType::from_raw` maps unrecognized values to `Unknown`.
#[test]
fn data_type_from_raw_unknown() {
    assert!(matches!(PmixDataType::from_raw(70), PmixDataType::Unknown));
    assert!(matches!(PmixDataType::from_raw(100), PmixDataType::Unknown));
    assert!(matches!(PmixDataType::from_raw(499), PmixDataType::Unknown));
    assert!(matches!(PmixDataType::from_raw(500), PmixDataType::Unknown));
    assert!(matches!(PmixDataType::from_raw(u16::MAX), PmixDataType::Unknown));
}
