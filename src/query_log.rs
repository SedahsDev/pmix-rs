//! Query and logging operations — `PMIx_Query_info` and non-blocking variant.
//!
//! This module provides safe Rust wrappers for querying information from the
//! PMIx host resource manager. The query API allows tools to request specific
//! attributes from the PMIx server without requiring prior publication.
//!
//! # Query model
//!
//! A query consists of one or more `PmixQuery` objects, each containing:
//! - A list of key names to request (`keys`)
//! - Optional qualifier info (`qualifiers`) that narrows or modifies the query
//!
//! Results are returned as a `QueryResults` which auto-frees the C allocation.
//!
//! # C API reference
//!
//! ```c
//! pmix_status_t PMIx_Query_info(pmix_query_t queries[], size_t nqueries,
//!                                pmix_info_t **results, size_t *nresults);
//! pmix_status_t PMIx_Query_info_nb(pmix_query_t queries[], size_t nqueries,
//!                                   pmix_info_cbfunc_t cbfunc, void *cbdata);
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
            std::mem::forget(info);
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
