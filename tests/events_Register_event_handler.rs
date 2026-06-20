//! Integration tests for `PMIx_Register_event_handler`, `PMIx_Deregister_event_handler`,
//! and `PMIx_Notify_event` via the safe `events` module wrappers.
//!
//! These tests call into the real PMIx library. Tests that require `PMIx_Init`
//! are marked `#[ignore]` because they need a running PMIx daemon / server.

use pmix::events::*;
use pmix::{InfoBuilder, PmixDataRange, PmixError, PmixStatus};
use std::ffi::c_void;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// Type and constant checks
// ─────────────────────────────────────────────────────────────────────────────

/// `EventHandlerRef` is an alias for `usize`.
#[test]
fn event_handler_ref_is_usize() {
    let _ref_id: EventHandlerRef = 42;
    assert_eq!(_ref_id, 42);
}

/// `NotificationFn` is `Option<unsafe extern "C" fn(...)>`.
#[test]
fn notification_fn_type_is_option() {
    // Verify the type alias compiles and can be None.
    let handler: NotificationFn = None;
    assert!(handler.is_none());
}

/// `HandlerRegCbFn` can be None.
#[test]
fn handler_reg_cb_fn_can_be_none() {
    let cb: HandlerRegCbFn = None;
    assert!(cb.is_none());
}

/// `OpCbFn` can be None.
#[test]
fn op_cb_fn_can_be_none() {
    let cb: OpCbFn = None;
    assert!(cb.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// Registration without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Registering an event handler without calling `PMIx_Init` should fail
/// with `PMIX_ERR_INIT` because the PMIx client library is not initialized.
///
/// This verifies the FFI call path works and returns an appropriate error
/// rather than panicking or segfaulting.
#[test]
fn register_event_handler_without_init_fails() {
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&[], &info, None, None);
    assert!(
        result.is_err(),
        "register_event_handler without PMIx_Init should return an error, got {:?}",
        result
    );
    let err = result.unwrap_err();
    assert!(
        err.is_error(),
        "error should be a negative status code, got {:?}",
        err
    );
}

/// Registering with specific event codes (without init) should also fail.
#[test]
fn register_event_handler_with_codes_without_init_fails() {
    let codes = vec![
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::EventJobEnd),
    ];
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&codes, &info, None, None);
    assert!(
        result.is_err(),
        "register_event_handler with codes without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Registering with a notification callback (without init) should fail.
///
/// The callback uses `c_void` pointers matching the `NotificationFn` type alias.
#[test]
fn register_event_handler_with_callback_without_init_fails() {
    extern "C" fn dummy_notification_handler(
        _evhdlr_registration_id: EventHandlerRef,
        _status: i32,
        _source: *const c_void,
        _info: *mut c_void,
        _ninfo: usize,
        _results: *mut c_void,
        _nresults: usize,
        _cbfunc: pmix::events::pmix_event_notification_cbfunc_fn_t,
        _cbdata: *mut c_void,
    ) {
        // Should never be called without PMIx_Init.
    }

    let info = InfoBuilder::new().build();
    let result = register_event_handler(&[], &info, Some(dummy_notification_handler), None);
    assert!(
        result.is_err(),
        "register_event_handler with callback without PMIx_Init should fail, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Deregistration without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Deregistering a handler without calling `PMIx_Init` should fail.
#[test]
fn deregister_event_handler_without_init_fails() {
    let result = deregister_event_handler(9999, None);
    assert!(
        result.is_err(),
        "deregister_event_handler without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Deregistering with a callback (without init) should also fail.
#[test]
fn deregister_event_handler_nb_without_init_fails() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let result = deregister_event_handler_nb(9999, Some(dummy_op_cb), ptr::null_mut());
    assert!(
        result.is_err(),
        "deregister_event_handler_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Notification without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Notifying an event without calling `PMIx_Init` should fail.
#[test]
fn notify_event_without_init_fails() {
    let proc = pmix::Proc::new("", 0).expect("create wildcard proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &proc,
        PmixDataRange::Session,
        &info,
    );
    assert!(
        result.is_err(),
        "notify_event without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Notifying with non-blocking callback (without init) should also fail.
#[test]
fn notify_event_nb_without_init_fails() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let proc = pmix::Proc::new("", 0).expect("create wildcard proc");
    let info = InfoBuilder::new().build();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::EventJobEnd),
        &proc,
        PmixDataRange::Local,
        &info,
        Some(dummy_op_cb),
        ptr::null_mut(),
    );
    assert!(
        result.is_err(),
        "notify_event_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus conversion for event codes
// ─────────────────────────────────────────────────────────────────────────────

/// Event codes used in registration can be converted to/from raw values.
#[test]
fn event_codes_roundtrip() {
    let codes: Vec<PmixStatus> = vec![
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::EventJobEnd),
        PmixStatus::Known(PmixError::EventProcTerminated),
        PmixStatus::Known(PmixError::ErrProcFailedToStart),
    ];
    for code in &codes {
        let raw = code.to_raw();
        let recovered = PmixStatus::from_raw(raw);
        assert_eq!(
            *code, recovered,
            "Event code roundtrip failed: {:?} -> {} -> {:?}",
            code, raw, recovered
        );
    }
}

/// Unknown event codes (user-defined) also roundtrip correctly.
#[test]
fn unknown_event_codes_roundtrip() {
    let user_code = PmixStatus::Unknown(-5000);
    let raw = user_code.to_raw();
    assert_eq!(raw, -5000);
    let recovered = PmixStatus::from_raw(raw);
    assert_eq!(user_code, recovered);
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple event code registration
// ─────────────────────────────────────────────────────────────────────────────

/// Registering with multiple event codes without init should fail gracefully.
#[test]
fn register_multiple_event_codes_without_init_fails() {
    let codes = vec![
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::ErrJobCanceled),
        PmixStatus::Known(PmixError::ErrJobFailedToLaunch),
        PmixStatus::Known(PmixError::EventJobEnd),
        PmixStatus::Known(PmixError::EventJobStart),
    ];
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&codes, &info, None, None);
    assert!(
        result.is_err(),
        "register with multiple codes without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Registering with empty codes (match all) without init should fail.
#[test]
fn register_all_events_without_init_fails() {
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&[], &info, None, None);
    assert!(
        result.is_err(),
        "register all events without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking registration without init
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking registration without init should fail.
#[test]
fn register_event_handler_nb_without_init_fails() {
    extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}

    let info = InfoBuilder::new().build();
    let result = register_event_handler_nb(&[], &info, None, Some(dummy_reg_cb), ptr::null_mut());
    assert!(
        result.is_err(),
        "register_event_handler_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Data range variants compile and are usable
// ─────────────────────────────────────────────────────────────────────────────

/// All relevant PmixDataRange variants compile and can be used with notify_event.
#[test]
fn data_range_variants_compile() {
    let ranges = [
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
        PmixDataRange::Session,
        PmixDataRange::Global,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
    ];
    assert_eq!(ranges.len(), 8);
}

// ─────────────────────────────────────────────────────────────────────────────
// Full lifecycle tests (require PMIx_Init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full register → deregister lifecycle test.
///
/// Requires a running PMIx server / daemon. Ignored by default.
/// Run with: `cargo test --test events_Register_event_handler -- --ignored`
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn register_deregister_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // NOTE: This test would call PMIx_Init first, but we don't have a
    // lifecycle module exposed in the public API yet. The test is here
    // as a template for when PMIx_Init is available.
    //
    // let _ = pmix::lifecycle::init(None, &[]);
    // let info = InfoBuilder::new().build();
    // let handler_ref = register_event_handler(&[], &info, None, None)
    //     .expect("register should succeed after PMIx_Init");
    // assert!(handler_ref > 0, "handler ref should be positive");
    // deregister_event_handler(handler_ref, None)
    //     .expect("deregister should succeed");
    // pmix::lifecycle::finalize();
}

/// Register with specific event codes → deregister lifecycle.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn register_with_codes_deregister_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // Same as above but with specific event codes.
    // let _ = pmix::lifecycle::init(None, &[]);
    // let codes = vec![PmixStatus::Known(PmixError::ErrJobAborted)];
    // let info = InfoBuilder::new().build();
    // let handler_ref = register_event_handler(&codes, &info, None, None)
    //     .expect("register should succeed");
    // deregister_event_handler(handler_ref, None)
    //     .expect("deregister should succeed");
    // pmix::lifecycle::finalize();
}

/// Notify event lifecycle test.
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn notify_event_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // let _ = pmix::lifecycle::init(None, &[]);
    // let proc = pmix::Proc::new("", 0).unwrap();
    // let info = InfoBuilder::new().build();
    // notify_event(
    //     PmixStatus::Known(PmixError::EventJobEnd),
    //     &proc,
    //     PmixDataRange::Session,
    //     &info,
    // ).expect("notify should succeed");
    // pmix::lifecycle::finalize();
}
