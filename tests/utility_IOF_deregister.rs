//! Tests for `iof_deregister` and `iof_deregister_blocking`.
//!
//! These tests verify the safe Rust interface and type system.
//! They do NOT require a running PMIx daemon — they test that the
//! API compiles and has the expected signature / behavior.

use pmix::{PmixStatus, utility::iof_deregister, utility::iof_deregister_blocking};

// ──────────────────────────────────────────────────────────────────────
// Compile-time tests — these just need to type-check
// ──────────────────────────────────────────────────────────────────────

/// `iof_deregister` accepts a closure that takes `PmixStatus`.
#[test]
fn test_iof_deregister_accepts_closure() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    // This should compile — the closure signature matches IoForwardDeregHandler.
    // We expect it to return an error since PMIx is not initialized,
    // but the important thing is that the types work.
    let result = iof_deregister(0, &[], move |status: PmixStatus| {
        let _ = status;
        called_clone.store(true, Ordering::SeqCst);
    });

    // Without a PMIx runtime, the FFI call will return an error.
    // The callback should NOT be invoked on error (we free it immediately).
    assert!(
        result.is_err(),
        "iof_deregister should fail without PMIx runtime"
    );
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked when iof_deregister fails"
    );
}

/// `iof_deregister_blocking` returns a Result without needing a callback.
#[test]
fn test_iof_deregister_blocking_signature() {
    let result = iof_deregister_blocking(0, &[]);
    // Without a PMIx runtime, this should fail.
    assert!(
        result.is_err(),
        "iof_deregister_blocking should fail without PMIx runtime"
    );
}

/// `iof_deregister` accepts non-empty directives slice.
#[test]
fn test_iof_deregister_with_directives() {
    // We can't easily construct pmix_info_t without the full PMIx types,
    // so just verify the API accepts a slice reference.
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    // Empty directives — the important thing is the types compile.
    let result = iof_deregister(42, &[], move |status: PmixStatus| {
        let _ = status;
        called_clone.store(true, Ordering::SeqCst);
    });

    assert!(result.is_err(), "should fail without PMIx runtime");
    assert!(
        !called.load(Ordering::SeqCst),
        "callback not called on error"
    );
}

/// `iof_deregister_blocking` with different handle values.
#[test]
fn test_iof_deregister_blocking_various_handles() {
    for handle in [0, 1, 42, usize::MAX] {
        let result = iof_deregister_blocking(handle, &[]);
        assert!(
            result.is_err(),
            "iof_deregister_blocking(handle={}) should fail without PMIx runtime",
            handle
        );
    }
}

/// `iof_deregister` callback receives PmixStatus, not raw i32.
/// This is a compile-time check — if the trait bound changes, this breaks.
#[test]
fn test_iof_deregister_callback_type() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    let status_received = Arc::new(AtomicI32::new(999));
    let status_clone = status_received.clone();

    let result = iof_deregister(0, &[], move |status: PmixStatus| {
        status_clone.store(status.to_raw(), Ordering::SeqCst);
    });

    assert!(result.is_err());
    assert_eq!(
        status_received.load(Ordering::SeqCst),
        999,
        "callback should not have been called"
    );
}
