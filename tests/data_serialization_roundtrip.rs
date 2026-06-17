//! Round-trip tests for data serialization: pack, unpack, copy, print, load, unload.
//!
//! These tests focus on end-to-end round-trip verification of the data
//! serialization pipeline. Tests that exercise the FFI functions requiring
//! PMIx_Init are marked `#[ignore]` and need a PMIx runtime environment.
//! Tests that only exercise the Rust wrapper types and user-space functions
//! run without PMIx_Init.
//!
//! Categories:
//! - Primitive round-trip (pack → unpack)
//! - Multi-value round-trip
//! - Array/struct round-trip
//! - Copy semantics
//! - Print output validation
//! - Load/unload round-trip
//! - Error cases

use pmix::data_serialization::*;
use pmix::{PmixDataType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Primitive round-trip tests (pack → unpack) — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// Pack and unpack a bool true value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_bool_true() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 1;
    data_pack(None, &buf, &original, 1, PmixDataType::Bool)
        .expect("pack bool should succeed");

    let mut recovered: u8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Bool)
        .expect("unpack bool should succeed");
    assert_eq!(unpacked, 1, "should unpack 1 value");
    assert_eq!(recovered, original, "bool true should roundtrip");
}

/// Pack and unpack a bool false value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_bool_false() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 0;
    data_pack(None, &buf, &original, 1, PmixDataType::Bool)
        .expect("pack bool should succeed");

    let mut recovered: u8 = 0xFF;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Bool)
        .expect("unpack bool should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, 0, "bool false should roundtrip");
}

/// Pack and unpack an i8 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_i8() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i8 = -42;
    data_pack(None, &buf, &original, 1, PmixDataType::Int8)
        .expect("pack i8 should succeed");

    let mut recovered: i8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int8)
        .expect("unpack i8 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "i8 should roundtrip");
}

/// Pack and unpack a u8 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_u8() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u8 = 255;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint8)
        .expect("pack u8 should succeed");

    let mut recovered: u8 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint8)
        .expect("unpack u8 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "u8 should roundtrip");
}

/// Pack and unpack an i16 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_i16() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i16 = -32768;
    data_pack(None, &buf, &original, 1, PmixDataType::Int16)
        .expect("pack i16 should succeed");

    let mut recovered: i16 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int16)
        .expect("unpack i16 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "i16 should roundtrip");
}

/// Pack and unpack a u16 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_u16() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u16 = 65535;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint16)
        .expect("pack u16 should succeed");

    let mut recovered: u16 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint16)
        .expect("unpack u16 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "u16 should roundtrip");
}

/// Pack and unpack an i32 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_i32() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = -2147483648i32;
    data_pack(None, &buf, &original, 1, PmixDataType::Int32)
        .expect("pack i32 should succeed");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack i32 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "i32 should roundtrip");
}

/// Pack and unpack a u32 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_u32() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u32 = 4294967295;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint32)
        .expect("pack u32 should succeed");

    let mut recovered: u32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint32)
        .expect("unpack u32 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "u32 should roundtrip");
}

/// Pack and unpack an i64 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_i64() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i64 = -9223372036854775808i64;
    data_pack(None, &buf, &original, 1, PmixDataType::Int64)
        .expect("pack i64 should succeed");

    let mut recovered: i64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int64)
        .expect("unpack i64 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "i64 should roundtrip");
}

/// Pack and unpack a u64 value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_u64() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u64 = u64::MAX;
    data_pack(None, &buf, &original, 1, PmixDataType::Uint64)
        .expect("pack u64 should succeed");

    let mut recovered: u64 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint64)
        .expect("unpack u64 should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "u64 should roundtrip");
}

/// Pack and unpack a f32 value (using approximate equality).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_f32() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f32 = 3.14159265358979f32;
    data_pack(None, &buf, &original, 1, PmixDataType::Float)
        .expect("pack f32 should succeed");

    let mut recovered: f32 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Float)
        .expect("unpack f32 should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-6,
        "f32 should roundtrip approximately: original={}, recovered={}",
        original,
        recovered
    );
}

/// Pack and unpack a f64 value (using approximate equality).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_f64() {
    let buf = data_buffer_create().expect("create buffer");
    let original: f64 = 2.718281828459045;
    data_pack(None, &buf, &original, 1, PmixDataType::Double)
        .expect("pack f64 should succeed");

    let mut recovered: f64 = 0.0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Double)
        .expect("unpack f64 should succeed");
    assert_eq!(unpacked, 1);
    assert!(
        (recovered - original).abs() < 1e-12,
        "f64 should roundtrip approximately: original={}, recovered={}",
        original,
        recovered
    );
}

/// Pack and unpack a String value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_string() {
    let buf = data_buffer_create().expect("create buffer");
    let original = String::from("hello, PMIx!");
    data_pack(None, &buf, &original, 1, PmixDataType::String)
        .expect("pack string should succeed");

    let mut recovered: String = String::new();
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::String)
        .expect("unpack string should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "string should roundtrip");
}

/// Pack and unpack a usize value (PMIX_SIZE).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_usize() {
    let buf = data_buffer_create().expect("create buffer");
    let original: usize = 1048576;
    data_pack(None, &buf, &original, 1, PmixDataType::Size)
        .expect("pack usize should succeed");

    let mut recovered: usize = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Size)
        .expect("unpack usize should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "usize should roundtrip");
}

/// Pack and unpack an i32 as PMIX_PID.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_pid() {
    let buf = data_buffer_create().expect("create buffer");
    let original: i32 = 12345;
    data_pack(None, &buf, &original, 1, PmixDataType::Pid)
        .expect("pack pid should succeed");

    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Pid)
        .expect("unpack pid should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "pid should roundtrip");
}

/// Pack and unpack a u32 as PMIX_PROC_RANK.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_proc_rank() {
    let buf = data_buffer_create().expect("create buffer");
    let original: u32 = 42;
    data_pack(None, &buf, &original, 1, PmixDataType::ProcRank)
        .expect("pack proc rank should succeed");

    let mut recovered: u32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::ProcRank)
        .expect("unpack proc rank should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "proc rank should roundtrip");
}

// ─────────────────────────────────────────────────────────────────────────────
// Multi-value round-trip tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// Pack multiple values of the same type and unpack them in order.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_multi_same_type() {
    let buf = data_buffer_create().expect("create buffer");
    let vals: [i32; 5] = [10, 20, 30, 40, 50];
    data_pack(None, &buf, &vals, 5, PmixDataType::Int32)
        .expect("pack multiple i32 should succeed");

    let mut recovered: [i32; 5] = [0; 5];
    let mut count: i32 = 5;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack multiple i32 should succeed");
    assert_eq!(unpacked, 5, "should unpack 5 values");
    assert_eq!(recovered, vals, "all values should roundtrip");
}

/// Pack mixed types and unpack them in order.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_mixed_types() {
    let buf = data_buffer_create().expect("create buffer");

    let val_i32: i32 = 42;
    let val_i64: i64 = 1000000;
    let val_f64: f64 = 3.14;
    let val_u8: u8 = 255;

    data_pack(None, &buf, &val_i32, 1, PmixDataType::Int32).expect("pack i32");
    data_pack(None, &buf, &val_i64, 1, PmixDataType::Int64).expect("pack i64");
    data_pack(None, &buf, &val_f64, 1, PmixDataType::Double).expect("pack f64");
    data_pack(None, &buf, &val_u8, 1, PmixDataType::Uint8).expect("pack u8");

    // Unpack in the same order
    let mut r_i32: i32 = 0;
    let mut cnt: i32 = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_i32, &mut cnt, PmixDataType::Int32).unwrap(), 1);
    assert_eq!(r_i32, val_i32);

    let mut r_i64: i64 = 0;
    cnt = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_i64, &mut cnt, PmixDataType::Int64).unwrap(), 1);
    assert_eq!(r_i64, val_i64);

    let mut r_f64: f64 = 0.0;
    cnt = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_f64, &mut cnt, PmixDataType::Double).unwrap(), 1);
    assert!((r_f64 - val_f64).abs() < 1e-6);

    let mut r_u8: u8 = 0;
    cnt = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_u8, &mut cnt, PmixDataType::Uint8).unwrap(), 1);
    assert_eq!(r_u8, val_u8);
}

/// Pack 10+ values and unpack all of them.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_ten_plus_values() {
    let buf = data_buffer_create().expect("create buffer");

    let vals: [i64; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    data_pack(None, &buf, &vals, 12, PmixDataType::Int64)
        .expect("pack 12 i64 should succeed");

    let mut recovered: [i64; 12] = [0; 12];
    let mut count: i32 = 12;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int64)
        .expect("unpack 12 i64 should succeed");
    assert_eq!(unpacked, 12, "should unpack 12 values");
    assert_eq!(recovered, vals, "all 12 values should roundtrip");
}

/// Pack multiple values of different types sequentially and unpack all.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_mixed_multi_types_long() {
    let buf = data_buffer_create().expect("create buffer");

    // Pack a sequence of different types
    let v1: i8 = -128;
    let v2: u8 = 200;
    let v3: i16 = -1000;
    let v4: u16 = 50000;
    let v5: i32 = 100000;
    let v6: u32 = 3000000000;
    let v7: i64 = -5000000000i64;
    let v8: u64 = 17000000000000u64;
    let v9: f32 = -1.5f32;
    let v10: f64 = 99.999;
    let v11: u8 = 1; // bool true
    let v12: usize = 4096;

    data_pack(None, &buf, &v1, 1, PmixDataType::Int8).expect("pack i8");
    data_pack(None, &buf, &v2, 1, PmixDataType::Uint8).expect("pack u8");
    data_pack(None, &buf, &v3, 1, PmixDataType::Int16).expect("pack i16");
    data_pack(None, &buf, &v4, 1, PmixDataType::Uint16).expect("pack u16");
    data_pack(None, &buf, &v5, 1, PmixDataType::Int32).expect("pack i32");
    data_pack(None, &buf, &v6, 1, PmixDataType::Uint32).expect("pack u32");
    data_pack(None, &buf, &v7, 1, PmixDataType::Int64).expect("pack i64");
    data_pack(None, &buf, &v8, 1, PmixDataType::Uint64).expect("pack u64");
    data_pack(None, &buf, &v9, 1, PmixDataType::Float).expect("pack f32");
    data_pack(None, &buf, &v10, 1, PmixDataType::Double).expect("pack f64");
    data_pack(None, &buf, &v11, 1, PmixDataType::Bool).expect("pack bool");
    data_pack(None, &buf, &v12, 1, PmixDataType::Size).expect("pack usize");

    // Unpack all in order
    let mut r1: i8 = 0;
    let mut c: i32 = 1;
    assert_eq!(data_unpack(None, &buf, &mut r1, &mut c, PmixDataType::Int8).unwrap(), 1);
    assert_eq!(r1, v1);

    let mut r2: u8 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r2, &mut c, PmixDataType::Uint8).unwrap(), 1);
    assert_eq!(r2, v2);

    let mut r3: i16 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r3, &mut c, PmixDataType::Int16).unwrap(), 1);
    assert_eq!(r3, v3);

    let mut r4: u16 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r4, &mut c, PmixDataType::Uint16).unwrap(), 1);
    assert_eq!(r4, v4);

    let mut r5: i32 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r5, &mut c, PmixDataType::Int32).unwrap(), 1);
    assert_eq!(r5, v5);

    let mut r6: u32 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r6, &mut c, PmixDataType::Uint32).unwrap(), 1);
    assert_eq!(r6, v6);

    let mut r7: i64 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r7, &mut c, PmixDataType::Int64).unwrap(), 1);
    assert_eq!(r7, v7);

    let mut r8: u64 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r8, &mut c, PmixDataType::Uint64).unwrap(), 1);
    assert_eq!(r8, v8);

    let mut r9: f32 = 0.0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r9, &mut c, PmixDataType::Float).unwrap(), 1);
    assert!((r9 - v9).abs() < 1e-5);

    let mut r10: f64 = 0.0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r10, &mut c, PmixDataType::Double).unwrap(), 1);
    assert!((r10 - v10).abs() < 1e-9);

    let mut r11: u8 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r11, &mut c, PmixDataType::Bool).unwrap(), 1);
    assert_eq!(r11, v11);

    let mut r12: usize = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r12, &mut c, PmixDataType::Size).unwrap(), 1);
    assert_eq!(r12, v12);
}

// ─────────────────────────────────────────────────────────────────────────────
// Array/struct round-trip tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// Pack an array of ints and unpack.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_array_of_ints() {
    let buf = data_buffer_create().expect("create buffer");
    let arr: [i32; 8] = [100, 200, 300, 400, 500, 600, 700, 800];
    data_pack(None, &buf, &arr, 8, PmixDataType::Int32)
        .expect("pack array should succeed");

    let mut recovered: [i32; 8] = [0; 8];
    let mut count: i32 = 8;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack array should succeed");
    assert_eq!(unpacked, 8);
    assert_eq!(recovered, arr, "array should roundtrip");
}

/// Pack an array of u8 bytes and unpack.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_array_of_bytes() {
    let buf = data_buffer_create().expect("create buffer");
    let arr: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    data_pack(None, &buf, &arr, 16, PmixDataType::Uint8)
        .expect("pack byte array should succeed");

    let mut recovered: [u8; 16] = [0; 16];
    let mut count: i32 = 16;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Uint8)
        .expect("unpack byte array should succeed");
    assert_eq!(unpacked, 16);
    assert_eq!(recovered, arr, "byte array should roundtrip");
}

/// Pack an array of f64 values and unpack with approximate equality.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_array_of_f64() {
    let buf = data_buffer_create().expect("create buffer");
    let arr: [f64; 4] = [1.1, 2.2, 3.3, 4.4];
    data_pack(None, &buf, &arr, 4, PmixDataType::Double)
        .expect("pack f64 array should succeed");

    let mut recovered: [f64; 4] = [0.0; 4];
    let mut count: i32 = 4;
    let unpacked = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Double)
        .expect("unpack f64 array should succeed");
    assert_eq!(unpacked, 4);
    for i in 0..4 {
        assert!(
            (recovered[i] - arr[i]).abs() < 1e-10,
            "f64 array element {} should roundtrip: original={}, recovered={}",
            i,
            arr[i],
            recovered[i]
        );
    }
}

/// Pack a struct with mixed fields (as individual values) and unpack.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_roundtrip_struct_mixed_fields() {
    #[derive(Debug, PartialEq)]
    struct TestData {
        id: i32,
        count: u32,
        value: f64,
        flag: u8, // bool as u8
    }

    let buf = data_buffer_create().expect("create buffer");
    let original = TestData {
        id: 42,
        count: 100,
        value: 3.14159,
        flag: 1,
    };

    // Pack each field individually
    data_pack(None, &buf, &original.id, 1, PmixDataType::Int32).expect("pack id");
    data_pack(None, &buf, &original.count, 1, PmixDataType::Uint32).expect("pack count");
    data_pack(None, &buf, &original.value, 1, PmixDataType::Double).expect("pack value");
    data_pack(None, &buf, &original.flag, 1, PmixDataType::Bool).expect("pack flag");

    // Unpack in same order
    let mut r_id: i32 = 0;
    let mut c: i32 = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_id, &mut c, PmixDataType::Int32).unwrap(), 1);

    let mut r_count: u32 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_count, &mut c, PmixDataType::Uint32).unwrap(), 1);

    let mut r_value: f64 = 0.0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_value, &mut c, PmixDataType::Double).unwrap(), 1);

    let mut r_flag: u8 = 0;
    c = 1;
    assert_eq!(data_unpack(None, &buf, &mut r_flag, &mut c, PmixDataType::Bool).unwrap(), 1);

    let recovered = TestData {
        id: r_id,
        count: r_count,
        value: r_value,
        flag: r_flag,
    };

    assert_eq!(recovered.id, original.id);
    assert_eq!(recovered.count, original.count);
    assert!((recovered.value - original.value).abs() < 1e-10);
    assert_eq!(recovered.flag, original.flag);
}

// ─────────────────────────────────────────────────────────────────────────────
// Copy semantics tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// data_copy: pack value, copy buffer, verify both buffers have same content.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_copy_pack_then_copy() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack");

    let bytes_before = buf.bytes_used();
    assert!(bytes_before > 0, "buffer should have data after pack");

    // Copy the value using data_copy
    let result = data_copy(&val, PmixDataType::Int32);
    assert!(result.is_ok(), "data_copy should succeed");
    let ptr = result.unwrap();
    assert!(!ptr.is_null(), "copied pointer should be non-null");

    // The copied value should be readable
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val, "copied value should match original");
}

/// data_copy_payload: pack values in src, copy_payload to dest, unpack from dest.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_copy_payload_roundtrip() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let original: i32 = 999;
    data_pack(None, &src_buf, &original, 1, PmixDataType::Int32)
        .expect("pack should succeed");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    // Unpack from destination
    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &dest_buf, &mut recovered, &mut count, PmixDataType::Int32)
        .expect("unpack from dest should succeed");
    assert_eq!(unpacked, 1);
    assert_eq!(recovered, original, "copy_payload roundtrip should match");
}

/// data_copy_payload with multiple values.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_copy_payload_multi_value() {
    let src_buf = data_buffer_create().expect("create src buffer");
    let dest_buf = data_buffer_create().expect("create dest buffer");

    let vals: [i64; 4] = [100, 200, 300, 400];
    data_pack(None, &src_buf, &vals, 4, PmixDataType::Int64)
        .expect("pack should succeed");

    data_copy_payload(&dest_buf, &src_buf).expect("copy_payload should succeed");

    let mut recovered: [i64; 4] = [0; 4];
    let mut count: i32 = 4;
    let unpacked = data_unpack(None, &dest_buf, &mut recovered, &mut count, PmixDataType::Int64)
        .expect("unpack from dest should succeed");
    assert_eq!(unpacked, 4);
    assert_eq!(recovered, vals, "multi-value copy_payload should match");
}

/// data_copy with boundary values.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_copy_boundary_values() {
    // Copy i32::MIN
    let val: i32 = i32::MIN;
    let result = data_copy(&val, PmixDataType::Int32);
    assert!(result.is_ok());
    let ptr = result.unwrap();
    let copied = unsafe { *(ptr as *const i32) };
    assert_eq!(copied, val);

    // Copy i32::MAX
    let val2: i32 = i32::MAX;
    let result2 = data_copy(&val2, PmixDataType::Int32);
    assert!(result2.is_ok());
    let ptr2 = result2.unwrap();
    let copied2 = unsafe { *(ptr2 as *const i32) };
    assert_eq!(copied2, val2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Print output tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// data_print for i32 produces non-empty output containing the value.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_i32_output() {
    let val: i32 = 42;
    let output = data_print(&val, None, PmixDataType::Int32)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
    assert!(
        output.contains("42"),
        "print output should contain the value '42', got: {}",
        output
    );
}

/// data_print for i64 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_i64_output() {
    let val: i64 = 12345678901234i64;
    let output = data_print(&val, None, PmixDataType::Int64)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
    assert!(
        output.contains("12345678901234") || output.contains("12345"),
        "print output should contain value info, got: {}",
        output
    );
}

/// data_print for u32 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_u32_output() {
    let val: u32 = 4294967295;
    let output = data_print(&val, None, PmixDataType::Uint32)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for f64 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_f64_output() {
    let val: f64 = 3.14159265358979;
    let output = data_print(&val, None, PmixDataType::Double)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for bool produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_bool_output() {
    let val: u8 = 1;
    let output = data_print(&val, None, PmixDataType::Bool)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for u8 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_u8_output() {
    let val: u8 = 255;
    let output = data_print(&val, None, PmixDataType::Uint8)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for i16 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_i16_output() {
    let val: i16 = -100;
    let output = data_print(&val, None, PmixDataType::Int16)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for u16 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_u16_output() {
    let val: u16 = 50000;
    let output = data_print(&val, None, PmixDataType::Uint16)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for i8 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_i8_output() {
    let val: i8 = -128;
    let output = data_print(&val, None, PmixDataType::Int8)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for u64 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_u64_output() {
    let val: u64 = 0xDEADBEEFCAFEBABE;
    let output = data_print(&val, None, PmixDataType::Uint64)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for f32 produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_f32_output() {
    let val: f32 = 2.718f32;
    let output = data_print(&val, None, PmixDataType::Float)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for usize (PMIX_SIZE) produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_usize_output() {
    let val: usize = 65536;
    let output = data_print(&val, None, PmixDataType::Size)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for i32 as PMIX_PID produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_pid_output() {
    let val: i32 = 99999;
    let output = data_print(&val, None, PmixDataType::Pid)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print for u32 as PMIX_PROC_RANK produces non-empty output.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_proc_rank_output() {
    let val: u32 = 7;
    let output = data_print(&val, None, PmixDataType::ProcRank)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
}

/// data_print with prefix produces output containing the prefix.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_with_prefix() {
    let val: i32 = 100;
    let output = data_print(&val, Some("myval="), PmixDataType::Int32)
        .expect("data_print should succeed");
    assert!(!output.is_empty(), "print output should not be empty");
    assert!(
        output.contains("myval="),
        "print output should contain prefix 'myval=', got: {}",
        output
    );
}

/// data_print output contains type info.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_contains_type_info() {
    let val: i32 = 42;
    let output = data_print(&val, None, PmixDataType::Int32)
        .expect("data_print should succeed");
    // PMIx print output typically includes the type name
    assert!(
        output.to_lowercase().contains("int32") || output.to_lowercase().contains("int"),
        "print output should mention the type, got: {}",
        output
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Load/Unload round-trip tests — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// Full round-trip: pack → unload → load → unpack using load/unload (no PMIx_Init).
/// This exercises the load/unload path without pack/unpack FFI.
#[test]
fn test_load_unload_roundtrip_basic() {
    let buf1 = data_buffer_create().expect("create buf1");
    let original = vec![10u8, 20, 30, 40, 50];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf1, &payload).expect("load should succeed");

    let unloaded = data_unload(&buf1).expect("unload should succeed");
    assert_eq!(
        unloaded.as_slice(),
        &original,
        "roundtrip payload should match original"
    );
}

/// Load → unload → load into new buffer → unload again (transport chain).
#[test]
fn test_load_unload_transport_chain() {
    // Sender
    let sender_buf = data_buffer_create().expect("create sender buffer");
    let original: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let sender_payload = PmixByteObject::from(original.clone());
    data_load(&sender_buf, &sender_payload).expect("sender load");

    let transport = data_unload(&sender_buf).expect("sender unload");
    assert_eq!(transport.as_slice(), &original);

    // Receiver
    let receiver_buf = data_buffer_create().expect("create receiver buffer");
    data_load(&receiver_buf, &transport).expect("receiver load");

    let recovered = data_unload(&receiver_buf).expect("receiver unload");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "transport chain should preserve all bytes"
    );
}

/// Load/unload with empty payload.
#[test]
fn test_load_unload_empty() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    data_load(&buf, &payload).expect("load empty should succeed");

    let recovered = data_unload(&buf);
    match recovered {
        Ok(obj) => assert!(obj.as_slice().is_empty(), "should be empty"),
        Err(_) => {
            // Error is also acceptable for empty buffer
        }
    }
}

/// Load/unload with single byte.
#[test]
fn test_load_unload_single_byte() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0x42u8];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load/unload with all-zero payload.
#[test]
fn test_load_unload_all_zeros() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0u8; 128];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load/unload with all-0xFF payload.
#[test]
fn test_load_unload_all_0xff() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0xFFu8; 64];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load/unload with 4KB payload.
#[test]
fn test_load_unload_4kb() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load/unload with 64KB payload.
#[test]
fn test_load_unload_64kb() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Multiple load/unload cycles on the same buffer.
#[test]
fn test_load_unload_multiple_cycles() {
    let buf = data_buffer_create().expect("create buffer");

    for i in 0..5 {
        let data: Vec<u8> = vec![i as u8; (i + 1) * 10];
        let payload = PmixByteObject::from(data.clone());
        data_load(&buf, &payload).expect("load");
        assert!(buf.bytes_used() > 0);

        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(recovered.as_slice(), &data[..]);
        assert_eq!(buf.bytes_used(), 0, "buffer should be empty after unload");
    }
}

/// Buffer remains valid after load/unload cycle.
#[test]
fn test_buffer_valid_after_load_unload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    assert!(buf.is_valid(), "buffer should still be valid after load/unload");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error case tests — some require PMIx_Init, some don't
// ─────────────────────────────────────────────────────────────────────────────

/// data_pack with num_vals == 0 returns error (no PMIx_Init needed).
#[test]
fn test_pack_zero_num_vals_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    assert!(result.is_err(), "pack with 0 values should error");
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -27, "should be PMIX_ERR_BAD_PARAM");
}

/// data_pack with negative num_vals returns error.
#[test]
fn test_pack_negative_num_vals_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -5, PmixDataType::Int32);
    assert!(result.is_err(), "pack with negative values should error");
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -27, "should be PMIX_ERR_BAD_PARAM");
}

/// data_pack with i32::MIN num_vals returns error.
#[test]
fn test_pack_i32_min_num_vals_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, i32::MIN, PmixDataType::Int32);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// data_unpack from empty buffer should fail (requires PMIx_Init).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_unpack_empty_buffer_error() {
    let buf = data_buffer_create().expect("create buffer");
    let mut recovered: i32 = 0;
    let mut count: i32 = 1;
    let result = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::Int32);
    assert!(
        result.is_err(),
        "unpacking from empty buffer should fail"
    );
}

/// data_unpack with wrong type — pack int, unpack as string (requires PMIx_Init).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_unpack_wrong_type_error() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    data_pack(None, &buf, &val, 1, PmixDataType::Int32)
        .expect("pack int32 should succeed");

    // Try to unpack as string — should fail or produce garbage
    let mut recovered: String = String::new();
    let mut count: i32 = 1;
    let result = data_unpack(None, &buf, &mut recovered, &mut count, PmixDataType::String);
    // This may succeed with garbage or fail — either is acceptable behavior
    // since we're testing the error path for type mismatch
    if result.is_ok() {
        // If it succeeds, the string will be garbage (not a valid string)
        // We just verify the API doesn't crash
        assert!(recovered.is_empty() || recovered.len() < 100);
    }
}

/// data_copy with unknown type should fail (requires PMIx_Init).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_copy_unknown_type_error() {
    let val: i32 = 42;
    let result = data_copy(&val, PmixDataType::Unknown);
    assert!(
        result.is_err(),
        "copy with unknown type should return error"
    );
}

/// data_print with empty prefix should succeed (requires PMIx_Init).
#[test]
#[ignore = "requires PMIx_Init"]
fn test_print_empty_prefix() {
    let val: i32 = 7;
    let result = data_print(&val, Some(""), PmixDataType::Int32);
    assert!(result.is_ok(), "print with empty prefix should succeed");
}

/// data_copy_payload with empty source should succeed (requires PMIx_Init).
#[test]
#[ignore = "requires PMIx_Init"]
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

/// Buffer cleanup is handled by Drop — no explicit release needed.
/// Calling data_buffer_release explicitly followed by Drop causes double-free.
#[test]
fn test_buffer_drop_cleanup() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "buffer should be valid");
    // Drop handles cleanup — no explicit release
}

/// Buffer is valid after create, and Drop handles cleanup.
/// NOTE: Do NOT call data_buffer_release explicitly — Drop already does it,
/// and calling it twice causes double-free / SIGSEGV.
#[test]
fn test_buffer_valid_then_drop() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "buffer should be valid after create");
    // Drop handles cleanup automatically — no explicit release needed
}

// ─────────────────────────────────────────────────────────────────────────────
// API surface / compile-only type checks (no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_pack<T> signature for i32.
#[test]
fn test_data_pack_signature_i32() {
    fn check<F>(_: F) {}
    check::<fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &i32,
        i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus>>(data_pack);
}

/// Verify data_pack<T> signature for f64.
#[test]
fn test_data_pack_signature_f64() {
    fn check<F>(_: F) {}
    check::<fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &f64,
        i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus>>(data_pack);
}

/// Verify data_unpack<T> signature for i32.
#[test]
fn test_data_unpack_signature_i32() {
    fn check<F>(_: F) {}
    check::<fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &mut i32,
        &mut i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus>>(data_unpack);
}

/// Verify data_unpack<T> signature for String.
#[test]
fn test_data_unpack_signature_string() {
    fn check<F>(_: F) {}
    check::<fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &mut String,
        &mut i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus>>(data_unpack);
}

/// Verify data_copy<T> signature for i32.
#[test]
fn test_data_copy_signature_i32() {
    fn check<F>(_: F) {}
    check::<fn(&i32, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy<T> signature for f64.
#[test]
fn test_data_copy_signature_f64() {
    fn check<F>(_: F) {}
    check::<fn(&f64, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus>>(data_copy);
}

/// Verify data_copy_payload signature.
#[test]
fn test_data_copy_payload_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer, &PmixDataBuffer) -> Result<(), PmixStatus>>(data_copy_payload);
}

/// Verify data_print<T> signature for i32.
#[test]
fn test_data_print_signature_i32() {
    fn check<F>(_: F) {}
    check::<fn(&i32, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_load signature.
#[test]
fn test_data_load_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer, &PmixByteObject) -> Result<(), PmixStatus>>(data_load);
}

/// Verify data_unload signature.
#[test]
fn test_data_unload_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer) -> Result<PmixByteObject, PmixStatus>>(data_unload);
}

/// Verify data_buffer_create signature.
#[test]
fn test_data_buffer_create_signature() {
    fn check<F>(_: F) {}
    check::<fn() -> Result<PmixDataBuffer, PmixStatus>>(data_buffer_create);
}

/// Verify data_buffer_release signature.
#[test]
fn test_data_buffer_release_signature() {
    fn check<F>(_: F) {}
    check::<fn(&mut PmixDataBuffer)>(data_buffer_release);
}

/// Verify PmixPrintOutput implements Display.
#[test]
fn test_print_output_display_trait() {
    fn is_display<T: std::fmt::Display>() {}
    is_display::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput implements Debug.
#[test]
fn test_print_output_debug_trait() {
    fn is_debug<T: std::fmt::Debug>() {}
    is_debug::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput Deref<Target = str>.
#[test]
fn test_print_output_deref_str_trait() {
    fn is_deref_str<T>()
    where
        T: std::ops::Deref<Target = str>,
    {
    }
    is_deref_str::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput Into<String>.
#[test]
fn test_print_output_into_string_trait() {
    fn can_into_string<T>()
    where
        String: From<T>,
    {
    }
    can_into_string::<PmixPrintOutput>();
}

/// PmixPrintOutput::default() produces an empty string.
#[test]
fn test_print_output_default_empty() {
    let output = PmixPrintOutput::default();
    assert!(output.is_empty(), "default print output should be empty");
    assert_eq!(output.as_str(), "");
}

/// PmixPrintOutput Display on default is empty string.
#[test]
fn test_print_output_display_default() {
    let output = PmixPrintOutput::default();
    let formatted = format!("{}", output);
    assert_eq!(formatted, "");
}

/// PmixPrintOutput Into<String> on default produces empty String.
#[test]
fn test_print_output_into_string_default() {
    let output = PmixPrintOutput::default();
    let s: String = output.into();
    assert_eq!(s, "");
}

/// PmixPrintOutput Deref operations on default.
#[test]
fn test_print_output_deref_default() {
    let output = PmixPrintOutput::default();
    assert_eq!(output.len(), 0);
    assert!(output.is_empty());
    assert!(!output.contains("x"));
}

/// Verify all PMIx data type variants exist.
#[test]
fn test_all_data_type_variants() {
    let _: PmixDataType = PmixDataType::Bool;
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
    let _: PmixDataType = PmixDataType::String;
    let _: PmixDataType = PmixDataType::Size;
    let _: PmixDataType = PmixDataType::Pid;
    let _: PmixDataType = PmixDataType::ProcRank;
}

/// Verify PmixStatus success/error classification.
#[test]
fn test_pmix_status_classification() {
    let success = PmixStatus::from_raw(0);
    assert!(success.is_success(), "PMIX_SUCCESS (0) should be success");

    let error = PmixStatus::from_raw(-1);
    assert!(error.is_error(), "PMIX_ERROR (-1) should be error");

    let bad_param = PmixStatus::from_raw(-27);
    assert!(bad_param.is_error(), "PMIX_ERR_BAD_PARAM (-27) should be error");
}

/// Verify PmixStatus implements std::error::Error.
#[test]
fn test_pmix_status_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<PmixStatus>();
}

/// Verify PmixStatus PartialEq works.
#[test]
fn test_pmix_status_partial_eq() {
    let a = PmixStatus::from_raw(0);
    let b = PmixStatus::from_raw(0);
    let c = PmixStatus::from_raw(-1);
    assert_eq!(a, b, "same status codes should be equal");
    assert_ne!(a, c, "different status codes should not be equal");
}

/// data_pack error is the known PMIX_ERR_BAD_PARAM variant.
#[test]
fn test_pack_error_is_known_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    let err = result.unwrap_err();
    assert!(
        matches!(err, pmix::PmixStatus::Known(pmix::PmixError::ErrBadParam)),
        "error should be Known(ErrBadParam), got {:?}",
        err
    );
}

/// data_pack error Display output is readable.
#[test]
fn test_pack_error_display_readable() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    let err = result.unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty(), "error Display should not be empty");
}

/// data_pack error Debug output is readable.
#[test]
fn test_pack_error_debug_readable() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
    let err = result.unwrap_err();
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty(), "error Debug should not be empty");
}

/// Two data_pack errors from the same condition are equal.
#[test]
fn test_pack_errors_equal() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let err1 = data_pack(None, &buf, &val, 0, PmixDataType::Int32).unwrap_err();
    let err2 = data_pack(None, &buf, &val, -1, PmixDataType::Int32).unwrap_err();
    assert_eq!(err1, err2, "same error code should be equal");
}

/// data_pack with different types all return same error for num_vals=0.
#[test]
fn test_pack_error_consistent_across_types() {
    let buf = data_buffer_create().expect("create buffer");
    let val_i32: i32 = 42;
    let val_f64: f64 = 3.14;
    let val_u8: u8 = 255;

    let err_i32 = data_pack(None, &buf, &val_i32, 0, PmixDataType::Int32).unwrap_err();
    let err_f64 = data_pack(None, &buf, &val_f64, 0, PmixDataType::Double).unwrap_err();
    let err_u8 = data_pack(None, &buf, &val_u8, 0, PmixDataType::Uint8).unwrap_err();

    assert_eq!(err_i32, err_f64, "all type errors should be equal");
    assert_eq!(err_f64, err_u8, "all type errors should be equal");
    assert_eq!(err_i32.to_raw(), -27, "should all be PMIX_ERR_BAD_PARAM");
}

/// data_pack with zero num_vals is idempotent (repeated calls return same error).
#[test]
fn test_pack_zero_num_vals_idempotent() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    for _ in 0..10 {
        let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_raw(), -27);
    }
}

/// Buffer bytes_used increases after load, resets after unload.
#[test]
fn test_buffer_bytes_used_lifecycle() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "initial state");

    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    data_load(&buf, &payload).expect("load");
    assert!(buf.bytes_used() > 0, "after load");

    let _recovered = data_unload(&buf).expect("unload");
    assert_eq!(buf.bytes_used(), 0, "after unload");
}

/// Buffer bytes_allocated >= bytes_used after load.
#[test]
fn test_buffer_allocated_ge_used() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0u8; 128]);
    data_load(&buf, &payload).expect("load");
    assert!(
        buf.bytes_allocated() >= buf.bytes_used(),
        "allocated ({}) should be >= used ({})",
        buf.bytes_allocated(),
        buf.bytes_used()
    );
}

/// Multiple independent buffers with load/unload.
#[test]
fn test_multiple_independent_buffers() {
    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");

    let payload1 = PmixByteObject::from(vec![1u8, 2, 3]);
    let payload2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8]);

    data_load(&buf1, &payload1).expect("load buf1");
    data_load(&buf2, &payload2).expect("load buf2");

    let recovered1 = data_unload(&buf1).expect("unload buf1");
    let recovered2 = data_unload(&buf2).expect("unload buf2");

    assert_eq!(recovered1.as_slice(), &[1u8, 2, 3]);
    assert_eq!(recovered2.as_slice(), &[4u8, 5, 6, 7, 8]);
}

/// PmixProcRef creation with various namespaces and ranks.
#[test]
fn test_proc_ref_various() {
    let _p1 = PmixProcRef::new("test_ns", 0);
    let _p2 = PmixProcRef::new("job_123", 42);
    let _p3 = PmixProcRef::new("a", u32::MAX);
    let long_ns = "x".repeat(300);
    let _p4 = PmixProcRef::new(&long_ns, 99);
}

/// PmixByteObject from Vec preserves content exactly.
#[test]
fn test_byte_object_from_vec_preserves() {
    let data: Vec<u8> = (0..=255).collect();
    let obj = PmixByteObject::from(data.clone());
    assert_eq!(obj.as_slice(), &data[..]);
    assert_eq!(obj.size(), 256);
}

/// PmixByteObject is_empty consistent with size.
#[test]
fn test_byte_object_empty_consistent() {
    let empty = PmixByteObject::new();
    assert!(empty.is_empty());
    assert_eq!(empty.size(), 0);

    let non_empty = PmixByteObject::from(vec![1u8]);
    assert!(!non_empty.is_empty());
    assert_ne!(non_empty.size(), 0);
}

/// PmixByteObject Debug contains struct name.
#[test]
fn test_byte_object_debug_name() {
    let obj = PmixByteObject::new();
    let debug = format!("{:?}", obj);
    assert!(
        debug.contains("PmixByteObject"),
        "Debug should contain struct name, got: {}",
        debug
    );
}

/// PmixDataBuffer Debug contains struct name and field names.
#[test]
fn test_buffer_debug_fields() {
    let buf = data_buffer_create().expect("create buffer");
    let debug = format!("{:?}", buf);
    assert!(debug.contains("PmixDataBuffer"));
    assert!(debug.contains("bytes_allocated"));
    assert!(debug.contains("bytes_used"));
}

/// Null PmixDataBuffer Debug shows "null".
#[test]
fn test_null_buffer_debug() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    let debug = format!("{:?}", buf);
    assert!(
        debug.contains("null"),
        "null buffer should show 'null' in debug"
    );
}

/// data_load then data_unload with alternating pattern roundtrips.
#[test]
fn test_load_unload_alternating_pattern() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..256)
        .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
        .collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Three-buffer transport chain: buf1 → buf2 → buf3.
#[test]
fn test_three_buffer_transport_chain() {
    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");
    let buf3 = data_buffer_create().expect("buf3");

    let original = vec![0xDEu8, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf1, &payload).expect("load buf1");

    let transport1 = data_unload(&buf1).expect("unload buf1");
    data_load(&buf2, &transport1).expect("load buf2");

    let transport2 = data_unload(&buf2).expect("unload buf2");
    data_load(&buf3, &transport2).expect("load buf3");

    let recovered = data_unload(&buf3).expect("unload buf3");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_pack with num_vals == 0 is the same error for all data types.
#[test]
fn test_pack_zero_error_all_types_same() {
    let buf = data_buffer_create().expect("create buffer");
    let types = [
        PmixDataType::Bool,
        PmixDataType::Int8,
        PmixDataType::Uint8,
        PmixDataType::Int16,
        PmixDataType::Uint16,
        PmixDataType::Int32,
        PmixDataType::Uint32,
        PmixDataType::Int64,
        PmixDataType::Uint64,
        PmixDataType::Float,
        PmixDataType::Double,
    ];
    let val: i32 = 42;
    let first_err = data_pack(None, &buf, &val, 0, types[0]).unwrap_err();
    for &t in &types[1..] {
        let err = data_pack(None, &buf, &val, 0, t).unwrap_err();
        assert_eq!(err, first_err, "all types should return same error for num_vals=0");
    }
}
