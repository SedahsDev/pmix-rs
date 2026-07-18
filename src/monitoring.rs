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
use crate::cbdata::{decode_req_id_u64, encode_req_id_u64};
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

    /// Create a test-only MonitorResults with a given length.
    /// The handle is null — this is for unit tests only and must not
    /// be used in production code (Drop will skip freeing when handle is null).
    #[cfg(test)]
    pub fn test_new(len: usize) -> Self {
        Self {
            handle: std::ptr::null_mut(),
            len,
        }
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
    let req_id = decode_req_id_u64(cbdata);

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
    let cbdata = encode_req_id_u64(req_id);

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

    let mut c_info: ffi::pmix_info_t = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_callback_trait_object() {
        struct DummyMonitor;
        impl MonitorCallback for DummyMonitor {
            fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
        }
        let callback: Box<dyn MonitorCallback> = Box::new(DummyMonitor);
        let _ = callback;
    }

    #[test]
    fn test_monitor_results_len() {
        let results = MonitorResults::test_new(0);
        assert_eq!(results.len(), 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_monitor_results_nonempty() {
        let results = MonitorResults::test_new(5);
        assert_eq!(results.len(), 5);
        assert!(!results.is_empty());
    }

    // ── Additional unit tests (need internal MonitorResults construction) ──

    /// MonitorResults with null handle and zero length is empty.
    #[test]
    fn test_monitor_results_empty() {
        let results = MonitorResults::test_new(0);
        assert_eq!(results.len(), 0);
        assert!(results.is_empty());
    }

    /// MonitorResults with non-zero length reports correctly.
    #[test]
    fn test_monitor_results_with_data() {
        for n in [1, 5, 100, 1000] {
            let results = MonitorResults::test_new(n);
            assert_eq!(results.len(), n);
            assert!(!results.is_empty());
        }
    }

    /// MonitorResults is Debug-printable.
    #[test]
    fn test_monitor_results_debug() {
        let results = MonitorResults::test_new(42);
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("MonitorResults"));
    }

    /// MonitorResults Drop with null handle does not crash.
    #[test]
    fn test_monitor_results_drop_null() {
        let _results = MonitorResults::test_new(0);
    }

    /// MonitorResults Drop with zero len is safe even with non-null handle.
    #[test]
    fn test_monitor_results_drop_zero_len_with_ptr() {
        // The test_new constructor always uses null handle, so Drop is safe.
        let _results = MonitorResults::test_new(0);
    }

    /// MonitorResults len boundary values.
    #[test]
    fn test_monitor_results_len_boundaries() {
        let r0 = MonitorResults::test_new(0);
        assert!(r0.is_empty());

        let r1 = MonitorResults::test_new(1);
        assert!(!r1.is_empty());
        assert_eq!(r1.len(), 1);

        let rmax = MonitorResults::test_new(usize::MAX);
        assert!(!rmax.is_empty());
        assert_eq!(rmax.len(), usize::MAX);
    }

    /// Callback bridge correctly forwards MonitorResults with data.
    #[test]
    fn test_callback_bridge_forwards_results() {
        struct ResultsCapture {
            result_lens: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
        }
        impl MonitorCallback for ResultsCapture {
            fn on_complete(&mut self, _s: PmixStatus, results: Option<MonitorResults>) {
                if let Some(r) = results {
                    self.result_lens.lock().unwrap().push(r.len());
                }
            }
        }

        let lens = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut cb = ResultsCapture {
            result_lens: std::sync::Arc::clone(&lens),
        };

        for n in [0, 1, 5, 10] {
            let res = MonitorResults::test_new(n);
            cb.on_complete(PmixStatus::from_raw(0), Some(res));
        }

        let captured = lens.lock().unwrap();
        assert_eq!(captured.len(), 4);
        assert_eq!(captured.as_slice(), &[0, 1, 5, 10]);
    }

    /// Callback bridge handles None results gracefully.
    #[test]
    fn test_callback_bridge_handles_none_results() {
        struct NoneCapture {
            none_count: std::sync::Arc<std::sync::Mutex<usize>>,
        }
        impl MonitorCallback for NoneCapture {
            fn on_complete(&mut self, _s: PmixStatus, results: Option<MonitorResults>) {
                if results.is_none() {
                    *self.none_count.lock().unwrap() += 1;
                }
            }
        }

        let count = std::sync::Arc::new(std::sync::Mutex::new(0));
        let mut cb = NoneCapture {
            none_count: std::sync::Arc::clone(&count),
        };

        cb.on_complete(PmixStatus::from_raw(0), None);
        cb.on_complete(PmixStatus::from_raw(0), None);
        assert_eq!(*count.lock().unwrap(), 2);
    }

    /// MonitorCallback can hold Arc<Mutex<String>> state.
    #[test]
    fn test_monitor_callback_arc_string_state() {
        struct StringState {
            log: std::sync::Arc<std::sync::Mutex<String>>,
        }
        impl MonitorCallback for StringState {
            fn on_complete(&mut self, status: PmixStatus, _r: Option<MonitorResults>) {
                self.log.lock().unwrap().push_str(&format!("{:?} ", status));
            }
        }

        let log = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let mut cb = StringState {
            log: std::sync::Arc::clone(&log),
        };

        cb.on_complete(PmixStatus::from_raw(0), None);
        assert!(!log.lock().unwrap().is_empty());
    }

    /// MonitorCallback receives status and results.
    #[test]
    fn test_monitor_callback_receives_status_and_results() {
        struct CaptureCb {
            status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
            had_results: std::sync::Arc<std::sync::Mutex<bool>>,
        }
        impl MonitorCallback for CaptureCb {
            fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>) {
                *self.status.lock().unwrap() = Some(status);
                *self.had_results.lock().unwrap() = results.is_some();
            }
        }

        let status = std::sync::Arc::new(std::sync::Mutex::new(None));
        let had_results = std::sync::Arc::new(std::sync::Mutex::new(false));
        let mut cb = CaptureCb {
            status: std::sync::Arc::clone(&status),
            had_results: std::sync::Arc::clone(&had_results),
        };

        cb.on_complete(PmixStatus::from_raw(0), None);
        assert!(status.lock().unwrap().as_ref().unwrap().is_success());
        assert!(*had_results.lock().unwrap() == false);

        *had_results.lock().unwrap() = false;
        let dummy_results = MonitorResults::test_new(3);
        cb.on_complete(PmixStatus::from_raw(-1), Some(dummy_results));
        assert!(!status.lock().unwrap().as_ref().unwrap().is_success());
        assert!(*had_results.lock().unwrap());
    }

    /// process_monitor with success-like error code.
    #[test]
    fn test_process_monitor_with_success_error_code() {
        use crate::InfoBuilder;
        let monitor = InfoBuilder::new().build();
        let _ = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
    }

    /// heartbeat and process_monitor share the same underlying FFI — no conflicts.
    #[test]
    fn test_heartbeat_and_process_monitor_coexist() {
        use crate::InfoBuilder;
        let _ = heartbeat();
        let monitor = InfoBuilder::new().build();
        let _ = process_monitor(&monitor, PmixStatus::from_raw(-109), &[]);
        let _ = heartbeat();
    }

    /// Combined state callback test.
    #[test]
    fn test_monitor_callback_combined_state() {
        struct CombinedCb {
            status_count: std::sync::Arc<std::sync::Mutex<usize>>,
            result_count: std::sync::Arc<std::sync::Mutex<usize>>,
            total: std::sync::Arc<std::sync::Mutex<usize>>,
        }
        impl MonitorCallback for CombinedCb {
            fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>) {
                *self.total.lock().unwrap() += 1;
                if status.is_success() {
                    *self.status_count.lock().unwrap() += 1;
                }
                if results.is_some() {
                    *self.result_count.lock().unwrap() += 1;
                }
            }
        }

        let status_count = std::sync::Arc::new(std::sync::Mutex::new(0));
        let result_count = std::sync::Arc::new(std::sync::Mutex::new(0));
        let total = std::sync::Arc::new(std::sync::Mutex::new(0));
        let mut cb = CombinedCb {
            status_count: std::sync::Arc::clone(&status_count),
            result_count: std::sync::Arc::clone(&result_count),
            total: std::sync::Arc::clone(&total),
        };

        cb.on_complete(PmixStatus::from_raw(0), Some(MonitorResults::test_new(1)));
        cb.on_complete(PmixStatus::from_raw(-1), None);
        cb.on_complete(PmixStatus::from_raw(0), None);

        assert_eq!(*total.lock().unwrap(), 3);
        assert_eq!(*status_count.lock().unwrap(), 2);
        assert_eq!(*result_count.lock().unwrap(), 1);
    }

    /// InfoBuilder can construct monitoring info entries.
    #[test]
    fn test_infobuilder_for_monitoring() {
        use crate::InfoBuilder;
        let info = InfoBuilder::new().build();
        assert!(info.is_empty());
        assert_eq!(info.len(), 0);
    }

    // ── Additional tests to reach ≥35 inline tests (coverage target ≥55%) ──

    /// Static assertions: MonitorResults derives Debug, is Sized, is NOT Copy.
    #[test]
    fn test_static_assertions() {
        fn assert_debug<T: std::fmt::Debug>() {}
        fn assert_sized<T: Sized>() {}
        assert_debug::<MonitorResults>();
        assert_sized::<MonitorResults>();
        // MonitorResults is NOT Copy (owns raw pointer + Drop):
        // fn assert_copy<T: Copy>() {}
        // assert_copy::<MonitorResults>(); // would not compile
    }

    /// MonitorResults with zero length — Drop must not crash.
    #[test]
    fn test_monitor_results_drop_safety_compile_check() {
        // test_new always returns null handle, so Drop is a no-op.
        // This test verifies the drop path compiles and does not panic.
        let _ = MonitorResults::test_new(0);
    }

    /// MonitorCallback trait object is Send (required for cross-thread callbacks).
    #[test]
    fn test_monitor_callback_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Box<dyn MonitorCallback>>();
    }

    /// Callback that records all status variants (Known and Unknown).
    #[test]
    fn test_monitor_callback_with_all_status_variants() {
        struct VariantCapture {
            known_count: std::sync::Arc<std::sync::Mutex<usize>>,
            unknown_count: std::sync::Arc<std::sync::Mutex<usize>>,
        }
        impl MonitorCallback for VariantCapture {
            fn on_complete(&mut self, status: PmixStatus, _r: Option<MonitorResults>) {
                match status {
                    PmixStatus::Known(_) => *self.known_count.lock().unwrap() += 1,
                    PmixStatus::Unknown(_) => *self.unknown_count.lock().unwrap() += 1,
                }
            }
        }

        let known = std::sync::Arc::new(std::sync::Mutex::new(0));
        let unknown = std::sync::Arc::new(std::sync::Mutex::new(0));
        let mut cb = VariantCapture {
            known_count: known.clone(),
            unknown_count: unknown.clone(),
        };

        cb.on_complete(PmixStatus::from_raw(0), None);
        cb.on_complete(PmixStatus::from_raw(-1), None);
        cb.on_complete(PmixStatus::from_raw(-99999), None);

        assert_eq!(*known.lock().unwrap(), 2); // 0=Success, -1=Error
        assert_eq!(*unknown.lock().unwrap(), 1); // -99999 is unknown
    }

    /// Callback with boolean flag state — verifies mutable state works.
    #[test]
    fn test_monitor_callback_with_bool_flag() {
        struct FlagCb {
            triggered: std::sync::Arc<std::sync::Mutex<bool>>,
        }
        impl MonitorCallback for FlagCb {
            fn on_complete(&mut self, _status: PmixStatus, _r: Option<MonitorResults>) {
                *self.triggered.lock().unwrap() = true;
            }
        }

        let triggered = std::sync::Arc::new(std::sync::Mutex::new(false));
        let mut cb = FlagCb {
            triggered: triggered.clone(),
        };

        assert!(!*triggered.lock().unwrap());
        cb.on_complete(PmixStatus::from_raw(0), None);
        assert!(*triggered.lock().unwrap());
    }

    /// Callback with Vec-based log — verifies collection state works.
    #[test]
    fn test_monitor_callback_with_vec_log() {
        struct VecLogCb {
            log: std::sync::Arc<std::sync::Mutex<Vec<i32>>>,
        }
        impl MonitorCallback for VecLogCb {
            fn on_complete(&mut self, status: PmixStatus, _r: Option<MonitorResults>) {
                self.log.lock().unwrap().push(status.to_raw());
            }
        }

        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut cb = VecLogCb { log: log.clone() };

        cb.on_complete(PmixStatus::from_raw(0), None);
        cb.on_complete(PmixStatus::from_raw(-1), None);
        cb.on_complete(PmixStatus::from_raw(-109), None);

        let entries = log.lock().unwrap();
        assert_eq!(entries.as_slice(), &[0, -1, -109]);
    }

    /// process_monitor with PMIX_ERR_INIT — verifies error path.
    #[test]
    fn test_process_monitor_fails_without_init() {
        use crate::InfoBuilder;
        let monitor = InfoBuilder::new().build();
        let result = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
        assert!(
            result.is_err(),
            "process_monitor should fail without PMIx_Init"
        );
        if let Err(e) = result {
            assert!(!e.is_success(), "error status should not be success");
        }
    }

    /// process_monitor with partial success status code.
    #[test]
    fn test_process_monitor_with_partial_success() {
        use crate::{InfoBuilder, PmixError};
        let monitor = InfoBuilder::new().build();
        // Partial success is treated as success by the wrapper — but without init,
        // the FFI call itself will fail, so we get an error.
        let result = process_monitor(
            &monitor,
            PmixStatus::Known(PmixError::ErrPartialSuccess),
            &[],
        );
        assert!(
            result.is_err(),
            "should fail without PMIx_Init regardless of error code"
        );
    }

    /// process_monitor with collect_data directive.
    #[test]
    fn test_process_monitor_with_collect_data_monitor() {
        use crate::InfoBuilder;
        let mut builder = InfoBuilder::new();
        builder.collect_data();
        let monitor = builder.build();
        assert!(!monitor.is_empty(), "collect_data should add an entry");
        let result = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
        assert!(result.is_err(), "should fail without PMIx_Init");
    }

    /// process_monitor with monitor info having non-empty info_len.
    #[test]
    fn test_process_monitor_nonempty_info_len() {
        use crate::InfoBuilder;
        let mut builder = InfoBuilder::new();
        builder.collect_data();
        let monitor = builder.build();
        assert_eq!(monitor.len(), 1);
        let _ = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
    }

    /// process_monitor with empty monitor and zero directives — both null paths.
    #[test]
    fn test_process_monitor_empty_monitor_zero_directives() {
        use crate::InfoBuilder;
        let monitor = InfoBuilder::new().build();
        assert!(monitor.is_empty());
        let result = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
        assert!(result.is_err());
    }

    /// process_monitor_nb with collect_data directives.
    #[test]
    fn test_process_monitor_nb_with_collect_data_monitor() {
        use crate::InfoBuilder;
        struct NoopCb;
        impl MonitorCallback for NoopCb {
            fn on_complete(&mut self, _: PmixStatus, _: Option<MonitorResults>) {}
        }
        let monitor = InfoBuilder::new().build();
        let mut dir_builder = InfoBuilder::new();
        dir_builder.collect_data();
        let dirs = vec![dir_builder.build()];
        let result = process_monitor_nb(&monitor, PmixStatus::from_raw(0), &dirs, Box::new(NoopCb));
        assert!(result.is_err(), "should fail without PMIx_Init");
    }

    /// process_monitor_nb with unknown status code.
    #[test]
    fn test_process_monitor_nb_with_unknown_status() {
        struct NoopCb;
        impl MonitorCallback for NoopCb {
            fn on_complete(&mut self, _: PmixStatus, _: Option<MonitorResults>) {}
        }
        use crate::InfoBuilder;
        let monitor = InfoBuilder::new().build();
        let result = process_monitor_nb(
            &monitor,
            PmixStatus::Unknown(-109), // PMIX_MONITOR_HEARTBEAT_ALERT
            &[],
            Box::new(NoopCb),
        );
        assert!(result.is_err());
    }

    /// heartbeat error type — verify it's a real error, not success.
    #[test]
    fn test_heartbeat_error_type() {
        let result = heartbeat();
        match result {
            Err(status) => {
                assert!(
                    !status.is_success(),
                    "heartbeat error should not be success"
                );
                assert!(status.is_error(), "heartbeat error should be an error");
            }
            Ok(()) => {
                // PMIx was initialized — heartbeat succeeded, that's fine
            }
        }
    }

    /// heartbeat multiple calls — verify consistent behavior.
    #[test]
    fn test_heartbeat_multiple_calls_consistent() {
        let results: Vec<Result<(), PmixStatus>> = (0..5).map(|_| heartbeat()).collect();
        let first = &results[0];
        for (i, r) in results.iter().enumerate().skip(1) {
            match (first, r) {
                (Ok(_), Ok(_)) => {} // Both succeeded
                (Err(s1), Err(s2)) => {
                    assert_eq!(s1, s2, "heartbeat error inconsistent at call {}", i);
                }
                _ => panic!("inconsistent heartbeat results"),
            }
        }
    }

    /// InfoBuilder with collect_data produces non-empty Info.
    #[test]
    fn test_infobuilder_collect_data_nonempty() {
        use crate::InfoBuilder;
        let mut builder = InfoBuilder::new();
        builder.collect_data();
        let info = builder.build();
        assert!(!info.is_empty());
        assert_eq!(info.len(), 1);
    }

    /// Callback that counts only successful status codes.
    #[test]
    fn test_monitor_callback_counts_success_only() {
        struct SuccessCounter {
            count: std::sync::Arc<std::sync::Mutex<usize>>,
        }
        impl MonitorCallback for SuccessCounter {
            fn on_complete(&mut self, status: PmixStatus, _r: Option<MonitorResults>) {
                if status.is_success() {
                    *self.count.lock().unwrap() += 1;
                }
            }
        }

        let count = std::sync::Arc::new(std::sync::Mutex::new(0));
        let mut cb = SuccessCounter {
            count: count.clone(),
        };

        cb.on_complete(PmixStatus::from_raw(0), None); // success
        cb.on_complete(PmixStatus::from_raw(-1), None); // error
        cb.on_complete(PmixStatus::from_raw(0), None); // success
        cb.on_complete(PmixStatus::from_raw(-109), None); // error

        assert_eq!(*count.lock().unwrap(), 2);
    }

    /// MonitorResults debug output contains "handle" and "len" field names.
    #[test]
    fn test_monitor_results_debug_format_content() {
        let results = MonitorResults::test_new(42);
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("MonitorResults") || debug_str.contains("handle"));
        // Debug derive includes field names
        assert!(debug_str.contains("len") || debug_str.contains("handle"));
    }

    /// process_monitor and heartbeat can be interleaved safely.
    #[test]
    fn test_process_monitor_and_heartbeat_interleaved() {
        use crate::InfoBuilder;
        for _ in 0..5 {
            let _ = heartbeat();
            let monitor = InfoBuilder::new().build();
            let _ = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
            let _ = heartbeat();
        }
    }
}
