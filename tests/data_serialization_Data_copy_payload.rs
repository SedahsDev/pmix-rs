//! Tests for `PMIx_Data_copy_payload`.
//!
//! `PMIx_Data_copy_payload` copies the raw data payload from one buffer to
//! another without interpreting the contents. It is non-destructive — the
//! source buffer's payload remains intact, and the destination buffer
//! accumulates (appends) the copied data.
//!
//! Requires `PMIx_Init` because it accesses `pmix_globals.mypeer` for
//! bfrops peer resolution.
//!
//! Tests that call the FFI functions are marked `#[ignore]` and need a
//! PMIx runtime environment to execute.

use pmix::data_serialization::*;
use pmix::{PmixDataType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_copy_payload signature: takes two &PmixDataBuffer, returns Result<(), PmixStatus>.
#[test]
fn test_data_copy_payload_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer, &PmixDataBuffer) -> Result<(), PmixStatus>>(data_copy_payload);
}

/// Verify PmixDataBuffer has is_valid method.
#[test]
fn test_data_buffer_is_valid() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer) -> bool>(PmixDataBuffer::is_valid);
}

/// Verify PmixDataBuffer has bytes_used method.
#[test]
fn test_data_buffer_bytes_used() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer) -> usize>(PmixDataBuffer::bytes_used);
}

/// Verify PmixDataBuffer has bytes_allocated method.
#[test]
fn test_data_buffer_bytes_allocated() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer) -> usize>(PmixDataBuffer::bytes_allocated);
}

/// Verify data_buffer_create is callable and returns Result<PmixDataBuffer, PmixStatus>.
#[test]
fn test_buffer_create_for_copy_payload() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "buffer should be valid");
}

/// Verify data_buffer_create produces a buffer with zero bytes used initially.
#[test]
fn test_buffer_create_empty() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(
        buf.bytes_used(),
        0,
        "new buffer should have zero bytes used"
    );
}

/// Verify data_buffer_create produces a buffer with bytes_allocated accessible.
#[test]
fn test_buffer_create_allocated() {
    let buf = data_buffer_create().expect("create buffer");
    // bytes_allocated is usize so always >= 0; just verify it's readable
    let _ = buf.bytes_allocated();
}

/// Verify PmixStatus::from_raw for PMIX_SUCCESS.
#[test]
fn test_pmix_status_success() {
    let success = PmixStatus::from_raw(0);
    assert!(success.is_success(), "PMIX_SUCCESS should be success");
}

/// Verify PmixStatus::is_error for PMIX_ERR_BAD_PARAM (-9 per spec).
#[test]
fn test_pmix_status_bad_param() {
    let bad_param = PmixStatus::from_raw(-9); // PMIX_ERR_BAD_PARAM
    assert!(bad_param.is_error(), "PMIX_ERR_BAD_PARAM should be error");
}

/// Verify PmixStatus::is_error for PMIX_ERR_NOT_SUPPORTED.
#[test]
fn test_pmix_status_not_supported() {
    let not_supported = PmixStatus::from_raw(-11); // PMIX_ERR_NOT_SUPPORTED
    assert!(
        not_supported.is_error(),
        "PMIX_ERR_NOT_SUPPORTED should be error"
    );
}

/// Verify PmixDataType has variants used by data_pack for copy_payload tests.
#[test]
fn test_data_type_variants_for_pack() {
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
    let _: PmixDataType = PmixDataType::String;
}

/// Verify PmixProcRef::new is callable (used as target/source for pack/unpack).
#[test]
fn test_pmix_proc_ref_new() {
    let proc = PmixProcRef::new("test_namespace", 0);
    let _ = proc; // compile-time check that it constructs
}

/// Verify as_mut_ptr returns a raw pointer.
#[test]
fn test_data_buffer_as_mut_ptr() {
    let buf = data_buffer_create().expect("create buffer");
    let _ptr: *mut std::ffi::c_void = buf.as_mut_ptr() as *mut std::ffi::c_void;
}

/// Verify PmixDataBuffer implements Debug.
#[test]
fn test_data_buffer_debug() {
    let buf = data_buffer_create().expect("create buffer");
    let debug_str = format!("{:?}", buf);
    assert!(
        debug_str.contains("PmixDataBuffer"),
        "Debug should contain struct name"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Copy payload from one buffer to another with a single packed i32 value.
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

/// Copy payload does not destroy the source buffer (non-destructive).
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

/// Copy payload with an empty source buffer should succeed (copy nothing).
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
    assert_eq!(
        dest_buf.bytes_used(),
        0,
        "dest should remain empty after copying empty source"
    );
}

/// Copy payload with multiple packed values of different types.
#[test]
#[ignore]
fn test_copy_payload_multiple_values() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    // Pack multiple values of different types
    let val1: i32 = 42;
    let val2: i64 = 1_000_000;
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

/// Copy payload then unpack from destination — roundtrip verification.
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

/// Multiple copy_payload calls accumulate (append) in the destination.
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

/// Copy payload preserves source after copying to multiple destinations.
#[test]
#[ignore]
fn test_copy_payload_source_integrity() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf1 = data_buffer_create().expect("create dest1");
    let dest_buf2 = data_buffer_create().expect("create dest2");

    let val: i64 = 999_999_999i64;
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

/// Copy payload with boundary i32 values (min and max).
#[test]
#[ignore]
fn test_copy_payload_boundary_i32() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val_min: i32 = i32::MIN;
    let val_max: i32 = i32::MAX;

    data_pack(None, &src_buf, &val_min, 1, PmixDataType::Int32).expect("pack min");
    data_pack(None, &src_buf, &val_max, 1, PmixDataType::Int32).expect("pack max");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with u64 boundary values.
#[test]
#[ignore]
fn test_copy_payload_boundary_u64() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: u64 = u64::MAX;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Uint64).expect("pack u64 max");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with bool values (true and false).
#[test]
#[ignore]
fn test_copy_payload_bool_values() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val_true: u8 = 1;
    let val_false: u8 = 0;

    data_pack(None, &src_buf, &val_true, 1, PmixDataType::Bool).expect("pack true");
    data_pack(None, &src_buf, &val_false, 1, PmixDataType::Bool).expect("pack false");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with float boundary values.
#[test]
#[ignore]
fn test_copy_payload_float_boundary() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: f32 = f32::MAX;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Float).expect("pack f32 max");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with double precision values.
#[test]
#[ignore]
fn test_copy_payload_double_precision() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: f64 = std::f64::consts::PI;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Double).expect("pack pi");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload roundtrip with i64 — pack, copy, unpack.
#[test]
#[ignore]
fn test_copy_payload_roundtrip_i64() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let original: i64 = -9_223_372_036_854_775_808i64;
    data_pack(None, &src_buf, &original, 1, PmixDataType::Int64).expect("pack should succeed");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    let mut recovered: i64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(
        None,
        &dest_buf,
        &mut recovered,
        &mut count,
        PmixDataType::Int64,
    )
    .expect("unpack from dest should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert_eq!(recovered, original, "recovered i64 should match original");
}

/// Copy payload roundtrip with f64 — pack, copy, unpack.
#[test]
#[ignore]
fn test_copy_payload_roundtrip_f64() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let original: f64 = 2.718281828459045;
    data_pack(None, &src_buf, &original, 1, PmixDataType::Double).expect("pack should succeed");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    let mut recovered: f64 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(
        None,
        &dest_buf,
        &mut recovered,
        &mut count,
        PmixDataType::Double,
    )
    .expect("unpack from dest should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert!(
        (recovered - original).abs() < f64::EPSILON,
        "recovered f64 should match original"
    );
}

/// Copy payload with size type.
#[test]
#[ignore]
fn test_copy_payload_size_type() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: usize = 1024;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Size).expect("pack size");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with status type.
#[test]
#[ignore]
fn test_copy_payload_status_type() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: i32 = 0; // PMIX_SUCCESS
    data_pack(None, &src_buf, &val, 1, PmixDataType::Status).expect("pack status");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with int8 and uint8 types.
#[test]
#[ignore]
fn test_copy_payload_int8_uint8() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val_i8: i8 = -128;
    let val_u8: u8 = 255;

    data_pack(None, &src_buf, &val_i8, 1, PmixDataType::Int8).expect("pack i8");
    data_pack(None, &src_buf, &val_u8, 1, PmixDataType::Uint8).expect("pack u8");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with int16 and uint16 types.
#[test]
#[ignore]
fn test_copy_payload_int16_uint16() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val_i16: i16 = -32768;
    let val_u16: u16 = 65535;

    data_pack(None, &src_buf, &val_i16, 1, PmixDataType::Int16).expect("pack i16");
    data_pack(None, &src_buf, &val_u16, 1, PmixDataType::Uint16).expect("pack u16");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload with uint32 type.
#[test]
#[ignore]
fn test_copy_payload_uint32() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let val: u32 = u32::MAX;
    data_pack(None, &src_buf, &val, 1, PmixDataType::Uint32).expect("pack u32");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(dest_buf.bytes_used() > 0, "dest should have data");
}

/// Copy payload from buffer with pre-existing dest data should append.
#[test]
#[ignore]
fn test_copy_payload_dest_has_existing_data() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    // Pre-fill destination with some data
    let dest_val: i32 = 100;
    data_pack(None, &dest_buf, &dest_val, 1, PmixDataType::Int32).expect("pack dest pre-fill");
    let dest_initial = dest_buf.bytes_used();

    // Source data
    let src_val: i32 = 200;
    data_pack(None, &src_buf, &src_val, 1, PmixDataType::Int32).expect("pack src");
    let src_bytes = src_buf.bytes_used();

    // Copy should append to existing dest data
    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(
        dest_buf.bytes_used() >= dest_initial + src_bytes,
        "dest should have original data plus copied data"
    );
}

/// Copy payload with a large number of small values.
#[test]
#[ignore]
fn test_copy_payload_many_small_values() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    for i in 0..100i32 {
        data_pack(None, &src_buf, &i, 1, PmixDataType::Int32).expect("pack each i32");
    }

    let src_bytes = src_buf.bytes_used();
    assert!(src_bytes > 0, "src should have data");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");
    assert!(
        dest_buf.bytes_used() >= src_bytes,
        "dest should have at least as much data as src"
    );
}

/// Copy payload self-copy (dest == src) — behavior depends on PMIx implementation.
/// The spec says it copies from src to dest; when they are the same buffer,
/// the result is implementation-defined. We test that it at least doesn't crash.
#[test]
#[ignore]
fn test_copy_payload_self_copy() {
    let buf = data_buffer_create().expect("create buffer");

    let val: i32 = 42;
    data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack should succeed");
    let bytes_before = buf.bytes_used();

    // Self-copy — should not crash
    let result = data_copy_payload(&buf, &buf);
    // May succeed or return error depending on implementation
    // The important thing is it doesn't segfault
    if result.is_ok() {
        assert!(
            buf.bytes_used() >= bytes_before,
            "self-copy should not reduce buffer size"
        );
    }
}

/// Copy payload roundtrip with mixed types — pack multiple, copy, unpack all.
#[test]
#[ignore]
fn test_copy_payload_roundtrip_mixed() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let v1: i32 = 42;
    let v2: i64 = 100_000;
    let v3: f64 = 3.14159;

    data_pack(None, &src_buf, &v1, 1, PmixDataType::Int32).expect("pack i32");
    data_pack(None, &src_buf, &v2, 1, PmixDataType::Int64).expect("pack i64");
    data_pack(None, &src_buf, &v3, 1, PmixDataType::Double).expect("pack f64");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    // Unpack in the same order
    let mut r1: i32 = 0;
    let mut r2: i64 = 0;
    let mut r3: f64 = 0.0;
    let mut count: i32 = 1;

    let n =
        data_unpack(None, &dest_buf, &mut r1, &mut count, PmixDataType::Int32).expect("unpack i32");
    assert_eq!(n, 1);
    assert_eq!(r1, v1);

    let n =
        data_unpack(None, &dest_buf, &mut r2, &mut count, PmixDataType::Int64).expect("unpack i64");
    assert_eq!(n, 1);
    assert_eq!(r2, v2);

    let n = data_unpack(None, &dest_buf, &mut r3, &mut count, PmixDataType::Double)
        .expect("unpack f64");
    assert_eq!(n, 1);
    assert!((r3 - v3).abs() < f64::EPSILON);
}
