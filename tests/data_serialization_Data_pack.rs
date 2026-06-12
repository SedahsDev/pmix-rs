//! Tests for `PMIx_Data_pack`, `PMIx_Data_unpack`, `PMIx_Data_load`,
//! `PMIx_Data_unload`, `PMIx_Data_copy`, and buffer management.
//!
//! Note: PMIx_Data_pack, PMIx_Data_unpack, and PMIx_Data_copy_payload
//! require PMIx_Init to have been called (they need pmix_globals.mypeer).
//! These are marked #[ignore] and should be run with a PMIx environment.
//!
//! PMIx_Data_buffer_create, PMIx_Data_buffer_release, PMIx_Data_load,
//! and PMIx_Data_unload operate entirely in user space and do NOT require
//! PMIx_Init — these tests run normally.

use pmix::PmixDataType;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Buffer management — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// data_buffer_create returns a valid, non-null buffer.
#[test]
fn test_buffer_create_returns_valid() {
    let buf = data_buffer_create().expect("data_buffer_create should succeed");
    assert!(buf.is_valid(), "buffer should be valid after creation");
}

/// data_buffer_create returns a buffer with zero allocated/used bytes initially.
#[test]
fn test_buffer_create_initial_state() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");
}

/// Multiple buffers can be created and released independently.
#[test]
fn test_multiple_buffers() {
    let buf1 = data_buffer_create().expect("create buf1");
    let buf2 = data_buffer_create().expect("create buf2");
    let buf3 = data_buffer_create().expect("create buf3");

    assert!(buf1.is_valid());
    assert!(buf2.is_valid());
    assert!(buf3.is_valid());
    // All dropped independently — no double-free
}

/// PmixDataBuffer implements Debug.
#[test]
fn test_buffer_debug() {
    let buf = data_buffer_create().expect("create buffer");
    let debug_str = format!("{:?}", buf);
    assert!(
        debug_str.contains("PmixDataBuffer"),
        "Debug output should contain struct name"
    );
}

/// PmixDataBuffer with null pointer debugs correctly.
#[test]
fn test_buffer_debug_null() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    let debug_str = format!("{:?}", buf);
    assert!(
        debug_str.contains("null"),
        "null buffer should show 'null' in debug"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// PmixByteObject::new creates an empty byte object.
#[test]
fn test_byte_object_new_empty() {
    let obj = PmixByteObject::new();
    assert!(obj.is_empty(), "new byte object should be empty");
    assert_eq!(obj.size(), 0, "new byte object should have size 0");
}

/// PmixByteObject::as_slice returns empty slice for empty object.
#[test]
fn test_byte_object_as_slice_empty() {
    let obj = PmixByteObject::new();
    let slice = obj.as_slice();
    assert!(
        slice.is_empty(),
        "slice of empty byte object should be empty"
    );
}

/// PmixByteObject implements Default.
#[test]
fn test_byte_object_default() {
    let obj = PmixByteObject::default();
    assert!(obj.is_empty(), "default byte object should be empty");
}

/// PmixByteObject implements Debug.
#[test]
fn test_byte_object_debug() {
    let obj = PmixByteObject::new();
    let debug_str = format!("{:?}", obj);
    assert!(
        debug_str.contains("PmixByteObject"),
        "Debug output should contain struct name"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcRef — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// PmixProcRef can be created with namespace and rank.
#[test]
fn test_proc_ref_new() {
    let _proc = PmixProcRef::new("test_namespace", 0);
}

/// PmixProcRef works with various ranks.
#[test]
fn test_proc_ref_various_ranks() {
    let _proc0 = PmixProcRef::new("job123", 0);
    let _proc1 = PmixProcRef::new("job123", 1);
    let _proc_max = PmixProcRef::new("job123", u32::MAX);
}

/// PmixProcRef with long namespace (truncates to 255 chars).
#[test]
fn test_proc_ref_long_namespace() {
    let long_ns = "a".repeat(300);
    let _proc = PmixProcRef::new(&long_ns, 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_load — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// data_load into empty buffer succeeds with empty payload.
#[test]
fn test_load_empty_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    let result = data_load(&buf, &payload);
    // Loading empty payload should succeed (nothing to load)
    assert!(result.is_ok(), "loading empty payload should succeed");
}

/// data_load into buffer and verify bytes_used increases.
#[test]
fn test_load_payload() {
    let buf = data_buffer_create().expect("create buffer");

    // Create a payload with some bytes
    let bytes = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let payload = PmixByteObject::from(bytes);
    assert_eq!(payload.size(), 8);

    let result = data_load(&buf, &payload);
    assert!(result.is_ok(), "load should succeed");
    assert!(buf.bytes_used() > 0, "buffer should have data after load");
}

/// data_load replaces buffer content (DESTRUCT then set up).
/// Note: PMIx_Data_load destructs the buffer first, so it replaces, not appends.
/// Also, it steals the payload pointer, so the payload is empty after the call.
#[test]
fn test_load_replaces_buffer() {
    let buf = data_buffer_create().expect("create buffer");

    let payload1 = PmixByteObject::from(vec![1u8, 2, 3]);
    let size1 = payload1.size();

    data_load(&buf, &payload1).expect("load first");
    assert_eq!(
        buf.bytes_used(),
        size1,
        "buffer should have first payload size"
    );

    // Second load replaces (not appends)
    let payload2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8, 9]);
    let size2 = payload2.size();

    data_load(&buf, &payload2).expect("load second (replaces)");
    assert_eq!(
        buf.bytes_used(),
        size2,
        "buffer should have second payload size (replacement)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_unload — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// data_unload from empty buffer — may succeed with empty payload or fail.
#[test]
fn test_unload_empty_buffer() {
    let buf = data_buffer_create().expect("create buffer");
    let result = data_unload(&buf);
    match result {
        Ok(payload) => {
            assert!(
                payload.as_slice().is_empty(),
                "empty buffer should yield empty payload"
            );
        }
        Err(_) => {
            // Error is also acceptable for empty buffer
        }
    }
}

/// data_load then data_unload should roundtrip the bytes.
#[test]
fn test_load_unload_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");

    let original = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "roundtrip payload should match original"
    );
}

/// data_load then data_unload with larger payload.
#[test]
fn test_load_unload_large_payload() {
    let buf = data_buffer_create().expect("create buffer");

    let original: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "large roundtrip payload should match"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_pack — requires PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Pack a single i32 (PMIX_INT32).
/// Requires PMIx_Init — needs pmix_globals.mypeer for find_peer().
#[test]
#[ignore]
fn test_pack_int32() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack int32 should succeed");
    assert_eq!(packed, 1, "should pack exactly 1 value");
    assert!(
        buf.bytes_used() > 0,
        "buffer should have data after packing"
    );
}

/// Pack a single u32 (PMIX_UINT32).
#[test]
#[ignore]
fn test_pack_uint32() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u32 = 100;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Uint32).expect("pack uint32 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single i64 (PMIX_INT64).
#[test]
#[ignore]
fn test_pack_int64() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i64 = -9223372036854775808i64;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Int64).expect("pack int64 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single u64 (PMIX_UINT64).
#[test]
#[ignore]
fn test_pack_uint64() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u64 = u64::MAX;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Uint64).expect("pack uint64 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single bool (PMIX_BOOL, packed as uint8_t).
#[test]
#[ignore]
fn test_pack_bool() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u8 = 1;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Bool).expect("pack bool should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single float (PMIX_FLOAT).
#[test]
#[ignore]
fn test_pack_float() {
    let buf = data_buffer_create().expect("create buffer");
    let val: f32 = 3.14f32;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Float).expect("pack float should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single double (PMIX_DOUBLE).
#[test]
#[ignore]
fn test_pack_double() {
    let buf = data_buffer_create().expect("create buffer");
    let val: f64 = 2.718281828;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Double).expect("pack double should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single i8 (PMIX_INT8).
#[test]
#[ignore]
fn test_pack_int8() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i8 = -128;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Int8).expect("pack int8 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single u8 (PMIX_UINT8 / PMIX_BYTE).
#[test]
#[ignore]
fn test_pack_uint8() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u8 = 255;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Uint8).expect("pack uint8 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single i16 (PMIX_INT16).
#[test]
#[ignore]
fn test_pack_int16() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i16 = -32768;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Int16).expect("pack int16 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single u16 (PMIX_UINT16).
#[test]
#[ignore]
fn test_pack_uint16() {
    let buf = data_buffer_create().expect("create buffer");
    let val: u16 = 65535;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Uint16).expect("pack uint16 should succeed");
    assert_eq!(packed, 1);
}

/// Pack a single isize (PMIX_SIZE).
#[test]
#[ignore]
fn test_pack_size() {
    let buf = data_buffer_create().expect("create buffer");
    let val: usize = 1024;
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Size).expect("pack size should succeed");
    assert_eq!(packed, 1);
}

/// Pack a PMIx status code (PMIX_STATUS).
#[test]
#[ignore]
fn test_pack_status() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 0; // PMIX_SUCCESS
    let packed =
        data_pack(None, &buf, &val, 1, PmixDataType::Status).expect("pack status should succeed");
    assert_eq!(packed, 1);
}

/// Pack multiple i32 values.
#[test]
#[ignore]
fn test_pack_multiple_int32() {
    let buf = data_buffer_create().expect("create buffer");
    let vals: [i32; 5] = [1, 2, 3, 4, 5];
    let packed = data_pack(None, &buf, &vals, 5, PmixDataType::Int32)
        .expect("pack multiple int32 should succeed");
    assert_eq!(packed, 5, "should pack 5 values");
}

/// Pack multiple u8 values.
#[test]
#[ignore]
fn test_pack_multiple_bytes() {
    let buf = data_buffer_create().expect("create buffer");
    let bytes: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let packed = data_pack(None, &buf, &bytes, 10, PmixDataType::Uint8)
        .expect("pack multiple bytes should succeed");
    assert_eq!(packed, 10);
}

/// Pack with num_vals = 0 should return error.
#[test]
fn test_pack_zero_values_returns_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    assert!(result.is_err(), "packing 0 values should return error");
}

/// Pack with negative num_vals should return error.
#[test]
fn test_pack_negative_values_returns_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
    assert!(
        result.is_err(),
        "packing negative num_vals should return error"
    );
}

/// Pack with an explicit target process reference.
#[test]
#[ignore]
fn test_pack_with_target() {
    let buf = data_buffer_create().expect("create buffer");
    let target = PmixProcRef::new("test_namespace", 42);
    let val: i32 = 123;
    let packed = data_pack(Some(target), &buf, &val, 1, PmixDataType::Int32)
        .expect("pack with target should succeed");
    assert_eq!(packed, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Pack + Unpack roundtrip — requires PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Pack and unpack a single i32 value.
#[test]
#[ignore]
fn test_pack_unpack_int32_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 42;
    data_pack(None, &buf, &original, 1, PmixDataType::Int32).expect("pack should succeed");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert_eq!(recovered, original, "recovered value should match original");
}

/// Pack and unpack a single i64 value.
#[test]
#[ignore]
fn test_pack_unpack_int64_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i64 = -9223372036854775808i64;
    data_pack(None, &buf, &original, 1, PmixDataType::Int64).expect("pack should succeed");

    let mut recovered: i64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int64)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Pack and unpack a single f64 value.
#[test]
#[ignore]
fn test_pack_unpack_double_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f64 = 3.14159265358979;
    data_pack(None, &buf, &original, 1, PmixDataType::Double).expect("pack should succeed");

    let mut recovered: f64 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Double)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-10,
        "recovered double should match original"
    );
}

/// Pack and unpack multiple i32 values.
#[test]
#[ignore]
fn test_pack_unpack_multiple_int32_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let originals: [i32; 5] = [10, 20, 30, 40, 50];
    data_pack(None, &buf, &originals, 5, PmixDataType::Int32).expect("pack should succeed");

    let mut recovered: [i32; 5] = [0; 5];
    let mut count: i32 = 5;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 5, "should unpack 5 values");
    assert_eq!(recovered, originals, "all values should match");
}

/// Pack and unpack u8 values.
#[test]
#[ignore]
fn test_pack_unpack_uint8_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 255;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint8).expect("pack should succeed");

    let mut recovered: u8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint8)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original);
}

/// Pack and unpack f32 value.
#[test]
#[ignore]
fn test_pack_unpack_float_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f32 = 2.718;
    data_pack(None, &buf, &original, 1, PmixDataType::Float).expect("pack should succeed");

    let mut recovered: f32 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Float)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-5,
        "recovered float should match original"
    );
}

/// Full roundtrip: pack -> unload -> create new buffer -> load -> unpack.
#[test]
#[ignore]
fn test_pack_unload_load_unpack_roundtrip() {
    let buf1 = data_buffer_create().expect("create buffer 1");
    let original: i32 = 42;
    data_pack(None, &buf1, &original, 1, PmixDataType::Int32).expect("pack should succeed");

    let payload = data_unload(&buf1).expect("unload should succeed");
    assert!(!payload.is_empty(), "payload should not be empty");
    assert!(payload.size() > 0, "payload should have positive size");
    drop(buf1);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load should succeed");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf2, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "roundtrip value should match");
}

/// Full roundtrip with multiple values.
#[test]
#[ignore]
fn test_full_roundtrip_multiple_values() {
    let buf1 = data_buffer_create().expect("create buffer 1");
    let originals: [i32; 3] = [100, 200, 300];
    data_pack(None, &buf1, &originals, 3, PmixDataType::Int32).expect("pack should succeed");

    let payload = data_unload(&buf1).expect("unload should succeed");
    drop(buf1);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load should succeed");

    let mut recovered: [i32; 3] = [0; 3];
    let mut count: i32 = 3;
    let unpacked = data_unpack(None, &buf2, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack should succeed");
    assert_eq!(unpacked, 3);
    assert_eq!(recovered, originals);
}

/// Full roundtrip with mixed types packed sequentially.
#[test]
#[ignore]
fn test_full_roundtrip_mixed_types() {
    let buf = data_buffer_create().expect("create buffer");

    let int_val: i32 = 42;
    let float_val: f32 = 3.14;
    let uint_val: u64 = 1000;

    data_pack(None, &buf, &int_val, 1, PmixDataType::Int32).expect("pack int32");
    data_pack(None, &buf, &float_val, 1, PmixDataType::Float).expect("pack float");
    data_pack(None, &buf, &uint_val, 1, PmixDataType::Uint64).expect("pack uint64");

    let payload = data_unload(&buf).expect("unload");
    drop(buf);

    let buf2 = data_buffer_create().expect("create buffer 2");
    data_load(&buf2, &payload).expect("load");

    let mut recovered_int: i32 = 0;
    let mut count: i32 = 1;
    data_unpack(
        None,
        &buf2,
        &mut recovered_int,
        &mut count,
        PmixDataType::Int32,
    )
    .expect("unpack int32");
    assert_eq!(recovered_int, int_val);

    let mut recovered_float: f32 = 0.0;
    data_unpack(
        None,
        &buf2,
        &mut recovered_float,
        &mut count,
        PmixDataType::Float,
    )
    .expect("unpack float");
    assert!((recovered_float - float_val).abs() < 1e-5);

    let mut recovered_uint: u64 = 0;
    data_unpack(
        None,
        &buf2,
        &mut recovered_uint,
        &mut count,
        PmixDataType::Uint64,
    )
    .expect("unpack uint64");
    assert_eq!(recovered_uint, uint_val);
}

// ─────────────────────────────────────────────────────────────────────────────
// data_copy_payload — requires PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Copy payload from one buffer to another.
#[test]
#[ignore]
fn test_copy_payload() {
    let src = data_buffer_create().expect("create src buffer");
    let val: i32 = 42;
    data_pack(None, &src, &val, 1, PmixDataType::Int32).expect("pack should succeed");

    let dest = data_buffer_create().expect("create dest buffer");
    data_copy_payload(&dest, &src).expect("copy_payload should succeed");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &dest, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack from copy should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, val);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// Pack with negative num_vals should return error (checked before FFI call).
#[test]
fn test_pack_negative_num_vals_returns_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
    assert!(
        result.is_err(),
        "packing negative num_vals should return error"
    );
}

/// Pack boundary values: i32::MIN and i32::MAX.
#[test]
#[ignore]
fn test_pack_int32_boundary_values() {
    let buf = data_buffer_create().expect("create buffer");

    let min_val: i32 = i32::MIN;
    data_pack(None, &buf, &min_val, 1, PmixDataType::Int32).expect("pack i32::MIN should succeed");

    let max_val: i32 = i32::MAX;
    data_pack(None, &buf, &max_val, 1, PmixDataType::Int32).expect("pack i32::MAX should succeed");

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

/// Pack zero values.
#[test]
#[ignore]
fn test_pack_zero_values() {
    let buf = data_buffer_create().expect("create buffer");
    let zero: i32 = 0;
    data_pack(None, &buf, &zero, 1, PmixDataType::Int32).expect("pack zero should succeed");

    let mut recovered: i32 = -1;
    let mut count: i32 = 1;
    data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32).expect("unpack zero");
    assert_eq!(recovered, 0);
}

/// PmixByteObject payload as_slice returns readable data.
#[test]
fn test_byte_object_as_slice_readable() {
    let buf = data_buffer_create().expect("create buffer");
    let bytes = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    let payload = PmixByteObject::from(bytes);
    data_load(&buf, &payload).expect("load should succeed");

    let unloaded = data_unload(&buf).expect("unload should succeed");
    let slice = unloaded.as_slice();
    assert!(!slice.is_empty(), "payload slice should not be empty");
    assert_eq!(slice, &[0xDE, 0xAD, 0xBE, 0xEF]);
}

/// Unpack from empty buffer should fail (requires PMIx_Init).
#[test]
#[ignore]
fn test_unpack_empty_buffer_fails() {
    let buf = data_buffer_create().expect("create buffer");
    let mut val: i32 = 0;
    let mut count: i32 = 1;
    let result = data_unpack(None, &buf, &mut val, &mut count, PmixDataType::Int32);
    assert!(result.is_err(), "unpacking from empty buffer should fail");
}

/// Unpack is non-destructive.
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
        .expect("second unpack should also succeed (non-destructive)");
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
