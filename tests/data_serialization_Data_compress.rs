//! Tests for `PMIx_Data_compress` and `PMIx_Data_decompress`.
//!
//! PMIx_Data_compress performs lossless compression using zlib. It returns
//! false if the data is too small to compress or if compression would not
//! produce a smaller result. PMIx_Data_decompress reverses the operation.
//!
//! Note: These functions require PMIx_Init to have been called because the
//! internal compression framework (pcompress) is selected during init.
//! Integration tests that exercise the FFI are marked #[ignore].
//!
//! The signature and type-safety tests below do NOT require PMIx_Init
//! and run normally.

use pmix::PmixStatus;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type-safety tests — no PMIx_Init required (no FFI calls)
// ─────────────────────────────────────────────────────────────────────────────

/// data_compress function exists and has the correct signature.
///
/// This verifies the function is callable with the expected parameter types
/// without actually invoking the FFI (which would segfault without PMIx_Init).
#[test]
fn test_data_compress_signature() {
    // Verify the function type:
    // fn(&[u8]) -> Result<Vec<u8>, PmixStatus>
    let _fn_ref: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_compress;
}

/// data_decompress function exists and has the correct signature.
#[test]
fn test_data_decompress_signature() {
    let _fn_ref: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
}

/// data_compress is publicly exported from the crate.
#[test]
fn test_data_compress_public_api() {
    use pmix::data_serialization;
    let _ = data_serialization::data_compress;
}

/// data_decompress is publicly exported from the crate.
#[test]
fn test_data_decompress_public_api() {
    use pmix::data_serialization;
    let _ = data_serialization::data_decompress;
}

/// data_compress returns Err for empty input without calling FFI.
#[test]
fn test_data_compress_empty_input() {
    let result = data_compress(&[]);
    assert!(
        result.is_err(),
        "data_compress should reject empty input without calling FFI"
    );
}

/// data_decompress returns Err for empty input without calling FFI.
#[test]
fn test_data_decompress_empty_input() {
    let result = data_decompress(&[]);
    assert!(
        result.is_err(),
        "data_decompress should reject empty input without calling FFI"
    );
}

/// data_compress error for empty input is PMIX_ERR_BAD_PARAM.
#[test]
fn test_data_compress_empty_is_bad_param() {
    let result = data_compress(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // PMIX_ERR_BAD_PARAM = -27
    assert_eq!(
        err.to_raw(),
        -27,
        "Empty input should return PMIX_ERR_BAD_PARAM (-27)"
    );
}

/// data_decompress error for empty input is PMIX_ERR_BAD_PARAM.
#[test]
fn test_data_decompress_empty_is_bad_param() {
    let result = data_decompress(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -27,
        "Empty input should return PMIX_ERR_BAD_PARAM (-27)"
    );
}

/// data_compress and data_decompress are both available in the same module.
#[test]
fn test_compress_decompress_pair_available() {
    use pmix::data_serialization;
    let _compress = data_serialization::data_compress;
    let _decompress = data_serialization::data_decompress;
}

/// Verify the return type is Result<Vec<u8>, PmixStatus> (compile-time check).
#[test]
fn test_data_compress_return_type() {
    // This is a compile-time type check — we don't call the function,
    // we just verify the type annotation compiles.
    fn check_type<T>(_: T) {}
    let _: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_compress;
    check_type(());
}

/// Verify the return type of data_decompress (compile-time check).
#[test]
fn test_data_decompress_return_type() {
    let _: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
}

/// Result<Vec<u8>, PmixStatus> is Send (important for async contexts).
#[test]
fn test_compress_return_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<Vec<u8>, PmixStatus>>();
}

/// PmixStatus from empty-input error implements Debug.
#[test]
fn test_compress_error_is_debug() {
    let result = data_compress(&[]);
    let err = result.unwrap_err();
    let _ = format!("{:?}", err);
}

/// data_compress function pointer can be stored and called.
#[test]
fn test_data_compress_function_pointer() {
    let f: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_compress;
    // Only call with empty input (no FFI)
    assert!(f(&[]).is_err());
}

/// data_decompress function pointer can be stored and called.
#[test]
fn test_data_decompress_function_pointer() {
    let f: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
    assert!(f(&[]).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Compress a large block of zeros (highly compressible).
///
/// Requires PMIx_Init — the compression framework is selected during init.
#[test]
#[ignore]
fn test_compress_zeros() {
    let data = vec![0u8; 4096];
    let result = data_compress(&data);
    assert!(result.is_ok(), "Compressing zeros should succeed");
    let compressed = result.unwrap();
    assert!(
        compressed.len() < data.len(),
        "Compressed data ({}) should be smaller than original ({})",
        compressed.len(),
        data.len()
    );
}

/// Decompress the result of compressing zeros.
#[test]
#[ignore]
fn test_decompress_zeros_roundtrip() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress zeros");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(
        decompressed, data,
        "Roundtrip should produce identical data"
    );
}

/// Compress and decompress a repeating pattern.
#[test]
#[ignore]
fn test_compress_decompress_pattern() {
    // Create a repeating pattern: 0,1,2,3,...,255,0,1,2,3,...
    let data: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let compressed = data_compress(&data).expect("compress pattern");
    assert!(compressed.len() < data.len(), "Pattern should compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// Compress a small block that may not be compressible.
#[test]
#[ignore]
fn test_compress_small_block() {
    let data = vec![1u8, 2, 3, 4, 5];
    let result = data_compress(&data);
    // Small data may not compress (returns false from C API)
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, data);
    }
}

/// Compress data that was packed via data_pack + data_unload.
#[test]
#[ignore]
fn test_compress_packed_buffer() {
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

    let result = data_compress(payload.as_slice());
    if result.is_ok() {
        let compressed = result.unwrap();
        assert!(compressed.len() <= payload.size());
        let decompressed = data_decompress(&compressed).expect("decompress");
        assert_eq!(decompressed, payload.as_slice());
    }
}

/// Decompress invalid data (not produced by PMIx_Data_compress).
#[test]
#[ignore]
fn test_decompress_invalid_data() {
    let invalid: Vec<u8> = std::iter::repeat(0xDEu8).take(400).collect();
    let result = data_decompress(&invalid);
    assert!(result.is_err(), "Decompressing invalid data should fail");
}

/// Compress then decompress with varying data sizes.
#[test]
#[ignore]
fn test_compress_decompress_various_sizes() {
    let sizes = [1024, 2048, 4096, 8192, 16384];
    for size in sizes {
        let data: Vec<u8> = (0..=255).cycle().take(size).collect();
        let result = data_compress(&data);
        if result.is_ok() {
            let compressed = result.unwrap();
            let decompressed = data_decompress(&compressed).expect("decompress");
            assert_eq!(decompressed, data, "Roundtrip failed for size {}", size);
        }
    }
}

/// Compress already-compressed data (double compression).
#[test]
#[ignore]
fn test_double_compress() {
    let data = vec![0u8; 8192];
    let first = data_compress(&data).expect("first compress");
    let second = data_compress(&first);
    if second.is_ok() {
        let second_compressed = second.unwrap();
        let first_again = data_decompress(&second_compressed).expect("decompress");
        assert_eq!(first_again, first);
        let original = data_decompress(&first_again).expect("decompress again");
        assert_eq!(original, data);
    }
}

/// Compress data with PmixByteObject input.
#[test]
#[ignore]
fn test_compress_byte_object_roundtrip() {
    let payload: PmixByteObject = vec![0xABu8; 4096].into();
    let compressed = data_compress(payload.as_slice()).expect("compress");
    assert!(compressed.len() < payload.size());
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, payload.as_slice());
}

/// Verify compress returns data smaller than input for compressible data.
#[test]
#[ignore]
fn test_compress_ratio() {
    let data = vec![0xFFu8; 8192];
    let compressed = data_compress(&data).expect("compress");
    let ratio = compressed.len() as f64 / data.len() as f64;
    assert!(
        ratio < 0.1,
        "Compression ratio for uniform data should be very low, got {:.2}",
        ratio
    );
}

/// Decompress with truncated compressed data should fail.
#[test]
#[ignore]
fn test_decompress_truncated() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress");
    let truncated = &compressed[..compressed.len() / 2];
    let result = data_decompress(truncated);
    assert!(result.is_err(), "Decompressing truncated data should fail");
}

/// Compress and decompress — output Vec<u8> is independently owned.
#[test]
#[ignore]
fn test_compress_output_is_owned_vec() {
    let data = vec![0u8; 4096];
    let compressed = data_compress(&data).expect("compress");
    let cloned = compressed.clone();
    assert_eq!(compressed, cloned);
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// Compress with a single byte (edge case).
#[test]
#[ignore]
fn test_compress_single_byte() {
    let data = vec![42u8];
    let result = data_compress(&data);
    // Single byte likely won't compress, but should not panic
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed, data);
    }
}

/// Compress with a large slice (64KB).
#[test]
#[ignore]
fn test_compress_large_slice() {
    let data = vec![0u8; 65536];
    let result = data_compress(&data);
    assert!(result.is_ok(), "Large slice should compress");
    let compressed = result.unwrap();
    assert!(compressed.len() < data.len());
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, data);
}

/// data_compress accepts PmixByteObject.as_slice().
#[test]
#[ignore]
fn test_compress_with_byte_object_slice() {
    let payload: PmixByteObject = vec![0u8; 4096].into();
    let result = data_compress(payload.as_slice());
    // payload should still be usable after compress
    assert_eq!(payload.size(), 4096);
    assert!(result.is_ok());
}

/// data_compress accepts array reference.
#[test]
#[ignore]
fn test_compress_array_input() {
    let arr: [u8; 4096] = [0; 4096];
    let result = data_compress(&arr);
    if result.is_ok() {
        let decompressed = data_decompress(&result.unwrap()).expect("decompress");
        assert_eq!(decompressed.as_slice(), &arr);
    }
}
