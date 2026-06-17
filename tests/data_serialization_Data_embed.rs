//! Tests for `PMIx_Data_embed`.
//!
//! PMIx_Data_embed embeds a raw byte payload into a data buffer without
//! clearing the source payload (unlike PMIx_Data_load which does clear it).
//!
//! Note: PMIx_Data_embed internally calls PMIx_Data_copy_payload which
//! requires PMIx_Init to have been called (it needs pmix_globals.mypeer).
//! These integration tests are marked #[ignore] and should be run with
//! a PMIx environment.
//!
//! The signature and type-safety tests below do NOT require PMIx_Init
//! and run normally.

use pmix::PmixStatus;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type-safety tests — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// data_embed function exists and has the correct signature.
///
/// This verifies the function is callable with the expected parameter types
/// without actually invoking the FFI (which would segfault without PMIx_Init).
#[test]
fn test_data_embed_signature() {
    // Verify the function type:
    // fn(&PmixDataBuffer, Option<&PmixByteObject>) -> Result<(), PmixStatus>
    let _fn_ref: fn(&PmixDataBuffer, Option<&PmixByteObject>) -> Result<(), PmixStatus> =
        data_embed;
}

/// data_embed is publicly exported from the crate.
#[test]
fn test_data_embed_public_api() {
    use pmix::data_serialization;
    let _ = data_serialization::data_embed;
}

/// PmixByteObject can be created from Vec<u8> for use with data_embed.
#[test]
fn test_byte_object_from_vec_for_embed() {
    let payload: PmixByteObject = vec![1u8, 2, 3, 4].into();
    assert_eq!(payload.size(), 4);
    assert_eq!(payload.as_slice(), &[1u8, 2, 3, 4]);
}

/// PmixByteObject::new creates an empty object suitable for data_embed.
#[test]
fn test_byte_object_empty_for_embed() {
    let payload = PmixByteObject::new();
    assert!(payload.is_empty());
    assert_eq!(payload.size(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_embed integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// data_embed with a valid payload succeeds.
///
/// Requires PMIx_Init — needs pmix_globals.mypeer for PMIx_Data_copy_payload.
#[test]
#[ignore]
fn test_embed_basic_success() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![1u8, 2, 3, 4].into();

    let result = data_embed(&buf, Some(&payload));
    assert!(
        result.is_ok(),
        "data_embed should succeed with valid payload"
    );
}

/// data_embed with None payload is a no-op and succeeds.
///
/// Requires PMIx_Init — the C function still runs its internal logic.
#[test]
#[ignore]
fn test_embed_none_payload() {
    let buf = data_buffer_create().expect("create buffer");

    let result = data_embed(&buf, None);
    assert!(
        result.is_ok(),
        "data_embed with None payload should succeed (no-op)"
    );
}

/// data_embed with an empty payload succeeds.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_empty_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();

    let result = data_embed(&buf, Some(&payload));
    assert!(
        result.is_ok(),
        "data_embed with empty payload should succeed"
    );
}

/// data_embed replaces existing buffer content (destructs then reconstructs).
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_replaces_buffer_content() {
    let buf = data_buffer_create().expect("create buffer");

    // Load initial data
    let initial: PmixByteObject = vec![10u8, 20, 30].into();
    data_load(&buf, &initial).expect("initial load");

    // Embed different data — should replace buffer content
    let replacement: PmixByteObject = vec![1u8, 2, 3, 4, 5].into();
    data_embed(&buf, Some(&replacement)).expect("embed should succeed");

    // Unload and verify the buffer now contains the replacement data
    let unloaded = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        unloaded.as_slice(),
        &[1u8, 2, 3, 4, 5],
        "buffer should contain the embedded data, not the initial data"
    );
}

/// data_embed does NOT clear the source payload (key difference from data_load).
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_preserves_source_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![42u8, 43, 44].into();

    let original_size = payload.size();
    let original_bytes = payload.as_slice().to_vec();

    data_embed(&buf, Some(&payload)).expect("embed should succeed");

    // Source payload should be unchanged after embed
    assert_eq!(
        payload.size(),
        original_size,
        "source payload size should be unchanged after data_embed"
    );
    assert_eq!(
        payload.as_slice(),
        original_bytes,
        "source payload bytes should be unchanged after data_embed"
    );
}

/// data_embed then data_unload roundtrips the bytes correctly.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_unload_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..=255).collect();
    let payload: PmixByteObject = original.clone().into();

    data_embed(&buf, Some(&payload)).expect("embed should succeed");

    let unloaded = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        unloaded.as_slice(),
        original.as_slice(),
        "roundtrip bytes should match original"
    );
}

/// data_embed with a single byte payload.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_single_byte() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![0xFFu8].into();

    data_embed(&buf, Some(&payload)).expect("embed single byte");

    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(unloaded.as_slice(), &[0xFFu8]);
}

/// data_embed with a large payload.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_large_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let large: Vec<u8> = (0..=255u8).cycle().take(10240).collect();
    let payload: PmixByteObject = large.clone().into();

    data_embed(&buf, Some(&payload)).expect("embed large payload");

    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(
        unloaded.as_slice(),
        large.as_slice(),
        "large payload roundtrip should match"
    );
}

/// Multiple data_embed calls on the same buffer — each replaces previous.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_multiple_replaces() {
    let buf = data_buffer_create().expect("create buffer");

    let first: PmixByteObject = vec![1u8].into();
    data_embed(&buf, Some(&first)).expect("first embed");

    let second: PmixByteObject = vec![2u8, 3].into();
    data_embed(&buf, Some(&second)).expect("second embed");

    let third: PmixByteObject = vec![4u8, 5, 6, 7].into();
    data_embed(&buf, Some(&third)).expect("third embed");

    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(
        unloaded.as_slice(),
        &[4u8, 5, 6, 7],
        "buffer should contain only the last embedded data"
    );
}

/// data_embed payload can be reused after embed (source not cleared).
///
/// This is the key behavioral difference from data_load: the same payload
/// can be embedded into multiple buffers.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_payload_reusable() {
    let buf1 = data_buffer_create().expect("create buf1");
    let buf2 = data_buffer_create().expect("create buf2");
    let payload: PmixByteObject = vec![100u8, 200].into();

    // Embed into first buffer
    data_embed(&buf1, Some(&payload)).expect("embed into buf1");

    // Same payload should still be embeddable into a second buffer
    data_embed(&buf2, Some(&payload)).expect("embed into buf2");

    // Verify both buffers contain the data
    let unloaded1 = data_unload(&buf1).expect("unload buf1");
    let unloaded2 = data_unload(&buf2).expect("unload buf2");

    assert_eq!(unloaded1.as_slice(), &[100u8, 200]);
    assert_eq!(unloaded2.as_slice(), &[100u8, 200]);
}

/// data_embed preserves the payload for subsequent as_slice reads.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_payload_still_readable() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![11u8, 22, 33].into();

    data_embed(&buf, Some(&payload)).expect("embed");

    // Payload should still be readable
    assert_eq!(payload.as_slice(), &[11u8, 22, 33]);
    assert_eq!(payload.size(), 3);
    assert!(!payload.is_empty());
}

/// data_embed with zero-valued bytes.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_zero_bytes() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![0u8, 0, 0, 0].into();

    data_embed(&buf, Some(&payload)).expect("embed zero bytes");

    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(unloaded.as_slice(), &[0u8, 0, 0, 0]);
}

/// data_embed buffer bytes_used reflects embedded payload size.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_buffer_bytes_used() {
    let buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![1u8, 2, 3, 4, 5, 6, 7, 8].into();

    data_embed(&buf, Some(&payload)).expect("embed");

    assert!(
        buf.bytes_used() >= 8,
        "buffer bytes_used should reflect embedded payload size (got {})",
        buf.bytes_used()
    );
}

/// data_embed then data_buffer_release cleans up without issues.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_then_release() {
    let mut buf = data_buffer_create().expect("create buffer");
    let payload: PmixByteObject = vec![1u8, 2, 3].into();

    data_embed(&buf, Some(&payload)).expect("embed");
    data_buffer_release(&mut buf);
    // No panic or double-free on release after embed
}

/// PmixByteObject from Vec<u8> then data_embed works end to end.
///
/// Requires PMIx_Init.
#[test]
#[ignore]
fn test_embed_from_vec_roundtrip() {
    let buf = data_buffer_create().expect("create buffer");

    // Create payload from a Vec
    let data = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
    let payload = PmixByteObject::from(data.clone());

    data_embed(&buf, Some(&payload)).expect("embed");

    let unloaded = data_unload(&buf).expect("unload");
    assert_eq!(unloaded.as_slice(), data.as_slice());
}
