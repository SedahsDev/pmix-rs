//! Tests for `PMIx_Data_load`.
//!
//! `PMIx_Data_load` loads a raw `pmix_byte_object_t` payload into a
//! `pmix_data_buffer_t`, replacing any existing buffer content. This is the
//! receiver-side counterpart to `data_unload`: the sender unloads a buffer
//! into a byte object for transport, and the receiver loads it back into a
//! buffer for unpacking.
//!
//! Key behavior (from C source):
//! - Destructs the buffer first (clears any existing content)
//! - Sets buffer pointers directly from the payload (no copy)
//! - **Consumes** the payload: sets payload->bytes = NULL and payload->size = 0
//!   after loading. The payload object is cleared and must not be reused.
//!
//! `PMIx_Data_load` requires `PMIx_Init` — the C implementation checks the
//! initialization state before operating. Run FFI integration tests under
//! the DVM:
//! ```bash
//! prterun -np 1 cargo test --test data_serialization_Data_load -- --ignored --test-threads=1
//! ```
//!
//! # C API
//! `pmix_status_t PMIx_Data_load(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`

use std::sync::OnceLock;

use pmix::{init, PmixStatus};
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Singleton PMIx init — PMIx can only be initialized once per process.
// The FFI integration tests below all share this single context.
// ─────────────────────────────────────────────────────────────────────────────

static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();

fn ensure_init() -> &'static pmix::Context {
    PMIX_CTX.get_or_init(|| init(None).expect("PMIx_Init failed — run under prterun"))
}

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only type checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_load signature: takes &PmixDataBuffer, &PmixByteObject, returns Result<(), PmixStatus>.
#[test]
fn test_data_load_signature() {
    fn check<F>(_: F) {}
    check::<fn(&PmixDataBuffer, &PmixByteObject) -> Result<(), PmixStatus>>(data_load);
}

/// Verify PmixDataBuffer has the expected API surface.
#[test]
fn test_data_buffer_api() {
    let buf = data_buffer_create().expect("create buffer");
    let _: *mut std::ffi::c_void = buf.as_mut_ptr() as *mut std::ffi::c_void;
    let _: bool = buf.is_valid();
    let _: usize = buf.bytes_allocated();
    let _: usize = buf.bytes_used();
}

/// Verify PmixDataBuffer implements Debug.
#[test]
fn test_data_buffer_debug() {
    let buf = data_buffer_create().expect("create buffer");
    let debug_str = format!("{:?}", buf);
    assert!(
        debug_str.contains("PmixDataBuffer"),
        "Debug output should contain struct name"
    );
}

/// Verify PmixDataBuffer::is_valid is true for a freshly created buffer.
#[test]
fn test_data_buffer_is_valid() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid(), "new buffer should be valid");
}

/// Verify PmixDataBuffer starts with 0 bytes used.
#[test]
fn test_data_buffer_empty() {
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");
}

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
// PMIx_Data_load — basic functionality (requires PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

/// data_load with an empty payload should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_empty_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::new();
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading empty payload should succeed, got {:?}",
        result
    );
}

/// data_load with a single byte payload should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_single_byte() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![42u8]);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading single byte should succeed, got {:?}",
        result
    );
}

/// data_load with a multi-byte payload should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_multi_byte() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![10u8, 20, 30, 40, 50]);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading multi-byte payload should succeed, got {:?}",
        result
    );
}

/// data_load with a large payload (4KB) should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_large_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading large payload should succeed, got {:?}",
        result
    );
}

/// data_load with all-zero bytes should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_all_zeros() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0u8; 64]);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading all zeros should succeed, got {:?}",
        result
    );
}

/// data_load with all-0xFF bytes should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_all_0xff() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0xFFu8; 128]);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading all 0xFF should succeed, got {:?}",
        result
    );
}

/// data_load with alternating byte pattern should succeed.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_alternating_pattern() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..256)
        .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
        .collect();
    let payload = PmixByteObject::from(original);
    let result = data_load(&buf, &payload);
    assert!(
        result.is_ok(),
        "loading alternating pattern should succeed, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Payload consumption — PMIx_Data_load clears the source payload
// ─────────────────────────────────────────────────────────────────────────────

/// PMIx_Data_load consumes the payload — after loading, the payload
/// should be empty (bytes=NULL, size=0) because the C implementation
/// transfers ownership of the data to the buffer.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_consumes_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![1u8, 2, 3, 4, 5];
    let payload = PmixByteObject::from(original.clone());
    assert_eq!(payload.size(), 5, "payload should have 5 bytes before load");

    data_load(&buf, &payload).expect("load should succeed");

    // The C implementation sets payload->bytes = NULL and payload->size = 0
    // after loading. Our wrapper should reflect this.
    assert!(
        payload.is_empty(),
        "payload should be empty after load (consumed by buffer)"
    );
}

/// Verify payload size is 0 after load.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_payload_size_zero_after() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![10u8, 20, 30]);
    assert_eq!(payload.size(), 3);

    data_load(&buf, &payload).expect("load should succeed");

    assert_eq!(payload.size(), 0, "payload size should be 0 after load");
}

/// Verify payload as_slice returns empty after load.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_payload_slice_empty_after() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![42u8, 84, 126]);
    assert_eq!(payload.as_slice(), &[42u8, 84, 126]);

    data_load(&buf, &payload).expect("load should succeed");

    assert!(
        payload.as_slice().is_empty(),
        "payload slice should be empty after load"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer state after load
// ─────────────────────────────────────────────────────────────────────────────

/// Buffer bytes_used should reflect the loaded payload size.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_buffer_bytes_used() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    assert_eq!(buf.bytes_used(), 0, "new buffer should have 0 bytes used");

    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4, 5]);
    data_load(&buf, &payload).expect("load should succeed");

    assert!(
        buf.bytes_used() > 0,
        "buffer should have data after load, bytes_used={}",
        buf.bytes_used()
    );
}

/// Buffer bytes_allocated should be >= payload size after load.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_buffer_bytes_allocated() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload_size = 1024usize;
    let payload = PmixByteObject::from(vec![0u8; payload_size]);
    data_load(&buf, &payload).expect("load should succeed");

    assert!(
        buf.bytes_allocated() >= payload_size,
        "buffer allocation should be >= payload size, allocated={}, payload={}",
        buf.bytes_allocated(),
        payload_size
    );
}

/// data_load replaces buffer content — second load should replace first.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_replaces_buffer_content() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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

// ─────────────────────────────────────────────────────────────────────────────
// Load/Unload roundtrips
// ─────────────────────────────────────────────────────────────────────────────

/// data_load then data_unload should roundtrip the bytes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_large_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(
        recovered.as_slice(),
        &original,
        "large roundtrip should match"
    );
}

/// data_load then data_unload with a single byte.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_single_byte_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![42u8];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with all zero bytes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_all_zeros_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0u8; 64];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with all 0xFF bytes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_all_0xff_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original = vec![0xFFu8; 128];
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// data_load then data_unload with alternating pattern.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_alternating_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..256)
        .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
        .collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load then unload with 64KB payload.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_unload_64kb_roundtrip() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple buffers — independence
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple buffers can each be loaded independently.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_multiple_buffers_load() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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

// ─────────────────────────────────────────────────────────────────────────────
// Transport chain — sender/receiver pattern
// ─────────────────────────────────────────────────────────────────────────────

/// Full transport chain: load into buf1, unload to payload, load into buf2, unload again.
/// Simulates the sender-receiver transport pattern.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_transport_chain() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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
#[ignore = "requires DVM-launched process (prterun)"]
fn test_two_transport_chains() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
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

// ─────────────────────────────────────────────────────────────────────────────
// Boundary cases
// ─────────────────────────────────────────────────────────────────────────────

/// Load with boundary-size payload (1 byte).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_minimal_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![0x00u8]);
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &[0x00u8]);
}

/// Load with boundary-size payload (255 bytes).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_255_byte_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..255).map(|i| i as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load with boundary-size payload (256 bytes).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_256_byte_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..=255).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}

/// Load with boundary-size payload (1024 bytes).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_1024_byte_payload() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let original: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let payload = PmixByteObject::from(original.clone());
    data_load(&buf, &payload).expect("load should succeed");

    let recovered = data_unload(&buf).expect("unload should succeed");
    assert_eq!(recovered.as_slice(), &original);
}
