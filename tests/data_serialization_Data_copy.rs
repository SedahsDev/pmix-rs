//! Tests for `PMIx_Data_copy` and `PMIx_Data_copy_payload`.
//!
//! `PMIx_Data_copy` copies a data value of a specified PMIx type, allocating
//! memory for the destination internally. It requires `PMIx_Init` because it
//! accesses `pmix_globals.mypeer` to determine the active bfrops peer.
//!
//! `PMIx_Data_copy_payload` copies the raw payload from one buffer to another.
//! It also requires `PMIx_Init` for the same reason.
//!
//! Tests that call the FFI functions are marked `#[ignore]` and need a PMIx
//! runtime environment to execute.

use pmix::data_serialization::*;
use pmix::{PmixDataType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only type checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_copy<T> is callable with i32 and returns the right type.
#[test]
fn test_data_copy_signature_i32() {
    // Compile-time type check: data_copy<&i32> returns Result<*mut c_void, PmixStatus>
    fn check<F>(_: F) {}
    check::<fn(&i32, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with i64.
#[test]
fn test_data_copy_signature_i64() {
    fn check<F>(_: F) {}
    check::<fn(&i64, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with f64.
#[test]
fn test_data_copy_signature_f64() {
    fn check<F>(_: F) {}
    check::<fn(&f64, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with u32.
#[test]
fn test_data_copy_signature_u32() {
    fn check<F>(_: F) {}
    check::<fn(&u32, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with u8 (bool type).
#[test]
fn test_data_copy_signature_bool() {
    fn check<F>(_: F) {}
    check::<fn(&u8, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with usize (size type).
#[test]
fn test_data_copy_signature_size() {
    fn check<F>(_: F) {}
    check::<fn(&usize, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with i8.
#[test]
fn test_data_copy_signature_int8() {
    fn check<F>(_: F) {}
    check::<fn(&i8, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with u16.
#[test]
fn test_data_copy_signature_uint16() {
    fn check<F>(_: F) {}
    check::<fn(&u16, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with f32.
#[test]
fn test_data_copy_signature_float() {
    fn check<F>(_: F) {}
    check::<fn(&f32, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> is callable with u64.
#[test]
fn test_data_copy_signature_u64() {
    fn check<F>(_: F) {}
    check::<fn(&u64, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy_payload signature: takes two &PmixDataBuffer, returns Result<(), PmixStatus>.
#[test]
fn test_data_copy_payload_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer, &PmixDataBuffer) -> Result<(), PmixStatus>>(data_copy_payload);
}

/// Verify PmixDataType has expected variants for data_copy.
#[test]
fn test_data_type_variants() {
    // Verify key data type variants exist
    let _: PmixDataType = PmixDataType::Int8;
    let _: PmixDataType = PmixDataType::Uint8;
    let _: PmixDataType = PmixDataType::Int16;
    let _: PmixDataType = PmixDataType::Uint16;
    let _: PmixDataType = PmixDataType::Int32;
    let _: PmixDataType = PmixDataType::Uint32;
    let _: PmixDataType = PmixDataType::Int64;
    let _: PmixDataType = PmixDataType::Uint64;
    let _: PmixDataType = PmixDataType::Float;
    let _: PmixDataType = PmixDataType::Double;
    let _: PmixDataType = PmixDataType::Bool;
    let _: PmixDataType = PmixDataType::Size;
    let _: PmixDataType = PmixDataType::Status;
}

/// Verify PmixStatus::from_raw converts C status codes.
#[test]
fn test_pmix_status_from_raw() {
    let success = PmixStatus::from_raw(0);
    assert!(success.is_success(), "PMIX_SUCCESS should be success");
}

/// Verify PmixStatus::is_error for error codes.
#[test]
fn test_pmix_status_error() {
    let err = PmixStatus::from_raw(-1); // PMIX_ERROR
    assert!(err.is_error(), "PMIX_ERROR should be error");
}

/// Verify data_buffer_create works (used by copy_payload tests).
#[test]
fn test_buffer_create_for_copy_payload() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "buffer should be valid");
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Copy a single i32 value and verify the destination is allocated.
/// Requires PMIx_Init — needs pmix_globals.mypeer for find_peer().
#[test]
#[ignore]
fn test_data_copy_i32() {
    let val: i32 = 42;
    let result = data_copy(&val, PmixDataType::Int32);
    assert!(result.is_ok(), "data_copy of i32 should succeed");
    let ptr = result.unwrap();
    assert!(!ptr.is_null(), "copied pointer should be non-null");

    // The copied value should be readable
    // SAFETY: data_copy allocated memory containing a copy of the i32 value.
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val, "copied value should match original");
}

/// Copy a single i64 value.
#[test]
#[ignore]
fn test_data_copy_i64() {
    let val: i64 = -9223372036854775808i64;
    let result = data_copy(&val, PmixDataType::Int64);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i64) };
    assert_eq!(copied, val);
}

/// Copy a single u64 value.
#[test]
#[ignore]
fn test_data_copy_u64() {
    let val: u64 = u64::MAX;
    let result = data_copy(&val, PmixDataType::Uint64);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u64) };
    assert_eq!(copied, val);
}

/// Copy a single f32 value.
#[test]
#[ignore]
fn test_data_copy_f32() {
    let val: f32 = 3.14f32;
    let result = data_copy(&val, PmixDataType::Float);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const f32) };
    assert!((copied - val).abs() < f32::EPSILON);
}

/// Copy a single f64 value.
#[test]
#[ignore]
fn test_data_copy_f64() {
    let val: f64 = 2.718281828459045;
    let result = data_copy(&val, PmixDataType::Double);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const f64) };
    assert!((copied - val).abs() < f64::EPSILON);
}

/// Copy a single i8 value.
#[test]
#[ignore]
fn test_data_copy_i8() {
    let val: i8 = -128;
    let result = data_copy(&val, PmixDataType::Int8);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i8) };
    assert_eq!(copied, val);
}

/// Copy a single u8 value.
#[test]
#[ignore]
fn test_data_copy_u8() {
    let val: u8 = 255;
    let result = data_copy(&val, PmixDataType::Uint8);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u8) };
    assert_eq!(copied, val);
}

/// Copy a single i16 value.
#[test]
#[ignore]
fn test_data_copy_i16() {
    let val: i16 = -32768;
    let result = data_copy(&val, PmixDataType::Int16);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i16) };
    assert_eq!(copied, val);
}

/// Copy a single u16 value.
#[test]
#[ignore]
fn test_data_copy_u16() {
    let val: u16 = 65535;
    let result = data_copy(&val, PmixDataType::Uint16);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u16) };
    assert_eq!(copied, val);
}

/// Copy a bool value (packed as uint8_t).
#[test]
#[ignore]
fn test_data_copy_bool() {
    let val: u8 = 1;
    let result = data_copy(&val, PmixDataType::Bool);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u8) };
    assert_eq!(copied, val);
}

/// Copy a size value.
#[test]
#[ignore]
fn test_data_copy_size() {
    let val: usize = 1024;
    let result = data_copy(&val, PmixDataType::Size);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const usize) };
    assert_eq!(copied, val);
}

/// Copy a status value.
#[test]
#[ignore]
fn test_data_copy_status() {
    let val: i32 = 0; // PMIX_SUCCESS
    let result = data_copy(&val, PmixDataType::Status);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val);
}

/// Copy with an unknown data type should fail.
#[test]
#[ignore]
fn test_data_copy_unknown_type() {
    let val: i32 = 42;
    // PMIX_DATA_TYPE_MAX or a type not registered should fail
    let result = data_copy(&val, PmixDataType::Unknown);
    assert!(
        result.is_err(),
        "copy with unknown type should return error"
    );
}

/// Copy a bool false value.
#[test]
#[ignore]
fn test_data_copy_bool_false() {
    let val: u8 = 0;
    let result = data_copy(&val, PmixDataType::Bool);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u8) };
    assert_eq!(copied, val);
}

/// Copy with boundary i32 values.
#[test]
#[ignore]
fn test_data_copy_i32_min() {
    let val: i32 = i32::MIN;
    let result = data_copy(&val, PmixDataType::Int32);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val);
}

/// Copy with boundary i32 values.
#[test]
#[ignore]
fn test_data_copy_i32_max() {
    let val: i32 = i32::MAX;
    let result = data_copy(&val, PmixDataType::Int32);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val);
}

/// Copy with boundary u32 values.
#[test]
#[ignore]
fn test_data_copy_u32_max() {
    let val: u32 = u32::MAX;
    let result = data_copy(&val, PmixDataType::Uint32);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    assert!(!ptr.is_null());
    let copied = unsafe { *(ptr as *const u32) };
    assert_eq!(copied, val);
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_copy_payload — FFI integration (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Copy payload from one buffer to another.
/// Requires PMIx_Init — needs pmix_globals.mypeer for find_peer().
#[test]
#[ignore]
fn test_copy_payload_basic() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    // Pack some data into the source buffer first
    let val: i32 = 42;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Int32).expect("pack should succeed");
    assert!(src_buf.bytes_used() > 0, "src buffer should have data");

    // Copy payload from src to dest
    let result = data_copy_payload(&dest_buf, &src_buf);
    assert!(result.is_ok(), "copy_payload should succeed");
    assert!(
        dest_buf.bytes_used() > 0,
        "dest buffer should have data after copy"
    );
}

/// Copy payload does not destroy the source buffer.
#[test]
#[ignore]
fn test_copy_payload_source_preserved() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: i32 = 123;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Int32).expect("pack should succeed");
    let src_bytes = src_buf.bytes_used();

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    // Source buffer should still have its data
    assert_eq!(
        src_buf.bytes_used(),
        src_bytes,
        "source buffer should be unchanged after copy_payload"
    );
}

/// Copy payload with empty source buffer.
#[test]
#[ignore]
fn test_copy_payload_empty_source() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    // Both buffers are empty — copy should still succeed (copy nothing)
    let result = data_copy_payload(&dest_buf, &src_buf);
    assert!(
        result.is_ok(),
        "copy_payload of empty buffer should succeed"
    );
}

/// Copy payload with multiple packed values.
#[test]
#[ignore]
fn test_copy_payload_multiple_values() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    // Pack multiple values of different types
    let val1: i32 = 42;
    let val2: i64 = 1000000;
    let val3: f64 = 3.14;

    data_pack(None, &src_buf, &val1, 1, PmixDataType::Int32).expect("pack i32");
    data_pack(None, &src_buf, &val2, 1, PmixDataType::Int64).expect("pack i64");
    data_pack(None, &src_buf, &val3, 1, PmixDataType::Double).expect("pack f64");

    let src_bytes = src_buf.bytes_used();
    assert!(src_bytes > 0, "src should have data");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(
        dest_buf.bytes_used() >= src_bytes,
        "dest buffer should have at least as much data as src"
    );
}

/// Copy payload can be unpacked from destination.
#[test]
#[ignore]
fn test_copy_payload_roundtrip() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let original: i32 = 42;
    data_pack(None, &src_buf, &original, 1, PmixDataType::Int32).expect("pack should succeed");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    // Unpack from the destination buffer
    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(
        None,
        &dest_buf,
        &mut recovered,
        &mut count,
        PmixDataType::Int32,
    )
    .expect("unpack from dest should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert_eq!(recovered, original, "recovered value should match original");
}

/// Multiple copy_payload calls accumulate in destination.
#[test]
#[ignore]
fn test_copy_payload_accumulates() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: i32 = 42;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Int32).expect("pack should succeed");
    let src_bytes = src_buf.bytes_used();

    // First copy
    data_copy_payload(&dest_buf, &src_buf).expect("first copy");
    let after_first = dest_buf.bytes_used();

    // Second copy should append
    data_copy_payload(&dest_buf, &src_buf).expect("second copy");
    let after_second = dest_buf.bytes_used();

    assert!(
        after_second > after_first,
        "second copy_payload should increase dest buffer size"
    );
    assert_eq!(
        after_second - after_first,
        src_bytes,
        "second copy should add same amount as first"
    );
}

/// Copy payload with packed string data.
#[test]
#[ignore]
fn test_copy_payload_with_string() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let s = std::ffi::CString::new("hello world").expect("create cstring");
    data_pack(None, &src_buf, &s.as_ptr(), 1, PmixDataType::String).expect("pack string");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload preserves source after multiple copies.
#[test]
#[ignore]
fn test_copy_payload_source_integrity() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf1 = data_buffer_create().expect("create dest1");
    let dest_buf2 = data_buffer_create().expect("create dest2");

    let val: i64 = 999999999i64;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Int64).expect("pack should succeed");

    data_copy_payload(&dest_buf1, &src_buf).expect("copy to dest1");
    data_copy_payload(&dest_buf2, &src_buf).expect("copy to dest2");

    // Both destinations should have the same amount of data
    assert_eq!(
        dest_buf1.bytes_used(),
        dest_buf2.bytes_used(),
        "both destinations should have equal data"
    );
}
