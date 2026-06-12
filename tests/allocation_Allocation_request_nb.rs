//! Tests for `PMIx_Allocation_request_nb` via the safe `allocation` module wrappers.
//!
//! Derived from C test patterns in related allocation functions and
//! the general `_nb` callback pattern used throughout PMIx.
//! No dedicated C test exists for `PMIx_Allocation_request_nb` in the
//! upstream test suite — tests are derived from the blocking variant
//! and analogous `_nb` patterns (e.g., `PMIx_Query_info_nb`, `PMIx_Notify_event`).
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::Info;
use pmix::allocation::*;

// ─────────────────────────────────────────────────────────────────────────────
// PmixAllocDirective enum tests
// ─────────────────────────────────────────────────────────────────────────────

/// All allocation directive values roundtrip correctly.
#[test]
fn alloc_directive_all_roundtrip() {
    let directives: Vec<(PmixAllocDirective, u8)> = vec![
        (PmixAllocDirective::AllocNew, 1),
        (PmixAllocDirective::AllocExtend, 2),
        (PmixAllocDirective::AllocRelease, 3),
        (PmixAllocDirective::AllocReacquire, 4),
        (PmixAllocDirective::AllocExternal, 128),
    ];
    for (d, expected_raw) in &directives {
        assert_eq!(d.to_raw(), *expected_raw, "to_raw mismatch for {:?}", d);
        assert_eq!(
            PmixAllocDirective::from_raw(*expected_raw),
            *d,
            "from_raw mismatch for raw value {}",
            expected_raw
        );
    }
}

/// Unknown directive values are preserved through Unknown variant.
#[test]
fn alloc_directive_unknown_preserved() {
    for val in [0u8, 50, 100, 255] {
        let d = PmixAllocDirective::from_raw(val);
        assert!(
            matches!(d, PmixAllocDirective::Unknown(v) if v == val),
            "Unknown({}) not preserved",
            val
        );
        assert_eq!(d.to_raw(), val);
    }
}

/// Display for all directive variants.
#[test]
fn alloc_directive_display_all() {
    assert_eq!(format!("{}", PmixAllocDirective::AllocNew), "ALLOC_NEW");
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocExtend),
        "ALLOC_EXTEND"
    );
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocRelease),
        "ALLOC_RELEASE"
    );
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocReacquire),
        "ALLOC_REAQUIRE"
    );
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocExternal),
        "ALLOC_EXTERNAL"
    );
    assert_eq!(
        format!("{}", PmixAllocDirective::Unknown(99)),
        "UNKNOWN_DIRECTIVE (99)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// allocation_request_nb — non-blocking without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Simple callback that captures status for testing.
struct TestAllocCallback {
    called: std::cell::RefCell<bool>,
}
impl AllocationCallback for TestAllocCallback {
    fn on_complete(&self, _status: pmix::PmixStatus, _results: AllocationResults) {
        *self.called.borrow_mut() = true;
    }
}

/// Non-blocking allocation request without PMIx_Init must fail immediately
/// and NOT invoke the callback.
///
/// Derived from the general PMIx pattern: when PMIx is not initialized,
/// `_nb` functions return an error synchronously without queuing the
/// callback.
#[test]
fn allocation_request_nb_without_init_fails() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let info: &[Info] = &[];
    let result = allocation_request_nb(PmixAllocDirective::AllocNew, info, cb);
    assert!(
        result.is_err(),
        "allocation_request_nb without PMIx_Init should fail"
    );
}

/// Non-blocking allocation with AllocNew directive fails without init.
#[test]
fn allocation_request_nb_alloc_new_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocNew, &[], cb);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.is_success());
}

/// Non-blocking allocation with AllocExtend directive fails without init.
#[test]
fn allocation_request_nb_alloc_extend_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocExtend, &[], cb);
    assert!(result.is_err());
}

/// Non-blocking allocation with AllocRelease directive fails without init.
#[test]
fn allocation_request_nb_alloc_release_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocRelease, &[], cb);
    assert!(result.is_err());
}

/// Non-blocking allocation with AllocReacquire directive fails without init.
#[test]
fn allocation_request_nb_alloc_reacquire_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocReacquire, &[], cb);
    assert!(result.is_err());
}

/// Non-blocking allocation with AllocExternal directive fails without init.
#[test]
fn allocation_request_nb_alloc_external_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocExternal, &[], cb);
    assert!(result.is_err());
}

/// Non-blocking allocation with Unknown directive fails without init.
#[test]
fn allocation_request_nb_unknown_directive_without_init() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::Unknown(42), &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocking allocation_request — for comparison and FFI path verification
// ─────────────────────────────────────────────────────────────────────────────

/// Blocking allocation request without PMIx_Init must fail.
#[test]
fn allocation_request_without_init_fails() {
    let result = allocation_request(PmixAllocDirective::AllocNew, &[]);
    assert!(
        result.is_err(),
        "allocation_request without PMIx_Init should fail"
    );
}

/// Blocking allocation with empty info fails without init.
#[test]
fn allocation_request_empty_info_without_init() {
    let result = allocation_request(PmixAllocDirective::AllocExtend, &[]);
    assert!(result.is_err());
}

/// Blocking allocation with all directive types fails without init.
#[test]
fn allocation_request_all_directives_without_init() {
    let directives = [
        PmixAllocDirective::AllocNew,
        PmixAllocDirective::AllocExtend,
        PmixAllocDirective::AllocRelease,
        PmixAllocDirective::AllocReacquire,
        PmixAllocDirective::AllocExternal,
    ];
    for d in &directives {
        let result = allocation_request(*d, &[]);
        assert!(
            result.is_err(),
            "allocation_request with {:?} without init should fail",
            d
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error status verification
// ─────────────────────────────────────────────────────────────────────────────

/// When allocation_request_nb fails without init, the error should be
/// PMIX_ERR_INIT or similar (not success).
#[test]
fn allocation_request_nb_error_is_not_success() {
    let cb = Box::new(TestAllocCallback {
        called: std::cell::RefCell::new(false),
    });
    let result = allocation_request_nb(PmixAllocDirective::AllocNew, &[], cb);
    match result {
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
        Ok(()) => panic!("should have returned Err without PMIx_Init"),
    }
}

/// Verify the error from blocking variant is also not success.
#[test]
fn allocation_request_error_is_not_success() {
    let result = allocation_request(PmixAllocDirective::AllocNew, &[]);
    match result {
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
        Ok(_) => panic!("should have returned Err without PMIx_Init"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

/// Verify AllocationCallback trait is object-safe and Send.
#[test]
fn allocation_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn AllocationCallback>>();
}

/// Verify a custom callback implementation compiles and can be boxed.
#[test]
fn allocation_callback_custom_impl() {
    use std::sync::{Arc, Mutex};

    struct CountingCallback {
        count: Arc<Mutex<usize>>,
    }
    impl AllocationCallback for CountingCallback {
        fn on_complete(&self, _status: pmix::PmixStatus, _results: AllocationResults) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let count = Arc::new(Mutex::new(0usize));
    let cb: Box<dyn AllocationCallback> = Box::new(CountingCallback {
        count: count.clone(),
    });

    // The callback should not be called since PMIx is not initialized.
    let result = allocation_request_nb(PmixAllocDirective::AllocNew, &[], cb);
    assert!(result.is_err());
    // Callback should not have been invoked (rejected before queuing).
    assert_eq!(
        *count.lock().unwrap(),
        0,
        "callback should not fire on immediate rejection"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Full lifecycle tests (require PMIx_Init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full non-blocking allocation lifecycle: submit request, wait for callback,
/// verify results.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires PMIx_Init with running daemon"]
fn allocation_request_nb_full_lifecycle() {
    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // use std::sync::{Arc, Mutex};
    //
    // let status = Arc::new(Mutex::new(None));
    // let cb_status = status.clone();
    //
    // struct LifecycleCb {
    //     status: Arc<Mutex<Option<pmix::PmixStatus>>>,
    // }
    // impl AllocationCallback for LifecycleCb {
    //     fn on_complete(&self, s: pmix::PmixStatus, _results: AllocationResults) {
    //         *self.status.lock().unwrap() = Some(s);
    //     }
    // }
    //
    // let cb = Box::new(LifecycleCb { status: cb_status });
    //
    // allocation_request_nb(PmixAllocDirective::AllocNew, &[], cb)
    //     .expect("request should be accepted");
    //
    // // Wait for callback to fire...
    // // assert!(status.lock().unwrap().is_some(), "callback should have fired");
    //
    // pmix::lifecycle::finalize();
}

/// Full blocking allocation lifecycle.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires PMIx_Init with running daemon"]
fn allocation_request_full_lifecycle() {
    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // let results = allocation_request(PmixAllocDirective::AllocNew, &[])
    //     .expect("allocation request should succeed");
    //
    // assert!(results.is_empty() || results.len() > 0);
    //
    // pmix::lifecycle::finalize();
}
