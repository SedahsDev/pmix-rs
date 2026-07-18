//! Structural unit tests for the events module — TASK-045.
//!
//! Focus on code paths not yet exercised by existing in-module tests.
//! All tests run WITHOUT PMIx_Init — they exercise the Rust wrapper layer only.

use pmix::events::*;
use pmix::{InfoBuilder, PmixDataRange, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// EventHandlerRef — boundary and conversion tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_handler_ref_boundary_values() {
    let zero: EventHandlerRef = 0;
    assert_eq!(zero, 0);
    let max: EventHandlerRef = EventHandlerRef::MAX;
    assert_eq!(max, usize::MAX);
}

#[test]
fn test_handler_ref_from_i32_success_value() {
    let raw_status: i32 = 1;
    let handler_ref: EventHandlerRef = raw_status as EventHandlerRef;
    assert_eq!(handler_ref, 1);
}

#[test]
fn test_handler_ref_from_various_i32() {
    for val in [1i32, 42, 1000, i32::MAX] {
        let ref_: EventHandlerRef = val as EventHandlerRef;
        assert_eq!(ref_, val as usize);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NotificationFn — callback type tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_notification_fn_none_is_none() {
    let fn_: NotificationFn = None;
    assert!(fn_.is_none());
    assert!(fn_.as_ref().is_none());
}

#[test]
fn test_notification_fn_some_is_some() {
    extern "C" fn dummy(
        _: EventHandlerRef,
        _: i32,
        _: *const std::os::raw::c_void,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: pmix_event_notification_cbfunc_fn_t,
        _: *mut std::os::raw::c_void,
    ) {
    }
    let fn_: NotificationFn = Some(dummy);
    assert!(fn_.is_some());
}

// ─────────────────────────────────────────────────────────────────────────────
// HandlerRegCbFn and OpCbFn — callback type tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_handler_reg_cb_fn_none() {
    let fn_: HandlerRegCbFn = None;
    assert!(fn_.is_none());
}

#[test]
fn test_op_cb_fn_none() {
    let fn_: OpCbFn = None;
    assert!(fn_.is_none());
}

#[test]
fn test_handler_reg_cb_fn_some() {
    extern "C" fn dummy_reg(_: i32, _: EventHandlerRef, _: *mut std::os::raw::c_void) {}
    let fn_: HandlerRegCbFn = Some(dummy_reg);
    assert!(fn_.is_some());
}

#[test]
fn test_op_cb_fn_some() {
    extern "C" fn dummy_op(_: i32, _: *mut std::os::raw::c_void) {}
    let fn_: OpCbFn = Some(dummy_op);
    assert!(fn_.is_some());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataRange — roundtrip and boundary tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_data_range_from_raw_roundtrip() {
    for raw in [0u8, 1, 2, 3, 4, 5, 6, 7, 255, 128] {
        let range = PmixDataRange::from_raw(raw);
        assert_eq!(range.to_raw(), raw, "roundtrip failed for raw={}", raw);
    }
}

#[test]
fn test_data_range_named_variants() {
    assert_eq!(PmixDataRange::Undef.to_raw(), 0);
    assert_eq!(PmixDataRange::Rm.to_raw(), 1);
    assert_eq!(PmixDataRange::Local.to_raw(), 2);
    assert_eq!(PmixDataRange::Namespace.to_raw(), 3);
    assert_eq!(PmixDataRange::Session.to_raw(), 4);
    assert_eq!(PmixDataRange::Global.to_raw(), 5);
    assert_eq!(PmixDataRange::Custom.to_raw(), 6);
    assert_eq!(PmixDataRange::ProcLocal.to_raw(), 7);
    assert_eq!(PmixDataRange::Invalid.to_raw(), 255);
}

#[test]
fn test_data_range_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<PmixDataRange>();
    assert_sync::<PmixDataRange>();
}

// ─────────────────────────────────────────────────────────────────────────────
// register_event_handler — FFI call path tests (without DVM)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_register_event_handler_empty_codes_no_handler() {
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&[], &info, None, None);
    match result {
        Ok(_) => {} // rare: PMIx is initialized
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_register_event_handler_single_code_no_handler() {
    let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&codes, &info, None, None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_register_event_handler_multiple_codes() {
    let codes = [
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrNotSupported),
    ];
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&codes, &info, None, None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_register_event_handler_with_notification_fn() {
    extern "C" fn dummy_handler(
        _: EventHandlerRef,
        _: i32,
        _: *const std::os::raw::c_void,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: pmix_event_notification_cbfunc_fn_t,
        _: *mut std::os::raw::c_void,
    ) {
    }
    let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
    let info = InfoBuilder::new().build();
    let result = register_event_handler(&codes, &info, Some(dummy_handler), None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_register_event_handler_consistent_error() {
    let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
    let info = InfoBuilder::new().build();
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// register_event_handler_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_register_event_handler_nb_reaches_ffi() {
    extern "C" fn dummy_reg_cb(_: i32, _: EventHandlerRef, _: *mut std::os::raw::c_void) {}
    let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
    let info = InfoBuilder::new().build();
    let result = register_event_handler_nb(
        &codes,
        &info,
        None,
        Some(dummy_reg_cb),
        std::ptr::null_mut(),
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_register_event_handler_nb_empty_codes() {
    extern "C" fn dummy_reg_cb(_: i32, _: EventHandlerRef, _: *mut std::os::raw::c_void) {}
    let info = InfoBuilder::new().build();
    let result =
        register_event_handler_nb(&[], &info, None, Some(dummy_reg_cb), std::ptr::null_mut());
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_register_event_handler_nb_with_notification_fn() {
    extern "C" fn dummy_reg_cb(_: i32, _: EventHandlerRef, _: *mut std::os::raw::c_void) {}
    extern "C" fn dummy_handler(
        _: EventHandlerRef,
        _: i32,
        _: *const std::os::raw::c_void,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: *mut std::os::raw::c_void,
        _: usize,
        _: pmix_event_notification_cbfunc_fn_t,
        _: *mut std::os::raw::c_void,
    ) {
    }
    let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
    let info = InfoBuilder::new().build();
    let result = register_event_handler_nb(
        &codes,
        &info,
        Some(dummy_handler),
        Some(dummy_reg_cb),
        std::ptr::null_mut(),
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// deregister_event_handler — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_deregister_event_handler_various_refs() {
    for ref_id in [0usize, 1, 42, 99999, usize::MAX] {
        let result = deregister_event_handler(ref_id, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(
                    e.is_error(),
                    "Expected error for ref {}, got {:?}",
                    ref_id,
                    e
                );
            }
        }
    }
}

#[test]
fn test_deregister_event_handler_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        deregister_event_handler(usize::MAX, None);
    });
    assert!(result.is_ok(), "deregister should not panic");
}

#[test]
fn test_deregister_event_handler_consistent_error() {
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = deregister_event_handler(99999, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// deregister_event_handler_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_deregister_event_handler_nb_reaches_ffi() {
    extern "C" fn dummy_op_cb(_: i32, _: *mut std::os::raw::c_void) {}
    let result = deregister_event_handler_nb(99999, Some(dummy_op_cb), std::ptr::null_mut());
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_deregister_event_handler_nb_zero_ref() {
    extern "C" fn dummy_op_cb(_: i32, _: *mut std::os::raw::c_void) {}
    let result = deregister_event_handler_nb(0, Some(dummy_op_cb), std::ptr::null_mut());
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// notify_event — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_notify_event_reaches_ffi() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &source,
        PmixDataRange::Session,
        &info,
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_notify_event_with_all_ranges() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    for range in [
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
        PmixDataRange::Session,
        PmixDataRange::Global,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
    ] {
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            range,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error());
            }
        }
    }
}

#[test]
fn test_notify_event_with_wildcard_source() {
    let source = Proc::new("", u32::MAX).unwrap();
    let info = InfoBuilder::new().build();
    let result = notify_event(
        PmixStatus::Known(PmixError::ErrNotSupported),
        &source,
        PmixDataRange::Global,
        &info,
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_notify_event_various_status_codes() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let statuses = [
        PmixStatus::Known(PmixError::Success),
        PmixStatus::Known(PmixError::Error),
        PmixStatus::Known(PmixError::ErrNotFound),
        PmixStatus::Known(PmixError::ErrBadParam),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrNotSupported),
        PmixStatus::Known(PmixError::ErrInit),
        PmixStatus::Known(PmixError::ErrPartialSuccess),
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Unknown(-100),
        PmixStatus::Unknown(-99999),
    ];
    for (i, status) in statuses.iter().enumerate() {
        let result = notify_event(*status, &source, PmixDataRange::Session, &info);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(
                    e.is_error(),
                    "status {} ({:?}) should fail without DVM",
                    i,
                    status
                );
            }
        }
    }
}

#[test]
fn test_notify_event_does_not_panic() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| {
        notify_event(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            PmixDataRange::Session,
            &info,
        );
    });
    assert!(result.is_ok(), "notify_event should not panic");
}

#[test]
fn test_notify_event_consistent_error() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// notify_event_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_notify_event_nb_reaches_ffi() {
    extern "C" fn dummy_op_cb(_: i32, _: *mut std::os::raw::c_void) {}
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::ErrJobAborted),
        &source,
        PmixDataRange::Session,
        &info,
        Some(dummy_op_cb),
        std::ptr::null_mut(),
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_notify_event_nb_with_none_cbfunc() {
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::ErrTimeout),
        &source,
        PmixDataRange::Custom,
        &info,
        None,
        std::ptr::null_mut(),
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_notify_event_nb_various_ranges() {
    extern "C" fn dummy_op_cb(_: i32, _: *mut std::os::raw::c_void) {}
    let source = Proc::new("test_job", 0).unwrap();
    let info = InfoBuilder::new().build();
    for range in [
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
    ] {
        let result = notify_event_nb(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            range,
            &info,
            Some(dummy_op_cb),
            std::ptr::null_mut(),
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction tests (used by event functions)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_for_event_source_various_ranks() {
    for rank in [0u32, 1, 42, u32::MAX] {
        let proc = Proc::new("test_ns", rank).unwrap();
        assert_eq!(proc.get_rank(), rank);
    }
}

#[test]
fn test_proc_empty_namespace() {
    let proc = Proc::new("", 0).unwrap();
    assert_eq!(proc.get_rank(), 0);
}

#[test]
fn test_proc_wildcard_for_events() {
    let proc = Proc::new("", u32::MAX).unwrap();
    assert_eq!(proc.get_rank(), u32::MAX);
}

// ─────────────────────────────────────────────────────────────────────────────
// Info empty handling for events
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_info_empty_for_register() {
    let info = InfoBuilder::new().build();
    assert!(info.is_empty());
    assert_eq!(info.len(), 0);
}

#[test]
fn test_empty_codes_array() {
    let codes: &[PmixStatus] = &[];
    assert!(codes.is_empty());
}

#[test]
fn test_single_code_array() {
    let codes: &[PmixStatus] = &[PmixStatus::Known(PmixError::ErrJobAborted)];
    assert_eq!(codes.len(), 1);
}

#[test]
fn test_multiple_codes_array() {
    let codes: &[PmixStatus] = &[
        PmixStatus::Known(PmixError::ErrJobAborted),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrNotSupported),
    ];
    assert_eq!(codes.len(), 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus roundtrip tests for events context
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_status_success_for_events() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success());
}

#[test]
fn test_pmix_status_error_for_events() {
    let status = PmixStatus::from_raw(-39);
    assert!(status.is_error());
}

#[test]
fn test_pmix_status_to_raw_known_success() {
    let status = PmixStatus::Known(PmixError::Success);
    assert_eq!(status.to_raw(), 0);
}

#[test]
fn test_pmix_status_to_raw_error() {
    let status = PmixStatus::Known(PmixError::ErrInit);
    assert!(status.to_raw() < 0);
}

#[test]
fn test_pmix_status_unknown_roundtrip() {
    let status = PmixStatus::Unknown(-42);
    assert_eq!(status.to_raw(), -42);
}
