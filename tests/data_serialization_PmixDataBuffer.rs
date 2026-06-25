//! Tests for `PmixDataBuffer` — construction, accessors, and lifecycle.
//!
//! Most tests exercise the Rust-side wrapper without requiring PMIx_Init.
//! Tests that call `data_load` or `data_unload` need PMIx_Init and are
//! marked `#[ignore]` — run them under prterun:
//! ```bash
//! prterun -np 1 cargo test --test data_serialization_PmixDataBuffer -- --ignored --test-threads=1
//! ```

use std::sync::OnceLock;

use pmix::{data_serialization::*, init};

// ─────────────────────────────────────────────────────────────────────────────
// Singleton PMIx init — PMIx can only be initialized once per process.
// ─────────────────────────────────────────────────────────────────────────────

static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();

fn ensure_init() -> &'static pmix::Context {
    PMIX_CTX.get_or_init(|| init(None).expect("PMIx_Init failed — run under prterun"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer creation
// ─────────────────────────────────────────────────────────────────────────────

/// data_buffer_create() returns a valid, non-null buffer.
#[test]
fn test_buffer_create_returns_valid() {
    let buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid());
}

/// data_buffer_create() can be called multiple times.
#[test]
fn test_buffer_create_multiple() {
    let buf1 = data_buffer_create().expect("create buffer 1");
    let buf2 = data_buffer_create().expect("create buffer 2");
    assert!(buf1.is_valid());
    assert!(buf2.is_valid());
    // Both buffers are independent
}

/// data_buffer_create() produces buffers with initial state.
#[test]
fn test_buffer_create_initial_state() {
    let buf = data_buffer_create().expect("create buffer");
    // Fresh buffer should have some allocated memory (internal allocation)
    assert!(buf.is_valid());
    // bytes_allocated may be 0 or some initial size depending on PMIx impl
    let _ = buf.bytes_allocated();
    let _ = buf.bytes_used();
}

// ─────────────────────────────────────────────────────────────────────────────
// Accessors
// ─────────────────────────────────────────────────────────────────────────────

/// as_mut_ptr() returns a non-null pointer for a valid buffer.
#[test]
fn test_buffer_as_mut_ptr_valid() {
    let buf = data_buffer_create().expect("create buffer");
    let ptr = buf.as_mut_ptr();
    assert!(!ptr.is_null());
}

/// bytes_allocated() returns a non-negative value.
#[test]
fn test_buffer_bytes_allocated() {
    let buf = data_buffer_create().expect("create buffer");
    let allocated = buf.bytes_allocated();
    // Should be >= 0 (could be 0 for fresh buffer or some initial size)
    // bytes_allocated is usize, so always >= 0; just verify it doesn't panic
    let _ = allocated;
}

/// bytes_used() returns 0 for a fresh buffer.
#[test]
fn test_buffer_bytes_used_fresh() {
    let buf = data_buffer_create().expect("create buffer");
    // Fresh buffer should have 0 bytes used
    assert_eq!(buf.bytes_used(), 0);
}

/// Accessors work consistently on the same buffer.
#[test]
fn test_buffer_accessors_consistent() {
    let buf = data_buffer_create().expect("create buffer");
    let alloc1 = buf.bytes_allocated();
    let used1 = buf.bytes_used();
    let alloc2 = buf.bytes_allocated();
    let used2 = buf.bytes_used();
    // Multiple calls should return the same values
    assert_eq!(alloc1, alloc2);
    assert_eq!(used1, used2);
    // Used should not exceed allocated
    assert!(used2 <= alloc2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Release semantics
// ─────────────────────────────────────────────────────────────────────────────

/// data_buffer_release() invalidates the buffer.
#[test]
fn test_buffer_release_invalidates() {
    let mut buf = data_buffer_create().expect("create buffer");
    assert!(buf.is_valid());
    data_buffer_release(&mut buf);
    assert!(!buf.is_valid());
}

/// data_buffer_release() on already-released buffer is safe (no double-free).
#[test]
fn test_buffer_release_double_safe() {
    let mut buf = data_buffer_create().expect("create buffer");
    data_buffer_release(&mut buf);
    // Second release should be a no-op (pointer is null)
    data_buffer_release(&mut buf);
    // Should not crash
}

/// Buffer accessors return 0 after release.
#[test]
fn test_buffer_accessors_after_release() {
    let mut buf = data_buffer_create().expect("create buffer");
    data_buffer_release(&mut buf);
    // After release, buffer is invalid
    assert!(!buf.is_valid());
    // Accessors on invalid buffer should return 0
    assert_eq!(buf.bytes_allocated(), 0);
    assert_eq!(buf.bytes_used(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Drop / lifecycle
// ─────────────────────────────────────────────────────────────────────────────

/// Buffer is automatically released on drop.
#[test]
fn test_buffer_auto_drop() {
    {
        let buf = data_buffer_create().expect("create buffer");
        assert!(buf.is_valid());
    }
    // buf dropped here, should not crash (automatic release)
}

/// Manual release followed by drop is safe.
#[test]
fn test_buffer_manual_release_then_drop() {
    let mut buf = data_buffer_create().expect("create buffer");
    data_buffer_release(&mut buf);
    // buf will be dropped at end of scope, but pointer is null so no-op
}

/// Multiple buffers created and dropped in a loop.
#[test]
fn test_buffer_lifecycle_loop() {
    for _ in 0..20 {
        let buf = data_buffer_create().expect("create buffer");
        assert!(buf.is_valid());
        // buf dropped at end of iteration
    }
}

/// Debug trait works on PmixDataBuffer.
#[test]
fn test_buffer_debug() {
    let buf = data_buffer_create().expect("create buffer");
    let debug_str = format!("{:?}", buf);
    assert!(debug_str.contains("PmixDataBuffer"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration with PmixByteObject
// ─────────────────────────────────────────────────────────────────────────────

/// Buffer and byte object can coexist without issues.
#[test]
fn test_buffer_with_byte_object() {
    let buf = data_buffer_create().expect("create buffer");
    let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
    assert!(buf.is_valid());
    assert_eq!(payload.size(), 4);
    // Both can be used simultaneously
}

/// Empty byte object can be loaded into buffer.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_load_empty_payload_into_buffer() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let empty_payload = PmixByteObject::new();
    let result = data_load(&buf, &empty_payload);
    assert!(result.is_ok());
}

/// data_unload on empty buffer returns empty byte object.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unload_empty_buffer() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let _ctx = ensure_init();
    let buf = data_buffer_create().expect("create buffer");
    let payload = data_unload(&buf).expect("unload empty buffer");
    assert!(payload.is_empty());
    assert_eq!(payload.size(), 0);
}
