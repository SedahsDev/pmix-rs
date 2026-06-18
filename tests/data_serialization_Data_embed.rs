//! Tests for `data_embed` — embed raw binary payload into a data buffer.
//!
//! `data_embed` calls `PMIx_Data_embed` FFI which **segfaults** when PMIx
//! is not initialized. All functional tests are marked `#[ignore]`.
//! Non-ignored tests cover the `PmixByteObject` type used as payload.

use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject — pure Rust, safe without PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// PmixByteObject from empty vec is valid.
#[test]
fn test_byteobject_empty_for_embed() {
    let obj = PmixByteObject::from(vec![] as Vec<u8>);
    assert_eq!(obj.size(), 0);
    assert!(obj.as_slice().is_empty());
}

/// PmixByteObject from large vec for embed payload.
#[test]
fn test_byteobject_large_for_embed() {
    let data: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let obj = PmixByteObject::from(data.clone());
    assert_eq!(obj.size(), 4096);
    assert_eq!(obj.as_slice(), data.as_slice());
}

/// PmixByteObject from single byte.
#[test]
fn test_byteobject_single_byte_for_embed() {
    let obj = PmixByteObject::from(vec![0xFF]);
    assert_eq!(obj.size(), 1);
    assert_eq!(obj.as_slice(), &[0xFF]);
}

/// PmixByteObject as_slice returns correct data.
#[test]
fn test_byteobject_as_slice_for_embed() {
    let obj = PmixByteObject::from(vec![1u8, 2, 3]);
    assert_eq!(obj.as_slice(), &[1, 2, 3]);
}

/// PmixByteObject drop safety.
#[test]
fn test_byteobject_drop_safety() {
    let obj = PmixByteObject::from(vec![1u8, 2, 3, 4, 5]);
    drop(obj);
    // No use-after-free — the allocator freed the memory on drop.
}

/// PmixByteObject from vec with null bytes.
#[test]
fn test_byteobject_null_bytes_for_embed() {
    let data = vec![0u8, 0, 0, 0];
    let obj = PmixByteObject::from(data.clone());
    assert_eq!(obj.size(), 4);
    assert_eq!(obj.as_slice(), data.as_slice());
}

// ─────────────────────────────────────────────────────────────────────────────
// data_embed — requires PMIx_Init (FFI segfaults without it)
// ─────────────────────────────────────────────────────────────────────────────

/// data_embed with Some(payload).
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_with_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    let result = data_embed(&buf, Some(&payload));
    assert!(result.is_ok());
}

/// data_embed with None payload.
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_with_none_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let result = data_embed(&buf, None);
    assert!(result.is_ok());
}

/// data_embed with empty payload.
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_empty_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![]);
    let result = data_embed(&buf, Some(&payload));
    assert!(result.is_ok());
}

/// data_embed with large payload.
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_large_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0xABu8; 1024]);
    let result = data_embed(&buf, Some(&payload));
    assert!(result.is_ok());
}

/// Multiple data_embed calls on same buffer.
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_multiple_calls() {
    let buf = data_buffer_create().expect("create buffer");
    let payload1 = PmixByteObject::from(vec![1u8, 2]);
    let payload2 = PmixByteObject::from(vec![3u8, 4]);
    assert!(data_embed(&buf, Some(&payload1)).is_ok());
    assert!(data_embed(&buf, Some(&payload2)).is_ok());
}

/// data_embed then data_load on same buffer.
#[ignore = "requires PMIx_Init — PMIx_Data_embed segfaults without initialization"]
#[test]
fn test_embed_then_load() {
    let buf = data_buffer_create().expect("create buffer");
    let embed_payload = PmixByteObject::from(vec![1u8, 2, 3]);
    data_embed(&buf, Some(&embed_payload)).expect("embed");
    let load_payload = PmixByteObject::from(vec![4u8, 5, 6]);
    data_load(&buf, &load_payload).expect("load");
}
