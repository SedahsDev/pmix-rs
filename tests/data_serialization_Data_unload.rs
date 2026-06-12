//! Tests for `PMIx_Data_unload`.
//!
//! `PMIx_Data_unload` extracts the packed data from a `pmix_data_buffer_t`
//! as a raw `pmix_byte_object_t`. This is the final step in the sender-side
//! serialization pipeline: pack values into a buffer, then unload the buffer
//! into a byte object for transport (e.g., over the network).
//!
//! Unlike `PMIx_Data_pack` and `PMIx_Data_unpack`, `PMIx_Data_unload`
//! operates entirely in user space and does NOT require `PMIx_Init`.
//!
//! # C API
//! `pmix_status_t PMIx_Data_unload(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`

use pmix::PmixStatus;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only type checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_unload signature: takes &PmixDataBuffer, returns Result<PmixByteObject, PmixStatus>.
#[test]
fn test_data_unload_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer) -> Result<PmixByteObject, PmixStatus>>(data_unload);
}

/// Verify PmixByteObject has the expected API surface.
#[test]
fn test_byte_object_api() {
    let obj = PmixByteObject::new();
    // These should all compile
    let _: &[u8] = obj.as_slice();
    let _: usize = obj.size();
    let _: bool = obj.is_empty();
}

/// Verify PmixByteObject implements Default.
#[test]
fn test_byte_object_default() {
    let obj = PmixByteObject::default();
    assert!(obj.is_empty(), "default byte object should be empty");
    assert_eq!(obj.size(), 0, "default byte object should have size 0");
}

/// Verify PmixByteObject implements Debug.
#[test]
fn test_byte_object_debug() {
    let obj = PmixByteObject::new();
    let debug_str = format!("{:?}", obj);
    assert!(
        debug_str.contains("PmixByteObject"),
        "Debug output should contain struct name"
    );
}

/// Verify PmixByteObject::from(Vec<u8>) creates a byte object with correct size.
#[test]
fn test_byte_object_from_vec() {
    let bytes = vec![1u8, 2, 3, 4, 5];
    let obj = PmixByteObject::from(bytes);
    assert_eq!(obj.size(), 5, "byte object size should match vec length");
    assert_eq!(
        obj.as_slice(),
        &[1, 2, 3, 4, 5],
        "byte object slice should match original vec"
    );
}

/// Verify PmixByteObject::from(Vec<u8>) with empty vec produces an empty object.
#[test]
fn test_byte_object_from_empty_vec() {
    let bytes = vec![];
    let obj = PmixByteObject::from(bytes);
    assert!(obj.is_empty(), "byte object from empty vec should be empty");
    assert_eq!(obj.size(), 0);
}

/// Verify PmixByteObject::as_slice returns empty slice for null/zero-size object.
#[test]
fn test_byte_object_as_slice_empty() {
    let obj = PmixByteObject::new();
    let slice = obj.as_slice();
    assert!(
        slice.is_empty(),
        "slice of empty byte object should be empty"
    );
}

/// Verify PmixByteObject frees memory on drop (no leak / double-free).
#[test]
fn test_byte_object_drop() {
    let bytes = vec![10u8; 256];
    let _obj = PmixByteObject::from(bytes);
    // dropped here — should not crash or leak
}

/// Verify multiple PmixByteObjects can be created and dropped independently.
#[test]
fn test_multiple_byte_objects() {
    let obj1 = PmixByteObject::from(vec![1u8, 2, 3]);
    let obj2 = PmixByteObject::from(vec![4u8, 5, 6, 7]);
    let obj3 = PmixByteObject::new();
    assert_eq!(obj1.size(), 3);
    assert_eq!(obj2.size(), 4);
    assert!(obj3.is_empty());
    // all dropped independently — no double-free
}

/// Verify PmixByteObject::from with large vec.
#[test]
fn test_byte_object_from_large_vec() {
    let bytes: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let obj = PmixByteObject::from(bytes.clone());
    assert_eq!(obj.size(), 4096);
    assert_eq!(obj.as_slice(), &bytes[..]);
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_unload — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// data_unload from a freshly created (empty) buffer.
/// PMIx may return success with an empty payload or an error — both are valid.
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
            // Error is also acceptable for an empty buffer — PMIx may
            // reject unloading a buffer that has no packed data.
        }
    }
}

/// data_load then data_unload should roundtrip the bytes.
/// This is the core test: load raw bytes into a buffer, then unload them
/// and verify the payload matches the original.
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

/// data_load then data_unload with a larger payload (1024 bytes).
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

/// data_load then data_unload with a single byte.
#[test]
fn test_load_unload_single_byte() {
    let buf = data_buffer_create().expect("create buffer");

    let original = vec![42u8];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with all zero bytes.
#[test]
fn test_load_unload_all_zeros() {
    let buf = data_buffer_create().expect("create buffer");

    let original = vec![0u8; 64];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with all 0xFF bytes.
#[test]
fn test_load_unload_all_0xff() {
    let buf = data_buffer_create().expect("create buffer");

    let original = vec![0xFFu8; 128];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with alternating pattern.
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

/// PMIx_Data_unload is destructive — it clears the buffer after unloading.
/// A second unload on the same buffer returns an empty payload (buffer is now empty).
#[test]
fn test_unload_is_destructive() {
    let buf = data_buffer_create().expect("create buffer");

    let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    // First unload — succeeds with data
    let recovered1 = data_unload(&buf).expect("first unload should succeed");
    assert_eq!(recovered1.as_slice(), &original);

    // Second unload — buffer is now empty (unload is destructive)
    // PMIx may return success with empty payload or error on empty buffer
    let result2 = data_unload(&buf);
    match result2 {
        Ok(payload2) => {
            assert!(
                payload2.as_slice().is_empty(),
                "second unload of emptied buffer should yield empty payload"
            );
        }
        Err(_) => {
            // Error is also acceptable for an empty buffer
        }
    }
}

/// data_unload returns an owned PmixByteObject — the returned object
/// should be independent of the buffer and survive buffer release.
#[test]
fn test_unload_returns_owned_payload() {
    let payload;
    {
        let buf = data_buffer_create().expect("create buffer");
        let original = vec![100u8, 200, 50];
        let load_payload = PmixByteObject::from(original.clone());
        data_load(&buf, &load_payload).expect("load should succeed");

        payload = data_unload(&buf).expect("unload should succeed");
        assert_eq!(payload.as_slice(), &original);
        // buf is dropped here — payload should still be valid
    }

    // payload should still be accessible after buffer is gone
    assert_eq!(payload.as_slice(), &[100u8, 200, 50]);
    assert_eq!(payload.size(), 3);
}

/// data_load replaces buffer content, then data_unload should reflect the new content.
#[test]
fn test_load_replace_then_unload() {
    let buf = data_buffer_create().expect("create buffer");

    // First load
    let payload1 = PmixByteObject::from(vec![1u8, 2, 3]);
    data_load(&buf, &payload1).expect("first load");

    let recovered1 = data_unload(&buf).expect("first unload");
    assert_eq!(recovered1.as_slice(), &[1u8, 2, 3]);

    // Second load replaces buffer content (PMIx_Data_load is destructive)
    let payload2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8]);
    data_load(&buf, &payload2).expect("second load");

    let recovered2 = data_unload(&buf).expect("second unload");
    assert_eq!(recovered2.as_slice(), &[4u8, 5, 6, 7, 8]);
}

/// data_unload payload survives buffer drop — the returned byte object
/// owns its memory independently of the buffer.
#[test]
fn test_unload_payload_survives_buffer_drop() {
    let payload;
    {
        let buf = data_buffer_create().expect("create buffer");
        let original = vec![42u8, 84, 126];
        let load_payload = PmixByteObject::from(original.clone());
        data_load(&buf, &load_payload).expect("load");

        payload = data_unload(&buf).expect("unload");
        assert_eq!(payload.as_slice(), &original);
    } // buf dropped here

    // payload should still be valid
    assert_eq!(payload.as_slice(), &[42u8, 84, 126]);
    assert_eq!(payload.size(), 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataBuffer interaction — no PMIx_Init required
// ─────────────────────────────────────────────────────────────────────────────

/// Buffer bytes_used increases after data_load, and data_unload recovers the data.
#[test]
fn test_buffer_bytes_used_after_load_unload() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");

    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    data_load(&buf, &payload).expect("load");
    assert!(buf.bytes_used() > 0, "buffer should have data after load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.size(), 4);
}

/// Multiple buffers can each be loaded and unloaded independently.
#[test]
fn test_multiple_buffers_load_unload() {
    let buf1 = data_buffer_create().expect("create buf1");
    let buf2 = data_buffer_create().expect("create buf2");

    let payload1 = PmixByteObject::from(vec![1u8, 2, 3]);
    let payload2 = PmixByteObject::from(vec![4u8, 5, 6, 7, 8]);

    data_load(&buf1, &payload1).expect("load buf1");
    data_load(&buf2, &payload2).expect("load buf2");

    let recovered1 = data_unload(&buf1).expect("unload buf1");
    let recovered2 = data_unload(&buf2).expect("unload buf2");

    assert_eq!(recovered1.as_slice(), &[1u8, 2, 3]);
    assert_eq!(recovered2.as_slice(), &[4u8, 5, 6, 7, 8]);
}

/// data_unload with a buffer that has boundary-size payload (1 byte).
#[test]
fn test_unload_minimal_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0x00u8]);
    data_load(&buf, &payload).expect("load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &[0x00u8]);
}

/// data_unload with a buffer that has boundary-size payload (255 bytes).
#[test]
fn test_unload_255_byte_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..255).map(|i| i as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_unload with a very large payload (64KB).
#[test]
fn test_unload_64kb_payload() {
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load");

    let recovered = data_unload(&buf).expect("unload");
    assert_eq!(recovered.as_slice(), &original);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus error handling
// ─────────────────────────────────────────────────────────────────────────────

/// Verify PmixStatus::from_raw converts C status codes correctly.
#[test]
fn test_pmix_status_from_raw_success() {
    let success = PmixStatus::from_raw(0);
    assert!(success.is_success(), "PMIX_SUCCESS (0) should be success");
}

/// Verify PmixStatus::from_raw converts error codes.
#[test]
fn test_pmix_status_from_raw_error() {
    let err = PmixStatus::from_raw(-1); // PMIX_ERROR
    assert!(err.is_error(), "PMIX_ERROR (-1) should be error");
}

/// Verify PmixStatus Debug output.
#[test]
fn test_pmix_status_debug() {
    let status = PmixStatus::from_raw(0);
    let debug_str = format!("{:?}", status);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration: load -> unload -> load -> unload chain
// ─────────────────────────────────────────────────────────────────────────────

/// Full roundtrip chain: load into buf1, unload to payload, load into buf2, unload again.
/// Simulates the sender-receiver transport pattern.
#[test]
fn test_transport_chain() {
    // Sender side
    let sender_buf = data_buffer_create().expect("create sender buffer");
    let original = vec![0xDEu8, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let sender_payload = PmixByteObject::from(original.clone());
    data_load(&sender_buf, &sender_payload).expect("sender load");

    let transport_payload = data_unload(&sender_buf).expect("sender unload");
    assert_eq!(transport_payload.as_slice(), &original);

    // Receiver side
    let receiver_buf = data_buffer_create().expect("create receiver buffer");
    data_load(&receiver_buf, &transport_payload).expect("receiver load");

    let recovered = data_unload(&receiver_buf).expect("receiver unload");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "full chain should preserve data"
    );
}

/// Two sequential transport chains with different data should not interfere.
#[test]
fn test_two_transport_chains() {
    // Chain 1
    let buf1 = data_buffer_create().expect("create buf1");
    let data1 = vec![1u8, 2, 3];
    data_load(&buf1, &PmixByteObject::from(data1.clone())).expect("load1");
    let payload1 = data_unload(&buf1).expect("unload1");

    // Chain 2
    let buf2 = data_buffer_create().expect("create buf2");
    let data2 = vec![4u8, 5, 6, 7];
    data_load(&buf2, &PmixByteObject::from(data2.clone())).expect("load2");
    let payload2 = data_unload(&buf2).expect("unload2");

    // Verify both chains are independent
    assert_eq!(payload1.as_slice(), &data1);
    assert_eq!(payload2.as_slice(), &data2);
}
