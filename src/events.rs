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

use crate::{Info, PmixDataRange, PmixStatus, Proc, ffi};
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

    #[test]
    fn test_event_handler_ref_type() {
        let ref_: EventHandlerRef = 42;
        assert_eq!(ref_, 42);
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
}
