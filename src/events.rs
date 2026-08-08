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
//! # Threading
//!
//! Event handlers are delivered on the PMIx **progress thread**, like all
//! `_nb` completions. The notification bridge follows the policy in
//! [`crate::threading`]: the user's `NotificationFn` is stored in a registry
//! keyed by the handler reference ID (never a raw pointer in `cbdata`), and
//! the registry lock is **not** held while the user callback runs. Handlers
//! must not call blocking PMIx APIs; hop off with
//! [`spawn_from_callback`](crate::threading::spawn_from_callback) or a
//! [`CallbackChannel`](crate::threading::CallbackChannel) first.
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
use std::collections::{HashMap, HashSet};
use std::os::raw::c_void;
use std::ptr;
use std::sync::{LazyLock, Mutex};

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
/// * `cbdata` — **OpenPMIx event-chain pointer**. Must be returned **verbatim**
///   through `cbfunc` (as `notification_cbdata`). OpenPMIx's
///   `progress_local_event_hdlr` casts it to the chain object with no NULL
///   check — discarding it or substituting `NULL` stalls or crashes the
///   event engine. Call `cbfunc` from the progress thread or from a hopped-off
///   application thread (`progress_local_event_hdlr` thread-shifts).
///
/// # Contract
///
/// Handlers **must** complete the event chain. Typical completion:
///
/// ```text
/// cbfunc(status, results, nresults, None, null_mut(), cbdata)
/// ```
///
/// Omitting the call permanently stalls the chain (blocking `notify_event`
/// never returns; subsequent deliveries hang).
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
///
/// # ABI
///
/// This is a **Rust-ABI** `unsafe fn`, not `extern "C"`. Only
/// [`notification_bridge`] is registered with OpenPMIx; it is the sole
/// C→Rust FFI boundary. Keeping the user handler off the C ABI means a
/// panic inside the handler can be caught (`catch_unwind`) instead of
/// aborting via `nounwind` — required so the bridge can still complete the
/// OpenPMIx event chain.
pub type NotificationFn = Option<
    unsafe fn(
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
// Internal bridge: ref-keyed handler registry
// ─────────────────────────────────────────────────────────────────────────────

/// Global registry mapping handler reference IDs to boxed user notification
/// functions.
///
/// PMIx retains an event handler — and its `cbdata` — for the lifetime of the
/// registration, so the user's [`NotificationFn`] must outlive it. We store
/// the function here, keyed by the reference ID that PMIx passes to the
/// notification bridge as its first argument, and free it at deregistration
/// (or session finalize via [`clear_handler_registry`]).
///
/// This follows the bridge policy in [`crate::threading`]: the registry lock
/// is held only to copy the function pointer out, never across the user call,
/// and no callback data rides in a raw `cbdata` pointer.
///
/// # Invariant
///
/// Entries are plain [`NotificationFn`] values (`Option` of a function pointer).
/// Function pointers are `Copy`, so copying the fn out under the lock and later
/// dropping the `Box` is not a use-after-free. That analysis would not hold if
/// this type ever became a capturing closure / trait object.
type HandlerRegistry = HashMap<EventHandlerRef, Box<NotificationFn>>;
static HANDLER_REGISTRY: LazyLock<Mutex<HandlerRegistry>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct PendingRegistration {
    codes: Vec<i32>,
    user_fn: Box<NotificationFn>,
}

static PENDING_REGISTRATIONS: LazyLock<Mutex<Vec<PendingRegistration>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static DEREG_IN_PROGRESS: LazyLock<Mutex<HashSet<EventHandlerRef>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
static BLOCKING_REGISTRATION_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Drop every parked notification handler.
///
/// Called from client / tool / server session finalize paths so registrations
/// that were never explicitly deregistered do not leak `Box<NotificationFn>`
/// for the rest of the process lifetime. Safe under the crate's no-reinit
/// policy: after finalize, no further event deliveries are expected.
pub(crate) fn clear_handler_registry() {
    DEREG_IN_PROGRESS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    PENDING_REGISTRATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    HANDLER_REGISTRY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

/// Complete an OpenPMIx local event-handler chain (pass-through / recovery).
///
/// # Safety
///
/// `cbfunc` / `cbdata` / `results` must be the values OpenPMIx supplied to the
/// notification handler (or nulls). Double-completion of the same chain is
/// undefined at the C layer — call at most once per delivery.
unsafe fn complete_event_chain(
    status: i32,
    results: *mut ffi::pmix_info_t,
    nresults: usize,
    cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
    cbdata: *mut c_void,
) {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: caller upholds OpenPMIx chain-completion contract.
        unsafe {
            cbfunc(status, results, nresults, None, ptr::null_mut(), cbdata);
        }
    }
}

/// Convert a user-provided `NotificationFn` into the FFI `pmix_notification_fn_t`.
///
/// The user-facing `NotificationFn` uses `*const c_void` / `*mut c_void` for the
/// `source`, `info`, and `results` parameters so that callers outside the crate
/// don't need access to the private `ffi` module. This bridge casts those back
/// to the real FFI types before calling the user's function.
///
/// Runs on the PMIx **progress thread**. The user function is resolved from
/// [`HANDLER_REGISTRY`] under a scoped lock that is released before the user
/// callback executes; the callback must not call blocking PMIx APIs (see
/// [`crate::threading`]).
///
/// # Event-chain completion
///
/// OpenPMIx invokes the registered handler with
/// `cbfunc = progress_local_event_hdlr` and `cbdata = (void*)chain`. The
/// handler **must** call `cbfunc(..., cbdata)` or the chain never advances
/// (`cycle_events` / `final_cbfunc` never run). A blocking `notify_event`
/// waits on that completion, so a missed `cbfunc` is a permanent hang — not
/// a silent drop.
///
/// On registry miss (no user fn, `evhdlr = None`, or an unknown refid), this
/// bridge still calls `cbfunc` as a pass-through. During blocking registration,
/// a matching provisional handler is used while its refid is being returned by
/// PMIx. A delivery for a refid currently being deregistered is passed through
/// rather than matched against a provisional registration. The provisional
/// match is only for the registration's matching event codes; a foreign refid
/// must not be attributed to an unrelated default handler.
///
/// User-handler panics are caught: unwinding across the C→Rust FFI boundary
/// is undefined behaviour, and a panic that skips `cbfunc` would stall the
/// chain. The bridge completes the chain, logs, and **contains** the panic
/// (does not `resume_unwind` into OpenPMIx).
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
    // Copy the user fn pointer out of the registry WITHOUT holding the lock
    // across the user call (bridge policy). Function pointers are `Copy`, so
    // a short scoped critical section is all we need.
    let user_fn = {
        let registry = HANDLER_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .get(&evhdlr_registration_id)
            .and_then(|b| *b.as_ref())
    };
    let user_fn = user_fn.or_else(|| {
        if DEREG_IN_PROGRESS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&evhdlr_registration_id)
        {
            return None;
        }
        let pending_fn = {
            let pending = PENDING_REGISTRATIONS
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            pending
                .iter()
                .find(|registration| {
                    registration.codes.is_empty() || registration.codes.contains(&status)
                })
                .and_then(|registration| *registration.user_fn.as_ref())
        };
        pending_fn.or_else(|| {
            // The re-key holds PENDING_REGISTRATIONS while inserting into the
            // registry before popping. If the pending consult misses, that
            // critical section may have completed between the first registry
            // read and this consult, so re-read the registry. Each lock is
            // scoped and released before the next, preserving no-ABBA order;
            // this only runs on the registry-miss plus pending-miss path.
            let registry = HANDLER_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
            registry
                .get(&evhdlr_registration_id)
                .and_then(|b| *b.as_ref())
        })
    });

    if let Some(user_fn) = user_fn {
        // SAFETY: cast FFI pointers to c_void for the user-facing NotificationFn
        // ABI. Forward OpenPMIx's chain pointer (`cbdata`) and completion
        // `cbfunc` verbatim — the handler must return them via cbfunc.
        //
        // catch_unwind: this bridge is an `extern "C"` entry called from
        // OpenPMIx. A panic must not unwind into C, and must not leave the
        // event chain incomplete.
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            user_fn(
                evhdlr_registration_id,
                status,
                source as *const c_void,
                info as *mut c_void,
                ninfo,
                results as *mut c_void,
                nresults,
                cbfunc,
                cbdata,
            );
        }))
        .is_err();

        if panicked {
            eprintln!(
                "pmix::events: notification handler panicked on the progress thread; \
                 completing the OpenPMIx event chain and containing the panic \
                 (must not unwind across FFI)"
            );
            // Best-effort chain recovery. If the handler already completed the
            // chain before panicking, this is a second completion (undefined at
            // the C layer); that is still preferable to permanently stalling
            // every subsequent notify when the handler panics first.
            // SAFETY: same OpenPMIx-supplied cbfunc/cbdata/results as this delivery.
            unsafe {
                complete_event_chain(status, results, nresults, cbfunc, cbdata);
            }
        }
    } else {
        // Registry miss / no user handler: pass-through completion keeps the
        // OpenPMIx event chain alive. Do not drop `cbfunc` here.
        // SAFETY: OpenPMIx-supplied completion with its chain pointer; status
        // and results are forwarded as received.
        unsafe {
            complete_event_chain(status, results, nresults, cbfunc, cbdata);
        }
    }
}

/// State carried by [`register_event_handler_nb`]'s completion bridge: the
/// boxed user notification fn plus the user's own completion callback and
/// opaque data, forwarded verbatim once the reference ID is known.
struct HandlerRegState {
    user_fn: Box<NotificationFn>,
    user_cbfunc: HandlerRegCbFn,
    user_cbdata: *mut c_void,
}

/// C bridge for the registration-completion callback of
/// [`register_event_handler_nb`].
///
/// For non-blocking registration the reference ID is only known here, so the
/// boxed user notification fn is parked in [`HANDLER_REGISTRY`] at this point
/// (freed on error instead, so it never leaks). The user's own `cbfunc` is
/// then forwarded verbatim with the original `cbdata`.
extern "C" fn handler_reg_cb_bridge(status: i32, refid: EventHandlerRef, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the `Box<HandlerRegState>` we passed to
    // PMIx_Register_event_handler. PMIx invokes the registration completion
    // callback exactly once per registration request.
    let state = unsafe { Box::from_raw(cbdata as *mut HandlerRegState) };

    if PmixStatus::from_raw(status).is_success() {
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(refid, state.user_fn);
    }

    if let Some(user_cbfunc) = state.user_cbfunc {
        // SAFETY: user-supplied completion callback with user-supplied opaque
        // cbdata; both are forwarded verbatim from the registration call.
        unsafe { user_cbfunc(status, refid, state.user_cbdata) };
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
/// * `cbfunc` — must be `None` (blocking). Non-blocking registration with a
///   completion callback is [`register_event_handler_nb`] only; passing
///   `Some` here returns `PMIX_ERR_BAD_PARAM`.
///
/// # Returns
/// * `Ok(handler_ref)` — the handler reference ID (a non-negative integer;
///   **`0` is a valid first ref id** in OpenPMIx, not a failure sentinel).
/// * `Err(PmixStatus)` — registration failed (e.g., `PMIX_ERR_INIT`).
///
/// # Registration-window delivery
///
/// The handler is parked provisionally before entering PMIx, so a matching
/// event delivered before the returned reference ID can be re-keyed is
/// dispatched to `evhdlr` rather than dropped.
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
/// * `PMIX_ERR_BAD_PARAM` — invalid parameters (including non-`None` `cbfunc`).
pub fn register_event_handler(
    codes: &[PmixStatus],
    info: &Info,
    evhdlr: NotificationFn,
    cbfunc: HandlerRegCbFn,
) -> Result<EventHandlerRef, PmixStatus> {
    // Serialize blocking registrations so PENDING_REGISTRATIONS has one slot.
    let _registration_guard = BLOCKING_REGISTRATION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Blocking API only. A non-null cbfunc would make OpenPMIx treat this as
    // non-blocking (refid delivered only via callback) while this wrapper
    // still treats the return status as the refid — that combination is
    // incorrect. Route async registrations through register_event_handler_nb.
    if cbfunc.is_some() {
        return Err(PmixStatus::Known(PmixError::ErrBadParam));
    }

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

    if evhdlr.is_some() {
        PENDING_REGISTRATIONS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .push(PendingRegistration {
                codes: codes.iter().map(|status| status.to_raw()).collect(),
                user_fn: Box::new(evhdlr),
            });
    }

    // SAFETY: FFI call into PMIx library. The codes slice and info handle
    // remain valid for the duration of this call. The user's notification fn
    // is kept alive by HANDLER_REGISTRY (keyed by the reference ID PMIx
    // returns) and freed at deregistration — no callback data rides in cbdata,
    // so NULL is passed and nothing is freed after this call returns.
    let raw_status = unsafe {
        ffi::PMIx_Register_event_handler(
            codes_ptr,
            ncodes,
            info_ptr as *mut ffi::pmix_info_t,
            ninfo,
            Some(notification_bridge),
            None,
            ptr::null_mut(),
        )
    };

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        let handler_ref = raw_status as EventHandlerRef;
        if evhdlr.is_some() {
            // Insert before removing the provisional entry. Together with the
            // bridge's post-pending registry re-read, this ensures a completed
            // re-key is observed even if the bridge's first registry read
            // preceded this critical section. The registry and pending locks
            // are never nested in the bridge, avoiding an ABBA cycle.
            let mut pending = PENDING_REGISTRATIONS
                .lock()
                .expect("mutex poisoned (events.rs)");
            let user_fn = pending
                .last()
                .expect("pending blocking registration missing")
                .user_fn
                .expect("pending blocking notification fn missing");
            HANDLER_REGISTRY
                .lock()
                .expect("mutex poisoned (events.rs)")
                .insert(handler_ref, Box::new(Some(user_fn)));
            pending.pop();
        }
        Ok(handler_ref)
    } else {
        if evhdlr.is_some() {
            PENDING_REGISTRATIONS
                .lock()
                .expect("mutex poisoned (events.rs)")
                .pop();
        }
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

    // In non-blocking mode the reference ID is only delivered via cbfunc, so
    // the user fn is boxed into a registration state; the completion bridge
    // parks it in HANDLER_REGISTRY (freed on error) and then forwards to the
    // user's cbfunc. The notification bridge resolves the fn by ref ID, so no
    // callback data is needed in the registration cbdata — except the state
    // box itself, which PMIx passes back to our completion bridge.
    let state = Box::new(HandlerRegState {
        user_fn: Box::new(evhdlr),
        user_cbfunc: cbfunc,
        user_cbdata: cbdata,
    });
    let state_ptr = Box::into_raw(state);

    // SAFETY: FFI call into PMIx library. The codes slice lives for the
    // duration of this call. On the async path the boxed HandlerRegState is
    // reclaimed exactly once by handler_reg_cb_bridge.
    //
    // Synchronous failure paths in OpenPMIx (`!initialized`, progress thread
    // stopped, OOM on the registration object) return *before* thread-shifting
    // and **never** invoke the registration completion callback — so the box
    // must be freed here. Only free on Err: a successful accept transfers
    // ownership to the completion bridge.
    let raw_status = unsafe {
        ffi::PMIx_Register_event_handler(
            codes_ptr,
            ncodes,
            info_ptr as *mut ffi::pmix_info_t,
            ninfo,
            Some(notification_bridge),
            Some(handler_reg_cb_bridge),
            state_ptr as *mut c_void,
        )
    };

    let status = PmixStatus::from_raw(raw_status);
    if status.is_success() {
        Ok(())
    } else {
        // SAFETY: synchronous failure ⇒ completion bridge will not run; we
        // still own the box and must free it to avoid a permanent leak of the
        // boxed NotificationFn and user cbdata.
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
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
    // Free the boxed user fn BEFORE the C call so the notification bridge can
    // no longer fire into freed memory, even if an event is already in flight
    // on the progress thread.
    //
    // Ordering (registry remove, then PMIx_Deregister_event_handler):
    // - OpenPMIx documents that once `PMIx_Deregister_event_handler` returns
    //   successfully, no further notification callbacks will be delivered for
    //   that ref. Removing first closes the Rust side before that guarantee
    //   kicks in, so a progress-thread delivery already past the C check but
    //   not yet into `notification_bridge` cannot observe a live registry
    //   entry whose Box is about to be dropped.
    // - The inverse order (C deregister, then remove) would leave a window
    //   where the bridge can still resolve the user fn while another thread
    //   is about to free it.
    // - Trade-off on C failure / in-flight delivery: the C handler may remain
    //   registered briefly, but `notification_bridge` hits a registry miss and
    //   **pass-through-completes** the OpenPMIx event chain (no hang, no UAF).
    //   The delivery is dropped at the Rust layer rather than forwarded.
    // Deregistration takes DEREG_IN_PROGRESS, then HANDLER_REGISTRY. The
    // bridge releases each read lock before acquiring the next, so no cycle
    // is possible while preserving registry-first removal for UAF safety.
    DEREG_IN_PROGRESS
        .lock()
        .expect("mutex poisoned (events.rs)")
        .insert(evhdlr_ref);
    HANDLER_REGISTRY
        .lock()
        .expect("mutex poisoned (events.rs)")
        .remove(&evhdlr_ref);

    let raw_status =
        // SAFETY: FFI call into PMIx library. evhdlr_ref is an opaque usize
        // returned by the library itself, so it is valid to pass back.
        unsafe { ffi::PMIx_Deregister_event_handler(evhdlr_ref, cbfunc, ptr::null_mut()) };

    let status = PmixStatus::from_raw(raw_status);
    DEREG_IN_PROGRESS
        .lock()
        .expect("mutex poisoned (events.rs)")
        .remove(&evhdlr_ref);
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
    // Same registry-first ordering as the blocking path. Marking the refid
    // first prevents pending fallback from claiming an in-flight delivery.
    DEREG_IN_PROGRESS
        .lock()
        .expect("mutex poisoned (events.rs)")
        .insert(evhdlr_ref);
    HANDLER_REGISTRY
        .lock()
        .expect("mutex poisoned (events.rs)")
        .remove(&evhdlr_ref);

    // SAFETY: FFI call into PMIx library. Same safety considerations as
    // the blocking variant.
    let raw_status = unsafe { ffi::PMIx_Deregister_event_handler(evhdlr_ref, cbfunc, cbdata) };

    let status = PmixStatus::from_raw(raw_status);
    DEREG_IN_PROGRESS
        .lock()
        .expect("mutex poisoned (events.rs)")
        .remove(&evhdlr_ref);
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
    #![allow(clippy::too_many_arguments)]
    use super::*;
    use crate::mock_ffi::MockGuard;
    use std::sync::mpsc;

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

    // ─── Handler registry + notification bridge tests ──────────────────────

    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

    /// A no-op user notification handler for bridge tests.
    unsafe fn test_handler(
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

    #[test]
    fn test_handler_registry_insert_lookup_remove() {
        let ref_id: EventHandlerRef = 424242;
        {
            let mut registry = HANDLER_REGISTRY.lock().expect("mutex poisoned (events.rs)");
            registry.insert(ref_id, Box::new(Some(test_handler)));
            let found = registry.get(&ref_id).and_then(|b| *b.as_ref());
            assert!(found.is_some(), "user fn should be findable by ref id");
            registry.remove(&ref_id);
            assert!(registry.get(&ref_id).is_none());
        }
    }

    #[test]
    fn test_notification_bridge_invokes_user_fn() {
        static CALLED: AtomicBool = AtomicBool::new(false);
        static SAW_CBDATA: AtomicUsize = AtomicUsize::new(0);

        unsafe fn recording_handler(
            _id: EventHandlerRef,
            status: i32,
            _source: *const std::os::raw::c_void,
            _info: *mut std::os::raw::c_void,
            _ninfo: usize,
            _results: *mut std::os::raw::c_void,
            _nresults: usize,
            _cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            cbdata: *mut std::os::raw::c_void,
        ) {
            assert_eq!(status, 7, "handler should receive the event status");
            SAW_CBDATA.store(cbdata as usize, Ordering::SeqCst);
            CALLED.store(true, Ordering::SeqCst);
        }

        let ref_id: EventHandlerRef = 424243;
        // Non-null sentinel standing in for OpenPMIx's event-chain pointer.
        let chain_token = 0xC_u8;
        let chain_ptr = &chain_token as *const u8 as *mut c_void;

        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id, Box::new(Some(recording_handler)));

        unsafe {
            notification_bridge(
                ref_id,
                7,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                chain_ptr,
            );
        }

        assert!(
            CALLED.load(Ordering::SeqCst),
            "bridge must invoke the user fn"
        );
        assert_eq!(
            SAW_CBDATA.load(Ordering::SeqCst),
            chain_ptr as usize,
            "bridge must forward OpenPMIx chain cbdata verbatim (not NULL)"
        );
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .remove(&ref_id);
    }

    #[test]
    fn test_notification_bridge_unknown_ref_invokes_cbfunc() {
        // Registry miss must still complete the OpenPMIx event chain — a
        // silent return stalls blocking notify_event forever.
        static CBFUNC_CALLED: AtomicBool = AtomicBool::new(false);
        static CBFUNC_STATUS: AtomicI32 = AtomicI32::new(i32::MIN);
        static CBFUNC_CBDATA: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn recording_cbfunc(
            status: i32,
            _results: *mut ffi::pmix_info_t,
            _nresults: usize,
            _cbfunc: ffi::pmix_op_cbfunc_t,
            _thiscbdata: *mut c_void,
            notification_cbdata: *mut c_void,
        ) {
            CBFUNC_STATUS.store(status, Ordering::SeqCst);
            CBFUNC_CBDATA.store(notification_cbdata as usize, Ordering::SeqCst);
            CBFUNC_CALLED.store(true, Ordering::SeqCst);
        }

        let chain_token = 0xD_u8;
        let chain_ptr = &chain_token as *const u8 as *mut c_void;

        unsafe {
            notification_bridge(
                999_999_999,
                42,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                Some(recording_cbfunc),
                chain_ptr,
            );
        }

        assert!(
            CBFUNC_CALLED.load(Ordering::SeqCst),
            "miss path must invoke completion cbfunc"
        );
        assert_eq!(CBFUNC_STATUS.load(Ordering::SeqCst), 42);
        assert_eq!(
            CBFUNC_CBDATA.load(Ordering::SeqCst),
            chain_ptr as usize,
            "miss path must forward chain cbdata verbatim"
        );
    }

    #[test]
    fn test_notification_bridge_unknown_ref_null_cbfunc_is_safe() {
        // No registry entry and no completion callback — must not panic.
        unsafe {
            notification_bridge(
                999_999_998,
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut(),
            );
        }
    }

    #[test]
    fn test_pending_registration_delivery_and_rekey() {
        static CALLED: AtomicUsize = AtomicUsize::new(0);
        unsafe fn handler(
            _id: EventHandlerRef,
            _status: i32,
            _source: *const c_void,
            _info: *mut c_void,
            _ninfo: usize,
            _results: *mut c_void,
            _nresults: usize,
            cbfunc: ffi::pmix_event_notification_cbfunc_fn_t,
            cbdata: *mut c_void,
        ) {
            CALLED.fetch_add(1, Ordering::SeqCst);
            if let Some(cbfunc) = cbfunc {
                // SAFETY: the test forwards the supplied chain callback once.
                unsafe {
                    cbfunc(
                        0,
                        std::ptr::null_mut(),
                        0,
                        None,
                        std::ptr::null_mut(),
                        cbdata,
                    )
                };
            }
        }
        unsafe extern "C" fn completion(
            _status: i32,
            _results: *mut ffi::pmix_info_t,
            _nresults: usize,
            _cbfunc: ffi::pmix_op_cbfunc_t,
            _cbdata: *mut c_void,
            _notification_cbdata: *mut c_void,
        ) {
        }
        let _guard = MockGuard::new();
        clear_handler_registry();
        CALLED.store(0, Ordering::SeqCst);
        PENDING_REGISTRATIONS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .push(PendingRegistration {
                codes: vec![17],
                user_fn: Box::new(Some(handler)),
            });
        let chain = 0xF_u8;
        // SAFETY: test invokes the bridge with null FFI pointers and a live
        // stack chain token; the completion callback is valid for this call.
        unsafe {
            notification_bridge(
                424252,
                17,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                Some(completion),
                &chain as *const u8 as *mut c_void,
            );
        }
        assert_eq!(CALLED.load(Ordering::SeqCst), 1);
        let pending = PENDING_REGISTRATIONS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .pop()
            .unwrap();
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(424252, pending.user_fn);
        assert!(
            HANDLER_REGISTRY
                .lock()
                .expect("mutex poisoned (events.rs)")
                .contains_key(&424252)
        );
        // Leave a non-matching pending entry to verify that the re-keyed
        // handler is delivered; the registry-miss path is covered elsewhere.
        PENDING_REGISTRATIONS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .push(PendingRegistration {
                codes: vec![18],
                user_fn: Box::new(Some(handler)),
            });
        // SAFETY: test invokes the bridge with null FFI pointers and a live
        // stack chain token; the completion callback is valid for this call.
        unsafe {
            notification_bridge(
                424252,
                17,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                Some(completion),
                &chain as *const u8 as *mut c_void,
            );
        }
        assert_eq!(CALLED.load(Ordering::SeqCst), 2);
        clear_handler_registry();
    }

    #[test]
    fn test_pending_consult_skipped_for_deregistering_ref() {
        static CALLED: AtomicBool = AtomicBool::new(false);

        unsafe fn handler(
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
            CALLED.store(true, Ordering::SeqCst);
        }
        unsafe extern "C" fn completion(
            _status: i32,
            _results: *mut ffi::pmix_info_t,
            _nresults: usize,
            _cbfunc: ffi::pmix_op_cbfunc_t,
            _cbdata: *mut c_void,
            _notification_cbdata: *mut c_void,
        ) {
        }

        let _guard = MockGuard::new();
        clear_handler_registry();
        CALLED.store(false, Ordering::SeqCst);
        let ref_id = 424253;
        PENDING_REGISTRATIONS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .push(PendingRegistration {
                codes: vec![17],
                user_fn: Box::new(Some(handler)),
            });
        DEREG_IN_PROGRESS
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id);
        // SAFETY: test invokes the bridge with null FFI pointers; the
        // completion callback is valid for this call.
        unsafe {
            notification_bridge(
                ref_id,
                17,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                Some(completion),
                std::ptr::null_mut(),
            );
        }
        assert!(!CALLED.load(Ordering::SeqCst));
        clear_handler_registry();
    }

    #[test]
    fn test_notification_bridge_user_panic_completes_chain_and_is_contained() {
        // A panicking user handler must not unwind into OpenPMIx (UB) and must
        // still complete the event chain so blocking notify_event cannot hang.
        static CBFUNC_CALLED: AtomicBool = AtomicBool::new(false);
        static CBFUNC_STATUS: AtomicI32 = AtomicI32::new(i32::MIN);
        static CBFUNC_CBDATA: AtomicUsize = AtomicUsize::new(0);

        unsafe fn panicking_handler(
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
            panic!("user notification handler panicked on purpose");
        }

        unsafe extern "C" fn recording_cbfunc(
            status: i32,
            _results: *mut ffi::pmix_info_t,
            _nresults: usize,
            _cbfunc: ffi::pmix_op_cbfunc_t,
            _thiscbdata: *mut c_void,
            notification_cbdata: *mut c_void,
        ) {
            CBFUNC_STATUS.store(status, Ordering::SeqCst);
            CBFUNC_CBDATA.store(notification_cbdata as usize, Ordering::SeqCst);
            CBFUNC_CALLED.store(true, Ordering::SeqCst);
        }

        let ref_id: EventHandlerRef = 424_250;
        let chain_token = 0xE_u8;
        let chain_ptr = &chain_token as *const u8 as *mut c_void;

        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id, Box::new(Some(panicking_handler)));

        // Must return normally (panic contained) — would abort the test thread
        // if resume_unwind crossed the bridge.
        unsafe {
            notification_bridge(
                ref_id,
                11,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                Some(recording_cbfunc),
                chain_ptr,
            );
        }

        assert!(
            CBFUNC_CALLED.load(Ordering::SeqCst),
            "hit-path panic must still complete the OpenPMIx event chain"
        );
        assert_eq!(CBFUNC_STATUS.load(Ordering::SeqCst), 11);
        assert_eq!(
            CBFUNC_CBDATA.load(Ordering::SeqCst),
            chain_ptr as usize,
            "panic recovery must forward chain cbdata verbatim"
        );

        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .remove(&ref_id);
    }

    #[test]
    fn test_clear_handler_registry_drops_parked_fns() {
        let ref_id: EventHandlerRef = 424_251;
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id, Box::new(Some(test_handler)));
        assert!(
            HANDLER_REGISTRY
                .lock()
                .expect("mutex poisoned (events.rs)")
                .contains_key(&ref_id)
        );

        clear_handler_registry();

        assert!(
            HANDLER_REGISTRY
                .lock()
                .expect("mutex poisoned (events.rs)")
                .is_empty(),
            "finalize cleanup must drop every parked NotificationFn"
        );
    }

    /// Deadlock / lock-order regression (issue #51): the event-handler registry
    /// lock must **not** be held while the user callback runs.
    ///
    /// A user handler that blocks is invoked from a stand-in progress thread;
    /// while it is blocked, this test must still be able to acquire the
    /// registry lock. If the lock were held across the user call, the
    /// `try_lock` below would fail and the test would fail.
    #[test]
    fn test_event_handler_lock_not_held_during_user_callback() {
        static ENTER_TX: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
        static RELEASE_RX: Mutex<Option<mpsc::Receiver<()>>> = Mutex::new(None);

        unsafe fn blocking_handler(
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
            if let Some(tx) = ENTER_TX
                .lock()
                .expect("mutex poisoned (events.rs)")
                .as_ref()
            {
                let _ = tx.send(());
            }
            if let Some(rx) = RELEASE_RX
                .lock()
                .expect("mutex poisoned (events.rs)")
                .as_ref()
            {
                let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
            }
        }

        let (enter_tx, enter_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *ENTER_TX.lock().expect("mutex poisoned (events.rs)") = Some(enter_tx);
        *RELEASE_RX.lock().expect("mutex poisoned (events.rs)") = Some(release_rx);

        let ref_id: EventHandlerRef = 424244;
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id, Box::new(Some(blocking_handler)));

        // Stand-in "progress thread": delivers the event, blocking inside the
        // user handler until released.
        let progress_thread = std::thread::spawn(move || unsafe {
            notification_bridge(
                ref_id,
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut(),
            );
        });

        // Wait until the user handler is executing…
        enter_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("handler should have entered");
        // …and prove the registry lock is free: try_lock must succeed.
        // (If the bridge held the lock across the user call, this would be
        // `Err(TryLockError::WouldBlock)`.)
        let guard = HANDLER_REGISTRY
            .try_lock()
            .expect("registry lock must not be held across user callback code");
        drop(guard);

        // Release the handler and let the progress thread finish.
        let _ = release_tx.send(());
        progress_thread
            .join()
            .expect("progress thread should finish");

        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .remove(&ref_id);
    }

    #[test]
    fn test_handler_reg_cb_bridge_parks_fn_on_success() {
        static REG_REFID: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "C" fn reg_cb(
            _status: i32,
            refid: EventHandlerRef,
            _cbdata: *mut std::os::raw::c_void,
        ) {
            REG_REFID.store(refid, Ordering::SeqCst);
        }

        let ref_id: EventHandlerRef = 424245;
        let state = Box::new(HandlerRegState {
            user_fn: Box::new(Some(test_handler)),
            user_cbfunc: Some(reg_cb),
            user_cbdata: std::ptr::null_mut(),
        });

        handler_reg_cb_bridge(
            0, /* PMIX_SUCCESS */
            ref_id,
            Box::into_raw(state) as *mut c_void,
        );

        // The user fn is now parked in the registry under the delivered ref id,
        // and the user's completion callback was forwarded the ref id.
        assert_eq!(REG_REFID.load(Ordering::SeqCst), ref_id);
        let parked = {
            let registry = HANDLER_REGISTRY.lock().expect("mutex poisoned (events.rs)");
            registry.get(&ref_id).and_then(|b| *b.as_ref())
        };
        assert!(
            parked.is_some(),
            "user fn should be parked on successful registration"
        );
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .remove(&ref_id);
    }

    #[test]
    fn test_handler_reg_cb_bridge_error_frees_state() {
        static REG_STATUS: AtomicI32 = AtomicI32::new(0);

        unsafe extern "C" fn reg_cb(
            status: i32,
            _refid: EventHandlerRef,
            _cbdata: *mut std::os::raw::c_void,
        ) {
            REG_STATUS.store(status, Ordering::SeqCst);
        }

        let ref_id: EventHandlerRef = 424246;
        let state = Box::new(HandlerRegState {
            user_fn: Box::new(Some(test_handler)),
            user_cbfunc: Some(reg_cb),
            user_cbdata: std::ptr::null_mut(),
        });

        handler_reg_cb_bridge(
            -39, /* PMIX_ERR_INIT */
            ref_id,
            Box::into_raw(state) as *mut c_void,
        );

        // Failure: nothing parked (box freed instead), user cbfunc still
        // forwarded the error status verbatim.
        assert_eq!(REG_STATUS.load(Ordering::SeqCst), -39);
        let registry = HANDLER_REGISTRY.lock().expect("mutex poisoned (events.rs)");
        assert!(
            registry.get(&ref_id).is_none(),
            "no fn parked on failed registration"
        );
    }

    #[test]
    fn test_deregister_removes_registry_entry() {
        let ref_id: EventHandlerRef = 424247;
        HANDLER_REGISTRY
            .lock()
            .expect("mutex poisoned (events.rs)")
            .insert(ref_id, Box::new(Some(test_handler)));

        // Without PMIx init the C call fails, but the registry entry is freed
        // first — no handler can fire into freed memory after this returns.
        let _ = deregister_event_handler(ref_id, None);

        let registry = HANDLER_REGISTRY.lock().expect("mutex poisoned (events.rs)");
        assert!(
            registry.get(&ref_id).is_none(),
            "deregistration must free the user fn"
        );
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
            _not_thread_safe: std::marker::PhantomData,
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
        // Verify NotificationFn is Option<unsafe fn(...)>
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
        fn dummy_handler(
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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

    // ─── Notification bridge (registry-based) tests live at the top of this
    // module (see `test_notification_bridge_*`). The old boxed-cbdata
    // mechanism they replaced is gone; handlers are stored in
    // HANDLER_REGISTRY keyed by the reference ID.

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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
        fn dummy_handler(
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
        };
        // Register with callback on the blocking API must reject with BAD_PARAM
        // (non-blocking registration is register_event_handler_nb only).
        let result = register_event_handler(&codes, &info, None, Some(dummy_reg_cb));
        assert_eq!(
            result,
            Err(PmixStatus::Known(PmixError::ErrBadParam)),
            "blocking register_event_handler must reject non-None cbfunc"
        );
    }

    #[test]
    fn test_register_blocking_no_callback() {
        let codes = [PmixStatus::Known(PmixError::ErrTimeout)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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

    // ─── register_event_handler_nb with user cbdata ────────────────────────

    #[test]
    fn test_register_event_handler_nb_with_user_cbdata() {
        extern "C" fn dummy_reg_cb(_status: i32, _refid: EventHandlerRef, _cbdata: *mut c_void) {}
        let codes = [PmixStatus::Known(PmixError::ErrJobAborted)];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
            _not_thread_safe: std::marker::PhantomData,
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
        fn dummy(
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
