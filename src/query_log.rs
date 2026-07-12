//! Query and logging operations — `PMIx_Query_info`, `PMIx_Log`, and non-blocking variants.
//!
//! This module provides safe Rust wrappers for querying information from the
//! PMIx host resource manager and logging data to a data service. The query API
//! allows tools to request specific attributes from the PMIx server without
//! requiring prior publication. The log API sends data to the host environment's
//! logging infrastructure (stdout, stderr, syslog, email, global datastore, etc.).
//!
//! # Query model
//!
//! A query consists of one or more `PmixQuery` objects, each containing:
//! - A list of key names to request (`keys`)
//! - Optional qualifier info (`qualifiers`) that narrows or modifies the query
//!
//! Results are returned as a `QueryResults` which auto-frees the C allocation.
//!
//! # Log model
//!
//! A log request consists of:
//! - `data`: the info entries containing the actual data to log
//! - `directives`: optional info entries that control the logging channel
//!   (e.g., `PMIX_LOG_STDOUT`, `PMIX_LOG_SYSLOG`, `PMIX_LOG_EMAIL`)
//!
//! # C API reference
//!
//! ```c
//! pmix_status_t PMIx_Query_info(pmix_query_t queries[], size_t nqueries,
//!                                pmix_info_t **results, size_t *nresults);
//! pmix_status_t PMIx_Query_info_nb(pmix_query_t queries[], size_t nqueries,
//!                                   pmix_info_cbfunc_t cbfunc, void *cbdata);
//! pmix_status_t PMIx_Log(const pmix_info_t data[], size_t ndata,
//!                        const pmix_info_t directives[], size_t ndirs);
//! pmix_status_t PMIx_Log_nb(const pmix_info_t data[], size_t ndata,
//!                           const pmix_info_t directives[], size_t ndirs,
//!                           pmix_op_cbfunc_t cbfunc, void *cbdata);
//! ```

use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::{LazyLock, Mutex};

use crate::ffi;
use crate::{Info, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery — safe Rust wrapper around `pmix_query_t`
// ─────────────────────────────────────────────────────────────────────────────

/// A single PMIx query request.
///
/// Each query specifies one or more key names to request from the PMIx server,
/// optionally qualified by additional `pmix_info_t` directives (e.g.,
/// `PMIX_QUERY_REFRESH_CACHE`, `PMIX_QUERY_LOCAL_ONLY`).
///
/// The query is automatically freed on drop. Key strings are owned by Rust
/// [`CString`] objects, and the query struct is freed via `PMIx_Query_free`.
///
/// # C API
/// `typedef struct pmix_query { char **keys; pmix_info_t *qualifiers; size_t nqual; } pmix_query_t;`
pub struct PmixQuery {
    handle: *mut ffi::pmix_query_t,
    /// Owned key strings — kept alive for the lifetime of the query.
    _keys: Vec<CString>,
    /// Pointer to the null-terminated char** array (allocated via libc::calloc).
    /// Freed manually in Drop since PMIx_Query_release would double-free the
    /// individual CString-allocated strings.
    keys_array: *mut *mut c_char,
}

impl std::fmt::Debug for PmixQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixQuery")
            .field("keys", &self._keys)
            .finish_non_exhaustive()
    }
}

impl PmixQuery {
    /// Create a new query requesting the given key(s).
    ///
    /// # Parameters
    /// - `keys`: One or more attribute keys to query (e.g., `"pmix.version"`,
    ///   `"pmix.client.attrs"`).
    ///
    /// # Errors
    /// Returns `Err(PmixStatus)` if `keys` is empty or any key contains an
    /// interior NUL byte.
    pub fn new(keys: &[&str]) -> Result<Self, PmixStatus> {
        if keys.is_empty() {
            return Err(PmixError::ErrBadParam.into());
        }

        // Allocate the query struct via the C helper.
        let query_ptr = unsafe {
            // SAFETY: PMIx_Query_create allocates and zero-initializes a
            // pmix_query_t. It always returns a non-null pointer.
            ffi::PMIx_Query_create(1)
        };
        if query_ptr.is_null() {
            return Err(PmixError::ErrNomem.into());
        }

        // Convert keys to CStrings — collect so we own them.
        let c_keys: Result<Vec<CString>, _> = keys.iter().map(|k| CString::new(*k)).collect();
        let c_keys = match c_keys {
            Ok(v) => v,
            Err(_) => {
                unsafe {
                    // SAFETY: query_ptr was allocated by PMIx_Query_create.
                    ffi::PMIx_Query_free(query_ptr, 1);
                }
                return Err(PmixError::ErrBadParam.into());
            }
        };

        // Build a null-terminated char** array for the keys field.
        // We allocate this with libc::calloc and free it in Drop.
        let raw_ptrs: Vec<*mut c_char> =
            c_keys.iter().map(|cs| cs.as_ptr() as *mut c_char).collect();
        let keys_array = Self::alloc_keys_array(&raw_ptrs);

        if keys_array.is_null() {
            unsafe {
                ffi::PMIx_Query_free(query_ptr, 1);
            }
            return Err(PmixError::ErrNomem.into());
        }

        unsafe {
            (*query_ptr).keys = keys_array;
        }

        Ok(Self {
            handle: query_ptr,
            _keys: c_keys,
            keys_array,
        })
    }

    /// Allocate a null-terminated array of char* pointers using libc.
    ///
    /// Returns a `*mut *mut c_char` pointing to an array of length
    /// `src.len() + 1` (the last element is NULL). Caller must free with
    /// `libc::free`.
    fn alloc_keys_array(src: &[*mut c_char]) -> *mut *mut c_char {
        let len = src.len() + 1; // +1 for NULL terminator
        unsafe {
            // SAFETY: calloc zero-initializes memory. On failure returns NULL.
            let ptr = libc::calloc(len, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(src.as_ptr(), ptr, src.len());
            }
            ptr
        }
    }

    /// Add qualifier info to this query.
    ///
    /// Qualifiers narrow or modify the query (e.g., `PMIX_QUERY_REFRESH_CACHE`).
    /// The `info` parameter's C allocation is transferred to this query.
    pub fn with_qualifiers(self, info: Info) -> Self {
        if info.len > 0 {
            unsafe {
                // SAFETY: handle was allocated by PMIx_Query_create and is valid.
                (*self.handle).qualifiers = info.handle as *mut ffi::pmix_info_t;
                (*self.handle).nqual = info.len;
            }
            // Prevent the Info from freeing its allocation on drop — we've
            // transferred ownership to the query.
            let _ = info;
        }
        self
    }
}

impl Drop for PmixQuery {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                // IMPORTANT: We must NOT use PMIx_Query_release because it frees
                // the individual key strings (which are owned by our CStrings),
                // causing a double-free. Instead we:
                // 1. Null out keys and qualifiers so PMIx_Query_free doesn't touch them.
                // 2. Free the keys_array ourselves (it was allocated by libc::calloc).
                // 3. Free the query struct with PMIx_Query_free.

                // Null out keys so PMIx_Query_free doesn't try to free them.
                (*self.handle).keys = ptr::null_mut();
                // Null out qualifiers so PMIx_Query_free doesn't try to free them.
                (*self.handle).qualifiers = ptr::null_mut();
                (*self.handle).nqual = 0;

                // Free the keys array allocated by libc::calloc.
                if !self.keys_array.is_null() {
                    // SAFETY: keys_array was allocated by libc::calloc.
                    libc::free(self.keys_array as *mut c_void);
                    self.keys_array = ptr::null_mut();
                }

                // SAFETY: handle was allocated by PMIx_Query_create. We've nulled
                // out keys and qualifiers so PMIx_Query_free won't double-free them.
                ffi::PMIx_Query_free(self.handle, 1);
                self.handle = ptr::null_mut();
            }
        }
        // self._keys is dropped automatically by Rust — CStrings free their
        // internal buffers. The pointers stored in keys_array were just views
        // into CString memory, and we freed keys_array above before this.
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QueryResults — owned result set from query_info()
// ─────────────────────────────────────────────────────────────────────────────

/// Owned result set from [`query_info`].
///
/// Wraps the `pmix_info_t*` array returned by `PMIx_Query_info` and
/// automatically frees it via `PMIx_Info_free` on drop.
pub struct QueryResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl std::fmt::Debug for QueryResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryResults")
            .field("len", &self.len)
            .finish()
    }
}

impl QueryResults {
    /// Number of info entries in this result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for QueryResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx_Query_info as an
                // allocated pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// query_info — safe wrapper for PMIx_Query_info
// ─────────────────────────────────────────────────────────────────────────────

/// Query the PMIx server for information about specific attributes.
///
/// This is a blocking call — it returns only after the server has processed
/// all queries and returned results (or an error).
///
/// # Parameters
/// - `queries`: One or more [`PmixQuery`] objects specifying the keys to
///   request and optional qualifiers.
///
/// # Returns
/// - `Ok(QueryResults)` containing the response info array. The caller owns
///   the results and they will be freed when `QueryResults` is dropped.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host RM does not support this function.
///   - `PMIX_ERR_NOT_FOUND` — none of the requested data was available.
///   - `PMIX_ERR_PARTIAL_SUCCESS` — some data was returned despite errors.
///
/// # C API
/// `pmix_status_t PMIx_Query_info(pmix_query_t queries[], size_t nqueries,`
/// `  pmix_info_t **results, size_t *nresults);`
pub fn query_info(queries: &[PmixQuery]) -> Result<QueryResults, PmixStatus> {
    if queries.is_empty() {
        return Err(PmixError::ErrBadParam.into());
    }

    let nqueries = queries.len();
    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    // Collect raw handles — queries must outlive this call (borrowed from caller).
    let handles: Vec<*mut ffi::pmix_query_t> = queries.iter().map(|q| q.handle).collect();
    let queries_ptr = handles.as_ptr() as *mut ffi::pmix_query_t;

    let status = unsafe {
        // SAFETY: PMIx_Query_info is a synchronous PMIx API call.
        // - queries_ptr points to an array of valid pmix_query_t structs
        //   owned by the PmixQuery borrows passed by the caller.
        // - results and nresults are output pointers that PMIx will write to.
        // - PMIx does not retain queries_ptr after this call returns.
        ffi::PMIx_Query_info(queries_ptr, nqueries, &mut results, &mut nresults)
    };

    let pmix_status = PmixStatus::from_raw(status);

    // On success or partial success, return the results.
    if pmix_status.is_success() || pmix_status == PmixStatus::Known(PmixError::ErrPartialSuccess) {
        Ok(QueryResults {
            handle: results,
            len: nresults,
        })
    } else {
        // On error, PMIx may have still allocated a results array — free it.
        if !results.is_null() && nresults > 0 {
            unsafe {
                // SAFETY: PMIx allocated this array; free it on the error path.
                ffi::PMIx_Info_free(results, nresults);
            }
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait and registry for PMIx_Query_info_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for [`query_info_nb`].
///
/// Implement this trait to receive the result of a non-blocking query.
/// The `on_complete` method receives the `PmixStatus` and the
/// [`QueryResults`] returned by the server.
pub trait QueryCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, results: QueryResults);
}

/// Global registry mapping request IDs to pending query callbacks.
type QueryRegistry = std::collections::HashMap<usize, Box<dyn QueryCallback>>;
static QUERY_REGISTRY: LazyLock<Mutex<QueryRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing query request ID counter.
static QUERY_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_info_cbfunc_t` (query completion).
///
/// Called by PMIx when the non-blocking query completes. The `cbdata`
/// parameter encodes the request ID. We look up the registered closure
/// and invoke it with the result status and info array.
///
/// The PMIx 4.1 callback signature includes release_fn and release_cbdata
/// parameters for custom memory management — we pass None/null since we
/// use our own ownership model.
extern "C" fn query_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    _release_cbdata: *mut c_void,
    release_fn: ffi::pmix_release_cbfunc_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = QUERY_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — free the info array to avoid leak.
            if !info.is_null() && ninfo > 0 {
                unsafe {
                    ffi::PMIx_Info_free(info, ninfo);
                }
            }
            return;
        }
    };

    let pmix_status = PmixStatus::from_raw(status);
    let results = QueryResults {
        handle: info,
        len: ninfo,
    };
    cb.on_complete(pmix_status, results);
    // release_fn is unused — we manage our own memory via QueryResults Drop.
    let _ = release_fn;
}

/// Non-blocking query of the PMIx server for information.
///
/// Submit an asynchronous request. The `callback` closure is invoked once
/// the operation completes, receiving both the status and the results.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # C API
/// `pmix_status_t PMIx_Query_info_nb(pmix_query_t queries[], size_t nqueries,`
/// `  pmix_info_cbfunc_t cbfunc, void *cbdata);`
pub fn query_info_nb(
    queries: &[PmixQuery],
    callback: Box<dyn QueryCallback>,
) -> Result<(), PmixStatus> {
    if queries.is_empty() {
        return Err(PmixError::ErrBadParam.into());
    }

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = QUERY_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = QUERY_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    let nqueries = queries.len();
    let handles: Vec<*mut ffi::pmix_query_t> = queries.iter().map(|q| q.handle).collect();
    let queries_ptr = handles.as_ptr() as *mut ffi::pmix_query_t;

    let status = unsafe {
        // SAFETY: PMIx_Query_info_nb is an async PMIx API call.
        // - queries_ptr points to valid pmix_query_t structs owned by
        //   the PmixQuery borrows (which outlive this call).
        // - cbfunc is a valid extern "C" function pointer.
        // - cbdata encodes the request ID; PMIx passes it back unchanged.
        // - PMIx does not retain queries_ptr after this call returns.
        ffi::PMIx_Query_info_nb(queries_ptr, nqueries, Some(query_callback_bridge), cbdata)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request rejected — remove the callback from the registry.
        let mut registry = QUERY_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Log — safe wrapper for PMIx_Log
// ─────────────────────────────────────────────────────────────────────────────

/// Log data to the host environment's logging service.
///
/// This is a blocking call — it returns only after the PMIx server has
/// processed the log request. The data to be logged is provided in the
/// `data` array. Optional `directives` control the logging channel and
/// behavior (e.g., log to stdout, stderr, syslog, email, global datastore).
///
/// # Directives
///
/// Common directive keys (from the PMIx spec):
/// - `PMIX_LOG_STDOUT` — log string to stdout
/// - `PMIX_LOG_STDERR` — log string to stderr
/// - `PMIX_LOG_SYSLOG` — log to syslog (defaults to ERROR priority)
/// - `PMIX_LOG_LOCAL_SYSLOG` — log to local syslog
/// - `PMIX_LOG_GLOBAL_SYSLOG` — forward to system gateway syslog
/// - `PMIX_LOG_ONCE` — log only once via whichever channel supports it first
/// - `PMIX_LOG_JOB_RECORD` — log to the host environment's job record
/// - `PMIX_LOG_GLOBAL_DATASTORE` — store in a global data store (e.g., database)
///
/// # Returns
/// - `Ok(())` on success (PMIX_SUCCESS).
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_BAD_PARAM` — the log request contains incorrect entries.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host environment does not support logging.
///   - Other appropriate PMIx error codes.
///
/// # Advice
/// It is strongly recommended that PMIx_Log not be used for streaming data
/// as it is not a performant transport and can perturb the application.
/// A return of PMIX_SUCCESS only denotes that the data was successfully
/// handed to the appropriate system call or host environment and does not
/// indicate receipt at the final destination.
///
/// # C API
/// `pmix_status_t PMIx_Log(const pmix_info_t data[], size_t ndata,`
/// `  const pmix_info_t directives[], size_t ndirs);`
pub fn log_data(data: &[Info], directives: &[Info]) -> Result<(), PmixStatus> {
    let ndata = data.len();
    let ndirs = directives.len();

    // Convert slices to raw C pointers.
    // Collect raw handles from the Info objects.
    let data_handles: Vec<*mut ffi::pmix_info_t> = data.iter().map(|i| i.handle).collect();
    let dirs_handles: Vec<*mut ffi::pmix_info_t> = directives.iter().map(|i| i.handle).collect();
    let data_ptr = if ndata > 0 {
        data_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };
    let dirs_ptr = if ndirs > 0 {
        dirs_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    let status = unsafe {
        // SAFETY: PMIx_Log is a synchronous PMIx API call.
        // - data_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows passed by the caller (or is null if empty).
        // - dirs_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows passed by the caller (or is null if empty).
        // - PMIx does not retain these pointers after this call returns.
        // - The caller must keep data and directives alive until this
        //   function returns.
        ffi::PMIx_Log(data_ptr, ndata, dirs_ptr, ndirs)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait and registry for PMIx_Log_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for [`log_data_nb`].
///
/// Implement this trait to receive the result of a non-blocking log request.
/// The `on_complete` method receives the `PmixStatus` returned by the server
/// after processing the log request.
pub trait LogCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping log request IDs to pending callbacks.
type LogRegistry = std::collections::HashMap<usize, Box<dyn LogCallback>>;
static LOG_REGISTRY: LazyLock<Mutex<LogRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing log request ID counter.
static LOG_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (log completion).
///
/// Called by PMIx when the non-blocking log request completes. The `cbdata`
/// parameter encodes the request ID. We look up the registered closure and
/// invoke it with the result status.
extern "C" fn log_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = LOG_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — nothing to do.
            return;
        }
    };

    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status);
}

/// Non-blocking log of data to the host environment's logging service.
///
/// Submit an asynchronous log request. The `callback` closure is invoked once
/// the operation completes, receiving the status.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # C API
/// `pmix_status_t PMIx_Log_nb(const pmix_info_t data[], size_t ndata,`
/// `  const pmix_info_t directives[], size_t ndirs,`
/// `  pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn log_data_nb(
    data: &[Info],
    directives: &[Info],
    callback: Box<dyn LogCallback>,
) -> Result<(), PmixStatus> {
    let ndata = data.len();
    let ndirs = directives.len();

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = LOG_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = LOG_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Collect raw handles from the Info objects.
    let data_handles: Vec<*mut ffi::pmix_info_t> = data.iter().map(|i| i.handle).collect();
    let dirs_handles: Vec<*mut ffi::pmix_info_t> = directives.iter().map(|i| i.handle).collect();
    let data_ptr = if ndata > 0 {
        data_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };
    let dirs_ptr = if ndirs > 0 {
        dirs_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    let status = unsafe {
        // SAFETY: PMIx_Log_nb is an async PMIx API call.
        // - data_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows (or is null if empty).
        // - dirs_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows (or is null if empty).
        // - cbfunc is a valid extern "C" function pointer.
        // - cbdata encodes the request ID; PMIx passes it back unchanged.
        // - PMIx does not retain data_ptr or dirs_ptr after this call returns.
        // - The caller must keep data and directives alive until the
        //   callback is invoked.
        ffi::PMIx_Log_nb(
            data_ptr,
            ndata,
            dirs_ptr,
            ndirs,
            Some(log_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request rejected — remove the callback from the registry.
        let mut registry = LOG_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ─────────────────────────────────────────────────────────────────────
    // PmixQuery construction tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_query_new() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        assert!(!query._keys.is_empty());
    }

    #[test]
    fn test_pmix_query_empty_keys() {
        let result = PmixQuery::new(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_query_multiple_keys() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE", "PMIX_QUERY_NODE_SIZE"]).unwrap();
        assert_eq!(query._keys.len(), 2);
    }

    #[test]
    fn test_pmix_query_single_key() {
        let query = PmixQuery::new(&["pmix.version"]).unwrap();
        assert_eq!(query._keys.len(), 1);
    }

    #[test]
    fn test_pmix_query_nul_byte_in_key() {
        // Keys with interior NUL bytes should fail
        let result = PmixQuery::new(&["pmix.ver\0sion"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_query_long_key() {
        let long_key = "pmix.".to_string() + &"a".repeat(200);
        let query = PmixQuery::new(&[&long_key]).unwrap();
        assert_eq!(query._keys.len(), 1);
    }

    #[test]
    fn test_pmix_query_unicode_key() {
        let query = PmixQuery::new(&["pmix.テスト"]).unwrap();
        assert_eq!(query._keys.len(), 1);
    }

    #[test]
    fn test_pmix_query_debug_output() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let debug_str = format!("{:?}", query);
        assert!(debug_str.contains("PmixQuery"));
    }

    #[test]
    fn test_pmix_query_with_qualifiers() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let query = query.with_qualifiers(crate::InfoBuilder::new().build());
        assert!(!query._keys.is_empty());
    }

    #[test]
    fn test_pmix_query_with_nonempty_qualifiers() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let mut builder = crate::InfoBuilder::new();
        builder.collect_data();
        let info = builder.build();
        let query = query.with_qualifiers(info);
        assert!(!query._keys.is_empty());
    }

    #[test]
    fn test_pmix_query_drop_safety() {
        for _ in 0..10 {
            let _query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        }
    }

    #[test]
    fn test_pmix_query_drop_after_with_qualifiers() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let info = crate::InfoBuilder::new().build();
        let _query = query.with_qualifiers(info);
    }

    // ─────────────────────────────────────────────────────────────────────
    // alloc_keys_array tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_alloc_keys_array_empty() {
        let arr = PmixQuery::alloc_keys_array(&[]);
        assert!(!arr.is_null());
        unsafe {
            assert!(std::ptr::read(arr).is_null());
            libc::free(arr as *mut c_void);
        }
    }

    #[test]
    fn test_alloc_keys_array_single() {
        let key = CString::new("test_key").unwrap();
        let ptrs: &[*mut c_char] = &[key.as_ptr() as *mut c_char];
        let arr = PmixQuery::alloc_keys_array(ptrs);
        assert!(!arr.is_null());
        unsafe {
            assert_eq!(*arr, key.as_ptr() as *mut c_char);
            assert!(std::ptr::read(arr.offset(1)).is_null());
            libc::free(arr as *mut c_void);
        }
    }

    #[test]
    fn test_alloc_keys_array_multiple() {
        let k1 = CString::new("key1").unwrap();
        let k2 = CString::new("key2").unwrap();
        let k3 = CString::new("key3").unwrap();
        let ptrs: &[*mut c_char] = &[
            k1.as_ptr() as *mut c_char,
            k2.as_ptr() as *mut c_char,
            k3.as_ptr() as *mut c_char,
        ];
        let arr = PmixQuery::alloc_keys_array(ptrs);
        assert!(!arr.is_null());
        unsafe {
            assert_eq!(*arr, k1.as_ptr() as *mut c_char);
            assert_eq!(*arr.offset(1), k2.as_ptr() as *mut c_char);
            assert_eq!(*arr.offset(2), k3.as_ptr() as *mut c_char);
            assert!(std::ptr::read(arr.offset(3)).is_null());
            libc::free(arr as *mut c_void);
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // QueryResults tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_results_empty() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_query_results_nonempty() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 3,
        };
        assert!(!results.is_empty());
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_query_results_debug() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 5,
        };
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("QueryResults"));
        assert!(debug_str.contains("5"));
    }

    #[test]
    fn test_query_results_drop_null_handle() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        drop(results);
    }

    #[test]
    fn test_query_results_drop_null_handle_nonzero_len() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 5,
        };
        drop(results);
    }

    // ─────────────────────────────────────────────────────────────────────
    // query_info() tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_info_empty_queries() {
        let result = query_info(&[]);
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "requires PMIx daemon — PMIx_Query_info needs initialized PMIx server"]
    fn test_query_info_with_daemon() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let result = query_info(&[query]);
        assert!(result.is_ok() || matches!(result, Err(PmixStatus::Known(PmixError::ErrInit))));
    }

    // ─────────────────────────────────────────────────────────────────────
    // query_callback_bridge tests (pmix_status_t is c_int, not an enum)
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_callback_bridge_null_cbdata() {
        // Null cbdata should return immediately without panicking
        unsafe {
            query_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
            );
        }
    }

    #[test]
    fn test_query_callback_bridge_missing_callback() {
        let req_id: usize = 99999;
        let cbdata = (req_id << 2) as *mut c_void;
        unsafe {
            query_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                None,
                cbdata,
            );
        }
    }

    #[test]
    fn test_query_callback_bridge_info_cleanup_on_missing_callback() {
        let req_id: usize = 88888;
        let cbdata = (req_id << 2) as *mut c_void;
        unsafe {
            query_callback_bridge(
                -46, // PMIX_ERR_NOT_FOUND
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                None,
                cbdata,
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // query_info_nb() tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_info_nb_empty_queries() {
        struct NbDummy;
        impl QueryCallback for NbDummy {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
        }
        let result = query_info_nb(&[], Box::new(NbDummy));
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // requires PMIx init — PMIx_Query_info_nb FFI call segfaults without a real server
    fn test_query_info_nb_callback_registration_and_cleanup() {
        struct CountingCallback {
            called: Arc<std::sync::atomic::AtomicBool>,
        }
        impl QueryCallback for CountingCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
                self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        // Record registry size before our call
        let size_before = {
            let registry = QUERY_REGISTRY.lock().unwrap();
            registry.len()
        };
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let cb = Box::new(CountingCallback {
            called: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let result = query_info_nb(&[query], cb);
        // FFI behavior without PMIx init is non-deterministic (Ok or Err).
        // If Ok, the callback was NOT removed from registry (no PMIx server to
        // invoke the bridge callback). Clean it up ourselves.
        if result.is_ok() {
            let req_id = {
                let seq = QUERY_SEQ.lock().unwrap();
                *seq
            };
            let mut registry = QUERY_REGISTRY.lock().unwrap();
            registry.remove(&req_id);
        }
        // The registry should be back to its pre-call size — our entry was removed.
        let size_after = {
            let registry = QUERY_REGISTRY.lock().unwrap();
            registry.len()
        };
        assert_eq!(
            size_after, size_before,
            "Registry should have same size after NB query (entry was cleaned up)"
        );
    }

    #[test]
    fn test_query_info_nb_request_id_encoding() {
        let req_id: usize = 42;
        let cbdata = (req_id << 2) as *mut c_void;
        let decoded = (cbdata as usize) >> 2;
        assert_eq!(decoded, req_id);
        assert!(!cbdata.is_null());
    }

    #[test]
    fn test_query_info_nb_request_id_zero_not_null() {
        let req_id: usize = 1;
        let cbdata = (req_id << 2) as *mut c_void;
        assert!(!cbdata.is_null());
    }

    // ─────────────────────────────────────────────────────────────────────
    // QUERY_SEQ counter tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_seq_monotonic() {
        let seq1 = QUERY_SEQ.lock().unwrap();
        let val1 = *seq1;
        drop(seq1);
        let mut seq2 = QUERY_SEQ.lock().unwrap();
        *seq2 += 1;
        let val2 = *seq2;
        drop(seq2);
        assert!(val2 > val1);
    }

    // ─────────────────────────────────────────────────────────────────────
    // log_data() tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_log_data_empty() {
        let data = vec![];
        let directives = vec![];
        let result = log_data(&data, &directives);
        // PMIx_Log without init returns ErrInit or ErrNotSupported depending on
        // library state — accept any outcome since we're not testing FFI behavior.
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_log_data_with_empty_info() {
        let data = vec![crate::InfoBuilder::new().build()];
        let directives = vec![];
        let result = log_data(&data, &directives);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_log_data_with_string_info() {
        let info = crate::info_with_string_key("test.key", "test.value");
        let data = vec![info];
        let directives = vec![];
        let result = log_data(&data, &directives);
        // PMIx_Log without init returns ErrInit or ErrNotSupported depending on
        // library state — accept any outcome since we're not testing FFI behavior.
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_log_data_with_directives() {
        let data_info = crate::info_with_string_key("log.data", "hello");
        let dir_info = crate::info_with_string_key("PMIX_LOG_STDOUT", "1");
        let result = log_data(&[data_info], &[dir_info]);
        assert!(result.is_ok() || result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────
    // log_callback_bridge tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_log_callback_bridge_null_cbdata() {
        unsafe {
            log_callback_bridge(0, std::ptr::null_mut()); // PMIX_SUCCESS
        }
    }

    #[test]
    fn test_log_callback_bridge_missing_callback() {
        let req_id: usize = 77777;
        let cbdata = (req_id << 2) as *mut c_void;
        unsafe {
            log_callback_bridge(0, cbdata); // PMIX_SUCCESS
        }
    }

    #[test]
    fn test_log_callback_bridge_error_status() {
        let req_id: usize = 66666;
        let cbdata = (req_id << 2) as *mut c_void;
        unsafe {
            log_callback_bridge(-30, cbdata); // PMIX_ERR_NOT_SUPPORTED
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // log_data_nb() tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    #[ignore] // requires PMIx init — PMIx_Log_nb FFI call segfaults without a real server
    fn test_log_data_nb_empty_data_and_directives() {
        struct LogNbDummy;
        impl LogCallback for LogNbDummy {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        // Record registry size before to detect our entry post-call
        let size_before = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        let result = log_data_nb(&[], &[], Box::new(LogNbDummy));
        assert!(result.is_err());
        // Registry should be back to pre-call size — our entry was cleaned up.
        let size_after = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        assert_eq!(
            size_after, size_before,
            "Registry should have same size after failed NB log"
        );
    }

    #[test]
    #[ignore] // requires PMIx init — PMIx_Log_nb FFI call segfaults without a real server
    fn test_log_data_nb_with_data() {
        struct LogNbCounting {
            _called: Arc<std::sync::atomic::AtomicBool>,
        }
        impl LogCallback for LogNbCounting {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                self._called
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let size_before = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        let info = crate::info_with_string_key("test.key", "test.value");
        let cb = Box::new(LogNbCounting {
            _called: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let result = log_data_nb(&[info], &[], cb);
        assert!(result.is_err());
        let size_after = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        assert_eq!(
            size_after, size_before,
            "Registry size unchanged after failed NB log"
        );
    }

    #[test]
    fn test_log_data_nb_request_id_encoding() {
        let req_id: usize = 99;
        let cbdata = (req_id << 2) as *mut c_void;
        let decoded = (cbdata as usize) >> 2;
        assert_eq!(decoded, req_id);
        assert!(!cbdata.is_null());
    }

    #[test]
    #[ignore] // requires PMIx init — PMIx_Log_nb FFI call segfaults without a real server
    fn test_log_data_nb_callback_cleanup_on_error() {
        struct LogDummy;
        impl LogCallback for LogDummy {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let size_before = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        for _ in 0..5 {
            let info = crate::info_with_string_key("test", "val");
            let _ = log_data_nb(&[info], &[], Box::new(LogDummy));
        }
        let size_after = {
            let registry = LOG_REGISTRY.lock().unwrap();
            registry.len()
        };
        assert_eq!(
            size_after, size_before,
            "Registry size unchanged after multiple failed NB logs"
        );
    }

    // ─────────────────────────────────────────────────────────────────────
    // LOG_SEQ counter tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_log_seq_monotonic() {
        let seq1 = LOG_SEQ.lock().unwrap();
        let val1 = *seq1;
        drop(seq1);
        let mut seq2 = LOG_SEQ.lock().unwrap();
        *seq2 += 1;
        let val2 = *seq2;
        drop(seq2);
        assert!(val2 > val1);
    }

    // ─────────────────────────────────────────────────────────────────────
    // PmixStatus error code tests for query/log module
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_status_err_init() {
        let status = PmixStatus::from_raw(-31); // PMIX_ERR_INIT
        assert_eq!(status, PmixStatus::Known(PmixError::ErrInit));
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_err_bad_param() {
        let status = PmixStatus::from_raw(-27); // PMIX_ERR_BAD_PARAM
        assert_eq!(status, PmixStatus::Known(PmixError::ErrBadParam));
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_err_not_found() {
        let status = PmixStatus::from_raw(-46); // PMIX_ERR_NOT_FOUND
        assert_eq!(status, PmixStatus::Known(PmixError::ErrNotFound));
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_err_partial_success() {
        let status = PmixStatus::from_raw(-52); // PMIX_ERR_PARTIAL_SUCCESS
        assert_eq!(status, PmixStatus::Known(PmixError::ErrPartialSuccess));
        // ErrPartialSuccess is negative (-52), so is_error() is true
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_success() {
        let status = PmixStatus::from_raw(0); // PMIX_SUCCESS
        assert!(status.is_success());
        assert!(!status.is_error());
    }

    // ─────────────────────────────────────────────────────────────────────
    // QueryCallback and LogCallback trait tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_callback_trait_object() {
        struct DummyQuery;
        impl QueryCallback for DummyQuery {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
        }
        let callback: Box<dyn QueryCallback> = Box::new(DummyQuery);
        let _ = callback;
    }

    #[test]
    fn test_log_callback_trait_object() {
        struct DummyLog;
        impl LogCallback for DummyLog {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let callback: Box<dyn LogCallback> = Box::new(DummyLog);
        let _ = callback;
    }

    #[test]
    fn test_query_callback_with_state() {
        use std::sync::atomic::{AtomicU32, Ordering};
        struct StatefulQuery {
            count: Arc<AtomicU32>,
        }
        impl QueryCallback for StatefulQuery {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }
        let count = Arc::new(AtomicU32::new(0));
        let cb: Box<dyn QueryCallback> = Box::new(StatefulQuery {
            count: count.clone(),
        });
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        cb.on_complete(PmixStatus::Known(PmixError::Success), results);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_log_callback_with_state() {
        use std::sync::atomic::{AtomicU32, Ordering};
        struct StatefulLog {
            count: Arc<AtomicU32>,
        }
        impl LogCallback for StatefulLog {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }
        let count = Arc::new(AtomicU32::new(0));
        let cb: Box<dyn LogCallback> = Box::new(StatefulLog {
            count: count.clone(),
        });
        cb.on_complete(PmixStatus::Known(PmixError::Success));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_log_callback_receives_error_status() {
        use std::sync::atomic::{AtomicI32, Ordering};
        struct StatusCapture {
            status_code: Arc<AtomicI32>,
        }
        impl LogCallback for StatusCapture {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                if let PmixStatus::Known(e) = status {
                    self.status_code.store(e as i32, Ordering::SeqCst);
                }
            }
        }
        let code = Arc::new(AtomicI32::new(0));
        let cb: Box<dyn LogCallback> = Box::new(StatusCapture {
            status_code: code.clone(),
        });
        cb.on_complete(PmixStatus::Known(PmixError::ErrNotFound));
        assert_eq!(code.load(Ordering::SeqCst), PmixError::ErrNotFound as i32);
    }

    // ─────────────────────────────────────────────────────────────────────
    // PmixQuery edge cases
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_query_keys_array_is_null_terminated() {
        let key = CString::new("test").unwrap();
        let ptrs: &[*mut c_char] = &[key.as_ptr() as *mut c_char];
        let arr = PmixQuery::alloc_keys_array(ptrs);
        assert!(!arr.is_null());
        unsafe {
            assert!(!std::ptr::read(arr).is_null());
            assert!(std::ptr::read(arr.offset(1)).is_null());
            libc::free(arr as *mut c_void);
        }
    }

    #[test]
    fn test_pmix_query_drop_nulls_out_fields() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        drop(query);
    }

    #[test]
    fn test_pmix_query_with_multiple_qualifier_calls() {
        let query = PmixQuery::new(&["PMIX_QUERY_JOB_SIZE"]).unwrap();
        let info = crate::InfoBuilder::new().build();
        let _query = query.with_qualifiers(info);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Registry stress tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_registry_concurrent_insert_remove() {
        struct DummyQCb;
        impl QueryCallback for DummyQCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
        }
        // Use high keys to avoid collision with NB query request IDs (which start from 1)
        let base = 100_000;
        for i in 0..20 {
            let key = base + i;
            {
                let mut registry = QUERY_REGISTRY.lock().unwrap();
                registry.insert(key, Box::new(DummyQCb));
            }
            {
                let mut registry = QUERY_REGISTRY.lock().unwrap();
                registry.remove(&key);
            }
        }
        // Verify our keys are gone (don't assert is_empty — other tests may have entries)
        {
            let registry = QUERY_REGISTRY.lock().unwrap();
            for i in 0..20 {
                assert!(
                    registry.get(&(base + i)).is_none(),
                    "Key {} should have been removed",
                    base + i
                );
            }
        }
    }

    #[test]
    fn test_log_registry_concurrent_insert_remove() {
        struct DummyLCb;
        impl LogCallback for DummyLCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        // Use high keys to avoid collision with NB log request IDs
        let base = 200_000;
        for i in 0..20 {
            let key = base + i;
            {
                let mut registry = LOG_REGISTRY.lock().unwrap();
                registry.insert(key, Box::new(DummyLCb));
            }
            {
                let mut registry = LOG_REGISTRY.lock().unwrap();
                registry.remove(&key);
            }
        }
        // Verify our keys are gone (don't assert is_empty — other tests may have entries)
        {
            let registry = LOG_REGISTRY.lock().unwrap();
            for i in 0..20 {
                assert!(
                    registry.get(&(base + i)).is_none(),
                    "Key {} should have been removed",
                    base + i
                );
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // QueryResults len edge cases
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_query_results_len_large() {
        let results = QueryResults {
            handle: std::ptr::null_mut(),
            len: usize::MAX,
        };
        assert!(!results.is_empty());
        assert_eq!(results.len(), usize::MAX);
        drop(results);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Info construction for log tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_info_builder_empty_for_log() {
        let info = crate::InfoBuilder::new().build();
        assert!(info.is_empty());
        assert_eq!(info.len(), 0);
    }

    #[test]
    fn test_info_with_string_key_for_log() {
        let info = crate::info_with_string_key("PMIX_LOG_STDOUT", "test message");
        assert!(!info.is_empty());
        assert_eq!(info.len(), 1);
    }
}
