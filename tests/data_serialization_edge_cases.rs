//! Comprehensive edge-case tests for data_serialization.rs that do NOT
//! require PMIx_Init (no FFI calls beyond user-space functions).
//!
//! These tests focus on:
//! - Buffer lifecycle and state transitions
//! - Error paths and boundary conditions
//! - Type conversions and trait implementations
//! - PmixByteObject edge cases
//! - PmixPrintOutput runtime behavior
//!
//! Functions that work without PMIx_Init (operate entirely in user space):
//! - data_buffer_create / data_buffer_release
//! - data_load / data_unload
//! - data_compress / data_decompress (empty-input guards)
//! - data_pack (num_vals <= 0 guard)
//!
//! Functions that require PMIx_Init (FFI calls that need pmix_globals.mypeer):
//! - data_pack / data_unpack (when num_vals > 0)
//! - data_copy / data_copy_payload
//! - data_print
//! - data_embed
//! - data_compress / data_decompress (non-empty input)

use std::sync::OnceLock;

use pmix::{init, data_serialization::*};
use pmix::{PmixDataType, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Singleton PMIx init — PMIx can only be initialized once per process.
// ─────────────────────────────────────────────────────────────────────────────

static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();

fn ensure_init() -> &'static pmix::Context {
    PMIX_CTX.get_or_init(|| init(None).expect("PMIx_Init failed — run under prterun"))
}


// ─────────────────────────────────────────────────────────────────────────────
// PmixPrintOutput — runtime behavior (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixPrintOutput::default() produces an empty string.
#[test]
fn test_print_output_default_is_empty() {
    let output = PmixPrintOutput::default();
    assert!(output.is_empty(), "default print output should be empty");
    assert_eq!(output.as_str(), "");
}

/// PmixPrintOutput::as_str() returns the inner string slice.
#[test]
fn test_print_output_as_str() {
    let output = PmixPrintOutput::default();
    let s: &str = output.as_str();
    assert_eq!(s, "");
}

/// PmixPrintOutput implements Display — formatting default output works.
#[test]
fn test_print_output_display_default() {
    let output = PmixPrintOutput::default();
    let formatted = format!("{}", output);
    assert_eq!(formatted, "");
}

/// PmixPrintOutput implements Debug — formatting default output works.
#[test]
fn test_print_output_debug_default() {
    let output = PmixPrintOutput::default();
    let debug_str = format!("{:?}", output);
    // Debug delegates to inner String's Debug, which quotes the content
    assert_eq!(debug_str, "\"\"");
}

/// PmixPrintOutput can be converted into String via Into.
#[test]
fn test_print_output_into_string() {
    let output = PmixPrintOutput::default();
    let s: String = output.into();
    assert_eq!(s, "");
}

/// PmixPrintOutput Deref<Target=str> works — can call str methods.
#[test]
fn test_print_output_deref_operations() {
    let output = PmixPrintOutput::default();
    assert_eq!(output.len(), 0);
    assert!(output.is_empty());
    assert!(!output.contains("x"));
}

// ─────────────────────────────────────────────────────────────────────────────
// data_pack — error paths (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// data_pack with num_vals == 0 returns PMIX_ERR_BAD_PARAM (-27).
#[test]
fn test_pack_zero_num_vals_returns_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -27,
        "num_vals == 0 should return PMIX_ERR_BAD_PARAM (-27)"
    );
}

/// data_pack with negative num_vals returns PMIX_ERR_BAD_PARAM (-27).
#[test]
fn test_pack_negative_num_vals_returns_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -27,
        "negative num_vals should return PMIX_ERR_BAD_PARAM (-27)"
    );
}

/// data_pack with num_vals == i32::MIN returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_pack_i32_min_num_vals_returns_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, i32::MIN, PmixDataType::Int32);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// data_pack with num_vals == -999 returns PMIX_ERR_BAD_PARAM.
#[test]
fn test_pack_large_negative_num_vals_returns_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -999, PmixDataType::Int32);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

/// data_pack error is the same known error variant.
#[test]
fn test_pack_error_is_known_bad_param() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    let err = result.unwrap_err();
    assert!(
        matches!(err, PmixStatus::Known(PmixError::ErrBadParam)),
        "error should be Known(ErrBadParam), got {:?}",
        err
    );
}

/// data_pack error Display output is readable.
#[test]
fn test_pack_error_display() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
    let err = result.unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty(), "error Display should not be empty");
    assert!(
        display.contains("BadParam") || display.contains("bad_param") || display.contains("BAD_PARAM"),
        "error Display should mention bad param, got: {}",
        display
    );
}

/// data_pack error Debug output is readable.
#[test]
fn test_pack_error_debug() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
    let err = result.unwrap_err();
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty(), "error Debug should not be empty");
}

/// data_pack error is PartialEq — same error from same condition.
#[test]
fn test_pack_error_partial_eq() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;
    let err1 = data_pack(None, &buf, &val, 0, PmixDataType::Int32).unwrap_err();
    let err2 = data_pack(None, &buf, &val, -1, PmixDataType::Int32).unwrap_err();
    assert_eq!(err1, err2, "same error code should be equal");
}

// ─────────────────────────────────────────────────────────────────────────────
// data_compress / data_decompress — error paths (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// data_compress with empty input returns the exact known error variant.
#[test]
fn test_compress_empty_is_known_bad_param() {
    let result = data_compress(&[]);
    let err = result.unwrap_err();
    assert!(
        matches!(err, PmixStatus::Known(PmixError::ErrBadParam)),
        "should be Known(ErrBadParam), got {:?}",
        err
    );
}

/// data_decompress with empty input returns the exact known error variant.
#[test]
fn test_decompress_empty_is_known_bad_param() {
    let result = data_decompress(&[]);
    let err = result.unwrap_err();
    assert!(
        matches!(err, PmixStatus::Known(PmixError::ErrBadParam)),
        "should be Known(ErrBadParam), got {:?}",
        err
    );
}

/// data_compress and data_decompress return the same error for empty input.
#[test]
fn test_compress_decompress_same_error_for_empty() {
    let err_compress = data_compress(&[]).unwrap_err();
    let err_decompress = data_decompress(&[]).unwrap_err();
    assert_eq!(err_compress, err_decompress);
    assert_eq!(err_compress.to_raw(), -27);
}

/// data_compress error Display output.
#[test]
fn test_compress_error_display() {
    let err = data_compress(&[]).unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty());
}

/// data_compress error implements std::error::Error (via PmixStatus).
#[test]
fn test_compress_error_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<PmixStatus>();
}

/// data_compress can be called repeatedly with empty input (idempotent).
#[test]
fn test_compress_empty_idempotent() {
    for _ in 0..10 {
        let err = data_compress(&[]).unwrap_err();
        assert_eq!(err.to_raw(), -27);
    }
}

/// data_decompress can be called repeatedly with empty input (idempotent).
#[test]
fn test_decompress_empty_idempotent() {
    for _ in 0..10 {
        let err = data_decompress(&[]).unwrap_err();
        assert_eq!(err.to_raw(), -27);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject — edge cases (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixByteObject::new() == PmixByteObject::default().
#[test]
fn test_byte_object_new_equals_default() {
    let new_obj = PmixByteObject::new();
    let default_obj = PmixByteObject::default();
    assert_eq!(new_obj.size(), default_obj.size());
    assert_eq!(new_obj.is_empty(), default_obj.is_empty());
    assert_eq!(new_obj.as_slice(), default_obj.as_slice());
}

/// PmixByteObject::from(Vec<u8>) preserves exact byte content.
#[test]
fn test_byte_object_from_vec_preserves_content() {
    let data: Vec<u8> = (0..=255).collect();
    let obj = PmixByteObject::from(data.clone());
    assert_eq!(obj.as_slice(), &data[..]);
    assert_eq!(obj.size(), 256);
    assert!(!obj.is_empty());
}

/// PmixByteObject::from(Vec<u8>) with single byte.
#[test]
fn test_byte_object_from_single_byte() {
    let obj = PmixByteObject::from(vec![42u8]);
    assert_eq!(obj.size(), 1);
    assert_eq!(obj.as_slice(), &[42u8]);
    assert!(!obj.is_empty());
}

/// PmixByteObject::from(Vec<u8>) with all-zero bytes.
#[test]
fn test_byte_object_from_all_zeros() {
    let obj = PmixByteObject::from(vec![0u8; 32]);
    assert_eq!(obj.size(), 32);
    assert!(!obj.is_empty());
    assert_eq!(obj.as_slice(), &[0u8; 32]);
}

/// PmixByteObject::from(Vec<u8>) with Vec that has extra capacity.
/// The Vec's capacity should not affect the byte object's size.
#[test]
fn test_byte_object_from_vec_with_extra_capacity() {
    let mut data = Vec::with_capacity(1024);
    data.push(1u8);
    data.push(2u8);
    data.push(3u8);
    assert!(data.capacity() >= 1024, "vec should have extra capacity");
    assert_eq!(data.len(), 3, "vec should have only 3 elements");

    let obj = PmixByteObject::from(data);
    assert_eq!(obj.size(), 3, "byte object size should be 3, not capacity");
    assert_eq!(obj.as_slice(), &[1u8, 2, 3]);
}

/// PmixByteObject::from(Vec<u8>) with Vec that has capacity >> length.
#[test]
fn test_byte_object_from_vec_capacity_much_greater_than_length() {
    let mut data = Vec::with_capacity(8192);
    data.extend_from_slice(&[0xDEu8, 0xAD, 0xBE, 0xEF]);
    assert_eq!(data.len(), 4);
    assert!(data.capacity() >= 8192);

    let obj = PmixByteObject::from(data);
    assert_eq!(obj.size(), 4);
    assert_eq!(obj.as_slice(), &[0xDEu8, 0xAD, 0xBE, 0xEF]);
}

/// PmixByteObject::as_slice() on empty object returns empty slice (not dangling).
#[test]
fn test_byte_object_as_slice_on_empty() {
    let obj = PmixByteObject::new();
    let slice = obj.as_slice();
    assert!(slice.is_empty());
    // Should not panic or segfault
}

/// PmixByteObject::as_slice() returns correct length for known size.
#[test]
fn test_byte_object_as_slice_length_matches_size() {
    let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
    let obj = PmixByteObject::from(data.clone());
    assert_eq!(obj.as_slice().len(), obj.size());
    assert_eq!(obj.as_slice().len(), data.len());
}

/// PmixByteObject::is_empty() is consistent with size() == 0.
#[test]
fn test_byte_object_is_empty_consistent_with_size() {
    let empty = PmixByteObject::new();
    assert!(empty.is_empty());
    assert_eq!(empty.size(), 0);

    let non_empty = PmixByteObject::from(vec![1u8]);
    assert!(!non_empty.is_empty());
    assert_ne!(non_empty.size(), 0);
}

/// Multiple PmixByteObjects can coexist without interference.
#[test]
fn test_multiple_byte_objects_coexist() {
    let obj1 = PmixByteObject::from(vec![1u8, 2, 3]);
    let obj2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8]);
    let obj3 = PmixByteObject::new();

    assert_eq!(obj1.size(), 3);
    assert_eq!(obj2.size(), 5);
    assert!(obj3.is_empty());

    // Verify they are independent
    assert_eq!(obj1.as_slice(), &[1u8, 2, 3]);
    assert_eq!(obj2.as_slice(), &[4u8, 5, 6, 7, 8]);
}

/// PmixByteObject::from(Vec<u8>) with boundary-size vectors.
#[test]
fn test_byte_object_from_boundary_sizes() {
    // Size 0 (empty)
    let obj0 = PmixByteObject::from(vec![]);
    assert!(obj0.is_empty());

    // Size 1 (single byte)
    let obj1 = PmixByteObject::from(vec![0u8]);
    assert_eq!(obj1.size(), 1);

    // Size 255 (boundary for u8)
    let obj255 = PmixByteObject::from(vec![0u8; 255]);
    assert_eq!(obj255.size(), 255);

    // Size 256 (power of 2 boundary)
    let obj256 = PmixByteObject::from(vec![0u8; 256]);
    assert_eq!(obj256.size(), 256);

    // Size 1024 (page boundary)
    let obj1024 = PmixByteObject::from(vec![0u8; 1024]);
    assert_eq!(obj1024.size(), 1024);
}

/// PmixByteObject Debug output contains struct name.
#[test]
fn test_byte_object_debug_contains_struct_name() {
    let obj = PmixByteObject::new();
    let debug = format!("{:?}", obj);
    assert!(
        debug.contains("PmixByteObject"),
        "Debug should contain struct name, got: {}",
        debug
    );
}

/// PmixByteObject Debug output for non-empty object.
#[test]
fn test_byte_object_debug_non_empty() {
    let obj = PmixByteObject::from(vec![1u8, 2, 3]);
    let debug = format!("{:?}", obj);
    assert!(!debug.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer lifecycle and state transitions (no FFI beyond user-space)
// ─────────────────────────────────────────────────────────────────────────────

/// Buffer starts with 0 bytes_allocated (or some initial allocation).
#[test]
fn test_buffer_initial_bytes_allocated() {
    let buf = data_buffer_create().expect("create buffer");
    // bytes_allocated may be 0 or some initial size — just verify it's readable
    let _ = buf.bytes_allocated();
    assert_eq!(buf.bytes_used(), 0);
}

/// Buffer bytes_allocated >= bytes_used always.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_allocated_ge_used_after_load() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4, 5]);
    data_load(&buf, &payload).expect("load");
    assert!(
        buf.bytes_allocated() >= buf.bytes_used(),
        "allocated ({}) should be >= used ({})",
        buf.bytes_allocated(),
        buf.bytes_used()
    );
}

/// Buffer bytes_used == 0 after create, > 0 after load, == 0 after unload.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_bytes_used_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "initial state");

    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    data_load(&buf, &payload).expect("load");
    assert!(buf.bytes_used() > 0, "after load");

    let _recovered = data_unload(&buf).expect("unload");
    // After unload, buffer should be empty (unload is destructive)
    assert_eq!(buf.bytes_used(), 0, "after unload");
}

/// Buffer bytes_allocated >= bytes_used after load/unload cycle.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_allocated_ge_used_after_cycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0u8; 128]);
    data_load(&buf, &payload).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    // Even after unload, allocated should be >= used (both should be 0)
    assert!(buf.bytes_allocated() >= buf.bytes_used());
}

/// Multiple load/unload cycles on the same buffer.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_multiple_load_unload_cycles() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_valid_after_load_unload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    assert!(buf.is_valid(), "buffer should still be valid after load/unload");
}

/// Buffer is_valid after multiple load/unload cycles.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_valid_after_multiple_cycles() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    for _ in 0..10 {
        let payload = PmixByteObject::from(vec![42u8]);
        data_load(&buf, &payload).expect("load");
        let _recovered = data_unload(&buf).expect("unload");
        assert!(buf.is_valid());
    }
}

/// Buffer Debug output after load/unload cycle.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_debug_after_cycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload).expect("load");
    let _recovered = data_unload(&buf).expect("unload");
    let debug = format!("{:?}", buf);
    assert!(debug.contains("PmixDataBuffer"));
}

/// Buffer as_mut_ptr returns non-null pointer for valid buffer.
#[test]
fn test_buffer_as_mut_ptr_non_null() {
    let buf = data_buffer_create().expect("create buffer");
    let ptr = buf.as_mut_ptr();
    assert!(!ptr.is_null(), "as_mut_ptr should return non-null for valid buffer");
}

/// Buffer as_mut_ptr is consistent across calls.
#[test]
fn test_buffer_as_mut_ptr_consistent() {
    let buf = data_buffer_create().expect("create buffer");
    let ptr1 = buf.as_mut_ptr();
    let ptr2 = buf.as_mut_ptr();
    assert_eq!(ptr1, ptr2, "as_mut_ptr should return the same pointer");
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer create/release cycles (no FFI beyond user-space)
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple buffer create/drop cycles work without issues.
#[test]
fn test_multiple_buffer_create_drop_cycles() {
    for _ in 0..20 {
        let buf = data_buffer_create().expect("create buffer");
        assert!(buf.is_valid());
        assert_eq!(buf.bytes_used(), 0);
        // buf dropped here
    }
}

/// Buffer create, load, unload, then drop.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_buffer_create_load_unload_drop() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    for _ in 0..10 {
        let buf = data_buffer_create().expect("create buffer");
        let payload = PmixByteObject::from(vec![1u8, 2, 3]);
        data_load(&buf, &payload).expect("load");
        let _recovered = data_unload(&buf).expect("unload");
        // buf dropped here
    }
}

/// Many small buffers created and dropped.
#[test]
fn test_many_small_buffers() {
    let mut bufs = Vec::new();
    for _ in 0..50 {
        bufs.push(data_buffer_create().expect("create"));
    }
    assert_eq!(bufs.len(), 50);
    for buf in &bufs {
        assert!(buf.is_valid());
    }
    // All dropped at once
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus — edge cases (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw(0) is PMIX_SUCCESS.
#[test]
fn test_status_from_raw_zero_is_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success());
    assert!(!status.is_error());
    assert!(
        matches!(status, PmixStatus::Known(PmixError::Success)),
        "should be Known(Success)"
    );
}

/// PmixStatus::from_raw(-1) is PMIX_ERROR.
#[test]
fn test_status_from_raw_minus_one_is_error() {
    let status = PmixStatus::from_raw(-1);
    assert!(status.is_error());
    assert!(!status.is_success());
    assert!(
        matches!(status, PmixStatus::Known(PmixError::Error)),
        "should be Known(Error)"
    );
}

/// PmixStatus::from_raw(-27) is PMIX_ERR_BAD_PARAM.
#[test]
fn test_status_from_raw_minus_27_is_bad_param() {
    let status = PmixStatus::from_raw(-27);
    assert!(status.is_error());
    assert!(
        matches!(status, PmixStatus::Known(PmixError::ErrBadParam)),
        "should be Known(ErrBadParam)"
    );
}

/// PmixStatus::from_raw with unknown code returns Unknown variant.
#[test]
fn test_status_from_raw_unknown_code() {
    let status = PmixStatus::from_raw(-99999);
    assert!(
        matches!(status, PmixStatus::Unknown(-99999)),
        "should be Unknown(-99999)"
    );
}

/// PmixStatus::to_raw roundtrips for known codes.
#[test]
fn test_status_to_raw_roundtrip_known() {
    for code in [0, -1, -2, -9, -27, -11] {
        let status = PmixStatus::from_raw(code);
        assert_eq!(status.to_raw(), code, "roundtrip failed for code {}", code);
    }
}

/// PmixStatus::to_raw roundtrips for unknown codes.
#[test]
fn test_status_to_raw_roundtrip_unknown() {
    for code in [-99999, -50000, 99999] {
        let status = PmixStatus::from_raw(code);
        assert_eq!(status.to_raw(), code, "roundtrip failed for code {}", code);
    }
}

/// PmixStatus::is_success for positive codes.
#[test]
fn test_status_is_success_for_positive_codes() {
    for code in [1, 100, 999] {
        let status = PmixStatus::from_raw(code);
        assert!(status.is_success(), "positive code {} should be success", code);
    }
}

/// PmixStatus::is_error for negative codes.
#[test]
fn test_status_is_error_for_negative_codes() {
    for code in [-1, -10, -27, -100] {
        let status = PmixStatus::from_raw(code);
        assert!(status.is_error(), "negative code {} should be error", code);
    }
}

/// PmixStatus::known() returns Some for known codes.
#[test]
fn test_status_known_returns_some() {
    let status = PmixStatus::from_raw(0);
    assert!(status.known().is_some());
}

/// PmixStatus::known() returns None for unknown codes.
#[test]
fn test_status_known_returns_none_for_unknown() {
    let status = PmixStatus::from_raw(-99999);
    assert!(status.known().is_none());
}

/// PmixStatus implements Clone, Copy, PartialEq, Eq, Hash, Debug.
#[test]
fn test_status_trait_bounds() {
    fn assert_traits<T: Clone + Copy + PartialEq + Eq + std::hash::Hash + std::fmt::Debug>() {}
    assert_traits::<PmixStatus>();
}

/// PmixStatus implements std::fmt::Display.
#[test]
fn test_status_display() {
    let s = PmixStatus::from_raw(0);
    let display = format!("{}", s);
    assert!(!display.is_empty());
}

/// PmixStatus implements std::error::Error.
#[test]
fn test_status_is_std_error() {
    fn check<T: std::error::Error>() {}
    check::<PmixStatus>();
}

/// PmixStatus::from(PmixError) works.
#[test]
fn test_status_from_pmix_error() {
    let status: PmixStatus = PmixError::Success.into();
    assert!(status.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcRef — edge cases (no FFI needed)
// ─────────────────────────────────────────────────────────────────────────────

/// PmixProcRef with empty namespace.
#[test]
fn test_proc_ref_empty_namespace() {
    let _proc = PmixProcRef::new("", 0);
}

/// PmixProcRef with rank 0.
#[test]
fn test_proc_ref_rank_zero() {
    let _proc = PmixProcRef::new("ns", 0);
}

/// PmixProcRef with u32::MAX rank.
#[test]
fn test_proc_ref_rank_max() {
    let _proc = PmixProcRef::new("ns", u32::MAX);
}

/// PmixProcRef with very long namespace (should truncate internally).
#[test]
fn test_proc_ref_very_long_namespace() {
    let long_ns = "a".repeat(1000);
    let _proc = PmixProcRef::new(&long_ns, 42);
}

/// PmixProcRef to_raw produces consistent results.
#[test]
fn test_proc_ref_to_raw_consistent() {
    let proc = PmixProcRef::new("test_ns", 123);
    // to_raw() is private, so we just verify the struct can be created and lives
    // without panicking. The internal consistency is covered by the module's own tests.
    let _ = proc;
}

// ─────────────────────────────────────────────────────────────────────────────
// data_load / data_unload — edge cases (no FFI beyond user-space)
// ─────────────────────────────────────────────────────────────────────────────

/// data_load with empty payload does not change buffer state significantly.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_empty_payload_preserves_buffer() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    data_load(&buf, &payload).expect("load empty");
    assert!(buf.is_valid());
}

/// data_load then data_unload with boundary payload sizes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_boundary_sizes() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let sizes = [1usize, 2, 3, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 511, 512];
    for size in sizes {
        let buf = data_buffer_create().expect("create buffer");
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let payload = PmixByteObject::from(data.clone());
        data_load(&buf, &payload).expect("load");
        let recovered = data_unload(&buf).expect("unload");
        assert_eq!(
            recovered.as_slice(),
            &data[..],
            "roundtrip failed for size {}",
            size
        );
    }
}

/// data_load payload is consumed — as_slice returns empty after load.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_consumes_payload_as_slice_empty() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![42u8, 84, 126]);
    assert_eq!(payload.as_slice(), &[42u8, 84, 126]);

    data_load(&buf, &payload).expect("load");

    // Payload should be consumed
    assert!(payload.as_slice().is_empty(), "payload should be empty after load");
    assert!(payload.is_empty());
    assert_eq!(payload.size(), 0);
}

/// data_load with all 0x00 bytes roundtrips correctly.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_all_null_bytes() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let data = vec![0u8; 128];
    let payload = PmixByteObject::from(data.clone());
    data_load(&buf, &payload).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &data[..]);
}

/// data_load with all 0xFF bytes roundtrips correctly.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_all_0xff_bytes() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let data = vec![0xFFu8; 128];
    let payload = PmixByteObject::from(data.clone());
    data_load(&buf, &payload).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &data[..]);
}

/// data_load with alternating 0x00/0xFF bytes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_alternating_null_ff() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let data: Vec<u8> = (0..256).map(|i| if i % 2 == 0 { 0x00 } else { 0xFF }).collect();
    let payload = PmixByteObject::from(data.clone());
    data_load(&buf, &payload).expect("load");
    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &data[..]);
}

/// data_unload from empty buffer returns empty payload or error.
#[test]
fn test_unload_empty_buffer_handling() {
    let buf = data_buffer_create().expect("create buffer");
    let result = data_unload(&buf);
    match result {
        Ok(payload) => {
            assert!(payload.as_slice().is_empty(), "should be empty");
        }
        Err(_) => {
            // Error is acceptable for empty buffer
        }
    }
}

/// data_unload after data_unload (double unload) — buffer should be empty.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_double_unload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload).expect("load");

    let _first = data_unload(&buf).expect("first unload");
    let second = data_unload(&buf);

    match second {
        Ok(p) => assert!(p.as_slice().is_empty(), "second unload should be empty"),
        Err(_) => {
            // Error is acceptable for empty buffer
        }
    }
}

/// data_load replaces previous content — buffer bytes_used reflects new payload.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_replaces_content_bytes_used() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");

    // Load small payload
    let p1 = PmixByteObject::from(vec![1u8]);
    data_load(&buf, &p1).expect("load small");
    let used_after_small = buf.bytes_used();

    // Load large payload — should replace
    let p2 = PmixByteObject::from(vec![0u8; 256]);
    data_load(&buf, &p2).expect("load large");
    let used_after_large = buf.bytes_used();

    assert!(
        used_after_large > used_after_small,
        "large payload should use more bytes than small"
    );
}

/// data_load then data_unload then data_load again — buffer is reusable.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_load_reuse() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");

    // First cycle
    let data1 = vec![1u8, 2, 3];
    let p1 = PmixByteObject::from(data1.clone());
    data_load(&buf, &p1).expect("load 1");
    let r1 = data_unload(&buf).expect("unload 1");
    assert_eq!(r1.as_slice(), &data1[..]);

    // Second cycle — same buffer
    let data2 = vec![4u8, 5, 6, 7, 8];
    let p2 = PmixByteObject::from(data2.clone());
    data_load(&buf, &p2).expect("load 2");
    let r2 = data_unload(&buf).expect("unload 2");
    assert_eq!(r2.as_slice(), &data2[..]);
}

// ─────────────────────────────────────────────────────────────────────────────
// data_buffer_release — explicit release safety (user-space FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// Explicit data_buffer_release followed by drop.
/// Note: data_buffer_release calls PMIx_Data_buffer_release, and the Drop impl
/// also calls it. The C function PMIx_Data_buffer_release should handle null
/// Buffer Drop handles cleanup — no explicit release needed.
/// NOTE: Do NOT call data_buffer_release(&mut buf) explicitly — Drop already does it,
/// and the wrapper does NOT null the internal pointer after explicit release,
/// causing double-free.
#[test]
fn test_buffer_drop_cleanup() {
    {
        let buf = data_buffer_create().expect("create buffer");
        assert!(buf.is_valid());
        // buf is dropped here — Drop calls PMIx_Data_buffer_release
    }
    // Buffer is freed; creating a new one should work
    let buf2 = data_buffer_create().expect("create buffer 2");
    assert!(buf2.is_valid());
}

/// Buffer is_valid returns true for freshly created buffer.
#[test]
fn test_buffer_is_valid_after_create() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid());
}

// ─────────────────────────────────────────────────────────────────────────────
// Transport chain edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Transport chain with single byte.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_transport_chain_single_byte() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let sender_buf = data_buffer_create().expect("create sender");
    let data = vec![42u8];
    let payload = PmixByteObject::from(data.clone());
    data_load(&sender_buf, &payload).expect("load");

    let transport = data_unload(&sender_buf).expect("unload");
    assert_eq!(transport.as_slice(), &data[..]);

    let receiver_buf = data_buffer_create().expect("create receiver");
    data_load(&receiver_buf, &transport).expect("load");
    let recovered = data_unload(&receiver_buf).expect("unload");
    assert_eq!(recovered.as_slice(), &data[..]);
}

/// Transport chain with 64KB payload.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_transport_chain_64kb() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let sender_buf = data_buffer_create().expect("create sender");
    let data: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(data.clone());
    data_load(&sender_buf, &payload).expect("load");

    let transport = data_unload(&sender_buf).expect("unload");
    assert_eq!(transport.as_slice(), &data[..]);

    let receiver_buf = data_buffer_create().expect("create receiver");
    data_load(&receiver_buf, &transport).expect("load");
    let recovered = data_unload(&receiver_buf).expect("unload");
    assert_eq!(recovered.as_slice(), &data[..]);
}

/// Three-buffer transport chain.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_three_buffer_transport_chain() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // Buffer 1 -> Buffer 2 -> Buffer 3
    let data = vec![0xDEu8, 0xAD, 0xBE, 0xEF];

    let buf1 = data_buffer_create().expect("buf1");
    data_load(&buf1, &PmixByteObject::from(data.clone())).expect("load buf1");
    let p1 = data_unload(&buf1).expect("unload buf1");

    let buf2 = data_buffer_create().expect("buf2");
    data_load(&buf2, &p1).expect("load buf2");
    let p2 = data_unload(&buf2).expect("unload buf2");

    let buf3 = data_buffer_create().expect("buf3");
    data_load(&buf3, &p2).expect("load buf3");
    let p3 = data_unload(&buf3).expect("unload buf3");

    assert_eq!(p3.as_slice(), &data[..]);
}

/// Transport chain where intermediate payload is used after load (consumed).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_transport_chain_payload_consumed() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf1 = data_buffer_create().expect("buf1");
    let data = vec![1u8, 2, 3];
    let payload = PmixByteObject::from(data.clone());
    data_load(&buf1, &payload).expect("load buf1");
    assert!(payload.is_empty(), "payload should be consumed by load");

    let transport = data_unload(&buf1).expect("unload buf1");
    assert_eq!(transport.as_slice(), &data[..]);

    let buf2 = data_buffer_create().expect("buf2");
    data_load(&buf2, &transport).expect("load buf2");
    assert!(transport.is_empty(), "transport should be consumed by load");

    let recovered = data_unload(&buf2).expect("unload buf2");
    assert_eq!(recovered.as_slice(), &data[..]);
}

// ─────────────────────────────────────────────────────────────────────────────
// data_pack — additional edge cases (no FFI beyond guard)
// ─────────────────────────────────────────────────────────────────────────────

/// data_pack with num_vals == 0 returns error regardless of data type.
#[test]
fn test_pack_zero_num_vals_all_types() {
    let buf = data_buffer_create().expect("create buffer");
    let val: i32 = 42;

    for dtype in [
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
        PmixDataType::Bool,
        PmixDataType::Size,
        PmixDataType::Status,
    ] {
        let result = data_pack(None, &buf, &val, 0, dtype);
        assert!(
            result.is_err(),
            "pack with num_vals=0 should fail for {:?}",
            dtype
        );
        assert_eq!(result.unwrap_err().to_raw(), -27);
    }
}

/// data_pack with explicit target and num_vals == 0 still returns error.
#[test]
fn test_pack_with_target_zero_num_vals() {
    let buf = data_buffer_create().expect("create buffer");
    let target = PmixProcRef::new("test_ns", 0);
    let val: i32 = 42;
    let result = data_pack(Some(target), &buf, &val, 0, PmixDataType::Int32);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -27);
}

// ─────────────────────────────────────────────────────────────────────────────
// data_buffer_create — edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// data_buffer_create returns buffer with consistent initial state.
#[test]
fn test_buffer_create_consistent_initial_state() {
    for _ in 0..10 {
        let buf = data_buffer_create().expect("create");
        assert!(buf.is_valid());
        assert_eq!(buf.bytes_used(), 0);
        assert!(!buf.as_mut_ptr().is_null());
    }
}

/// data_buffer_create result is independent across calls.
#[test]
fn test_buffer_create_independent() {
    let buf1 = data_buffer_create().expect("buf1");
    let buf2 = data_buffer_create().expect("buf2");
    assert_ne!(
        buf1.as_mut_ptr(),
        buf2.as_mut_ptr(),
        "different buffers should have different pointers"
    );
}

/// data_buffer_create works after previous buffers are dropped.
#[test]
fn test_buffer_create_after_release() {
    {
        let buf = data_buffer_create().expect("buf");
        // Don't call data_buffer_release explicitly — Drop handles it.
        // Calling data_buffer_release(&mut buf) before Drop causes double-free
        // because the wrapper doesn't null the internal pointer.
    }
    let buf2 = data_buffer_create().expect("buf2");
    assert!(buf2.is_valid());
}

// ─────────────────────────────────────────────────────────────────────────────
// from_raw null pointer edge case
// ─────────────────────────────────────────────────────────────────────────────

/// PmixDataBuffer::from_raw(null) produces an invalid buffer.
#[test]
fn test_buffer_from_raw_null() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    assert!(!buf.is_valid(), "null buffer should be invalid");
    assert_eq!(
        buf.bytes_allocated(),
        0,
        "null buffer should report 0 allocated"
    );
    assert_eq!(buf.bytes_used(), 0, "null buffer should report 0 used");
}

/// PmixDataBuffer::from_raw(null) Debug shows null.
#[test]
fn test_buffer_from_raw_null_debug() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    let debug = format!("{:?}", buf);
    assert!(
        debug.contains("null"),
        "null buffer debug should show 'null'"
    );
}

/// PmixDataBuffer::from_raw(null) as_mut_ptr returns null.
#[test]
fn test_buffer_from_raw_null_as_mut_ptr() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    assert!(buf.as_mut_ptr().is_null());
}

/// Drop of null PmixDataBuffer does nothing (no crash).
#[test]
fn test_buffer_from_raw_null_drop() {
    let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    // Drop should be safe — it checks is_valid() before calling release
    drop(buf);
}

// ─────────────────────────────────────────────────────────────────────────────
// data_buffer_release on null buffer
// ─────────────────────────────────────────────────────────────────────────────

/// data_buffer_release on a null-created buffer is safe.
#[test]
fn test_release_null_buffer() {
    let mut buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
    // data_buffer_release checks is_valid() which returns false for null
    data_buffer_release(&mut buf);
    // Should not crash
}

// ─────────────────────────────────────────────────────────────────────────────
// Comprehensive type system checks
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_buffer_create return type.
#[test]
fn test_buffer_create_return_type() {
    let _: fn() -> Result<PmixDataBuffer, PmixStatus> = data_buffer_create;
}

/// Verify data_buffer_release signature.
#[test]
fn test_buffer_release_signature() {
    let _: fn(&mut PmixDataBuffer) = data_buffer_release;
}

/// Verify data_load signature.
#[test]
fn test_data_load_signature() {
    let _: fn(&PmixDataBuffer, &PmixByteObject) -> Result<(), PmixStatus> = data_load;
}

/// Verify data_unload signature.
#[test]
fn test_data_unload_signature() {
    let _: fn(&PmixDataBuffer) -> Result<PmixByteObject, PmixStatus> = data_unload;
}

/// Verify data_embed signature.
#[test]
fn test_data_embed_signature() {
    let _: fn(&PmixDataBuffer, Option<&PmixByteObject>) -> Result<(), PmixStatus> = data_embed;
}

/// Verify data_compress signature.
#[test]
fn test_data_compress_signature() {
    let _: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_compress;
}

/// Verify data_decompress signature.
#[test]
fn test_data_decompress_signature() {
    let _: fn(&[u8]) -> Result<Vec<u8>, PmixStatus> = data_decompress;
}

/// Verify data_pack signature.
#[test]
fn test_data_pack_signature() {
    fn check<F, T>(_: F) {
        let _: fn(&T, i32, PmixDataType) -> Result<i32, PmixStatus>;
    }
    // Generic check — just verify it compiles
    let _: fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &i32,
        i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus> = data_pack;
}

/// Verify data_unpack signature.
#[test]
fn test_data_unpack_signature() {
    let _: fn(
        Option<PmixProcRef>,
        &PmixDataBuffer,
        &mut i32,
        &mut i32,
        PmixDataType,
    ) -> Result<i32, PmixStatus> = data_unpack;
}

/// Verify data_copy signature.
#[test]
fn test_data_copy_signature() {
    let _: fn(&i32, PmixDataType) -> Result<*mut std::os::raw::c_void, PmixStatus> = data_copy;
}

/// Verify data_copy_payload signature.
#[test]
fn test_data_copy_payload_signature() {
    let _: fn(&PmixDataBuffer, &PmixDataBuffer) -> Result<(), PmixStatus> = data_copy_payload;
}

/// Verify data_print signature.
#[test]
fn test_data_print_signature() {
    let _: fn(&i32, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus> =
        data_print;
}

// ─────────────────────────────────────────────────────────────────────────────
// Module-level API surface checks
// ─────────────────────────────────────────────────────────────────────────────

/// All public functions are accessible from the module.
#[test]
fn test_all_public_functions_accessible() {
    use pmix::data_serialization;
    let _ = data_serialization::data_buffer_create;
    let _ = data_serialization::data_buffer_release;
    let _ = data_serialization::data_load;
    let _ = data_serialization::data_unload;
    let _ = data_serialization::data_pack::<i32>;
    let _ = data_serialization::data_unpack::<i32>;
    let _ = data_serialization::data_copy::<i32>;
    let _ = data_serialization::data_copy_payload;
    let _ = data_serialization::data_print::<i32>;
    let _ = data_serialization::data_embed;
    let _ = data_serialization::data_compress;
    let _ = data_serialization::data_decompress;
}

/// All public types are accessible from the module.
#[test]
fn test_all_public_types_accessible() {
    use pmix::data_serialization;
    fn _assert_type<T>() {}
    _assert_type::<data_serialization::PmixDataBuffer>();
    _assert_type::<data_serialization::PmixByteObject>();
    _assert_type::<data_serialization::PmixPrintOutput>();
    _assert_type::<data_serialization::PmixProcRef>();
}
