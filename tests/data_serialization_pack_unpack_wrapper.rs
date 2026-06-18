//! Tests for data_pack/data_unpack/data_copy FFI wrapper behavior.
//!
//! These tests exercise the Rust wrapper logic for pack/unpack/copy
//! without requiring PMIx_Init. Without init, FFI calls return errors,
//! but the Rust wrapper code (parameter validation, pointer handling,
//! type conversion, error propagation) is still fully exercised.
//!
//! NOTE: Passing Some(proc_ref) to data_pack/data_unpack causes SIGSEGV
//! when PMIx is not initialized. Only None (no target/source) is safe.

use pmix::data_serialization::*;
use pmix::PmixDataType;

// ─────────────────────────────────────────────────────────────────────────────
// data_pack — FFI wrapper exercise (various types, target=None)
// ─────────────────────────────────────────────────────────────────────────────

/// data_pack with i32 exercises the full FFI call path.
#[test]
fn test_pack_i32_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Int32);
    assert!(result.is_err());
}

/// data_pack with i64 exercises the FFI call path.
#[test]
fn test_pack_i64_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i64 = 999_999_999i64;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Int64);
    assert!(result.is_err());
}

/// data_pack with u32 exercises the FFI call path.
#[test]
fn test_pack_u32_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u32 = 42_000;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Uint32);
    assert!(result.is_err());
}

/// data_pack with u64 exercises the FFI call path.
#[test]
fn test_pack_u64_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u64 = 1_000_000_000u64;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Uint64);
    assert!(result.is_err());
}

/// data_pack with f64 (Double) exercises the FFI call path.
#[test]
fn test_pack_double_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: f64 = 3.14159;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Double);
    assert!(result.is_err());
}

/// data_pack with f32 (Float) exercises the FFI call path.
#[test]
fn test_pack_float_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: f32 = 3.14f32;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Float);
    assert!(result.is_err());
}

/// data_pack with bool exercises the FFI call path.
#[test]
fn test_pack_bool_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val: bool = true;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Bool);
    assert!(result.is_err());
}

/// data_pack with String exercises the FFI call path.
#[test]
fn test_pack_string_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let val = String::from("test string data");
    let result = data_pack(None, &buf, &val, 1, PmixDataType::String);
    assert!(result.is_err());
}

/// data_pack with multiple values (num_vals > 1).
#[test]
fn test_pack_multiple_values() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 10, PmixDataType::Int32);
    assert!(result.is_err());
}

/// Multiple consecutive data_pack calls on same buffer.
#[test]
fn test_pack_consecutive_calls() {
    let buf = data_buffer_create().expect("create buffer");
    for i in 0..5 {
        let val: i32 = i;
        let result = data_pack(None, &buf, &val, 1, PmixDataType::Int32);
        assert!(result.is_err());
    }
}

/// data_pack with empty namespace proc_ref (safe without init).
#[test]
fn test_pack_with_empty_namespace() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let proc_ref = PmixProcRef::new("", 0);
    let result = data_pack(Some(proc_ref), &buf, &val, 1, PmixDataType::Int32);
    assert!(result.is_err());
}

/// data_pack error status is PMIX_ERR_NOT_FOUND without init.
#[test]
fn test_pack_error_status() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 1, PmixDataType::Int32);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -46); // PMIX_ERR_NOT_FOUND
}

// ─────────────────────────────────────────────────────────────────────────────
// data_unpack — FFI wrapper exercise (target=None only)
// ─────────────────────────────────────────────────────────────────────────────

/// data_unpack exercises the FFI call path.
#[test]
fn test_unpack_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: i32 = 0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Int32);
    assert!(result.is_err());
}

/// data_unpack with f64 (Double) type.
#[test]
fn test_unpack_double_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: f64 = 0.0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Double);
    assert!(result.is_err());
}

/// data_unpack with String type.
#[test]
fn test_unpack_string_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val = String::new();
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::String);
    assert!(result.is_err());
}

/// data_unpack with u64 type.
#[test]
fn test_unpack_u64_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: u64 = 0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Uint64);
    assert!(result.is_err());
}

/// data_unpack with bool type.
#[test]
fn test_unpack_bool_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: bool = false;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Bool);
    assert!(result.is_err());
}

/// data_unpack error is PMIX_ERR_NOT_FOUND without init.
#[test]
fn test_unpack_error_is_not_found() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: i32 = 0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Int32);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -46); // PMIX_ERR_NOT_FOUND
}

/// data_unpack with max_num_values set to large value.
#[test]
fn test_unpack_large_max_num() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: i32 = 0;
    let mut max_num: i32 = 1000;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Int32);
    assert!(result.is_err());
}

/// Multiple consecutive data_unpack calls.
#[test]
fn test_unpack_consecutive_calls() {
    let buf = data_buffer_create().expect("create buffer");
    for _ in 0..5 {
        let mut val: i32 = 0;
        let mut max_num: i32 = 1;
        let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Int32);
        assert!(result.is_err());
    }
}

/// data_unpack with i64 type.
#[test]
fn test_unpack_i64_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: i64 = 0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Int64);
    assert!(result.is_err());
}

/// data_unpack with u32 type.
#[test]
fn test_unpack_u32_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: u32 = 0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Uint32);
    assert!(result.is_err());
}

/// data_unpack with Float type.
#[test]
fn test_unpack_float_ffi_path() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: f32 = 0.0;
    let mut max_num: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut max_num, PmixDataType::Float);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// data_load / data_unload — additional exercise
// ─────────────────────────────────────────────────────────────────────────────

/// data_load with populated payload exercises FFI call path.
#[test]
fn test_data_load_populated_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4, 5]);
    let result = data_load(&buf, &payload);
    assert!(result.is_ok());
}

/// data_unload on buffer with data exercises FFI call path.
#[test]
fn test_data_unload_buffer_with_data() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![10u8, 20, 30]);
    data_load(&buf, &payload).expect("load");
    let result = data_unload(&buf);
    assert!(result.is_ok());
    let unloaded = result.unwrap();
    assert_eq!(unloaded.size(), 3);
}

/// data_load then data_unload roundtrip with larger payload.
#[test]
fn test_load_unload_roundtrip_large() {
    let buf = data_buffer_create().expect("create buffer");
    let original = PmixByteObject::from(vec![1u8, 2, 3, 4, 5, 6, 7, 8]);
    data_load(&buf, &original).expect("load");
    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(unloaded.size(), 8);
    assert_eq!(unloaded.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcRef — construction tests
// ─────────────────────────────────────────────────────────────────────────────

/// PmixProcRef with typical namespace and rank.
#[test]
fn test_proc_ref_typical() {
    let _proc_ref = PmixProcRef::new("job-12345", 0);
}

/// PmixProcRef with u32::MAX rank (wildcard equivalent).
#[test]
fn test_proc_ref_max_rank() {
    let _proc_ref = PmixProcRef::new("test_ns", u32::MAX);
}

/// PmixProcRef construction works.
#[test]
fn test_proc_ref_construction() {
    let _proc_ref = PmixProcRef::new("test_ns", 42);
}
