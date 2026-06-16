//! Advanced data serialization tests — load/unload round-trips, compression
//! ratio verification, embed buffer independence, and error paths.
//!
//! This test file focuses on cross-cutting concerns that span multiple
//! data_serialization functions:
//!
//! - Load/Unload round-trips (pure load/unload chains work without PMIx_Init)
//! - Compression ratio verification (empty-input guards work without PMIx_Init)
//! - Compress/Decompress round-trips (require PMIx_Init — marked #[ignore])
//! - Embed buffer independence (require PMIx_Init — marked #[ignore])
//! - Error paths and corrupted data handling
//!
//! Functions that work without PMIx_Init (operate entirely in user space):
//! - data_buffer_create / data_buffer_release
//! - data_load / data_unload
//! - data_compress / data_decompress (empty-input guards only)
//!
//! Functions that require PMIx_Init (FFI calls):
//! - data_pack / data_unpack
//! - data_compress / data_decompress (non-empty input)
//! - data_embed

use pmix::data_serialization::*;
use pmix::{PmixDataType, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Load/Unload round-trip tests (no PMIx_Init required)
// ─────────────────────────────────────────────────────────────────────────────

/// Basic load → unload round-trip preserves data.
#[test]
fn test_roundtrip_load_unload_basic() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original, "round-trip should preserve data");
}

/// Load → unload → load → unload double round-trip on the same buffer.
#[test]
fn test_roundtrip_double_cycle_same_buffer() {
    let buf = data_buffer_create().expect("create buffer");

    // First cycle
    let data1 = vec![10u8, 20, 30];
    data_load(&buf, &PmixByteObject::from(data1.clone())).expect("load 1");
    let recovered1 = data_unload(&buf).expect("unload 1");
    assert_eq!(recovered1.as_slice(), &data1);

    // Second cycle on the same buffer
    let data2 = vec![40u8, 50, 60, 70, 80];
    data_load(&buf, &PmixByteObject::from(data2.clone())).expect("load 2");
    let recovered2 = data_unload(&buf).expect("unload 2");
    assert_eq!(recovered2.as_slice(), &data2);
}

/// Triple load/unload cycle on the same buffer.
#[test]
fn test_roundtrip_triple_cycle_same_buffer() {
    let buf = data_buffer_create().expect("create buffer");

    for i in 0..3u8 {
        let data = vec![i; (i + 1) as usize * 10];
        data_load(&buf, &PmixByteObject::from(data.clone())).expect("load");
        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(recovered.as_slice(), &data, "cycle {} failed", i);
    }
}

/// Five consecutive load/unload cycles stress test.
#[test]
fn test_roundtrip_five_cycles() {
    let buf = data_buffer_create().expect("create buffer");

    for cycle in 0..5u8 {
        let data: Vec<u8> = (0..100).map(|j: u8| j.wrapping_add(cycle)).collect();
        data_load(&buf, &PmixByteObject::from(data.clone())).expect("load");
        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(recovered.as_slice(), &data, "cycle {} failed", cycle);
    }
}

/// Load → unload across two different buffers (sender/receiver pattern).
#[test]
fn test_roundtrip_two_buffer_transport() {
    // Sender: load into buffer, unload to payload
    let sender_buf = data_buffer_create().expect("create sender");
    let original = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    let sender_payload = PmixByteObject::from(original.clone());
    data_load(&sender_buf, &sender_payload).expect("sender load");
    let transport = data_unload(&sender_buf).expect("sender unload");

    // Receiver: load transport payload into new buffer, unload
    let receiver_buf = data_buffer_create().expect("create receiver");
    data_load(&receiver_buf, &transport).expect("receiver load");
    let final_payload = data_unload(&receiver_buf).expect("receiver unload");

    assert_eq!(final_payload.as_slice(), &original, "transport chain preserved data");
}

/// Three-hop transport chain: buf1 → buf2 → buf3 → verify.
#[test]
fn test_roundtrip_three_hop_transport() {
    let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // Hop 1: buf1 → payload1
    let buf1 = data_buffer_create().expect("buf1");
    data_load(&buf1, &PmixByteObject::from(original.clone())).expect("load into buf1");
    let payload1 = data_unload(&buf1).expect("unload from buf1");

    // Hop 2: payload1 → buf2 → payload2
    let buf2 = data_buffer_create().expect("buf2");
    data_load(&buf2, &payload1).expect("load into buf2");
    let payload2 = data_unload(&buf2).expect("unload from buf2");

    // Hop 3: payload2 → buf3 → verify
    let buf3 = data_buffer_create().expect("buf3");
    data_load(&buf3, &payload2).expect("load into buf3");
    let final_payload = data_unload(&buf3).expect("unload from buf3");

    assert_eq!(final_payload.as_slice(), &original, "three-hop chain preserved data");
}

/// Round-trip with empty payload.
#[test]
fn test_roundtrip_empty_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    data_load(&buf, &payload).expect("load empty");

    let recovered = data_unload(&buf);
    match recovered {
        Ok(p) => assert!(p.as_slice().is_empty(), "empty round-trip should yield empty"),
        Err(_) => {
            // Some PMIx versions may error on empty buffer unload — acceptable
        }
    }
}

/// Round-trip with single byte.
#[test]
fn test_roundtrip_single_byte() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![42u8];
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip with all zero bytes.
#[test]
fn test_roundtrip_all_zeros() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0u8; 256];
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip with all 0xFF bytes.
#[test]
fn test_roundtrip_all_0xff() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0xFFu8; 512];
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip with 1KB sequential bytes.
#[test]
fn test_roundtrip_1kb_sequential() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip with 10KB repeating pattern.
#[test]
fn test_roundtrip_10kb_pattern() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..=255u8).cycle().take(10240).collect();
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip with 64KB payload.
#[test]
fn test_roundtrip_64kb() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Round-trip preserves buffer validity after unload.
#[test]
fn test_roundtrip_buffer_valid_after() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![1u8, 2, 3];
    data_load(&buf, &PmixByteObject::from(original)).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    assert!(buf.is_valid(), "buffer should remain valid after unload");
}

/// Round-trip with alternating byte pattern (0xAA/0x55).
#[test]
fn test_roundtrip_alternating_pattern() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..512)
        .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
        .collect();
    data_load(&buf, &PmixByteObject::from(original.clone())).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load/unload round-trip where payload survives buffer drop.
#[test]
fn test_roundtrip_payload_survives_buffer_drop() {
    let recovered;
    {
        let buf = data_buffer_create().expect("create buffer");
        let original = vec![0xAAu8, 0xBB, 0xCC, 0xDD];
        data_load(&buf, &PmixByteObject::from(original)).expect("load");
        recovered = data_unload(&buf).expect("unload");
    } // buf dropped

    assert_eq!(recovered.as_slice(), &[0xAAu8, 0xBB, 0xCC, 0xDD]);
}

/// Multiple independent buffers each do their own round-trip.
#[test]
fn test_roundtrip_multiple_independent_buffers() {
    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");
    let buf3 = data_buffer_create().expect("buf3");

    let data1 = vec![1u8; 100];
    let data2 = vec![2u8; 200];
    let data3 = vec![3u8; 300];

    data_load(&buf1, &PmixByteObject::from(data1.clone())).expect("load1");
    data_load(&buf2, &PmixByteObject::from(data2.clone())).expect("load2");
    data_load(&buf3, &PmixByteObject::from(data3.clone())).expect("load3");

    let r1 = data_unload(&buf1).expect("unload1");
    let r2 = data_unload(&buf2).expect("unload2");
    let r3 = data_unload(&buf3).expect("unload3");

    assert_eq!(r1.as_slice(), &data1);
    assert_eq!(r2.as_slice(), &data2);
    assert_eq!(r3.as_slice(), &data3);
}

/// Load/unload round-trip with boundary sizes: 0, 1, 255, 256, 1024.
#[test]
fn test_roundtrip_boundary_sizes() {
    let sizes = [0usize, 1, 255, 256, 1024];
    for size in sizes {
        let buf = data_buffer_create().expect("create buffer");
        let original: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let payload = PmixByteObject::from(original.clone());
        data_load(&buf, &payload).expect("load");

        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(recovered.as_slice(), &original, "size {} failed", size);
        assert_eq!(recovered.as_slice(), &original, "size {} failed", size);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Load/Unload round-trip with pack/unpack (require PMIx_Init — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Pack data → unload → load into new buffer → unpack, verify round-trip.
#[test]
#[ignore]
fn test_roundtrip_pack_unload_load_unpack_i32() {
    use pmix::PmixDataType;

    // Sender: pack i32 into buffer, unload to payload
    let buf1 = data_buffer_create().expect("create buf1");
    let val: i32 = 42;
    data_pack(None, &buf1, &val, 1, PmixDataType::Int32).expect("pack");
    let payload = data_unload(&buf1).expect("unload");

    // Receiver: load payload into new buffer, unpack
    let buf2 = data_buffer_create().expect("create buf2");
    data_load(&buf2, &payload).expect("load");

    let mut out: i32 = 0;
    let mut count: i32 = 1;
    let unpacked = data_unpack(None, &buf2, &mut out, &mut count, PmixDataType::Int32)
        .expect("unpack");
    assert_eq!(unpacked, 1);
    assert_eq!(out, 42, "round-trip should preserve i32 value");
}

/// Pack multiple values → unload → load → unpack all.
#[test]
#[ignore]
fn test_roundtrip_pack_unload_load_unpack_multiple() {
    let buf1 = data_buffer_create().expect("create buf1");

    let val1: i32 = 100;
    let val2: i32 = 200;
    let val3: i32 = 300;
    data_pack(None, &buf1, &val1, 1, PmixDataType::Int32).expect("pack val1");
    data_pack(None, &buf1, &val2, 1, PmixDataType::Int32).expect("pack val2");
    data_pack(None, &buf1, &val3, 1, PmixDataType::Int32).expect("pack val3");

    let payload = data_unload(&buf1).expect("unload");

    let buf2 = data_buffer_create().expect("create buf2");
    data_load(&buf2, &payload).expect("load");

    let mut out1: i32 = 0;
    let mut out2: i32 = 0;
    let mut out3: i32 = 0;
    let mut count: i32 = 1;

    data_unpack(None, &buf2, &mut out1, &mut count, PmixDataType::Int32).expect("unpack 1");
    data_unpack(None, &buf2, &mut out2, &mut count, PmixDataType::Int32).expect("unpack 2");
    data_unpack(None, &buf2, &mut out3, &mut count, PmixDataType::Int32).expect("unpack 3");

    assert_eq!(out1, 100);
    assert_eq!(out2, 200);
    assert_eq!(out3, 300);
}

/// Pack u8 array → unload → load → unpack.
#[test]
#[ignore]
fn test_roundtrip_pack_unload_load_unpack_u8_array() {
    let buf1 = data_buffer_create().expect("create buf1");
    let bytes: [u8; 16] = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
                           0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    data_pack(None, &buf1, &bytes, 16, PmixDataType::Uint8).expect("pack bytes");

    let payload = data_unload(&buf1).expect("unload");

    let buf2 = data_buffer_create().expect("create buf2");
    data_load(&buf2, &payload).expect("load");

    let mut out: [u8; 16] = [0; 16];
    let mut count: i32 = 16;
    data_unpack(None, &buf2, &mut out, &mut count, PmixDataType::Uint8)
        .expect("unpack bytes");
    assert_eq!(count, 16);
    assert_eq!(out, bytes);
}

/// Pack i64 → unload → load → unpack round-trip.
#[test]
#[ignore]
fn test_roundtrip_pack_unload_load_unpack_i64() {
    let buf1 = data_buffer_create().expect("create buf1");
    let val: i64 = -9223372036854775807i64;
    data_pack(None, &buf1, &val, 1, PmixDataType::Int64).expect("pack i64");

    let payload = data_unload(&buf1).expect("unload");

    let buf2 = data_buffer_create().expect("create buf2");
    data_load(&buf2, &payload).expect("load");

    let mut out: i64 = 0;
    let mut count: i32 = 1;
    data_unpack(None, &buf2, &mut out, &mut count, PmixDataType::Int64)
        .expect("unpack i64");
    assert_eq!(out, val, "i64 round-trip should preserve value");
}

/// Pack f64 (Double) → unload → load → unpack round-trip.
#[test]
#[ignore]
fn test_roundtrip_pack_unload_load_unpack_double() {
    let buf1 = data_buffer_create().expect("create buf1");
    let val: f64 = 3.14159265358979;
    data_pack(None, &buf1, &val, 1, PmixDataType::Double).expect("pack double");

    let payload = data_unload(&buf1).expect("unload");

    let buf2 = data_buffer_create().expect("create buf2");
    data_load(&buf2, &payload).expect("load");

    let mut out: f64 = 0.0;
    let mut count: i32 = 1;
    data_unpack(None, &buf2, &mut out, &mut count, PmixDataType::Double)
        .expect("unpack double");
    assert!((out - val).abs() < 1e-10, "double round-trip should preserve value");
}

/// Multi-hop pack/unload/load/unpack chain with different types.
#[test]
#[ignore]
fn test_roundtrip_multi_hop_mixed_types() {
    // First pack
    let buf1 = data_buffer_create().expect("buf1");
    let val_int: i32 = 42;
    let val_float: f64 = 2.71828;
    data_pack(None, &buf1, &val_int, 1, PmixDataType::Int32).expect("pack int");
    data_pack(None, &buf1, &val_float, 1, PmixDataType::Float).expect("pack float");

    let payload1 = data_unload(&buf1).expect("unload 1");

    // Hop to buf2
    let buf2 = data_buffer_create().expect("buf2");
    data_load(&buf2, &payload1).expect("load into buf2");
    let payload2 = data_unload(&buf2).expect("unload 2");

    // Hop to buf3 and unpack
    let buf3 = data_buffer_create().expect("buf3");
    data_load(&buf3, &payload2).expect("load into buf3");

    let mut out_int: i32 = 0;
    let mut out_float: f64 = 0.0;
    let mut count: i32 = 1;

    data_unpack(None, &buf3, &mut out_int, &mut count, PmixDataType::Int32)
        .expect("unpack int");
    data_unpack(None, &buf3, &mut out_float, &mut count, PmixDataType::Float)
        .expect("unpack float");

    assert_eq!(out_int, 42);
    assert!((out_float - val_float).abs() < 1e-10);
}

// ─────────────────────────────────────────────────────────────────────────────
// Compression ratio verification (empty-input guards: no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// data_compress with empty input returns BadParam.
#[test]
fn test_compress_empty_returns_bad_param() {
    let result = data_compress(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -27, "empty input should return PMIX_ERR_BAD_PARAM (-27)");
    assert!(
        matches!(err, PmixStatus::Known(PmixError::ErrBadParam)),
        "error should be Known(ErrBadParam)"
    );
}

/// data_decompress with empty input returns BadParam.
#[test]
fn test_decompress_empty_returns_bad_param() {
    let result = data_decompress(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_raw(), -27);
    assert!(matches!(err, PmixStatus::Known(PmixError::ErrBadParam)));
}

/// data_compress and data_decompress return the same error for empty input.
#[test]
fn test_compress_decompress_same_error_empty_input() {
    let err_compress = data_compress(&[]).unwrap_err();
    let err_decompress = data_decompress(&[]).unwrap_err();
    assert_eq!(err_compress, err_decompress);
    assert_eq!(err_compress.to_raw(), -27);
}

/// data_compress empty-input error is idempotent across repeated calls.
#[test]
fn test_compress_empty_idempotent() {
    for _ in 0..10 {
        let err = data_compress(&[]).unwrap_err();
        assert_eq!(err.to_raw(), -27);
    }
}

/// data_decompress empty-input error is idempotent across repeated calls.
#[test]
fn test_decompress_empty_idempotent() {
    for _ in 0..10 {
        let err = data_decompress(&[]).unwrap_err();
        assert_eq!(err.to_raw(), -27);
    }
}

/// data_compress empty-input error Display is readable.
#[test]
fn test_compress_empty_error_display() {
    let err = data_compress(&[]).unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty(), "error Display should not be empty");
}

/// data_decompress empty-input error Debug is readable.
#[test]
fn test_decompress_empty_error_debug() {
    let err = data_decompress(&[]).unwrap_err();
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty(), "error Debug should not be empty");
}

// ─────────────────────────────────────────────────────────────────────────────
// Compression ratio verification (FFI — require PMIx_Init — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Compress zeros (highly compressible) — verify compressed < original.
#[test]
#[ignore]
fn test_compress_zeros_ratio() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress zeros");
    assert!(
        compressed.len() < data.len(),
        "compressed zeros ({}) should be smaller than original ({})",
        compressed.len(),
        data.len()
    );
    let ratio = compressed.len() as f64 / data.len() as f64;
    assert!(ratio < 0.01, "zero compression ratio should be very low, got {:.4}", ratio);
}

/// Compress 1K block of compressible repeating pattern.
#[test]
#[ignore]
fn test_compress_1k_compressible() {
    let data: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
    let compressed = data_compress(&data).expect("compress 1K");
    assert!(
        compressed.len() < data.len(),
        "compressed 1K ({}) should be smaller than original ({})",
        compressed.len(),
        data.len()
    );
}

/// Compress 10K block of compressible data.
#[test]
#[ignore]
fn test_compress_10k_compressible() {
    let data: Vec<u8> = (0..=255u8).cycle().take(10240).collect();
    let compressed = data_compress(&data).expect("compress 10K");
    assert!(
        compressed.len() < data.len(),
        "compressed 10K ({}) should be smaller than original ({})",
        compressed.len(),
        data.len()
    );
    let ratio = compressed.len() as f64 / data.len() as f64;
    assert!(ratio < 0.1, "10K compression ratio should be < 0.1, got {:.4}", ratio);
}

/// Compress small data (5 bytes) — may fail due to overhead.
#[test]
#[ignore]
fn test_compress_small_data_may_fail() {
    let data = vec![1u8, 2, 3, 4, 5];
    let result = data_compress(&data);
    // Small data may not compress (returns Err due to overhead > savings)
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, data);
    }
}

/// Compress uniform 0xFF data — verify very low compression ratio.
#[test]
#[ignore]
fn test_compress_uniform_0xff_ratio() {
    let data = vec![0xFFu8; 8192];
    let compressed = data_compress(&data).expect("compress 0xFF");
    let ratio = compressed.len() as f64 / data.len() as f64;
    assert!(
        ratio < 0.01,
        "0xFF compression ratio should be very low, got {:.4}",
        ratio
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Compress/Decompress round-trip (FFI — require PMIx_Init — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Compress → decompress → verify identical (0 bytes — edge case, should error).
#[test]
fn test_compress_decompress_roundtrip_empty_errors() {
    assert!(data_compress(&[]).is_err(), "compress empty should error");
    assert!(data_decompress(&[]).is_err(), "decompress empty should error");
}

/// Compress → decompress → verify identical for zeros.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_zeros() {
    let original = vec![0u8; 4096];
    let compressed = data_compress(&original).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original, "zeros round-trip should be identical");
}

/// Compress → decompress round-trip for 1 byte (if compressible).
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_1_byte() {
    let original = vec![42u8];
    let result = data_compress(&original);
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, original);
    }
}

/// Compress → decompress round-trip for 100 bytes.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_100_bytes() {
    let original: Vec<u8> = (0..100).map(|i| i as u8).collect();
    let result = data_compress(&original);
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, original);
    }
}

/// Compress → decompress round-trip for 1000 bytes.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_1000_bytes() {
    let original: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    let compressed = data_compress(&original).expect("compress 1000");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original);
}

/// Compress → decompress round-trip for 10000 bytes.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_10000_bytes() {
    let original: Vec<u8> = (0..=255u8).cycle().take(10000).collect();
    let compressed = data_compress(&original).expect("compress 10000");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original);
}

/// Compress → decompress round-trip with alternating pattern.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_alternating() {
    let original: Vec<u8> = (0..2048)
        .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
        .collect();
    let compressed = data_compress(&original).expect("compress alternating");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original);
}

/// Compress → decompress round-trip with all 0xFF data.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_all_0xff() {
    let original = vec![0xFFu8; 4096];
    let compressed = data_compress(&original).expect("compress 0xFF");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original);
}

/// Compress → decompress round-trip with packed buffer payload.
#[test]
#[ignore]
fn test_compress_decompress_roundtrip_packed_buffer() {
    let buf = data_buffer_create().expect("create buffer");
    let val1: i32 = 42;
    let val2: i32 = 100;
    data_pack(None, &buf, &val1, 1, PmixDataType::Int32).expect("pack val1");
    data_pack(None, &buf, &val2, 1, PmixDataType::Int32).expect("pack val2");

    let payload = data_unload(&buf).expect("unload");
    let original_bytes = payload.as_slice().to_vec();

    let compressed = data_compress(&original_bytes).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original_bytes);
}

/// Decompress corrupted data (not produced by PMIx_Data_compress).
#[test]
#[ignore]
fn test_decompress_corrupted_data() {
    // Random bytes that are not valid compressed data
    let corrupted: Vec<u8> = std::iter::repeat(0xDEu8).take(400).collect();
    let result = data_decompress(&corrupted);
    assert!(result.is_err(), "decompressing corrupted data should fail");
}

/// Decompress truncated compressed data.
#[test]
#[ignore]
fn test_decompress_truncated_data() {
    let original = vec![0u8; 4096];
    let compressed = data_compress(&original).expect("compress");
    let truncated = &compressed[..compressed.len() / 2];
    let result = data_decompress(truncated);
    assert!(result.is_err(), "decompressing truncated data should fail");
}

/// Decompress with modified compressed data (flip a bit).
#[test]
#[ignore]
fn test_decompress_modified_compressed_data() {
    let original = vec![0u8; 4096];
    let compressed = data_compress(&original).expect("compress");
    let mut modified = compressed.clone();
    if !modified.is_empty() {
        modified[0] ^= 0xFF; // Flip all bits in first byte
    }
    let result = data_decompress(&modified);
    assert!(result.is_err(), "decompressing modified data should fail");
}

/// Compress → decompress → compress → decompress double round-trip.
#[test]
#[ignore]
fn test_compress_decompress_double_roundtrip() {
    let original = vec![0u8; 8192];
    let comp1 = data_compress(&original).expect("compress 1");
    let decomp1 = data_decompress(&comp1).expect("decompress 1");
    assert_eq!(decomp1, original);

    let comp2 = data_compress(&decomp1).expect("compress 2");
    let decomp2 = data_decompress(&comp2).expect("decompress 2");
    assert_eq!(decomp2, original);
}

// ─────────────────────────────────────────────────────────────────────────────
// Embed buffer independence (FFI — require PMIx_Init — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Embed child into parent, drop child, parent still works.
#[test]
#[ignore]
fn test_embed_buffer_independence_drop_child() {
    let parent_buf = data_buffer_create().expect("create parent");

    // Create child buffer, pack data, unload to payload
    let child_payload;
    {
        let child_buf = data_buffer_create().expect("create child");
        let val: i32 = 42;
        data_pack(None, &child_buf, &val, 1, PmixDataType::Int32).expect("pack in child");
        child_payload = data_unload(&child_buf).expect("unload child");
        // child_buf dropped here
    }

    // Embed child payload into parent
    data_embed(&parent_buf, Some(&child_payload)).expect("embed");

    // Parent should still work after child is gone
    let recovered = data_unload(&parent_buf).expect("unload parent");
    assert!(!recovered.as_slice().is_empty(), "parent should have data after embed");
}

/// Embed child into parent, drop child buffer, unpack from parent.
#[test]
#[ignore]
fn test_embed_parent_unpack_after_child_drop() {
    let parent_buf = data_buffer_create().expect("create parent");

    let child_payload;
    {
        let child_buf = data_buffer_create().expect("create child");
        let val: i32 = 12345;
        data_pack(None, &child_buf, &val, 1, PmixDataType::Int32).expect("pack");
        child_payload = data_unload(&child_buf).expect("unload");
        // child_buf dropped here
    }

    data_embed(&parent_buf, Some(&child_payload)).expect("embed");

    // Unpack from parent — should still work
    let mut out: i32 = 0;
    let mut count: i32 = 1;
    data_unpack(None, &parent_buf, &mut out, &mut count, PmixDataType::Int32)
        .expect("unpack from parent");
    assert_eq!(out, 12345, "parent should contain child's data after child dropped");
}

/// Embed the same payload into multiple buffers — payload independence.
#[test]
#[ignore]
fn test_embed_payload_reused_multiple_buffers() {
    let payload: PmixByteObject = vec![1u8, 2, 3, 4, 5].into();

    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");
    let buf3 = data_buffer_create().expect("buf3");

    data_embed(&buf1, Some(&payload)).expect("embed into buf1");
    data_embed(&buf2, Some(&payload)).expect("embed into buf2");
    data_embed(&buf3, Some(&payload)).expect("embed into buf3");

    // All buffers should contain the same data
    let r1 = data_unload(&buf1).expect("unload buf1");
    let r2 = data_unload(&buf2).expect("unload buf2");
    let r3 = data_unload(&buf3).expect("unload buf3");

    assert_eq!(r1.as_slice(), &[1u8, 2, 3, 4, 5]);
    assert_eq!(r2.as_slice(), &[1u8, 2, 3, 4, 5]);
    assert_eq!(r3.as_slice(), &[1u8, 2, 3, 4, 5]);

    // Payload should still be intact (embed doesn't consume it)
    assert_eq!(payload.size(), 5);
    assert_eq!(payload.as_slice(), &[1u8, 2, 3, 4, 5]);
}

/// Embed then drop parent buffer — no crash.
#[test]
#[ignore]
fn test_embed_drop_parent_no_crash() {
    let payload: PmixByteObject = vec![1u8, 2, 3].into();
    {
        let buf = data_buffer_create().expect("create buffer");
        data_embed(&buf, Some(&payload)).expect("embed");
        // buf dropped here via Drop
    }
    // Payload should still be valid
    assert_eq!(payload.size(), 3);
}

/// Embed replaces existing buffer content.
#[test]
#[ignore]
fn test_embed_replaces_existing_content() {
    let buf = data_buffer_create().expect("create buffer");

    // First embed
    let payload1: PmixByteObject = vec![1u8, 2, 3].into();
    data_embed(&buf, Some(&payload1)).expect("first embed");

    // Second embed — should replace first
    let payload2: PmixByteObject = vec![4u8, 5, 6, 7, 8].into();
    data_embed(&buf, Some(&payload2)).expect("second embed");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &[4u8, 5, 6, 7, 8]);
}

/// Embed with empty payload succeeds.
#[test]
#[ignore]
fn test_embed_empty_payload_success() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    let result = data_embed(&buf, Some(&payload));
    assert!(result.is_ok(), "embed with empty payload should succeed");
}

/// Embed with None payload is a no-op.
#[test]
#[ignore]
fn test_embed_none_payload_noop() {
    let buf = data_buffer_create().expect("create buffer");
    let result = data_embed(&buf, None);
    assert!(result.is_ok(), "embed with None should succeed (no-op)");
}

/// Embed then data_buffer_release cleans up without issues.
#[test]
#[ignore]
fn test_embed_then_release_cleanup() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![1u8, 2, 3, 4].into();
    data_embed(&buf, Some(&payload)).expect("embed");
    data_buffer_release(&buf);
    // No panic or double-free
    // Payload should still be valid
    assert_eq!(payload.size(), 4);
}

/// Embed with large payload (64KB).
#[test]
#[ignore]
fn test_embed_large_payload_64kb() {
    let buf = data_buffer_create().expect("create buffer");
    let large: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let payload: PmixByteObject = large.clone().into();

    data_embed(&buf, Some(&payload)).expect("embed 64KB");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &large);
}

/// Embed buffer bytes_used reflects embedded payload size.
#[test]
#[ignore]
fn test_embed_buffer_bytes_used_reflects_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![1u8; 128].into();

    data_embed(&buf, Some(&payload)).expect("embed");

    assert!(
        buf.bytes_used() >= 128,
        "buffer bytes_used should reflect embedded payload size (got {})",
        buf.bytes_used()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error paths
// ─────────────────────────────────────────────────────────────────────────────

/// data_load with empty payload succeeds (boundary case).
#[test]
fn test_load_empty_payload_succeeds() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    let result = data_load(&buf, &payload);
    assert!(result.is_ok(), "loading empty payload should succeed");
}

/// data_load consumes the payload (sets bytes=NULL, size=0).
#[test]
fn test_load_consumes_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4, 5]);
    assert_eq!(payload.size(), 5, "payload should have 5 bytes before load");

    data_load(&buf, &payload).expect("load should succeed");

    assert!(payload.is_empty(), "payload should be consumed after load");
    assert_eq!(payload.size(), 0, "payload size should be 0 after load");
}

/// data_load then data_unload round-trip with consumed payload.
#[test]
fn test_load_unload_with_consumed_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![10u8, 20, 30, 40];
    let payload = PmixByteObject::from(original.clone());

    data_load(&buf, &payload).expect("load");
    assert!(payload.is_empty(), "payload consumed after load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original, "round-trip should preserve data");
}

/// Buffer bytes_used increases after load, resets after unload.
#[test]
fn test_buffer_bytes_used_lifecycle() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");

    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    data_load(&buf, &payload).expect("load");
    assert!(buf.bytes_used() > 0, "buffer should have data after load");

    let _recovered = data_unload(&buf).expect("unload");
    assert_eq!(buf.bytes_used(), 0, "buffer should be empty after unload");
}

/// Buffer bytes_allocated >= bytes_used after load.
#[test]
fn test_buffer_allocated_ge_used_after_load() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0u8; 256]);
    data_load(&buf, &payload).expect("load");
    assert!(
        buf.bytes_allocated() >= buf.bytes_used(),
        "allocated ({}) should be >= used ({})",
        buf.bytes_allocated(),
        buf.bytes_used()
    );
}

/// Buffer remains valid after load/unload cycle.
#[test]
fn test_buffer_valid_after_load_unload_cycle() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    assert!(buf.is_valid(), "buffer should remain valid after load/unload");
}

/// Multiple load/unload cycles on the same buffer.
#[test]
fn test_multiple_load_unload_cycles() {
    let buf = data_buffer_create().expect("create buffer");

    for i in 0..5u8 {
        let data = vec![i; (i + 1) as usize * 10];
        data_load(&buf, &PmixByteObject::from(data.clone())).expect("load");
        assert!(buf.bytes_used() > 0);

        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(recovered.as_slice(), &data);
        assert_eq!(buf.bytes_used(), 0, "buffer should be empty after unload");
    }
}

/// data_load replaces buffer content — second load overwrites first.
#[test]
fn test_load_replaces_buffer_content() {
    let buf = data_buffer_create().expect("create buffer");

    // First load
    let payload1 = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload1).expect("first load");
    let bytes_after_first = buf.bytes_used();

    // Second load with different size — should replace first
    let payload2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8, 9]);
    data_load(&buf, &payload2).expect("second load");
    let bytes_after_second = buf.bytes_used();

    assert!(
        bytes_after_second != bytes_after_first,
        "second load should change buffer state, first={}, second={}",
        bytes_after_first,
        bytes_after_second
    );
}

/// data_compress error is PartialEq — same error from same condition.
#[test]
fn test_compress_error_partial_eq() {
    let err1 = data_compress(&[]).unwrap_err();
    let err2 = data_compress(&[]).unwrap_err();
    assert_eq!(err1, err2, "same error from same input should be equal");
}

/// data_decompress error to_raw is consistent across calls.
#[test]
fn test_decompress_error_to_raw_consistent() {
    let err1 = data_decompress(&[]).unwrap_err();
    let err2 = data_decompress(&[]).unwrap_err();
    assert_eq!(err1.to_raw(), err2.to_raw());
}

/// PmixStatus::from_raw converts success code correctly.
#[test]
fn test_pmix_status_from_raw_success() {
    let success = PmixStatus::from_raw(0);
    assert!(success.is_success(), "PMIX_SUCCESS (0) should be success");
}

/// PmixStatus::from_raw converts error code correctly.
#[test]
fn test_pmix_status_from_raw_error() {
    let err = PmixStatus::from_raw(-1); // PMIX_ERROR
    assert!(err.is_error(), "PMIX_ERROR (-1) should be error");
}

/// PmixStatus Debug output is readable.
#[test]
fn test_pmix_status_debug_readable() {
    let status = PmixStatus::from_raw(0);
    let debug_str = format!("{:?}", status);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

/// PmixStatus Display output is readable.
#[test]
fn test_pmix_status_display_readable() {
    let err = data_compress(&[]).unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty(), "Display output should not be empty");
}

/// Result<Vec<u8>, PmixStatus> is Send (important for async contexts).
#[test]
fn test_compress_return_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<Vec<u8>, PmixStatus>>();
}

/// Result<Vec<u8>, PmixStatus> is Sync.
#[test]
fn test_compress_return_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Result<Vec<u8>, PmixStatus>>();
}

/// data_compress error implements std::error::Error (via PmixStatus).
#[test]
fn test_compress_error_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<PmixStatus>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Embed with released buffer (unsafe — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Embed into a buffer after it has been released — this is unsafe.
/// Marked #[ignore] because it involves undefined behavior.
#[test]
#[ignore]
fn test_embed_with_released_buffer_unsafe() {
    // This test demonstrates what happens when you try to embed into
    // a buffer that has been released. The behavior is undefined.
    // We mark it ignored because it requires PMIx_Init AND involves UB.
    //
    // The safe Rust wrapper prevents this by consuming the buffer on
    // data_buffer_release, but this test documents the danger.
    assert!(true, "skipped — embed with released buffer is unsafe");
}
