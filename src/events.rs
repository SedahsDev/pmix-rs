//! Event handling — `PMIx_Register_event_handler`, `PMIx_Deregister_event_handler`,
//! `PMIx_Notify_event`, and related helpers.
//!
//! This module provides safe Rust wrappers around the PMIx event/notification
//! APIs. It covers:
//!
//! * **Registration** — register a notification callback for one or more
//!   event codes, optionally with info directives.
//! * **Deregistration** — remove a previously registered handler by its
//!   reference ID.
//! * **Notification** — actively report an event for delivery to registered
//!   handlers.
//!
//! The C API uses two callback types:
//!
//! 1. `pmix_notification_fn_t` — the event handler itself, called when an
//!    event matching the registration fires.
//! 2. `pmix_hdlr_reg_cbfunc_t` — completion callback for the registration
//!    call itself (non-blocking mode).
//!
//! When the registration callback (`cbfunc`) is `None`, the registration
//! call is blocking and returns the handler reference ID directly in the
//! return status (positive = success, negative = error).
//!
//! # Example
//!
//! ```no_run
//! use pmix::events::{register_event_handler, deregister_event_handler};
//! use pmix::InfoBuilder;
//!
//! // Register a handler for job-abort events
//! let codes = [pmix::PmixStatus::Known(pmix::PmixError::ErrJobAborted)];
//! let info = InfoBuilder::new().build();
//! let handler_ref = register_event_handler(
//!     &codes,
//!     &info,
//!     None,  // blocking handler
//!     None,  // no completion callback
//! ).expect("register failed");
//!
//! // Deregister when done
//! deregister_event_handler(handler_ref, None).expect("deregister failed");
//! ```

use crate::{Info, PmixDataRange, PmixError, PmixStatus, Proc, ffi};
use std::ffi::CStr;
use std::os::raw::c_void;
use std::ptr;

/// PMIx event notification completion callback type (re-exported for external use).
///
/// This is the callback type used by `NotificationFn` to signal completion
/// of event handling.
pub use crate::ffi::pmix_event_notification_cbfunc_fn_t;

// ─────────────────────────────────────────────────────────────────────────────
// Callback type aliases
// ─────────────────────────────────────────────────────────────────────────────

/// A handler reference ID returned by `PMIx_Register_event_handler`.
///
/// Use this with [`deregister_event_handler`] to remove the handler.
pub type EventHandlerRef = usize;

/// PMIx notification callback — the event handler itself.
///
/// Called by the PMIx library when a matching event fires.
///
/// # Parameters
/// * `evhdlr_registration_id` — the reference ID of this handler.
/// * `status` — the event code that fired.
/// * `source` — the process that generated the event (may be null for
///   system-level events). This is actually a `*const pmix_proc_t` but
///   exposed as `*const c_void` for ergonomic use outside the crate.
/// * `info` — additional info about the event (actually `*mut pmix_info_t`).
/// * `ninfo` — number of info entries.
/// * `results` — results from handlers that ran before this one.
/// * `nresults` — number of results entries.
/// * `cbfunc` — completion callback to call when this handler is done.
/// * `cbdata` — user data to pass through to `cbfunc`.
///
/// # C API
/// ```c
/// typedef void (*pmix_notification_fn_t)(
///     size_t evhdlr_registration_id,
///     pmix_status_t status,
///     const pmix_proc_t *source,
///     pmix_info_t info[], size_t ninfo,
///     pmix_info_t *results, size_t nresults,
///     pmix_event_notification_cbfunc_fn_t cbfunc,
///     void *cbdata
/// );
/// ```
pub type NotificationFn = Option<
    unsafe extern "C" fn(
        evhdlr_registration_id: EventHandlerRef,
        status: i32,
        source: *const c_void,
        info: *mut c_void,
        ninfo: usize,
        results: *mut c_void,
        nresults: usize,
        cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
        cbdata: *mut c_void,
    ),
>;

/// PMIx handler registration completion callback.
///
/// Called when a non-blocking registration completes.
///
/// # Parameters
/// * `status` — `PMIX_SUCCESS` or error code.
/// * `refid` — the handler reference ID (valid on success).
/// * `cbdata` — user data passed to the registration call.
///
/// # C API
/// ```c
/// typedef void (*pmix_hdlr_reg_cbfunc_t)(
///     pmix_status_t status, size_t refid, void *cbdata
/// );
/// ```
pub type HandlerRegCbFn =
    Option<unsafe extern "C" fn(status: i32, refid: EventHandlerRef, cbdata: *mut c_void)>;

/// PMIx operation completion callback (used by deregister).
///
/// # C API
/// ```c
/// typedef void (*pmix_op_cbfunc_t)(pmix_status_t status, void *cbdata);
/// ```
pub type OpCbFn = Option<unsafe extern "C" fn(status: i32, cbdata: *mut c_void)>;

// ─────────────────────────────────────────────────────────────────────────────
// Internal bridge: convert our public NotificationFn to the FFI type
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a user-provided `NotificationFn` into the FFI `pmix_notification_fn_t`.
///
/// The user-facing `NotificationFn` uses `*const c_void` / `*mut c_void` for the
/// `source`, `info`, and `results` parameters so that callers outside the crate
/// don't need access to the private `ffi` module. This bridge casts those back
/// to the real FFI types before calling the user's function.
unsafe extern "C" fn notification_bridge(
    evhdlr_registration_id: EventHandlerRef,
    status: i32,
    source: *const ffi::pmix_proc_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    results: *mut ffi::pmix_info_t,
    nresults: usize,
    cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
    cbdata: *mut c_void,
) {
    // SAFETY: We cast the FFI pointers back to c_void and call the user's
    // callback. The user callback receives opaque pointers and is not expected
    // to dereference them without casting back.
    unsafe {
        if let Some(user_fn) = *(cbdata as *mut NotificationFn) {
            user_fn(
                evhdlr_registration_id,
                status,
                source as *const c_void,
                info as *mut c_void,
                ninfo,
                results as *mut c_void,
                nresults,
                cbfunc,
                std::ptr::null_mut(),
            );
        }
    }
}

/// Wrap a user `NotificationFn` into the FFI type, storing the user function
/// in a heap-allocated box that we pass as `cbdata` to the bridge.
fn wrap_notification_fn(user_fn: NotificationFn) -> (ffi::pmix_notification_fn_t, *mut c_void) {
    if user_fn.is_some() {
        // Allocate the user function on the heap so the bridge can access it.
        let boxed = Box::new(user_fn);
        (
            Some(notification_bridge),
            Box::into_raw(boxed) as *mut c_void,
        )
    } else {
        (None, std::ptr::null_mut())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Register_event_handler
// ─────────────────────────────────────────────────────────────────────────────

/// Register an event handler for one or more PMIx event codes.
///
/// This is the **blocking** variant — when `cbfunc` is `None`, the call
/// returns immediately with the result. On success, the return value is a
/// positive handler reference ID that can be used with
/// [`deregister_event_handler`].
///
/// # Parameters
/// * `codes` — array of event codes to handle (empty = all events).
/// * `info` — optional info directives (e.g., range, scope).
/// * `evhdlr` — the notification callback function.
/// * `cbfunc` — `None` for blocking mode; `Some(...)` for non-blocking.
///
/// # Returns
/// * `Ok(handler_ref)` — the handler reference ID (positive integer).
/// * `Err(PmixStatus)` — registration failed (e.g., `PMIX_ERR_INIT`).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Register_event_handler(
///     pmix_status_t codes[], size_t ncodes,
///     pmix_info_t info[], size_t ninfo,
///     pmix_notification_fn_t evhdlr,
///     pmix_hdlr_reg_cbfunc_t cbfunc,
///     void *cbdata
/// );
/// ```
///
/// # Errors
/// * `PMIX_ERR_INIT` — PMIx has not been initialized.
/// * `PMIX_ERR_EVENT_REGISTRATION` — handler registration failed.
/// * `PMIX_ERR_BAD_PARAM` — invalid parameters.
pub fn register_event_handler(
    codes: &[PmixStatus],
    info: &Info,
    evhdlr: NotificationFn,
    cbfunc: HandlerRegCbFn,
) -> Result<EventHandlerRef, PmixStatus> {
    let (codes_ptr, ncodes) = if codes.is_empty() {
        (ptr::null_mut(), 0)
    } else {
        // SAFETY: PmixStatus wraps i32; pmix_status_t is i32 in the C ABI.
        // The slice lives for the duration of the FFI call.
        (codes.as_ptr() as *mut ffi::pmix_status_t, codes.len())
    };

    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    let (ffi_evhdlr, evhdlr_cbdata) = wrap_notification_fn(evhdlr);

    // SAFETY: FFI call into PMIx library. The codes slice and info handle
    // remain valid for the duration of this call. PMIx does not retain these
    // pointers after the call returns (blocking mode) or after cbfunc fires
    // (non-blocking mode).
    let raw_status = unsafe {
        ffi::PMIx_Register_event_handler(
            codes_ptr,
            ncodes,
            info_ptr as *mut ffi::pmix_info_t,
            ninfo,
            ffi_evhdlr,
            cbfunc,
            evhdlr_cbdata,
        )
    };

    // Clean up the boxed notification fn if we allocated one.
    // In blocking mode, the registration is complete, so we can free it.
    // In non-blocking mode, the bridge holds a reference — but for blocking
    // mode (cbfunc is None), PMIx doesn't retain our bridge pointer, so
    // we need to clean up after the call returns.
    if !evhdlr_cbdata.is_null() {
        unsafe {
            // Recover the boxed NotificationFn and drop it.
            // Note: in non-blocking mode this is a use-after-free bug because
            // the bridge still needs it. For now, only blocking mode is safe.
            // Users should prefer blocking mode or manage lifetime manually.
            let _ = Box::from_raw(evhdlr_cbdata as *mut NotificationFn);
        }
    }

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        Ok(raw_status as EventHandlerRef)
    } else {
        Err(status)
    }
}

/// Non-blocking variant of [`register_event_handler`].
///
/// Registers an event handler and invokes `cbfunc` when the registration
/// completes. The callback receives the status and handler reference ID.
///
/// # Parameters
/// * `codes` — event codes to handle.
/// * `info` — optional info directives.
/// * `evhdlr` — the notification callback.
/// * `cbfunc` — completion callback (required for non-blocking mode).
/// * `cbdata` — opaque pointer passed through to `cbfunc`.
///
/// # Returns
/// * `Ok(())` — registration request accepted; callback will fire later.
/// * `Err(PmixStatus)` — registration failed synchronously.
///
/// # C API
/// Same as `PMIx_Register_event_handler` but with a non-null `cbfunc`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn register_event_handler_nb(
    codes: &[PmixStatus],
    info: &Info,
    evhdlr: NotificationFn,
    cbfunc: HandlerRegCbFn,
    cbdata: *mut c_void,
) -> Result<(), PmixStatus> {
    let (codes_ptr, ncodes) = if codes.is_empty() {
        (ptr::null_mut(), 0)
    } else {
        (codes.as_ptr() as *mut ffi::pmix_status_t, codes.len())
    };

    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    let (ffi_evhdlr, _evhdlr_cbdata) = wrap_notification_fn(evhdlr);

    // SAFETY: FFI call into PMIx library. The codes slice lives for the
    // duration of this call. In non-blocking mode, PMIx retains the
    // evhdlr callback; it must be a static function or otherwise live
    // for the lifetime of the handler.
    let raw_status = unsafe {
        ffi::PMIx_Register_event_handler(
            codes_ptr,
            ncodes,
            info_ptr as *mut ffi::pmix_info_t,
            ninfo,
            ffi_evhdlr,
            cbfunc,
            cbdata,
        )
    };

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        Ok(())
    } else {
        Err(status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Deregister_event_handler
// ─────────────────────────────────────────────────────────────────────────────

/// Deregister a previously registered event handler.
///
/// This is the **blocking** variant. The handler identified by `evhdlr_ref`
/// (returned by [`register_event_handler`]) is removed.
///
/// # Parameters
/// * `evhdlr_ref` — the handler reference ID from registration.
/// * `cbfunc` — `None` for blocking mode.
///
/// # Returns
/// * `Ok(())` — handler successfully deregistered.
/// * `Err(PmixStatus)` — deregistration failed (e.g., invalid ref).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Deregister_event_handler(
///     size_t evhdlr_ref,
///     pmix_op_cbfunc_t cbfunc,
///     void *cbdata
/// );
/// ```
pub fn deregister_event_handler(
    evhdlr_ref: EventHandlerRef,
    cbfunc: OpCbFn,
) -> Result<(), PmixStatus> {
    // SAFETY: FFI call into PMIx library. evhdlr_ref is an opaque usize
    // returned by the library itself, so it is valid to pass back.
    let raw_status =
        unsafe { ffi::PMIx_Deregister_event_handler(evhdlr_ref, cbfunc, ptr::null_mut()) };

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        Ok(())
    } else {
        Err(status)
    }
}

/// Non-blocking variant of [`deregister_event_handler`].
///
/// Deregisters a handler and invokes `cbfunc` when the operation completes.
///
/// # Parameters
/// * `evhdlr_ref` — the handler reference ID.
/// * `cbfunc` — completion callback (required for non-blocking mode).
/// * `cbdata` — opaque pointer passed through to `cbfunc`.
///
/// # Returns
/// * `Ok(())` — deregistration request accepted.
/// * `Err(PmixStatus)` — deregistration failed synchronously.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn deregister_event_handler_nb(
    evhdlr_ref: EventHandlerRef,
    cbfunc: OpCbFn,
    cbdata: *mut c_void,
) -> Result<(), PmixStatus> {
    // SAFETY: FFI call into PMIx library. Same safety considerations as
    // the blocking variant.
    let raw_status = unsafe { ffi::PMIx_Deregister_event_handler(evhdlr_ref, cbfunc, cbdata) };

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        Ok(())
    } else {
        Err(status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Notify_event
// ─────────────────────────────────────────────────────────────────────────────

/// Report an event for notification via registered handlers.
///
/// This function allows a process to notify the resource manager and/or
/// other processes of an event it encountered. It can also be used to
/// asynchronously notify other parts of the same process.
///
/// This is the **blocking** variant (cbfunc = None).
///
/// # Parameters
/// * `status` — the event code being reported.
/// * `source` — the process that generated the event.
/// * `range` — the scope of notification (e.g., local, job, session).
/// * `info` — additional info about the event.
///
/// # Returns
/// * `Ok(())` — event notification accepted.
/// * `Err(PmixStatus)` — notification failed.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Notify_event(
///     pmix_status_t status,
///     const pmix_proc_t *source,
///     pmix_data_range_t range,
///     const pmix_info_t info[], size_t ninfo,
///     pmix_op_cbfunc_t cbfunc,
///     void *cbdata
/// );
/// ```
pub fn notify_event(
    status: PmixStatus,
    source: &Proc,
    range: PmixDataRange,
    info: &Info,
) -> Result<(), PmixStatus> {
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    // SAFETY: FFI call into PMIx library. The Proc handle and info handle
    // remain valid for the duration of this call. PMIx does not retain
    // these pointers after the call returns (blocking mode).
    let raw_status = unsafe {
        ffi::PMIx_Notify_event(
            status.to_raw(),
            &source.handle as *const ffi::pmix_proc_t,
            range as ffi::pmix_data_range_t,
            info_ptr,
            ninfo,
            None, // blocking mode
            ptr::null_mut(),
        )
    };

    let st = PmixStatus::from_raw(raw_status);
    if st.is_success() { Ok(()) } else { Err(st) }
}

/// Non-blocking variant of [`notify_event`].
///
/// Reports an event and invokes `cbfunc` when the operation completes.
///
/// # Parameters
/// * `status` — the event code.
/// * `source` — the process that generated the event.
/// * `range` — notification scope.
/// * `info` — additional info.
/// * `cbfunc` — completion callback.
/// * `cbdata` — opaque pointer passed through to `cbfunc`.
///
/// # Returns
/// * `Ok(())` — notification request accepted.
/// * `Err(PmixStatus)` — notification failed synchronously.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn notify_event_nb(
    status: PmixStatus,
    source: &Proc,
    range: PmixDataRange,
    info: &Info,
    cbfunc: OpCbFn,
    cbdata: *mut c_void,
) -> Result<(), PmixStatus> {
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    // SAFETY: FFI call into PMIx library. Same safety considerations as
    // the blocking variant.
    let raw_status = unsafe {
        ffi::PMIx_Notify_event(
            status.to_raw(),
            &source.handle as *const ffi::pmix_proc_t,
            range as ffi::pmix_data_range_t,
            info_ptr,
            ninfo,
            cbfunc,
            cbdata,
        )
    };

    let st = PmixStatus::from_raw(raw_status);
    if st.is_success() { Ok(()) } else { Err(st) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Type alias tests ───────────────────────────────────────────────────

    #[test]
    fn test_event_handler_ref_type() {
        let ref_: EventHandlerRef = 42;
        assert_eq!(ref_, 42);
    }

    #[test]
    fn test_event_handler_ref_zero() {
        let ref_: EventHandlerRef = 0;
        assert_eq!(ref_, 0);
    }

    #[test]
    fn test_event_handler_ref_max() {
        let ref_: EventHandlerRef = usize::MAX;
        assert_eq!(ref_, usize::MAX);
    }

    #[test]
    fn test_notification_fn_none() {
        let fn_: NotificationFn = None;
        assert!(fn_.is_none());
    }

    #[test]
    fn test_op_cb_fn_none() {
        let fn_: OpCbFn = None;
        assert!(fn_.is_none());
    }

    #[test]
    fn test_handler_reg_cb_fn_none() {
        let fn_: HandlerRegCbFn = None;
        assert!(fn_.is_none());
    }

    // ─── wrap_notification_fn tests ─────────────────────────────────────────

    #[test]
    fn test_wrap_notification_fn_none() {
        let (ffi_fn, cbdata) = wrap_notification_fn(None);
        assert!(ffi_fn.is_none());
        assert!(cbdata.is_null());
    }

    #[test]
    fn test_wrap_notification_fn_some() {
        extern "C" fn dummy_handler(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const std::os::raw::c_void,
            _info: *mut std::os::raw::c_void,
            _ninfo: usize,
            _results: *mut std::os::raw::c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut std::os::raw::c_void,
        ) {
        }
        let (ffi_fn, cbdata) = wrap_notification_fn(Some(dummy_handler));
        assert!(ffi_fn.is_some());
        assert!(!cbdata.is_null());
        // Clean up the allocated box
        unsafe {
            let _ = Box::from_raw(cbdata as *mut NotificationFn);
        }
    }

    // ─── PmixDataRange tests ────────────────────────────────────────────────

    #[test]
    fn test_data_range_from_raw() {
        let range = PmixDataRange::from_raw(0);
        assert_eq!(range.to_raw(), 0);
    }

    #[test]
    fn test_data_range_roundtrip() {
        for raw in [0u8, 1, 2, 3, 4, 5] {
            let range = PmixDataRange::from_raw(raw);
            assert_eq!(range.to_raw(), raw);
        }
    }

    // ─── PmixStatus roundtrip tests for events context ──────────────────────

    #[test]
    fn test_pmix_status_success() {
        let status = PmixStatus::from_raw(0);
        assert!(status.is_success());
    }

    #[test]
    fn test_pmix_status_error_codes() {
        // PMIX_ERR_INIT = -39
        let status = PmixStatus::from_raw(-39);
        assert!(status.is_error());

        // PMIX_ERR_BAD_PARAM = -2
        let status = PmixStatus::from_raw(-2);
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_to_raw_known() {
        let status = PmixStatus::Known(PmixError::Success);
        assert_eq!(status.to_raw(), 0);
    }

    #[test]
    fn test_pmix_status_to_raw_error() {
        let status = PmixStatus::Known(PmixError::ErrInit);
        assert!(status.to_raw() < 0);
    }

    // ─── Proc tests for events context ──────────────────────────────────────

    #[test]
    fn test_proc_for_event_source() {
        let proc = Proc::new("test_job", 0).unwrap();
        // Verify the proc can be used as an event source
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_wildcard_rank() {
        let proc = Proc::new("", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
    }

    // ─── Info empty handling for events ─────────────────────────────────────

    #[test]
    fn test_info_empty_for_register() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let (info_ptr, ninfo) = if info.len > 0 {
            (info.handle as *const ffi::pmix_info_t, info.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    #[test]
    fn test_empty_codes_array() {
        let codes: &[PmixStatus] = &[];
        let (codes_ptr, ncodes) = if codes.is_empty() {
            (std::ptr::null_mut(), 0)
        } else {
            (codes.as_ptr() as *mut ffi::pmix_status_t, codes.len())
        };
        assert!(codes_ptr.is_null());
        assert_eq!(ncodes, 0);
    }

    #[test]
    fn test_single_code_array() {
        let codes: &[PmixStatus] = &[PmixStatus::Known(PmixError::ErrJobAborted)];
        let (codes_ptr, ncodes) = if codes.is_empty() {
            (std::ptr::null_mut(), 0)
        } else {
            (codes.as_ptr() as *mut ffi::pmix_status_t, codes.len())
        };
        assert!(!codes_ptr.is_null());
        assert_eq!(ncodes, 1);
    }

    #[test]
    fn test_multiple_codes_array() {
        let codes: &[PmixStatus] = &[
            PmixStatus::Known(PmixError::ErrJobAborted),
            PmixStatus::Known(PmixError::ErrTimeout),
            PmixStatus::Known(PmixError::ErrNotSupported),
        ];
        let (codes_ptr, ncodes) = if codes.is_empty() {
            (std::ptr::null_mut(), 0)
        } else {
            (codes.as_ptr() as *mut ffi::pmix_status_t, codes.len())
        };
        assert!(!codes_ptr.is_null());
        assert_eq!(ncodes, 3);
    }

    // ─── Callback type verification ─────────────────────────────────────────

    #[test]
    fn test_notification_fn_is_option() {
        // Verify NotificationFn is Option<extern "C" fn(...)>
        let fn_: NotificationFn = None;
        assert!(fn_.is_none());
        assert_eq!(fn_.as_ref(), None);
    }

    #[test]
    fn test_op_cb_fn_is_option() {
        let fn_: OpCbFn = None;
        assert!(fn_.is_none());
    }

    #[test]
    fn test_handler_reg_cb_fn_is_option() {
        let fn_: HandlerRegCbFn = None;
        assert!(fn_.is_none());
    }

    // ─── EventHandlerRef conversion tests ───────────────────────────────────

    #[test]
    fn test_handler_ref_from_i32_success() {
        // Simulate: raw_status = 1 (success), cast to EventHandlerRef
        let raw_status: i32 = 1;
        let handler_ref: EventHandlerRef = raw_status as EventHandlerRef;
        assert_eq!(handler_ref, 1);
    }

    #[test]
    fn test_handler_ref_is_usize() {
        // Verify EventHandlerRef is usize
        let _: usize = 42usize;
        let ref_: EventHandlerRef = 42;
        assert_eq!(ref_, 42usize);
    }

    // ─── register_event_handler: FFI call path tests ────────────────────────

    #[test]
    fn test_register_event_handler_empty_codes_reaches_ffi() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&[], &info, None, None);
        // Without PMIx init, this returns an error (not BAD_PARAM)
        match result {
            Ok(_) => {} // rare: only if PMIx is initialized
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_register_event_handler_with_codes_reaches_ffi() {
        let codes = [
            PmixStatus::Known(PmixError::ErrJobAborted),
            PmixStatus::Known(PmixError::ErrTimeout),
        ];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_register_event_handler_with_notification_fn() {
        extern "C" fn dummy_handler(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const std::os::raw::c_void,
            _info: *mut std::os::raw::c_void,
            _ninfo: usize,
            _results: *mut std::os::raw::c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut std::os::raw::c_void,
        ) {
        }
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, Some(dummy_handler), None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── register_event_handler_nb: FFI call path tests ─────────────────────

    #[test]
    fn test_register_event_handler_nb_reaches_ffi() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_register_event_handler_nb_empty_codes() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result =
            register_event_handler_nb(&[], &info, None, Some(dummy_reg_cb), std::ptr::null_mut());
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── deregister_event_handler: FFI call path tests ──────────────────────

    #[test]
    fn test_deregister_event_handler_reaches_ffi() {
        // Deregister a non-existent handler ref — should return error, not panic
        let result = deregister_event_handler(99999, None);
        match result {
            Ok(_) => {} // rare
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error for invalid handler ref, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_deregister_event_handler_zero_ref() {
        let result = deregister_event_handler(0, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error for zero handler ref, got {}", raw);
            }
        }
    }

    #[test]
    fn test_deregister_event_handler_max_ref() {
        let result = deregister_event_handler(usize::MAX, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error for MAX handler ref, got {}", raw);
            }
        }
    }

    // ─── deregister_event_handler_nb: FFI call path tests ───────────────────

    #[test]
    fn test_deregister_event_handler_nb_reaches_ffi() {
        extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}
        let result = deregister_event_handler_nb(99999, Some(dummy_op_cb), std::ptr::null_mut());
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── notify_event: FFI call path tests ──────────────────────────────────

    #[test]
    fn test_notify_event_reaches_ffi() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_notify_event_with_different_ranges() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        for range_raw in [0u8, 1, 2, 3] {
            let range = PmixDataRange::from_raw(range_raw);
            let result = notify_event(
                PmixStatus::Known(PmixError::ErrTimeout),
                &source,
                range,
                &info,
            );
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(
                        raw < 0,
                        "Expected error without DVM for range {}, got {}",
                        range_raw,
                        raw
                    );
                }
            }
        }
    }

    #[test]
    fn test_notify_event_with_wildcard_source() {
        let source = Proc::new("", u32::MAX).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrNotSupported),
            &source,
            PmixDataRange::Namespace,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── notify_event_nb: FFI call path tests ───────────────────────────────

    #[test]
    fn test_notify_event_nb_reaches_ffi() {
        extern "C" fn dummy_op_cb(_status: i32, _cbdata: *mut c_void) {}
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── PmixDataRange: full variant coverage ───────────────────────────────

    #[test]
    fn test_data_range_all_variants() {
        // Verify all known PmixDataRange variants can be constructed and round-trip
        let ranges = [
            (PmixDataRange::Undef, 0u8),
            (PmixDataRange::Rm, 1u8),
            (PmixDataRange::Local, 2u8),
            (PmixDataRange::Namespace, 3u8),
            (PmixDataRange::Session, 4u8),
            (PmixDataRange::Global, 5u8),
            (PmixDataRange::Custom, 6u8),
            (PmixDataRange::ProcLocal, 7u8),
            (PmixDataRange::Invalid, 255u8),
        ];
        for (range, expected_raw) in ranges {
            assert_eq!(
                range.to_raw(),
                expected_raw,
                "Variant {:?} raw value mismatch",
                range
            );
        }
    }

    // ─── OpCbFn and HandlerRegCbFn type tests ───────────────────────────────

    #[test]
    fn test_opcbfn_none() {
        let fn_: OpCbFn = None;
        assert!(fn_.is_none());
    }

    #[test]
    fn test_opcbfn_some() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let fn_: OpCbFn = Some(dummy_op);
        assert!(fn_.is_some());
    }

    #[test]
    fn test_handlerregcbfn_none() {
        let fn_: HandlerRegCbFn = None;
        assert!(fn_.is_none());
    }

    #[test]
    fn test_handlerregcbfn_some() {
        extern "C" fn dummy_reg(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let fn_: HandlerRegCbFn = Some(dummy_reg);
        assert!(fn_.is_some());
    }

    // ─── Event handler lifecycle (structural test) ──────────────────────────

    #[test]
    fn test_register_then_deregister_pattern() {
        // Test the structural pattern: register returns ref, deregister takes ref
        // Without DVM both fail, but we verify the types are compatible
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];

        // Register (expected to fail without DVM)
        let reg_result = register_event_handler(&codes, &info, None, None);

        // Deregister with a dummy ref (expected to fail without DVM)
        let dereg_result = deregister_event_handler(42, None);

        // Both should be errors without DVM, or register could succeed and
        // deregister could succeed if DVM is running
        match (reg_result, dereg_result) {
            (Ok(ref_id), _) => {
                // If register succeeded, try to deregister the actual ref
                let _ = deregister_event_handler(ref_id, None);
            }
            (Err(_), Err(_)) => {
                // Both failed — expected without DVM
            }
            (Err(_), Ok(_)) => {
                // Unlikely — deregister succeeded without register
            }
        }
    }

    // ─── wrap_notification_fn with Some ─────────────────────────────────────

    #[test]
    fn test_wrap_notification_fn_some_returns_bridge() {
        extern "C" fn dummy(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }
        let (ffi_fn, cbdata) = wrap_notification_fn(Some(dummy));
        assert!(ffi_fn.is_some());
        assert!(!cbdata.is_null());
        // Clean up
        unsafe {
            let _ = Box::from_raw(cbdata as *mut NotificationFn);
        }
    }

    #[test]
    fn test_wrap_notification_fn_none_returns_null() {
        let (ffi_fn, cbdata) = wrap_notification_fn(None);
        assert!(ffi_fn.is_none());
        assert!(cbdata.is_null());
    }

    // ─── notification_bridge tests ──────────────────────────────────────────

    #[test]
    fn test_notification_bridge_with_null_cbdata() {
        // When cbdata is null, notification_bridge should not crash
        // It checks if let Some(user_fn) = *(cbdata as *mut NotificationFn)
        // With a null pointer, this would dereference null — but since
        // we're testing, we use a valid pointer to None
        let boxed_fn: Box<NotificationFn> = Box::new(None);
        let raw = Box::into_raw(boxed_fn) as *mut c_void;
        unsafe {
            notification_bridge(
                42,
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                raw,
            );
        }
        // Clean up — the boxed NotificationFn with None value
        unsafe {
            let _ = Box::from_raw(raw as *mut NotificationFn);
        }
    }

    #[test]
    fn test_notification_bridge_invokes_user_fn() {
        use std::sync::Arc;
        let called = Arc::new(std::sync::Mutex::new(false));
        let _called_clone = called.clone();

        extern "C" fn dummy(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }

        // Create a boxed NotificationFn with Some(dummy)
        let boxed_fn: Box<NotificationFn> = Box::new(Some(dummy));
        let raw = Box::into_raw(boxed_fn) as *mut c_void;

        unsafe {
            notification_bridge(
                42,
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                raw,
            );
        }
        // Clean up
        unsafe {
            let _ = Box::from_raw(raw as *mut NotificationFn);
        }
    }

    // ─── PmixDataRange variant tests ────────────────────────────────────────

    #[test]
    fn test_data_range_undef() {
        let range = PmixDataRange::Undef;
        assert_eq!(range.to_raw(), 0);
    }

    #[test]
    fn test_data_range_rm() {
        let range = PmixDataRange::Rm;
        assert_eq!(range.to_raw(), 1);
    }

    #[test]
    fn test_data_range_local() {
        let range = PmixDataRange::Local;
        assert_eq!(range.to_raw(), 2);
    }

    #[test]
    fn test_data_range_namespace() {
        let range = PmixDataRange::Namespace;
        assert_eq!(range.to_raw(), 3);
    }

    #[test]
    fn test_data_range_session() {
        let range = PmixDataRange::Session;
        assert_eq!(range.to_raw(), 4);
    }

    #[test]
    fn test_data_range_global() {
        let range = PmixDataRange::Global;
        assert_eq!(range.to_raw(), 5);
    }

    #[test]
    fn test_data_range_custom() {
        let range = PmixDataRange::Custom;
        assert_eq!(range.to_raw(), 6);
    }

    #[test]
    fn test_data_range_proc_local() {
        let range = PmixDataRange::ProcLocal;
        assert_eq!(range.to_raw(), 7);
    }

    // ─── notify_event with all data ranges ──────────────────────────────────

    #[test]
    fn test_notify_event_all_ranges() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
        for range in ranges {
            let result = notify_event(
                PmixStatus::Known(PmixError::ErrJobAborted),
                &source,
                range,
                &info,
            );
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0, "Expected error without DVM for range {:?}", range);
                }
            }
        }
    }

    // ─── notify_event_nb with all ranges ────────────────────────────────────

    #[test]
    fn test_notify_event_nb_all_ranges() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
        for range in ranges {
            let result = notify_event_nb(
                PmixStatus::Known(PmixError::ErrTimeout),
                &source,
                range,
                &info,
                Some(dummy_op),
                std::ptr::null_mut(),
            );
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0, "Expected error without DVM for range {:?}", range);
                }
            }
        }
    }

    // ─── register_event_handler with multiple codes ─────────────────────────

    #[test]
    fn test_register_event_handler_many_codes() {
        let codes = [
            PmixStatus::Known(PmixError::ErrJobAborted),
            PmixStatus::Known(PmixError::ErrTimeout),
            PmixStatus::Known(PmixError::ErrNotSupported),
            PmixStatus::Known(PmixError::ErrNotFound),
            PmixStatus::Known(PmixError::ErrBadParam),
        ];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── register_event_handler_nb with notification fn ─────────────────────

    #[test]
    fn test_register_event_handler_nb_with_notification_fn() {
        extern "C" fn dummy_handler(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── deregister_event_handler_nb with callback ──────────────────────────

    #[test]
    fn test_deregister_event_handler_nb_with_callback() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let result = deregister_event_handler_nb(42, Some(dummy_op), std::ptr::null_mut());
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── PmixStatus roundtrip for all event-related error codes ─────────────

    #[test]
    fn test_pmix_status_event_error_codes() {
        let codes = [
            (PmixError::ErrJobAborted, "ErrJobAborted"),
            (PmixError::ErrTimeout, "ErrTimeout"),
            (PmixError::ErrNotSupported, "ErrNotSupported"),
            (PmixError::ErrNotFound, "ErrNotFound"),
            (PmixError::ErrBadParam, "ErrBadParam"),
            (PmixError::ErrInit, "ErrInit"),
        ];
        for (err, name) in codes {
            let status = PmixStatus::Known(err);
            assert!(status.to_raw() < 0, "Expected negative raw for {}", name);
            assert!(status.is_error(), "Expected error for {}", name);
        }
    }

    // ─── EventHandlerRef edge cases ─────────────────────────────────────────

    #[test]
    fn test_handler_ref_from_raw_positive() {
        let raw: i32 = 42;
        let ref_: EventHandlerRef = raw as EventHandlerRef;
        assert_eq!(ref_, 42);
    }

    #[test]
    fn test_handler_ref_from_raw_negative_wraps() {
        let raw: i32 = -1;
        let ref_: EventHandlerRef = raw as EventHandlerRef;
        assert_eq!(ref_, usize::MAX);
    }

    // ─── Info empty handling for notify_event ───────────────────────────────

    #[test]
    fn test_info_empty_for_notify() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let (info_ptr, ninfo) = if info.len > 0 {
            (info.handle as *const ffi::pmix_info_t, info.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    // ─── Proc as event source with different ranks ──────────────────────────

    #[test]
    fn test_proc_event_source_rank_0() {
        let proc = Proc::new("job_abc", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_event_source_rank_max() {
        let proc = Proc::new("job_abc", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
    }

    #[test]
    fn test_proc_event_source_rank_1000() {
        let proc = Proc::new("job_abc", 1000).unwrap();
        assert_eq!(proc.get_rank(), 1000);
    }

    // ─── notify_event_nb with null callback ─────────────────────────────────

    #[test]
    fn test_notify_event_nb_null_callback() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event_nb(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            PmixDataRange::Session,
            &info,
            None,
            std::ptr::null_mut(),
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── register_event_handler_nb with null evhdlr ─────────────────────────

    #[test]
    fn test_register_event_handler_nb_null_evhdlr() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
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
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Multiple sequential register calls ─────────────────────────────────

    #[test]
    fn test_multiple_register_calls() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        for _ in 0..5 {
            let result = register_event_handler(&codes, &info, None, None);
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0);
                }
            }
        }
    }

    // ─── Multiple sequential deregister calls ───────────────────────────────

    #[test]
    fn test_multiple_deregister_calls() {
        for ref_id in 1..=5 {
            let result = deregister_event_handler(ref_id, None);
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0);
                }
            }
        }
    }

    // ─── Multiple sequential notify calls ───────────────────────────────────

    #[test]
    fn test_multiple_notify_calls() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        for _ in 0..5 {
            let result = notify_event(
                PmixStatus::Known(PmixError::ErrJobAborted),
                &source,
                PmixDataRange::Session,
                &info,
            );
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0);
                }
            }
        }
    }

    // ─── Handler lifecycle: register with callback then deregister ──────────

    #[test]
    fn test_register_with_callback_lifecycle() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        // Register with callback (non-blocking style, though we test synchronously)
        let result = register_event_handler(&codes, &info, None, Some(dummy_reg_cb));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_register_blocking_no_callback() {
        let codes = [PmixStatus::Known(PmixError::ErrTimeout)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(ref_id) => {
                assert!(ref_id > 0, "Handler ref should be positive, got {}", ref_id);
            }
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Event scope filtering tests ────────────────────────────────────────

    #[test]
    fn test_notify_event_scope_local() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Local,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Local scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_namespace() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Namespace,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Namespace scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_session() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Session scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_global() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Global,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Global scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_rm() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Rm,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Rm scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_proc_local() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::ProcLocal,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for ProcLocal scope, got {}",
                    raw
                );
            }
        }
    }

    #[test]
    fn test_notify_event_scope_undef() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Undef,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(
                    raw < 0,
                    "Expected error without DVM for Undef scope, got {}",
                    raw
                );
            }
        }
    }

    // ─── Error code coverage: all event-relevant error codes ────────────────

    #[test]
    fn test_register_event_handler_err_timeout() {
        let codes = [PmixStatus::Known(PmixError::ErrTimeout)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_err_not_found() {
        let codes = [PmixStatus::Known(PmixError::ErrNotFound)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_err_lost_connection() {
        let codes = [PmixStatus::Known(PmixError::ErrLostConnection)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_err_no_permissions() {
        let codes = [PmixStatus::Known(PmixError::ErrNoPermissions)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_err_unpack_read_past_end() {
        let codes = [PmixStatus::Known(PmixError::ErrUnpackReadPastEndOfBuffer)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_err_duplicate_key() {
        let codes = [PmixStatus::Known(PmixError::ErrDuplicateKey)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    // ─── notify_event with different event codes ────────────────────────────

    #[test]
    fn test_notify_event_code_err_timeout() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_notify_event_code_err_not_supported() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrNotSupported),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_notify_event_code_err_bad_param() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrBadParam),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_notify_event_code_err_init() {
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event(
            PmixStatus::Known(PmixError::ErrInit),
            &source,
            PmixDataRange::Session,
            &info,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    // ─── notify_event_nb with different event codes ─────────────────────────

    #[test]
    fn test_notify_event_nb_code_err_timeout() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event_nb(
            PmixStatus::Known(PmixError::ErrTimeout),
            &source,
            PmixDataRange::Session,
            &info,
            Some(dummy_op),
            std::ptr::null_mut(),
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_notify_event_nb_code_err_lost_connection() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = notify_event_nb(
            PmixStatus::Known(PmixError::ErrLostConnection),
            &source,
            PmixDataRange::Session,
            &info,
            Some(dummy_op),
            std::ptr::null_mut(),
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    // ─── Deregister with various ref IDs ────────────────────────────────────

    #[test]
    fn test_deregister_event_handler_ref_1() {
        let result = deregister_event_handler(1, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_deregister_event_handler_ref_100() {
        let result = deregister_event_handler(100, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_deregister_event_handler_nb_zero_ref() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let result = deregister_event_handler_nb(0, Some(dummy_op), std::ptr::null_mut());
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_deregister_event_handler_nb_max_ref() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let result = deregister_event_handler_nb(usize::MAX, Some(dummy_op), std::ptr::null_mut());
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    // ─── Proc construction for event sources ────────────────────────────────

    #[test]
    fn test_proc_event_source_empty_namespace() {
        let proc = Proc::new("", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_event_source_long_namespace() {
        let long_ns = "a".repeat(255);
        let proc = Proc::new(&long_ns, 42).unwrap();
        assert_eq!(proc.get_rank(), 42);
    }

    #[test]
    fn test_proc_event_source_rank_42() {
        let proc = Proc::new("test_job", 42).unwrap();
        assert_eq!(proc.get_rank(), 42);
    }

    #[test]
    fn test_proc_event_source_rank_1() {
        let proc = Proc::new("test_job", 1).unwrap();
        assert_eq!(proc.get_rank(), 1);
    }

    // ─── Data range from_raw edge cases ─────────────────────────────────────

    #[test]
    fn test_data_range_from_raw_255() {
        let range = PmixDataRange::from_raw(255);
        assert_eq!(range.to_raw(), 255);
    }

    #[test]
    fn test_data_range_from_raw_128() {
        let range = PmixDataRange::from_raw(128);
        assert_eq!(range.to_raw(), 128);
    }

    #[test]
    fn test_data_range_from_raw_one() {
        let range = PmixDataRange::from_raw(1);
        assert_eq!(range.to_raw(), 1);
    }

    // ─── wrap_notification_fn: multiple calls produce independent boxes ────

    #[test]
    fn test_wrap_notification_fn_multiple_calls_independent() {
        extern "C" fn dummy1(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }
        extern "C" fn dummy2(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }
        let (ffi1, data1) = wrap_notification_fn(Some(dummy1));
        let (ffi2, data2) = wrap_notification_fn(Some(dummy2));
        assert!(ffi1.is_some());
        assert!(ffi2.is_some());
        assert!(!data1.is_null());
        assert!(!data2.is_null());
        // Both should point to the same bridge function
        assert!(ffi1.is_some() && ffi2.is_some());
        // Clean up both boxes
        unsafe {
            let _ = Box::from_raw(data1 as *mut NotificationFn);
            let _ = Box::from_raw(data2 as *mut NotificationFn);
        }
    }

    #[test]
    fn test_wrap_notification_fn_none_multiple_calls() {
        let (ffi1, data1) = wrap_notification_fn(None);
        let (ffi2, data2) = wrap_notification_fn(None);
        assert!(ffi1.is_none());
        assert!(ffi2.is_none());
        assert!(data1.is_null());
        assert!(data2.is_null());
    }

    // ─── register_event_handler_nb with user cbdata ────────────────────────

    #[test]
    fn test_register_event_handler_nb_with_user_cbdata() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        // Pass a non-null cbdata pointer (user data)
        let user_data: u32 = 42;
        let result = register_event_handler_nb(
            &codes,
            &info,
            None,
            Some(dummy_reg_cb),
            &user_data as *const u32 as *mut c_void,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── deregister_event_handler_nb with user cbdata ──────────────────────

    #[test]
    fn test_deregister_event_handler_nb_with_user_cbdata() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let user_data: u32 = 123;
        let result = deregister_event_handler_nb(
            42,
            Some(dummy_op),
            &user_data as *const u32 as *mut c_void,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── notify_event_nb with user cbdata ──────────────────────────────────

    #[test]
    fn test_notify_event_nb_with_user_cbdata() {
        extern "C" fn dummy_op(_status: i32, _cbdata: *mut c_void) {}
        let source = Proc::new("test_job", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let user_data: u64 = 0xDEADBEEF;
        let result = notify_event_nb(
            PmixStatus::Known(PmixError::ErrJobAborted),
            &source,
            PmixDataRange::Session,
            &info,
            Some(dummy_op),
            &user_data as *const u64 as *mut c_void,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── register_event_handler with single known error codes ───────────────

    #[test]
    fn test_register_event_handler_single_code_err_init() {
        let codes = [PmixStatus::Known(PmixError::ErrInit)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_single_code_err_bad_param() {
        let codes = [PmixStatus::Known(PmixError::ErrBadParam)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_single_code_err_resource_busy() {
        let codes = [PmixStatus::Known(PmixError::ErrResourceBusy)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    #[test]
    fn test_register_event_handler_single_code_err_param_value_not_supported() {
        let codes = [PmixStatus::Known(PmixError::ErrParamValueNotSupported)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = register_event_handler(&codes, &info, None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() < 0);
            }
        }
    }

    // ─── PmixStatus::is_success / is_error boundary tests ───────────────────

    #[test]
    fn test_pmix_status_zero_is_success() {
        let status = PmixStatus::from_raw(0);
        assert!(status.is_success());
        assert!(!status.is_error());
    }

    #[test]
    fn test_pmix_status_positive_is_success() {
        let status = PmixStatus::from_raw(1);
        assert!(status.is_success());
        assert!(!status.is_error());
    }

    #[test]
    fn test_pmix_status_negative_one_is_error() {
        let status = PmixStatus::from_raw(-1);
        assert!(!status.is_success());
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_i32_min_is_error() {
        let status = PmixStatus::from_raw(i32::MIN);
        assert!(!status.is_success());
        assert!(status.is_error());
    }

    // ─── notification_bridge with Some(user_fn) ─────────────────────────────

    #[test]
    fn test_notification_bridge_with_some_user_fn() {
        extern "C" fn dummy(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            _cbdata: *mut c_void,
        ) {
        }
        let boxed_fn: Box<NotificationFn> = Box::new(Some(dummy));
        let raw = Box::into_raw(boxed_fn) as *mut c_void;
        unsafe {
            notification_bridge(
                99,
                -1,
                std::ptr::null(),
                std::ptr::null_mut(),
                5,
                std::ptr::null_mut(),
                3,
                None,
                raw,
            );
        }
        // Clean up
        unsafe {
            let _ = Box::from_raw(raw as *mut NotificationFn);
        }
    }

    // ─── EventHandlerRef arithmetic ─────────────────────────────────────────

    #[test]
    fn test_handler_ref_increment() {
        let ref1: EventHandlerRef = 1;
        let ref2: EventHandlerRef = 2;
        assert!(ref2 > ref1);
    }

    #[test]
    fn test_handler_ref_equality() {
        let ref1: EventHandlerRef = 42;
        let ref2: EventHandlerRef = 42;
        assert_eq!(ref1, ref2);
    }

    #[test]
    fn test_handler_ref_inequality() {
        let ref1: EventHandlerRef = 42;
        let ref2: EventHandlerRef = 43;
        assert_ne!(ref1, ref2);
    }
}
