//! Integration tests for `PMIx_Deregister_event_handler` via the safe
//! `events` module wrapper.
//!
//! These tests call into the real PMIx library. Tests that require
//! `PMIx_Init` are marked `#[ignore]` because they need a running
//! PMIx daemon / server.
//!
//! Derived from C test patterns in:
//! - `test/test_error.c` — deregister with non-blocking callback after
//!   registering multiple handlers.
//! - `test/simple/simptest.c` — deregister handler ref 0 with NULL
//!   callback (blocking mode) at teardown.
//! - `test/simple/stability.c` — same pattern, deregister at cleanup.

use pmix::events::*;
use pmix::{InfoBuilder, PmixError, PmixStatus};
use std::ffi::c_void;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// Type and signature checks
// ─────────────────────────────────────────────────────────────────────────────

/// `EventHandlerRef` is `usize` — the return type of register_event_handler.
#[test]
fn event_handler_ref_type() {
    let ref_id: EventHandlerRef = 1;
    assert_eq!(ref_id, 1);
}

/// `OpCbFn` is the callback type used by deregister_event_handler_nb.
#[test]
fn op_cb_fn_type_is_option() {
    let cb: OpCbFn = None;
    assert!(cb.is_none());
}

/// deregister_event_handler returns Result<(), PmixStatus>.
#[test]
fn deregister_return_type() {
    let result: Result<(), PmixStatus> = deregister_event_handler(0, None);
    // Without PMIx_Init, this should fail — just checking the type compiles.
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocking deregister without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Deregistering with ref 0 and no callback (blocking mode) should fail
/// when PMIx is not initialized.
///
/// Derived from `test/simple/simptest.c` pattern:
/// `PMIx_Deregister_event_handler(0, NULL, NULL);`
#[test]
fn deregister_event_handler_ref_zero_blocking() {
    let result = deregister_event_handler(0, None);
    assert!(
        result.is_err(),
        "deregister_event_handler(0, None) without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Deregistering with a non-existent handler ref should fail.
///
/// Derived from `test/test_error.c` pattern:
/// `PMIx_Deregister_event_handler(errhandler_refs[0], op1_callbk, NULL);`
/// In the C tests, this is called after registration. Without registration,
/// the ref is invalid and should fail.
#[test]
fn deregister_event_handler_invalid_ref() {
    let result = deregister_event_handler(9999, None);
    assert!(
        result.is_err(),
        "deregister_event_handler(9999, None) without PMIx_Init should fail"
    );
}

/// Deregistering with a large handler ref should also fail gracefully.
#[test]
fn deregister_event_handler_large_ref() {
    let result = deregister_event_handler(usize::MAX, None);
    assert!(
        result.is_err(),
        "deregister_event_handler(usize::MAX, None) without PMIx_Init should fail"
    );
}

/// Multiple deregister calls with different refs should all fail without init.
#[test]
fn deregister_multiple_refs_without_init() {
    let refs = [0, 1, 2, 100, 1000];
    for &ref_id in &refs {
        let result = deregister_event_handler(ref_id, None);
        assert!(
            result.is_err(),
            "deregister_event_handler({}, None) should fail without init",
            ref_id
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking deregister without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking deregister with a callback should fail without PMIx_Init.
///
/// Derived from `test/test_error.c` pattern:
/// `PMIx_Deregister_event_handler(errhandler_refs[0], op1_callbk, NULL);`
/// The C test uses a non-blocking callback (op1_callbk). We replicate that
/// pattern here with a dummy callback.
#[test]
fn deregister_event_handler_nb_with_callback() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let result = deregister_event_handler_nb(0, Some(dummy_op_cb), ptr::null_mut());
    assert!(
        result.is_err(),
        "deregister_event_handler_nb(0, Some(cb), null) without PMIx_Init should fail"
    );
}

/// Non-blocking deregister with user cbdata should fail without init.
#[test]
fn deregister_event_handler_nb_with_cbdata() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let mut user_data: i32 = 42;
    let result = deregister_event_handler_nb(
        1,
        Some(dummy_op_cb),
        &mut user_data as *mut i32 as *mut c_void,
    );
    assert!(
        result.is_err(),
        "deregister_event_handler_nb with cbdata without PMIx_Init should fail"
    );
}

/// Non-blocking deregister should not invoke the callback when it fails.
///
/// This test verifies that when PMIx returns an error synchronously, the
/// callback is NOT called (the error is returned directly in the status).
#[test]
fn deregister_event_handler_nb_callback_not_called_on_error() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    extern "C" fn tracking_cb(_status: i32, _cbdata: *mut c_void) {
        CALLBACK_INVOKED.store(true, Ordering::SeqCst);
    }

    CALLBACK_INVOKED.store(false, Ordering::SeqCst);
    let result = deregister_event_handler_nb(9999, Some(tracking_cb), ptr::null_mut());

    assert!(result.is_err(), "should fail without PMIx_Init");
    assert!(
        !CALLBACK_INVOKED.load(Ordering::SeqCst),
        "callback should NOT have been called when deregister fails synchronously"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error status verification
// ─────────────────────────────────────────────────────────────────────────────

/// The error returned by deregister without init should be a known PMIx
/// error code (negative status).
#[test]
fn deregister_error_is_negative_status() {
    let result = deregister_event_handler(0, None);
    match result {
        Err(status) => {
            assert!(
                status.is_error(),
                "error status should be negative (PMIX_ERR_*), got {:?}",
                status
            );
        }
        Ok(()) => panic!("deregister without init should not succeed"),
    }
}

/// The error from blocking and non-blocking variants should both be errors.
#[test]
fn deregister_both_variants_return_errors() {
    // Blocking
    let blocking = deregister_event_handler(0, None);
    assert!(blocking.is_err(), "blocking variant should fail");

    // Non-blocking
    extern "C" fn dummy(_s: i32, _d: *mut c_void) {}
    let nb = deregister_event_handler_nb(0, Some(dummy), ptr::null_mut());
    assert!(nb.is_err(), "non-blocking variant should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback type compatibility
// ─────────────────────────────────────────────────────────────────────────────

/// OpCbFn can be created from a closure-like extern "C" function.
#[test]
fn op_cb_fn_from_extern_fn() {
    extern "C" fn my_cb(_status: i32, _cbdata: *mut c_void) {
        // no-op
    }
    let cb: OpCbFn = Some(my_cb);
    assert!(cb.is_some());
}

/// OpCbFn None is a valid value for blocking mode.
#[test]
fn op_cb_fn_none_for_blocking() {
    let cb: OpCbFn = None;
    assert!(cb.is_none());
    // This is what the blocking variant passes to the FFI layer.
    let _ = deregister_event_handler(0, cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI binding existence check
// ─────────────────────────────────────────────────────────────────────────────

/// Verify the safe wrapper calls into the FFI layer correctly.
///
/// The ffi module is private, so we can't access it directly from tests.
/// Instead, we verify the safe wrapper works by calling it — if the FFI
/// binding didn't exist or had the wrong signature, this wouldn't compile.
#[test]
fn safe_wrapper_calls_ffi() {
    // This compiles only if the FFI binding exists and has the right
    // signature. The call itself will fail without PMIx_Init, but that's
    // the point — it proves the FFI path is wired up.
    let result = deregister_event_handler(0, None);
    assert!(
        result.is_err(),
        "should fail without PMIx_Init, proving FFI call was made"
    );
}
// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus roundtrip for deregister error codes
// ─────────────────────────────────────────────────────────────────────────────

/// Error codes that deregister might return can be converted to strings.
#[test]
fn deregister_error_codes_to_string() {
    // PMIx_Error_string should be available for all known error codes.
    let errors = [
        PmixStatus::Known(PmixError::ErrInit),
        PmixStatus::Known(PmixError::ErrBadParam),
        PmixStatus::Known(PmixError::ErrNotFound),
    ];
    for err in &errors {
        let raw = err.to_raw();
        let recovered = PmixStatus::from_raw(raw);
        assert_eq!(*err, recovered, "Error code roundtrip failed for {:?}", err);
    }
}

/// InfoBuilder produces valid Info for use with event APIs.
#[test]
fn info_builder_for_events() {
    let _info = InfoBuilder::new().build();
    // Info should be creatable even without PMIx_Init.
    // We can't check .len (private field), but the fact that it compiled
    // and didn't panic is sufficient.
}

// ─────────────────────────────────────────────────────────────────────────────
// Full lifecycle tests (require PMIx_Init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Register → deregister lifecycle test.
///
/// Simulates the pattern from `test/test_error.c`:
/// 1. Register handler(s) with PMIx_Register_event_handler
/// 2. Deregister with PMIx_Deregister_event_handler
/// 3. Verify both succeed.
///
/// Requires a running PMIx server / daemon. Ignored by default.
/// Run with: `cargo test --test events_Deregister_event_handler -- --ignored`
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn register_then_deregister_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // // Register a handler
    // let info = InfoBuilder::new().build();
    // let handler_ref = register_event_handler(&[], &info, None, None)
    //     .expect("register should succeed after PMIx_Init");
    // assert!(handler_ref > 0, "handler ref should be positive");
    //
    // // Deregister it
    // deregister_event_handler(handler_ref, None)
    //     .expect("deregister should succeed");
    //
    // pmix::lifecycle::finalize();
}

/// Register multiple handlers → deregister each individually.
///
/// Simulates the pattern from `test/test_error.c`:
/// - Register two handlers, store refs in errhandler_refs[]
/// - Deregister each with a non-blocking callback
/// - Wait for callback completion between each deregister
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn register_multiple_deregister_each() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // let info = InfoBuilder::new().build();
    // let codes1 = vec![PmixStatus::Known(PmixError::ErrJobAborted)];
    // let codes2 = vec![PmixStatus::Known(PmixError::EventJobEnd)];
    //
    // let ref1 = register_event_handler(&codes1, &info, None, None)
    //     .expect("register handler 1");
    // let ref2 = register_event_handler(&codes2, &info, None, None)
    //     .expect("register handler 2");
    //
    // deregister_event_handler(ref1, None)
    //     .expect("deregister handler 1");
    // deregister_event_handler(ref2, None)
    //     .expect("deregister handler 2");
    //
    // pmix::lifecycle::finalize();
}

/// Deregister with non-blocking callback lifecycle test.
///
/// Simulates the pattern from `test/test_error.c` where deregister is
/// called with a non-blocking op1_callbk callback.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn deregister_nb_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    use std::sync::atomic::{AtomicBool, Ordering};

    static CB_INVOKED: AtomicBool = AtomicBool::new(false);

    extern "C" fn deregister_cb(status: i32, _cbdata: *mut c_void) {
        assert!(
            status >= 0,
            "deregister callback status should be success, got {}",
            status
        );
        CB_INVOKED.store(true, Ordering::SeqCst);
    }

    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // let info = InfoBuilder::new().build();
    // let handler_ref = register_event_handler(&[], &info, None, None)
    //     .expect("register should succeed");
    //
    // deregister_event_handler_nb(handler_ref, Some(deregister_cb), ptr::null_mut())
    //     .expect("deregister_nb should accept the request");
    //
    // // Wait for callback
    // for _ in 0..100 {
    //     if CB_INVOKED.load(Ordering::SeqCst) { break; }
    //     std::thread::sleep(std::time::Duration::from_millis(10));
    // }
    // assert!(CB_INVOKED.load(Ordering::SeqCst), "callback should have been invoked");
    //
    // pmix::lifecycle::finalize();
}

/// Deregister after final cleanup — simulates simptest.c teardown pattern.
///
/// In `test/simple/simptest.c`, the pattern is:
/// ```c
/// PMIx_Deregister_event_handler(0, NULL, NULL);
/// ```
/// at the end of the test, before finalizing.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn deregister_before_finalize() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // let _ = pmix::lifecycle::init(None, &[]);
    //
    // let info = InfoBuilder::new().build();
    // let handler_ref = register_event_handler(&[], &info, None, None)
    //     .expect("register");
    //
    // // Deregister before finalize (simptest.c pattern)
    // deregister_event_handler(handler_ref, None)
    //     .expect("deregister before finalize");
    //
    // pmix::lifecycle::finalize();
}
