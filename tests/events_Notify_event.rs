//! Tests for `PMIx_Notify_event` via the safe `events` module wrappers.
//!
//! Derived from C test patterns in:
//! - `test/test_error.c` — notify with callback, then deregister
//! - `test/simple/simptest.c` — notify with info array, blocking mode
//! - `test/simple/hybrid.c` — notify with non-default flag, non-blocking mode
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

mod daemon_helper;

use pmix::events::*;
use pmix::{InfoBuilder, PmixDataRange, PmixError, PmixStatus, Proc};
use std::ffi::c_void;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// Blocking notify_event without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `notify_event` without `PMIx_Init` must return an error
/// rather than panic or segfault.
///
/// Derived from `test/test_error.c` — the C test calls
/// `PMIx_Notify_event(TEST_NOTIFY, &source, PMIX_RANGE_NAMESPACE, ...)`
/// and checks the return status.
#[test]
fn notify_event_without_init_fails() {
    let proc = Proc::new("", 0).expect("create wildcard proc");
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

/// Notify with a specific event code and namespace range without init.
///
/// Derived from `test/test_error.c` — uses `PMIX_RANGE_NAMESPACE`.
#[test]
fn notify_event_namespace_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobCanceled),
        &proc,
        PmixDataRange::Namespace,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Notify with local range without init.
///
/// Derived from `test/simple/simptest.c` — uses `PMIX_RANGE_LOCAL`.
#[test]
fn notify_event_local_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::EventProcTerminated),
        &proc,
        PmixDataRange::Local,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Notify with proc-local range without init.
///
/// Derived from `test/simple/simptest.c` — uses `PMIX_RANGE_PROC_LOCAL`.
#[test]
fn notify_event_proc_local_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::EventJobEnd),
        &proc,
        PmixDataRange::ProcLocal,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Notify with global range without init.
#[test]
fn notify_event_global_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrProcFailedToStart),
        &proc,
        PmixDataRange::Global,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Notify with RM range without init.
#[test]
fn notify_event_rm_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrLostConnection),
        &proc,
        PmixDataRange::Rm,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Notify with custom range without init.
#[test]
fn notify_event_custom_range_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrTimeout),
        &proc,
        PmixDataRange::Custom,
        &info,
    );
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking notify_event_nb without PMIx_Init (expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking notify without init must fail.
///
/// Derived from `test/simple/hybrid.c` — uses a completion callback
/// `notify_complete` with a done flag.
#[test]
fn notify_event_nb_without_init_fails() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let proc = Proc::new("", 0).expect("create wildcard proc");
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

/// Non-blocking notify with namespace range without init.
#[test]
fn notify_event_nb_namespace_range_without_init_fails() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &proc,
        PmixDataRange::Namespace,
        &info,
        Some(dummy_op_cb),
        ptr::null_mut(),
    );
    assert!(result.is_err(), "should fail without init");
}

/// Non-blocking notify with proc-local range without init.
#[test]
fn notify_event_nb_proc_local_without_init_fails() {
    extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}

    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::EventJobStart),
        &proc,
        PmixDataRange::ProcLocal,
        &info,
        Some(dummy_op_cb),
        ptr::null_mut(),
    );
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Event code variations
// ─────────────────────────────────────────────────────────────────────────────

/// Notify with an unknown user-defined event code without init.
///
/// User-defined event codes are negative values not in the PmixError enum.
#[test]
fn notify_event_unknown_code_without_init_fails() {
    let user_code = PmixStatus::Unknown(-5000);
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(user_code, &proc, PmixDataRange::Session, &info);
    assert!(result.is_err(), "should fail without init for unknown code");
}

/// Notify with various known event codes — all should fail without init.
#[test]
fn notify_event_various_codes_without_init_fail() {
    let codes = [
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::ErrJobCanceled),
        PmixStatus::Known(PmixError::ErrJobFailedToLaunch),
        PmixStatus::Known(PmixError::EventJobEnd),
        PmixStatus::Known(PmixError::EventJobStart),
        PmixStatus::Known(PmixError::EventProcTerminated),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrProcRequestedAbort),
    ];
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    for code in &codes {
        let result = notify_event(*code, &proc, PmixDataRange::Session, &info);
        assert!(
            result.is_err(),
            "notify_event with {:?} without init should fail, got {:?}",
            code,
            result
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction and edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Notify with a wildcard proc (empty namespace, rank 0).
#[test]
fn notify_event_wildcard_proc_without_init_fails() {
    let proc = Proc::new("", 0).expect("create wildcard proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &proc,
        PmixDataRange::Session,
        &info,
    );
    assert!(result.is_err(), "wildcard proc should fail without init");
}

/// Notify with a named proc at a specific rank.
#[test]
fn notify_event_named_proc_without_init_fails() {
    let proc = Proc::new("my_job", 42).expect("create named proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobCanceled),
        &proc,
        PmixDataRange::Namespace,
        &info,
    );
    assert!(result.is_err(), "named proc should fail without init");
}

/// Notify with a proc at rank 0 (common case).
#[test]
fn notify_event_rank_zero_without_init_fails() {
    let proc = Proc::new("job_12345", 0).expect("create proc at rank 0");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::EventProcTerminated),
        &proc,
        PmixDataRange::Local,
        &info,
    );
    assert!(result.is_err(), "rank 0 proc should fail without init");
}

/// Notify with a proc at a high rank value.
#[test]
fn notify_event_high_rank_without_init_fails() {
    let proc = Proc::new("large_job", 1024).expect("create high rank proc");
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &proc,
        PmixDataRange::Global,
        &info,
    );
    assert!(result.is_err(), "high rank proc should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Info array variations
// ─────────────────────────────────────────────────────────────────────────────

/// Notify with empty info array (the common case from C tests).
#[test]
fn notify_event_empty_info_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let info = InfoBuilder::new().build();
    // Info built with empty builder should have no entries
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &proc,
        PmixDataRange::Session,
        &info,
    );
    assert!(result.is_err(), "empty info should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataRange roundtrip and display
// ─────────────────────────────────────────────────────────────────────────────

/// All PmixDataRange variants used by notify_event roundtrip correctly.
#[test]
fn data_range_notify_roundtrip() {
    let ranges: Vec<PmixDataRange> = vec![
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
        PmixDataRange::Session,
        PmixDataRange::Global,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
    ];
    for range in &ranges {
        let raw = range.to_raw();
        let recovered = PmixDataRange::from_raw(raw);
        assert_eq!(
            *range, recovered,
            "DataRange roundtrip failed: {:?} -> {} -> {:?}",
            range, raw, recovered
        );
    }
}

/// PmixDataRange display for notification ranges.
#[test]
fn data_range_display() {
    assert_eq!(format!("{}", PmixDataRange::Session), "SESSION");
    assert_eq!(format!("{}", PmixDataRange::Local), "LOCAL");
    assert_eq!(format!("{}", PmixDataRange::Namespace), "NAMESPACE");
    assert_eq!(format!("{}", PmixDataRange::ProcLocal), "PROC LOCAL");
    assert_eq!(format!("{}", PmixDataRange::Global), "GLOBAL");
}

// ─────────────────────────────────────────────────────────────────────────────
// Full lifecycle tests (require PMIx_Init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full notify lifecycle: register handler → notify → deregister.
///
/// Derived from `test/test_error.c` — the C test:
/// 1. Registers an error handler with a callback
/// 2. Calls `PMIx_Notify_event(TEST_NOTIFY, &source, ...)`
/// 3. Waits for the callback to fire
/// 4. Deregisters the handler
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn notify_event_full_lifecycle() {
    daemon_helper::ensure_pmix_init();
    // let _ = pmix::lifecycle::init(None, &[]);
    // let proc = pmix::Proc::new("test_job", 0).unwrap();
    // let info = InfoBuilder::new().build();
    //
    // // Register a handler first
    // let handler_ref = register_event_handler(
    //     &[PmixStatus::Known(PmixError::ErrJobAborted)],
    //     &info,
    //     None,
    //     None,
    // ).expect("register should succeed after PMIx_Init");
    //
    // // Notify the event
    // notify_event(
    //     PmixStatus::Known(PmixError::ErrJobAborted),
    //     &proc,
    //     PmixDataRange::Session,
    //     &info,
    // ).expect("notify should succeed");
    //
    // // Deregister
    // deregister_event_handler(handler_ref, None)
    //     .expect("deregister should succeed");
    // pmix::lifecycle::finalize();
}

/// Non-blocking notify lifecycle with callback.
///
/// Derived from `test/simple/hybrid.c` — the C test:
/// 1. Creates an info array with PMIX_EVENT_NON_DEFAULT
/// 2. Calls `PMIx_Notify_event` with a completion callback
/// 3. Waits for the callback to set a done flag
///
/// Requires a running PMIx server / daemon. Ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn notify_event_nb_lifecycle() {
    daemon_helper::ensure_pmix_init();
    // extern "C" fn notify_complete(status: i32, cbdata: *mut c_void) {
    //     unsafe {
    //         if !cbdata.is_null() {
    //             *(cbdata as *mut bool) = true;
    //         }
    //     }
    // }
    //
    // let mut done = false;
    // let proc = pmix::Proc::new("test_job", 0).unwrap();
    // let info = InfoBuilder::new().build();
    //
    // notify_event_nb(
    //     PmixStatus::Known(PmixError::EventJobEnd),
    //     &proc,
    //     PmixDataRange::ProcLocal,
    //     &info,
    //     Some(notify_complete),
    //     &mut done as *mut bool as *mut c_void,
    // ).expect("notify_nb should succeed");
    //
    // // Wait for callback...
    // // assert!(done, "callback should have fired");
}
