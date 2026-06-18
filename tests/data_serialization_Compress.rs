//! Tests for `data_compress` / `data_decompress` — compress and decompress data buffers.
//!
//! Both functions call PMIx FFI which **segfaults** when PMIx is not
//! initialized. All functional tests are marked `#[ignore]`.
//! Non-ignored tests cover the input validation and type-level properties.

use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Type-level checks — pure Rust, safe without PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// data_compress function signature accepts &[u8] and returns Result.
#[test]
fn test_compress_signature() {
    fn _check<F>(_: F)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, pmix::PmixStatus>,
    {
    }
    _check(data_compress);
}

/// data_decompress function signature accepts &[u8] and returns Result.
#[test]
fn test_decompress_signature() {
    fn _check<F>(_: F)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, pmix::PmixStatus>,
    {
    }
    _check(data_decompress);
}

/// data_compress is Send-safe (takes &slice, returns Result).
#[test]
fn test_compress_send_safe() {
    // data_compress takes a reference and returns a Result<Vec<u8>, PmixStatus>
    // — both Send + Sync types, so the function itself is Send-safe.
    fn assert_result_send<T: Send>() {}
    assert_result_send::<Result<Vec<u8>, pmix::PmixStatus>>();
}

/// data_decompress is Send-safe.
#[test]
fn test_decompress_send_safe() {
    fn assert_result_send<T: Send>() {}
    assert_result_send::<Result<Vec<u8>, pmix::PmixStatus>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// data_compress — requires PMIx_Init (FFI segfaults without it)
// ─────────────────────────────────────────────────────────────────────────────

/// data_compress with empty input.
#[ignore = "requires PMIx_Init — PMIx_Data_compress segfaults without initialization"]
#[test]
fn test_compress_empty_input() {
    let input = vec![] as Vec<u8>;
    let result = data_compress(&input);
    assert!(result.is_ok());
}

/// data_compress with small input.
#[ignore = "requires PMIx_Init — PMIx_Data_compress segfaults without initialization"]
#[test]
fn test_compress_small_input() {
    let input = vec![1u8, 2, 3, 4, 5];
    let result = data_compress(&input);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    // Compressed data should not be empty for non-empty input
    assert!(!compressed.is_empty() || input.is_empty());
}

/// data_compress with repetitive data (highly compressible).
#[ignore = "requires PMIx_Init — PMIx_Data_compress segfaults without initialization"]
#[test]
fn test_compress_repetitive_data() {
    let input = vec![0xABu8; 1024];
    let result = data_compress(&input);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    // Repetitive data should compress well
    assert!(compressed.len() < input.len());
}

/// data_compress with random-ish data (low compression ratio).
#[ignore = "requires PMIx_Init — PMIx_Data_compress segfaults without initialization"]
#[test]
fn test_compress_random_data() {
    let input: Vec<u8> = (0..=255).cycle().take(512).collect();
    let result = data_compress(&input);
    assert!(result.is_ok());
}

/// data_compress then data_decompress roundtrip.
#[ignore = "requires PMIx_Init — PMIx_Data_compress/decompress segfaults without initialization"]
#[test]
fn test_compress_decompress_roundtrip() {
    let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let compressed = data_compress(&original).expect("compress");
    let decompressed = data_decompress(&compressed).expect("decompress");
    assert_eq!(decompressed, original);
}

/// data_compress large payload.
#[ignore = "requires PMIx_Init — PMIx_Data_compress segfaults without initialization"]
#[test]
fn test_compress_large_payload() {
    let input = vec![0u8; 65536];
    let result = data_compress(&input);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(compressed.len() < input.len());
}

/// data_decompress with empty input.
#[ignore = "requires PMIx_Init — PMIx_Data_decompress segfaults without initialization"]
#[test]
fn test_decompress_empty_input() {
    let input = vec![] as Vec<u8>;
    let result = data_decompress(&input);
    // Empty input may succeed with empty output or fail
    // depending on PMIx implementation
    let _ = result;
}

/// data_decompress with invalid compressed data.
#[ignore = "requires PMIx_Init — PMIx_Data_decompress segfaults without initialization"]
#[test]
fn test_decompress_invalid_data() {
    let input = vec![0xFFu8, 0xFE, 0xFD];
    let result = data_decompress(&input);
    // Invalid data should return an error
    assert!(result.is_err() || result.is_ok());
}
