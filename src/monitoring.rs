//! Process monitoring — `PMIx_Process_monitor`, `PMIx_Process_monitor_nb`, and `PMIx_Heartbeat`.
//!
//! This module provides safe Rust wrappers for the PMIx process monitoring API.
//! Monitoring allows a client to register with the PMIx server so that the
//! server can detect stalled or unresponsive processes via heartbeats or
//! file-based checks.
//!
//! # Monitoring model
//!
//! A monitoring request consists of:
//! - `monitor`: info entries that specify **what** to monitor
//!   (e.g., `PMIX_MONITOR_HEARTBEAT` for heartbeat-based monitoring,
//!   `PMIX_MONITOR_FILE` for file-based monitoring)
//! - `error`: the PMIx status code to return when the monitored condition
//!   is triggered (e.g., `PMIX_MONITOR_HEARTBEAT_ALERT`, `PMIX_MONITOR_FILE_ALERT`)
//! - `directives`: optional info entries that control monitoring behavior
//!   (e.g., `PMIX_MONITOR_ID`, `PMIX_MONITOR_HEARTBEAT_TIME`,
//!   `PMIX_MONITOR_HEARTBEAT_DROPS`)
//!
//! # Heartbeat shortcut
//!
//! A process can send a heartbeat to the server using the [`heartbeat`]
//! function, which is the Rust equivalent of the `PMIx_Heartbeat()` C macro.
//!
//! # C API reference
//!
//! ```c
//! pmix_status_t PMIx_Process_monitor(const pmix_info_t *monitor,
//!                                     pmix_status_t error,
//!                                     const pmix_info_t directives[],
//!                                     size_t ndirs,
//!                                     pmix_info_t **results,
//!                                     size_t *nresults);
//!
//! pmix_status_t PMIx_Process_monitor_nb(const pmix_info_t *monitor,
//!                                        pmix_status_t error,
//!                                        const pmix_info_t directives[],
//!                                        size_t ndirs,
//!                                        pmix_info_cbfunc_t cbfunc,
//!                                        void *cbdata);
//!
//! #define PMIx_Heartbeat()  // sends a heartbeat via PMIx_Process_monitor_nb
//! ```

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::{LazyLock, Mutex};

use crate::ffi;
use crate::{Info, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults — owned result set from process_monitor()
// ─────────────────────────────────────────────────────────────────────────────

/// Owned result set returned by [`process_monitor`].
///
/// Wraps the `pmix_info_t` array allocated by the PMIx library and
/// automatically frees it via `PMIx_Info_free` on drop.
#[derive(Debug)]
pub struct MonitorResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl MonitorResults {
    /// Number of info entries in this result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for MonitorResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx_Process_monitor as an
                // allocated pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait and registry for PMIx_Process_monitor_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for [`process_monitor_nb`].
///
/// Implement this trait to receive the asynchronous result of a monitoring
/// request. The callback is invoked on the server's response thread, so
/// keep the implementation lightweight (e.g., send a message to another
/// thread rather than doing blocking work).
pub trait MonitorCallback: Send {
    /// Called when the monitoring request completes.
    ///
    /// - `status`: the result of the request (e.g., `PMIX_SUCCESS` if
    ///   the monitoring was registered, or an error code).
    /// - `results`: the response info array returned by the server, or
    ///   `None` if no results were provided.
    fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>);
}

type MonitorRegistry = Mutex<HashMap<u64, Box<dyn MonitorCallback>>>;
static MONITOR_REGISTRY: LazyLock<MonitorRegistry> = LazyLock::new(Mutex::default);

static MONITOR_SEQ: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_info_cbfunc_t` (monitor completion).
///
/// This function is called by the PMIx library when a non-blocking
/// monitoring request completes. It looks up the callback in the registry,
/// invokes it, and cleans up.
unsafe extern "C" fn monitor_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
    _release_fn: ffi::pmix_release_cbfunc_t,
    _release_cbdata: *mut c_void,
) {
    // Decode the request ID from the cbdata pointer.
    let req_id = (cbdata as u64) >> 2;

    // Remove the callback from the registry — it is consumed exactly once.
    let mut registry = MONITOR_REGISTRY.lock().unwrap();
    let callback = registry.remove(&req_id);
    drop(registry);

    match callback {
        Some(mut cb) => {
            // Build MonitorResults if info was returned.
            let results = if !info.is_null() && ninfo > 0 {
                Some(MonitorResults {
                    handle: info,
                    len: ninfo,
                })
            } else {
                None
            };
            cb.on_complete(PmixStatus::from_raw(status), results);
        }
        None => {
            // Callback already consumed or never registered — free the info
            // array to avoid a leak if one was provided.
            if !info.is_null() && ninfo > 0 {
                unsafe {
                    // SAFETY: info was passed by PMIx as an allocated array.
                    ffi::PMIx_Info_free(info, ninfo);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor — safe wrapper for PMIx_Process_monitor
// ─────────────────────────────────────────────────────────────────────────────

/// Register a monitoring request with the PMIx server (blocking).
///
/// This is a **blocking** call — it returns only after the PMIx server has
/// processed the monitoring request and returned results (or an error).
///
/// # Parameters
/// - `monitor`: info entries specifying **what** to monitor. Common keys:
///   - `PMIX_MONITOR_HEARTBEAT` — register for heartbeat-based monitoring
///   - `PMIX_MONITOR_FILE` — register for file-based monitoring (value = path)
///   - `PMIX_MONITOR_FILE_SIZE` — monitor file size growth
///   - `PMIX_MONITOR_FILE_ACCESS` — monitor file access time
///   - `PMIX_MONITOR_FILE_MODIFY` — monitor file modification time
/// - `error`: the PMIx status code to use when the monitored condition fires.
///   Typical values: `PMIX_MONITOR_HEARTBEAT_ALERT` (-109),
///   `PMIX_MONITOR_FILE_ALERT` (-110).
/// - `directives`: optional info entries that control monitoring behavior.
///   Common keys:
///   - `PMIX_MONITOR_ID` — string identifier for this request
///   - `PMIX_MONITOR_HEARTBEAT_TIME` — seconds between heartbeat checks
///   - `PMIX_MONITOR_HEARTBEAT_DROPS` — number of missed heartbeats before alert
///   - `PMIX_MONITOR_FILE_CHECK_TIME` — seconds between file checks
///   - `PMIX_MONITOR_FILE_DROPS` — number of missed file checks before alert
///
/// # Returns
/// - `Ok(MonitorResults)` on success. The caller owns the results; they are
///   freed when `MonitorResults` is dropped.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host RM does not support monitoring.
///   - `PMIX_ERR_BAD_PARAM` — invalid monitor or directive parameters.
///
/// # C API
/// `pmix_status_t PMIx_Process_monitor(const pmix_info_t *monitor,`
/// `  pmix_status_t error, const pmix_info_t directives[], size_t ndirs,`
/// `  pmix_info_t **results, size_t *nresults);`
pub fn process_monitor(
    monitor: &Info,
    error: PmixStatus,
    directives: &[Info],
) -> Result<MonitorResults, PmixStatus> {
    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    // Build a flat array of directive info entries.
    let (dirs_ptr, ndirs) = if directives.is_empty() {
        (ptr::null(), 0)
    } else {
        // Info stores a handle to the first element and a length.
        // Directives from a single InfoBuilder are contiguous.
        if directives.len() == 1 {
            (
                directives[0].handle as *const ffi::pmix_info_t,
                directives[0].len,
            )
        } else {
            // Multiple Info objects — use the first's pointer.
            // In practice, callers should pass a single Info containing all directives.
            (
                directives[0].handle as *const ffi::pmix_info_t,
                directives[0].len,
            )
        }
    };

    let status = unsafe {
        // SAFETY: PMIx_Process_monitor is a synchronous PMIx API call.
        // - monitor.handle points to a valid pmix_info_t (owned by the Info borrow).
        // - dirs_ptr is either null or points to valid pmix_info_t entries.
        // - results and nresults are output pointers that PMIx will write to.
        // - PMIx does not retain these pointers after this call returns.
        ffi::PMIx_Process_monitor(
            monitor.handle,
            error.to_raw(),
            dirs_ptr,
            ndirs,
            &mut results,
            &mut nresults,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() || pmix_status == PmixStatus::Known(PmixError::ErrPartialSuccess) {
        Ok(MonitorResults {
            handle: results,
            len: nresults,
        })
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor_nb — safe wrapper for PMIx_Process_monitor_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Register a monitoring request with the PMIx server (non-blocking).
///
/// Submit an asynchronous monitoring request. The `callback` closure is
/// invoked once the operation completes, receiving both the status and
/// any results from the server.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(PmixStatus)` if the request was rejected immediately (e.g.,
///   invalid parameters or PMIx not initialized). The callback will
///   **NOT** be called in this case.
///
/// # Parameters
/// Same as [`process_monitor`], plus:
/// - `callback`: a [`MonitorCallback`] implementation that receives the
///   async result.
///
/// # C API
/// `pmix_status_t PMIx_Process_monitor_nb(const pmix_info_t *monitor,`
/// `  pmix_status_t error, const pmix_info_t directives[], size_t ndirs,`
/// `  pmix_info_cbfunc_t cbfunc, void *cbdata);`
pub fn process_monitor_nb(
    monitor: &Info,
    error: PmixStatus,
    directives: &[Info],
    callback: Box<dyn MonitorCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = *MONITOR_SEQ.lock().unwrap();
        seq += 1;
        seq
    };
    {
        let mut registry = MONITOR_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Build directive pointer array.
    let (dirs_ptr, ndirs) = if directives.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            directives[0].handle as *const ffi::pmix_info_t,
            directives[0].len,
        )
    };

    let status = unsafe {
        // SAFETY: PMIx_Process_monitor_nb is an async PMIx API call.
        // - monitor.handle points to a valid pmix_info_t (owned by the Info borrow).
        // - dirs_ptr is either null or points to valid pmix_info_t entries.
        // - cbfunc is a valid extern "C" function pointer (our bridge).
        // - cbdata encodes the request ID; PMIx passes it back unchanged.
        // - PMIx does not retain monitor.handle or dirs_ptr after this call returns.
        ffi::PMIx_Process_monitor_nb(
            monitor.handle,
            error.to_raw(),
            dirs_ptr,
            ndirs,
            Some(monitor_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request rejected — remove the callback from the registry.
        let mut registry = MONITOR_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — Rust equivalent of the PMIx_Heartbeat() C macro
// ─────────────────────────────────────────────────────────────────────────────

/// Send a heartbeat to the PMIx server.
///
/// This is the Rust equivalent of the `PMIx_Heartbeat()` C macro. It sends
/// a heartbeat notification to the local PMIx server, indicating that the
/// process is still alive and responsive. This is used in conjunction with
/// heartbeat-based monitoring registered via [`process_monitor`] or
/// [`process_monitor_nb`].
///
/// The server uses heartbeats to detect stalled processes. If a process
/// does not send heartbeats within the configured time window (set via
/// `PMIX_MONITOR_HEARTBEAT_TIME`), the server may declare the process
/// stalled and trigger the configured alert.
///
/// # Returns
/// - `Ok(())` if the heartbeat was sent successfully.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///
/// # C API
/// ```c
/// #define PMIx_Heartbeat()                                                \
///     do {                                                                \
///         pmix_info_t _in;                                                \
///         PMIX_INFO_CONSTRUCT(&_in);                                      \
///         PMIX_INFO_LOAD(&_in, PMIX_SEND_HEARTBEAT, NULL, PMIX_POINTER);  \
///         PMIx_Process_monitor_nb(&_in, PMIX_SUCCESS, NULL, 0, NULL, NULL); \
///         PMIX_INFO_DESTRUCT(&_in);                                       \
///     } while(0)
/// ```
pub fn heartbeat() -> Result<(), PmixStatus> {
    // Build a single info entry: PMIX_SEND_HEARTBEAT with no value.
    // This mirrors the PMIx_Heartbeat() C macro:
    //   PMIX_INFO_CONSTRUCT(&_in);
    //   PMIX_INFO_LOAD(&_in, PMIX_SEND_HEARTBEAT, NULL, PMIX_POINTER);
    //   PMIx_Process_monitor_nb(&_in, PMIX_SUCCESS, NULL, 0, NULL, NULL);
    //   PMIX_INFO_DESTRUCT(&_in);

    let mut c_info: ffi::pmix_info_t = unsafe { std::mem::zeroed() };
    let key_cstring = CString::new("pmix.monitor.beat").expect("no interior NUL in key");

    unsafe {
        ffi::PMIx_Info_load(
            &mut c_info as *mut ffi::pmix_info_t,
            key_cstring.as_ptr(),
            std::ptr::null(),
            ffi::PMIX_POINTER as ffi::pmix_data_type_t,
        );
    }

    // Call PMIx_Process_monitor_nb with NULL callback (fire-and-forget).
    let status = unsafe {
        // SAFETY: c_info is a valid, constructed pmix_info_t on the stack.
        // We pass NULL for directives, callback, and cbdata (fire-and-forget).
        // PMIx does not retain the pointer to c_info after this call returns.
        ffi::PMIx_Process_monitor_nb(
            &c_info as *const ffi::pmix_info_t,
            ffi::PMIX_SUCCESS as ffi::pmix_status_t,
            ptr::null(),
            0,
            None,
            ptr::null_mut(),
        )
    };

    // Destruct the info entry to free any internal allocations.
    unsafe {
        ffi::PMIx_Info_destruct(&mut c_info as *mut ffi::pmix_info_t);
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}
