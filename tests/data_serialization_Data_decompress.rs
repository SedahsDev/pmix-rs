//! Tests for `PMIx_Data_decompress`.
//!
//! `PMIx_Data_decompress` reverses the lossless compression performed by
//! `PMIx_Data_compress`. It uses zlib internally. Only data produced by
//! `PMIx_Data_compress` can be safely decompressed — passing arbitrary
//! compressed data (e.g., raw zlib streams from other libraries) leads to
//! undefined behavior.
//!
//! The function requires `PMIx_Init` because the internal compression
//! framework (pcompress) is selected during init. Integration tests that
//! exercise the FFI are marked `#[ignore]`.
//!
//! The signature and type-safety tests below do NOT require PMIx_Init
//! and run normally.

use pmix::PmixStatus;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only type checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// data_decompress function exists and has the correct signature.
///
/// Verifies the function is callable with `&[u8]` and returns
/// `Result<Vec<u8>, PmixStatus>` without actually invoking the FFI
/// (which would segfault without PMIx_Init).
#[test]
fn test_data_decompress_signature() {
    let _fn_ref: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
}

/// data_decompress is publicly exported from the crate.
#[test]
fn test_data_decompress_public_api() {
    use pmix::data_serialization;
    let _ = data_serialization::data_decompress;
}

/// data_decompress returns Err for empty input without calling FFI.
///
/// The implementation checks `input.is_empty()` before making the FFI call,
/// so this test does not require PMIx_Init and will not segfault.
#[test]
fn test_data_decompress_empty_input() {
    let result = data_decompress(&[]);
    assert!(
        result.is_err(),
        "data_decompress should reject empty input without calling FFI"
    );
}

/// data_decompress error for empty input is PMIX_ERR_BAD_PARAM (-27).
#[test]
fn test_data_decompress_empty_is_bad_param() {
    let result = data_decompress(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // PMIX_ERR_BAD_PARAM = -27
    assert_eq!(
        err.to_raw(),
        -27,
        "Empty input should return PMIX_ERR_BAD_PARAM (-27)"
    );
}

/// Verify the return type of data_decompress (compile-time check).
#[test]
fn test_data_decompress_return_type() {
    let _: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
}

/// Result<Vec<u8>, PmixStatus> is Send (important for async contexts).
#[test]
fn test_decompress_return_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<Vec<u8>, PmixStatus>>();
}

/// Result<Vec<u8>, PmixStatus> is Sync (Vec and PmixStatus are Sync).
#[test]
fn test_decompress_return_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Result<Vec<u8>, PmixStatus>>();
}

/// PmixStatus from empty-input error implements Debug.
#[test]
fn test_decompress_error_is_debug() {
    let result = data_decompress(&[]);
    let err = result.unwrap_err();
    let debug_str = format!("{:?}", err);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

/// PmixStatus from empty-input error implements Display.
#[test]
fn test_decompress_error_is_display() {
    let result = data_decompress(&[]);
    let err = result.unwrap_err();
    let display_str = format!("{}", err);
    assert!(
        !display_str.is_empty(),
        "Display output should not be empty"
    );
}

/// data_decompress function pointer can be stored and called.
#[test]
fn test_data_decompress_function_pointer() {
    let f: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
    // Only call with empty input (no FFI, safe without PMIx_Init)
    assert!(f(&[]).is_err());
}

/// data_decompress can be called multiple times with empty input.
#[test]
fn test_data_decompress_multiple_empty_calls() {
    for _ in 0..5 {
        assert!(data_decompress(&[]).is_err());
    }
}

/// data_compress and data_decompress are both available in the same module.
#[test]
fn test_compress_decompress_pair_available() {
    use pmix::data_serialization;
    let _compress = data_serialization::data_compress;
    let _decompress = data_serialization::data_decompress;
}

/// data_decompress accepts &[u8] — verify slice coercion from Vec works.
#[test]
fn test_data_decompress_accepts_vec_slice() {
    let data: Vec<u8> = Vec::new();
    // This should compile: Vec<u8> coerces to &[u8]
    let result = data_decompress(&data);
    assert!(result.is_err());
}

/// data_decompress accepts &[u8] — verify slice coercion from array works.
#[test]
fn test_data_decompress_accepts_array_slice() {
    let arr: [u8; 0] = [];
    // This should compile: &[u8; 0] coerces to &[u8]
    let result = data_decompress(&arr);
    assert!(result.is_err());
}

/// data_decompress error is PartialEq (comparable with other PmixStatus).
#[test]
fn test_decompress_error_partial_eq() {
    let err1 = data_decompress(&[]).unwrap_err();
    let err2 = data_decompress(&[]).unwrap_err();
    assert_eq!(err1, err2, "Same error from same input should be equal");
}

/// data_decompress error to_raw is consistent across calls.
#[test]
fn test_decompress_error_to_raw_consistent() {
    let err1 = data_decompress(&[]).unwrap_err();
    let err2 = data_decompress(&[]).unwrap_err();
    assert_eq!(err1.to_raw(), err2.to_raw());
}

/// data_decompress with single-byte non-empty input — still hits the FFI
/// path (not caught by empty check), so this requires PMIx_Init.
/// Marked ignored because it calls the FFI.
#[test]
#[ignore]
fn test_decompress_single_byte_invalid() {
    // A single byte is not valid compressed data — should fail at FFI level
    let result = data_decompress(&[0u8]);
    assert!(result.is_err(), "Single byte is not valid compressed data");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Decompress a block of zeros that was compressed by data_compress.
///
/// Zeros are the most compressible data — zlib should produce very small
/// output that decompresses back to the original.
#[test]
#[ignore]
fn test_decompress_zeros_roundtrip() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress zeros");
    assert!(
        compressed.len() < data.len(),
        "Compressed zeros ({}) should be smaller than original ({})",
        compressed.len(),
        data.len()
    );
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(
        decompressed, data,
        "Roundtrip should produce identical data"
    );
}

/// Decompress a repeating pattern (0,1,2,...,255 repeated).
#[test]
#[ignore]
fn test_decompress_pattern_roundtrip() {
    let data: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let compressed = data_compress(&data).expect("compress pattern");
    assert!(compressed.len() < data.len(), "Pattern should compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// Decompress uniform data (all 0xFF).
#[test]
#[ignore]
fn test_decompress_uniform_data() {
    let data = vec![0xFFu8; 8192];
    let compressed = data_compress(&data).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// Decompress invalid data (not produced by PMIx_Data_compress) should fail.
#[test]
#[ignore]
fn test_decompress_invalid_data() {
    // Random bytes that are not valid PMIx compressed data
    let invalid: Vec<u8> = std::iter::repeat(0xDEu8).take(400).collect();
    let result = data_decompress(&invalid);
    assert!(result.is_err(), "Decompressing invalid data should fail");
}

/// Decompress truncated compressed data should fail.
#[test]
#[ignore]
fn test_decompress_truncated() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress");
    let truncated = &compressed[..compressed.len() / 2];
    let result = data_decompress(truncated);
    assert!(result.is_err(), "Decompressing truncated data should fail");
}

/// Decompress with varying data sizes — roundtrip integrity check.
#[test]
#[ignore]
fn test_decompress_various_sizes() {
    let sizes = [1024, 2048, 4096, 8192, 16384];
    for size in sizes {
        let data: Vec<u8> = (0..=255).cycle().take(size).collect();
        let compressed = data_compress(&data).expect("compress");
        let decompressed = data_decompress(&compressed).expect("decompress");
        assert_eq!(decompressed, data, "Roundtrip failed for size {}", size);
    }
}

/// Double decompress — compress twice, decompress twice.
#[test]
#[ignore]
fn test_double_decompress() {
    let data = vec![0u8; 8192];
    let first = data_compress(&data).expect("first compress");
    let second = data_compress(&first).expect("second compress");
    let first_again = data_decompress(&second).expect("first decompress");
    assert_eq!(first_again, first);
    let original = data_decompress(&first_again).expect("second decompress");
    assert_eq!(original, data);
}

/// Decompress packed buffer payload — roundtrip through pack/unload/compress/decompress.
#[test]
#[ignore]
fn test_decompress_packed_buffer() {
    use pmix::PmixDataType;

    let buf = data_buffer_create().expect("create buffer");
    let val1: i32 = 42;
    let val2: i32 = 100;
    let val3: i32 = 200;
    data_pack(None, &buf, &val1, 1, PmixDataType::Int32).expect("pack val1");
    data_pack(None, &buf, &val2, 1, PmixDataType::Int32).expect("pack val2");
    data_pack(None, &buf, &val3, 1, PmixDataType::Int32).expect("pack val3");

    let payload = data_unload(&buf).expect("unload");
    data_buffer_release(&buf);

    let compressed = data_compress(payload.as_slice()).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, payload.as_slice());
}

/// Decompress output is an independently owned Vec<u8>.
#[test]
#[ignore]
fn test_decompress_output_is_owned_vec() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    // Clone should work — Vec is independently owned
    let cloned = decompressed.clone();
    assert_eq!(decompressed, cloned);
    assert_eq!(decompressed, data);
}

/// Decompress with PmixByteObject input.
#[test]
#[ignore]
fn test_decompress_byte_object_roundtrip() {
    let payload: PmixByteObject = vec![0xABu8; 4096].into();
    let compressed = data_compress(payload.as_slice()).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, payload.as_slice());
}

/// Decompress with array input (slice coercion).
#[test]
#[ignore]
fn test_decompress_array_roundtrip() {
    let arr: [u8; 4096] = [0; 4096];
    let compressed = data_compress(&arr).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed.as_slice(), &arr);
}

/// Decompress — verify the returned Vec has correct capacity and length.
#[test]
#[ignore]
fn test_decompress_output_length() {
    let data = vec![42u8; 8192];
    let compressed = data_compress(&data).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed.len(), data.len());
    assert!(decompressed.capacity() >= data.len());
}

/// Decompress small data that may not have compressed well.
#[test]
#[ignore]
fn test_decompress_small_block() {
    let data = vec![1u8, 2, 3, 4, 5];
    let result = data_compress(&data);
    // Small data may not compress (returns false from C API)
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, data);
    }
}

/// Decompress single byte that was compressed.
#[test]
#[ignore]
fn test_decompress_single_byte() {
    let data = vec![42u8];
    let result = data_compress(&data);
    // Single byte likely won't compress, but should not panic
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, data);
    }
}

/// Decompress large slice (64KB).
#[test]
#[ignore]
fn test_decompress_large_slice() {
    let data = vec![0u8; 65536];
    let compressed = data_compress(&data).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// Decompress — verify compression ratio for uniform data.
#[test]
#[ignore]
fn test_decompress_compression_ratio() {
    let data = vec![0xFFu8; 8192];
    let compressed = data_compress(&data).expect("compress");
    let ratio = compressed.len() as f64 / data.len() as f64;
    assert!(
        ratio < 0.1,
        "Compression ratio for uniform data should be very low, got {:.2}",
        ratio
    );
    // Verify roundtrip
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}
