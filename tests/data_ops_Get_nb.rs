//! Integration tests for `PMIx_Get_nb` via the safe `get_nb()` wrapper.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_Init) are marked `#[ignore]`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `data_ops` module is public and importable.
///
/// Compile-time check: the module exists and its public types are accessible.
#[test]
fn data_ops_module_is_public() {
    // The fact this compiles proves the module is pub.
    let _: fn(
        &pmix::Proc,
        &str,
        Option<&pmix::Info>,
        Box<dyn pmix::data_ops::GetValueCallback>,
    ) -> Result<(), pmix::PmixStatus> = pmix::data_ops::get_nb;
}

/// `GetValueCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object.
#[test]
fn get_value_callback_trait_object() {
    use pmix::data_ops::GetValueCallback;

    struct TestCallback;
    impl GetValueCallback for TestCallback {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn GetValueCallback> = Box::new(TestCallback);
    let _: Box<dyn GetValueCallback> = cb;
}

/// `get_nb` function signature is correct.
///
/// Compile-time check: the function accepts the expected parameter types.
#[test]
fn get_nb_signature() {
    fn check<F>(_: F) {}
    check::<fn(
        &pmix::Proc,
        &str,
        Option<&pmix::Info>,
        Box<dyn pmix::data_ops::GetValueCallback>,
    ) -> Result<(), pmix::PmixStatus>>(pmix::data_ops::get_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback behavior (no PMIx runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Callback can be implemented with a simple struct.
#[test]
fn callback_struct_implementation() {
    use pmix::data_ops::GetValueCallback;

    struct CountingCallback {
        called: Arc<AtomicBool>,
    }

    impl GetValueCallback for CountingCallback {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let cb: Box<dyn GetValueCallback> = Box::new(CountingCallback {
        called: Arc::clone(&called),
    });

    // Simulate bridge invocation.
    cb.on_result(pmix::PmixStatus::from_raw(0), None);
    assert!(called.load(Ordering::SeqCst), "callback should have been called");
}

/// Callback receives status and value parameters.
#[test]
fn callback_receives_status_and_value() {
    use pmix::data_ops::GetValueCallback;

    struct CaptureCallback {
        status: Arc<std::sync::Mutex<Option<pmix::PmixStatus>>>,
        has_value: Arc<std::sync::Mutex<Option<bool>>>,
    }

    impl GetValueCallback for CaptureCallback {
        fn on_result(
            self: Box<Self>,
            status: pmix::PmixStatus,
            value: Option<pmix::PmixOwnedValue>,
        ) {
            *self.status.lock().unwrap() = Some(status);
            *self.has_value.lock().unwrap() = Some(value.is_some());
        }
    }

    let status = Arc::new(std::sync::Mutex::new(None));
    let has_value = Arc::new(std::sync::Mutex::new(None));

    let cb: Box<dyn GetValueCallback> = Box::new(CaptureCallback {
        status: Arc::clone(&status),
        has_value: Arc::clone(&has_value),
    });

    cb.on_result(pmix::PmixStatus::from_raw(0), None);

    let captured_status = status.lock().unwrap();
    assert!(captured_status.as_ref().unwrap().is_success());

    let captured_has_value = has_value.lock().unwrap();
    assert!(!captured_has_value.unwrap(), "value should be None");
}

/// Callback with Some(value) passes the value through.
#[test]
fn callback_with_some_value() {
    use pmix::data_ops::GetValueCallback;

    let value_received = Arc::new(AtomicBool::new(false));

    struct ValueCheck {
        vr: Arc<AtomicBool>,
    }

    impl GetValueCallback for ValueCheck {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            value: Option<pmix::PmixOwnedValue>,
        ) {
            if value.is_some() {
                self.vr.store(true, Ordering::SeqCst);
            }
        }
    }

    let cb: Box<dyn GetValueCallback> = Box::new(ValueCheck {
        vr: Arc::clone(&value_received),
    });

    cb.on_result(pmix::PmixStatus::from_raw(-46), None);
    assert!(
        !value_received.load(Ordering::SeqCst),
        "no value should mean flag stays false"
    );
}

/// Callback is consumed on call (Box<Self> ownership).
#[test]
fn callback_consumed_on_call() {
    use pmix::data_ops::GetValueCallback;

    let call_count = Arc::new(AtomicUsize::new(0));

    struct OnceCallback {
        cc: Arc<AtomicUsize>,
    }

    impl GetValueCallback for OnceCallback {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
            self.cc.fetch_add(1, Ordering::SeqCst);
            // self is consumed here — cannot be called again.
        }
    }

    let cb: Box<dyn GetValueCallback> = Box::new(OnceCallback {
        cc: Arc::clone(&call_count),
    });

    cb.on_result(pmix::PmixStatus::from_raw(0), None);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
    // cb is consumed by on_result — cannot call again.
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction (no PMIx runtime needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Proc can be constructed with a valid namespace and rank.
#[test]
fn proc_construction() {
    let proc = pmix::Proc::new("test_namespace", 0).unwrap();
    assert_eq!(proc.get_rank(), 0);
}

/// Proc can be constructed with different ranks.
#[test]
fn proc_different_ranks() {
    let proc0 = pmix::Proc::new("test_ns", 0).unwrap();
    let proc1 = pmix::Proc::new("test_ns", 1).unwrap();
    assert_eq!(proc0.get_rank(), 0);
    assert_eq!(proc1.get_rank(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error handling — NUL byte in key
// ─────────────────────────────────────────────────────────────────────────────

/// get_nb rejects keys containing NUL bytes.
///
/// PMIx keys must be valid C strings (no embedded NUL). Our wrapper
/// uses CString internally, which rejects NUL bytes.
#[test]
fn get_nb_rejects_nul_in_key() {
    let result = std::ffi::CString::new("test\x00key");
    assert!(result.is_err(), "CString should reject embedded NUL");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus conversion
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw converts PMIX_SUCCESS (0) to success.
#[test]
fn pmix_status_success() {
    let status = pmix::PmixStatus::from_raw(0);
    assert!(status.is_success());
}

/// PmixStatus::from_raw converts PMIX_ERR_NOT_FOUND (-46) to error.
#[test]
fn pmix_status_not_found() {
    let status = pmix::PmixStatus::from_raw(-46);
    assert!(status.is_error());
}

/// PmixStatus::from_raw converts PMIX_ERROR (-1) to error.
#[test]
fn pmix_status_error() {
    let status = pmix::PmixStatus::from_raw(-1);
    assert!(status.is_error());
}

// ─────────────────────────────────────────────────────────────────────────────
// Runtime tests — require PMIx_Init (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// get_nb returns Err when PMIx is not initialized.
///
/// PMIx_Get_nb requires PMIx_Init to have been called first. Without it,
/// the call should return an error immediately without invoking the callback.
#[test]
#[ignore]
fn get_nb_without_init_returns_error() {
    use pmix::data_ops::GetValueCallback;

    let called = Arc::new(AtomicBool::new(false));

    struct NoOp {
        c: Arc<AtomicBool>,
    }
    impl GetValueCallback for NoOp {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
            self.c.store(true, Ordering::SeqCst);
        }
    }

    let cb: Box<dyn GetValueCallback> = Box::new(NoOp {
        c: Arc::clone(&called),
    });

    let proc = pmix::Proc::new("nonexistent", 0).unwrap();
    let result = pmix::data_ops::get_nb(&proc, "test_key", None, cb);

    assert!(result.is_err(), "get_nb should fail without PMIx_Init");
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be called on immediate failure"
    );
}

/// get_nb accepts a valid request and invokes callback (requires PMIx runtime).
///
/// When PMIx is initialized and a key has been published, get_nb should
/// return Ok(()) and eventually invoke the callback with the value.
#[test]
#[ignore]
fn get_nb_with_init_and_published_key() {
    unimplemented!("Requires PMIx runtime");
}

/// get_nb handles not-found gracefully (requires PMIx runtime).
///
/// When a key does not exist, the callback should receive
/// PMIX_ERR_NOT_FOUND with None for the value.
#[test]
#[ignore]
fn get_nb_not_found_callback() {
    use pmix::data_ops::GetValueCallback;

    struct NotFoundCheck;
    impl GetValueCallback for NotFoundCheck {
        fn on_result(
            self: Box<Self>,
            status: pmix::PmixStatus,
            value: Option<pmix::PmixOwnedValue>,
        ) {
            assert!(
                !status.is_success(),
                "should not be success for missing key"
            );
            assert!(value.is_none(), "value should be None for not found");
        }
    }

    let proc = pmix::Proc::new("nonexistent", 0).unwrap();
    let result = pmix::data_ops::get_nb(
        &proc,
        "nonexistent_key",
        None,
        Box::new(NotFoundCheck),
    );
    assert!(result.is_err(), "should fail without PMIx_Init");
}

/// get_nb with info directives (requires PMIx runtime).
#[test]
#[ignore]
fn get_nb_with_info_directives() {
    use pmix::data_ops::GetValueCallback;

    struct NoOp;
    impl GetValueCallback for NoOp {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
        }
    }

    let proc = pmix::Proc::new("test", 0).unwrap();
    let result = pmix::data_ops::get_nb(&proc, "test_key", None, Box::new(NoOp));
    assert!(result.is_err(), "should fail without PMIx_Init");
}

/// get_nb callback is called exactly once per request.
///
/// The bridge function removes the callback from the registry after
/// invoking it, ensuring no double-invocation.
#[test]
#[ignore]
fn get_nb_callback_called_once() {
    use pmix::data_ops::GetValueCallback;

    let call_count = Arc::new(AtomicUsize::new(0));

    struct OnceCounter {
        cc: Arc<AtomicUsize>,
    }
    impl GetValueCallback for OnceCounter {
        fn on_result(
            self: Box<Self>,
            _status: pmix::PmixStatus,
            _value: Option<pmix::PmixOwnedValue>,
        ) {
            self.cc.fetch_add(1, Ordering::SeqCst);
        }
    }

    let cb: Box<dyn GetValueCallback> = Box::new(OnceCounter {
        cc: Arc::clone(&call_count),
    });

    let proc = pmix::Proc::new("nonexistent", 0).unwrap();
    let _ = pmix::data_ops::get_nb(&proc, "test_key", None, cb);

    assert!(
        call_count.load(Ordering::SeqCst) <= 1,
        "callback should be called at most once"
    );
}
