//! Dedicated tests for `PMIx_Data_unpack`.
//!
//! PMIx_Data_unpack requires PMIx_Init to have been called (it needs
//! `pmix_globals.mypeer` for peer detection). All tests that actually
//! invoke the FFI `PMIx_Data_unpack` are marked `#[ignore]` and need
//! a PMIx runtime environment.
//!
//! Tests that only exercise the Rust wrapper types (PmixDataBuffer,
//! PmixByteObject, PmixProcRef) and buffer management run without
//! PMIx_Init.

mod daemon_helper;

use std::sync::OnceLock;

use pmix::data_serialization::*;
use pmix::{PmixDataType, init};

// ─────────────────────────────────────────────────────────────────────────────
// Singleton PMIx init — PMIx can only be initialized once per process.
// ─────────────────────────────────────────────────────────────────────────────
static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();

fn ensure_init() -> &'static pmix::Context {
    PMIX_CTX.get_or_init(|| init(None).expect("PMIx_Init failed — run under prterun"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Type and buffer tests — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// PmixDataBuffer can be created and is valid.
#[test]
fn test_buffer_create_for_unpack() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "buffer should be valid");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");
}

/// PmixProcRef can be created as a source for unpack operations.
#[test]
fn test_proc_ref_as_source() {
    let _source = PmixProcRef::new("test_namespace", 0);
    let _source2 = PmixProcRef::new("job123", 42);
    let _source_max = PmixProcRef::new("ns", u32::MAX);
}

/// PmixProcRef with long namespace truncates properly.
#[test]
fn test_proc_ref_long_namespace() {
    let long_ns = "a".repeat(300);
    let _proc = PmixProcRef::new(&long_ns, 0);
}

/// PmixByteObject can be created and used with data_load/data_unload.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_byte_object_roundtrip() {
    daemon_helper::ensure_pmix_init();
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![10u8, 20, 30, 40, 50, 60];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "byte object roundtrip should match"
    );
}

/// PmixByteObject::new creates an empty, valid byte object.
#[test]
fn test_byte_object_empty() {
    let obj = PmixByteObject::new();
    assert!(obj.is_empty());
    assert_eq!(obj.size(), 0);
    assert!(obj.as_slice().is_empty());
}

/// PmixByteObject from Vec<u8> has correct size.
#[test]
fn test_byte_object_from_vec() {
    let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let obj = PmixByteObject::from(bytes.clone());
    assert_eq!(obj.size(), bytes.len());
    assert_eq!(obj.as_slice(), bytes.as_slice());
}

/// PmixDataBuffer Debug output includes allocated/used bytes.
#[test]
fn test_buffer_debug_output() {
    let buf = data_buffer_create().expect("create buffer");
    let debug = format!("{:?}", buf);
    assert!(debug.contains("PmixDataBuffer"));
    assert!(debug.contains("bytes_allocated"));
    assert!(debug.contains("bytes_used"));
}

/// Null PmixDataBuffer Debug output shows "null".
#[test]
fn test_buffer_debug_null() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    let debug = format!("{:?}", buf);
    assert!(debug.contains("null"));
}

/// Multiple buffers can coexist without interfering.
#[test]
fn test_multiple_buffers_independent() {
    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");
    let buf3 = data_buffer_create().expect("buf3");

    assert!(buf1.is_valid());
    assert!(buf2.is_valid());
    assert!(buf3.is_valid());
    // All drop independently — no double-free
}

/// PmixDataBuffer drop releases the buffer automatically.
/// Note: data_buffer_release takes &PmixDataBuffer (immutable), so it cannot
/// null the inner pointer. Calling it explicitly and then letting Drop run
/// causes a double-free. The correct pattern is to rely on Drop alone.
#[test]
fn test_buffer_drop_releases() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid());
    // buf is dropped here — Drop calls PMIx_Data_buffer_release
}

// ─────────────────────────────────────────────────────────────────────────────
// Unpack FFI tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Unpack a single i32 after packing.
#[test]
#[ignore]
fn test_unpack_int32() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 42;
    data_pack(None, &buf, &original, 1, PmixDataType::Int32).expect("pack");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack i32 should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert_eq!(count, 1, "count should be updated to 1");
    assert_eq!(recovered, original, "value should match");
}

/// Unpack a single i64 after packing.
#[test]
#[ignore]
fn test_unpack_int64() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i64 = -9223372036854775808i64;
    data_pack(None, &buf, &original, 1, PmixDataType::Int64).expect("pack");

    let mut recovered: i64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int64)
        .expect("unpack i64 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single u32 after packing.
#[test]
#[ignore]
fn test_unpack_uint32() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u32 = 0xFFFFFFFF;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint32).expect("pack");

    let mut recovered: u32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint32)
        .expect("unpack u32 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single u64 after packing.
#[test]
#[ignore]
fn test_unpack_uint64() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u64 = u64::MAX;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint64).expect("pack");

    let mut recovered: u64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint64)
        .expect("unpack u64 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single f32 after packing.
#[test]
#[ignore]
fn test_unpack_float() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f32 = 2.718281828f32;
    data_pack(None, &buf, &original, 1, PmixDataType::Float).expect("pack");

    let mut recovered: f32 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Float)
        .expect("unpack float should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-5,
        "float should match within tolerance"
    );
}

/// Unpack a single f64 after packing.
#[test]
#[ignore]
fn test_unpack_double() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f64 = 3.141592653589793;
    data_pack(None, &buf, &original, 1, PmixDataType::Double).expect("pack");

    let mut recovered: f64 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Double)
        .expect("unpack double should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-10,
        "double should match within tolerance"
    );
}

/// Unpack a single i8 after packing.
#[test]
#[ignore]
fn test_unpack_int8() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i8 = -128;
    data_pack(None, &buf, &original, 1, PmixDataType::Int8).expect("pack");

    let mut recovered: i8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int8)
        .expect("unpack i8 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single u8 (byte) after packing.
#[test]
#[ignore]
fn test_unpack_uint8() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 255;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint8).expect("pack");

    let mut recovered: u8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint8)
        .expect("unpack u8 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single i16 after packing.
#[test]
#[ignore]
fn test_unpack_int16() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i16 = -32768;
    data_pack(None, &buf, &original, 1, PmixDataType::Int16).expect("pack");

    let mut recovered: i16 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int16)
        .expect("unpack i16 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a single u16 after packing.
#[test]
#[ignore]
fn test_unpack_uint16() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u16 = 65535;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint16).expect("pack");

    let mut recovered: u16 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint16)
        .expect("unpack u16 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a bool (packed as u8) after packing.
#[test]
#[ignore]
fn test_unpack_bool() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 1;
    data_pack(None, &buf, &original, 1, PmixDataType::Bool).expect("pack");

    let mut recovered: u8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Bool)
        .expect("unpack bool should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a PMIx status code after packing.
#[test]
#[ignore]
fn test_unpack_status() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 0; // PMIX_SUCCESS
    data_pack(None, &buf, &original, 1, PmixDataType::Status).expect("pack");

    let mut recovered: i32 = -1;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Status)
        .expect("unpack status should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack a usize (PMIX_SIZE) after packing.
#[test]
#[ignore]
fn test_unpack_size() {
    let buf = data_buffer_create().expect("create buffer");
    let original: usize = 1024;
    data_pack(None, &buf, &original, 1, PmixDataType::Size).expect("pack");

    let mut recovered: usize = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Size)
        .expect("unpack size should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack multiple i32 values in a single call.
#[test]
#[ignore]
fn test_unpack_multiple_int32() {
    let buf = data_buffer_create().expect("create buffer");
    let originals: [i32; 5] = [10, 20, 30, 40, 50];
    data_pack(None, &buf, &originals, 5, PmixDataType::Int32).expect("pack");

    let mut recovered: [i32; 5] = [0; 5];
    let mut count: i32 = 5;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack multiple i32 should succeed");
    assert_eq!(unpacked, 5, "should unpack 5 values");
    assert_eq!(count, 5, "count should be 5");
    assert_eq!(recovered, originals, "all values should match");
}

/// Unpack multiple u8 values in a single call.
#[test]
#[ignore]
fn test_unpack_multiple_bytes() {
    let buf = data_buffer_create().expect("create buffer");
    let originals: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    data_pack(None, &buf, &originals, 10, PmixDataType::Uint8).expect("pack");

    let mut recovered: [u8; 10] = [0; 10];
    let mut count: i32 = 10;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint8)
        .expect("unpack multiple bytes should succeed");
    assert_eq!(unpacked, 10);
    assert_eq!(recovered, originals);
}

/// Unpack is non-destructive — same buffer can be read multiple times.
#[test]
#[ignore]
fn test_unpack_non_destructive() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 42;
    data_pack(None, &buf, &original, 1, PmixDataType::Int32).expect("pack");

    // First unpack
    let mut val1: i32 = 0;
    let mut count1: i32 = 1;
    let unpacked1 =
        data_unpack(None, &buf, &mut val1, &mut count1, PmixDataType::Int32).expect("first unpack");
    assert_eq!(unpacked1, 1);
    assert_eq!(val1, original);

    // Second unpack — should succeed because unpack is non-destructive
    let mut val2: i32 = 0;
    let mut count2: i32 = 1;
    let unpacked2 = data_unpack(None, &buf, &mut val2, &mut count2, PmixDataType::Int32)
        .expect("second unpack should also succeed");
    assert_eq!(unpacked2, 1);
    assert_eq!(val2, original);
}

/// Unpack with explicit source process reference.
#[test]
#[ignore]
fn test_unpack_with_source() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 123;
    data_pack(None, &buf, &original, 1, PmixDataType::Int32).expect("pack");

    let source = PmixProcRef::new("test_ns", 0);
    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(
        Some(source),
        &buf,
        &mut recovered,
        &mut count,
        PmixDataType::Int32,
    )
    .expect("unpack with source should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack boundary values: i32::MIN and i32::MAX.
#[test]
#[ignore]
fn test_unpack_boundary_values() {
    let buf = data_buffer_create().expect("create buffer");

    let min_val: i32 = i32::MIN;
    let max_val: i32 = i32::MAX;
    data_pack(None, &buf, &min_val, 1, PmixDataType::Int32).expect("pack MIN");
    data_pack(None, &buf, &max_val, 1, PmixDataType::Int32).expect("pack MAX");

    let mut recovered_min: i32 = 0;
    let mut count: i32 = 1;
    data_unpack(
        None,
        &buf,
        &mut recovered_min,
        &mut count,
        PmixDataType::Int32,
    )
    .expect("unpack MIN");
    assert_eq!(recovered_min, i32::MIN);

    let mut recovered_max: i32 = 0;
    data_unpack(
        None,
        &buf,
        &mut recovered_max,
        &mut count,
        PmixDataType::Int32,
    )
    .expect("unpack MAX");
    assert_eq!(recovered_max, i32::MAX);
}

/// Unpack zero value.
#[test]
#[ignore]
fn test_unpack_zero() {
    let buf = data_buffer_create().expect("create buffer");
    let zero: i32 = 0;
    data_pack(None, &buf, &zero, 1, PmixDataType::Int32).expect("pack zero");

    let mut recovered: i32 = -1;
    let mut count: i32 = 1;
    data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32).expect("unpack zero");
    assert_eq!(recovered, 0);
}

/// Full roundtrip: pack -> unload -> load into new buffer -> unpack.
#[test]
#[ignore]
fn test_full_roundtrip_unpack() {
    let buf1 = data_buffer_create().expect("create buffer 1");
    let original: i32 = 42;
    data_pack(None, &buf1, &original, 1, PmixDataType::Int32).expect("pack");

    let payload = data_unload(&buf1).expect("unload");
    assert!(!payload.is_empty(), "payload should not be empty");
    drop(buf1);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf2, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack after roundtrip");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Full roundtrip with multiple values.
#[test]
#[ignore]
fn test_full_roundtrip_multiple() {
    let buf1 = data_buffer_create().expect("create buffer 1");
    let originals: [i32; 3] = [100, 200, 300];
    data_pack(None, &buf1, &originals, 3, PmixDataType::Int32).expect("pack");

    let payload = data_unload(&buf1).expect("unload");
    drop(buf1);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load");

    let mut recovered: [i32; 3] = [0; 3];
    let mut count: i32 = 3;
    let unpacked = data_unpack(None, &buf2, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack after roundtrip");
    assert_eq!(unpacked, 3);
    assert_eq!(recovered, originals);
}

/// Full roundtrip with mixed types packed sequentially, then unpacked in order.
#[test]
#[ignore]
fn test_full_roundtrip_mixed_types() {
    let buf1 = data_buffer_create().expect("create buffer 1");

    let int_val: i32 = 42;
    let float_val: f32 = 3.14;
    let uint_val: u64 = 1000;

    data_pack(None, &buf1, &int_val, 1, PmixDataType::Int32).expect("pack int32");
    data_pack(None, &buf1, &float_val, 1, PmixDataType::Float).expect("pack float");
    data_pack(None, &buf1, &uint_val, 1, PmixDataType::Uint64).expect("pack uint64");

    let payload = data_unload(&buf1).expect("unload");
    drop(buf1);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load");

    // Unpack in the same order they were packed
    let mut r_int: i32 = 0;
    let mut count: i32 = 1;
    data_unpack(None, &buf2, &mut r_int, &mut count, PmixDataType::Int32).expect("unpack int32");
    assert_eq!(r_int, int_val);

    let mut r_float: f32 = 0.0;
    data_unpack(None, &buf2, &mut r_float, &mut count, PmixDataType::Float).expect("unpack float");
    assert!((r_float - float_val).abs() < 1e-5);

    let mut r_uint: u64 = 0;
    data_unpack(None, &buf2, &mut r_uint, &mut count, PmixDataType::Uint64).expect("unpack uint64");
    assert_eq!(r_uint, uint_val);
}

/// Unpack after data_copy_payload.
#[test]
#[ignore]
fn test_unpack_after_copy_payload() {
    let src = data_buffer_create().expect("create src");
    let original: i32 = 42;
    data_pack(None, &src, &original, 1, PmixDataType::Int32).expect("pack");

    let dest = data_buffer_create().expect("create dest");
    data_copy_payload(&dest, &src).expect("copy payload");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &dest, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack from copied buffer");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Unpack count is updated on success.
#[test]
#[ignore]
fn test_unpack_count_updated() {
    let buf = data_buffer_create().expect("create buffer");
    let originals: [i32; 4] = [1, 2, 3, 4];
    data_pack(None, &buf, &originals, 4, PmixDataType::Int32).expect("pack");

    let mut recovered: [i32; 4] = [0; 4];
    let mut count: i32 = 10; // Request more than packed
    let unpacked =
        data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32).expect("unpack");
    assert_eq!(unpacked, 4);
    assert_eq!(count, 4, "count should be updated to actual unpacked count");
}
