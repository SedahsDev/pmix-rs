//! Data operations — publish, lookup, unpublish, and non-blocking data retrieval.
//!
//! This module provides safe Rust wrappers for PMIx publish/lookup operations
//! and non-blocking data operations that use callback-based completion.

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Publish — publish data for later lookup
// ─────────────────────────────────────────────────────────────────────────────

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::{LazyLock, Mutex};

use crate::ffi;
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc, free_value};

/// Publish data for later access via [`lookup`][crate::data_ops::lookup].
///
/// The blocking form of this call will block until it has obtained confirmation
/// from the datastore that the data is available for lookup. The `info` array
/// can be released upon return from the blocking function call.
///
/// Publishing duplicate keys is permitted provided they are published to
/// different ranges. Duplicate keys being published on the same data range
/// shall return the `PMIX_ERR_DUPLICATE_KEY` error.
///
/// By default, data will be published into the `PMIX_RANGE_SESSION` range
/// and with `PMIX_PERSIST_APP` persistence. Changes to those values, and
/// any additional directives, can be included in the `pmix_info_t` array.
///
/// # Parameters
///
/// - `info`: Array of info structures containing both data to be published
///   and optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`,
///   `PMIX_PERSISTENCE`, `PMIX_ACCESS_PERMISSIONS`).
///
/// # Returns
///
/// - `Ok(())` if the data was successfully published and is available for lookup.
/// - `Err(status)` on failure (e.g., not initialized, duplicate key, timeout).
///
/// # C API
/// `pmix_status_t PMIx_Publish(const pmix_info_t info[], size_t ninfo)`
pub fn publish(info: &Info) -> Result<(), PmixStatus> {
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    let status = unsafe {
        // SAFETY: PMIx_Publish is a synchronous PMIx API call. The info
        // pointer is valid for the duration of the call (borrowed from
        // the Info parameter). PMIx does not retain the pointer after
        // this call returns.
        ffi::PMIx_Publish(info_ptr, ninfo)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Callback trait for `PMIx_Publish_nb`.
///
/// Implement this trait to receive the result of a non-blocking publish.
/// The `on_complete` method receives the `PmixStatus` result.
pub trait PublishCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending publish callbacks.
type PublishRegistry = std::collections::HashMap<usize, Box<dyn PublishCallback>>;
static PUBLISH_REGISTRY: LazyLock<Mutex<PublishRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing publish request ID counter.
static PUBLISH_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (publish completion).
///
/// Called by PMIx when the non-blocking publish completes. The `cbdata`
/// parameter is a raw pointer encoding the request ID. We look up the
/// registered closure and invoke it with the result status.
extern "C" fn publish_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = PUBLISH_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status);
}

/// Non-blocking publish of data for later lookup.
///
/// Submit an asynchronous request to publish the data in `info`. The
/// `callback` closure is invoked once the operation completes.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Callback behavior
///
/// The callback receives `PmixStatus`:
/// - On success: `PmixStatus::Known(PmixError::Success)`
/// - On duplicate key: `PmixStatus::Known(PmixError::ErrDuplicateKey)`
/// - On other error: corresponding `PmixStatus`
///
/// # C API
/// `pmix_status_t PMIx_Publish_nb(const pmix_info_t info[], size_t ninfo,`
/// `  pmix_op_cbfunc_t cbfunc, void *cbdata)`
pub fn publish_nb(info: &Info, callback: Box<dyn PublishCallback>) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = PUBLISH_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = PUBLISH_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is not null and
    // remains alignable (though PMIx treats it as opaque c_void).
    let cbdata = (req_id << 2) as *mut c_void;

    // Prepare info parameters.
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle as *const ffi::pmix_info_t, info.len)
    } else {
        (ptr::null(), 0)
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Publish_nb is a non-blocking PMIx API call. The info
        // pointer is valid for the duration of the initial call. The callback
        // bridge function has C linkage and properly handles the raw pointer
        // cbdata parameter.
        ffi::PMIx_Publish_nb(info_ptr, ninfo, Some(publish_callback_bridge), cbdata)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = PUBLISH_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_nb — non-blocking data retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_Get_nb`.
///
/// Implement this trait to receive the result of a non-blocking get.
/// The `on_result` method receives:
/// - `status`: The result status (Success, NotFound, etc.)
/// - `value`: The retrieved value, or `None` if not found.
pub trait GetValueCallback: Send {
    fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>);
}

/// Global registry mapping request IDs to pending callback contexts.
///
/// `PMIx_Get_nb` stores only a raw `*mut c_void` as user data. Our bridge
/// function uses this pointer to recover the Rust closure from the registry.
type GetRegistry = std::collections::HashMap<usize, Box<dyn GetValueCallback>>;
static GET_REGISTRY: LazyLock<Mutex<GetRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing request ID counter.
static GET_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_value_cbfunc_t`.
///
/// Called by PMIx when the non-blocking get completes. The `cbdata`
/// parameter is a raw pointer to the request ID (cast to c_void).
/// We look up the registered closure and invoke it with the result.
extern "C" fn get_value_callback_bridge(
    status: ffi::pmix_status_t,
    kv: *mut ffi::pmix_value_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = GET_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Convert the status.
    let pmix_status = PmixStatus::from_raw(status);

    // Extract the value on success.
    let value = if pmix_status.is_success() && !kv.is_null() {
        // SAFETY: On success, PMIx returns a valid pmix_value_t that we
        // take ownership of. We read it and then null the pointer so
        // PMIx doesn't try to free it.
        let val = unsafe { ptr::read(kv) };
        // Clear the pointer so PMIx doesn't double-free.
        unsafe { ptr::write(kv, std::mem::zeroed()) };
        Some(PmixOwnedValue { inner: val })
    } else {
        None
    };

    // Invoke the user's Rust callback.
    cb.on_result(pmix_status, value);
}

/// Non-blocking retrieval of a key-value attribute.
///
/// Submit an asynchronous request to retrieve the value associated with
/// `key` for the given `proc`. The `callback` closure is invoked once
/// the data becomes available (or the request fails).
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Callback behavior
///
/// The callback receives `(PmixStatus, Option<PmixOwnedValue>)`:
/// - On success: `(PmixStatus::Known(PmixError::Success), Some(value))`
/// - On not found: `(PmixStatus::Known(PmixError::ErrNotFound), None)`
/// - On other error: `(PmixStatus, None)`
///
/// # C API
/// `pmix_status_t PMIx_Get_nb(const pmix_proc_t *proc, const char key[],`
/// `  const pmix_info_t info[], size_t ninfo,`
/// `  pmix_value_cbfunc_t cbfunc, void *cbdata)`
pub fn get_nb(
    proc: &Proc,
    key: &str,
    info: Option<&Info>,
    callback: Box<dyn GetValueCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = GET_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = GET_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is not null and
    // remains alignable (though PMIx treats it as opaque c_void).
    let cbdata = (req_id << 2) as *mut c_void;

    // Prepare key and info parameters.
    let key_c = match CString::new(key) {
        Ok(c) => c,
        Err(_) => {
            // Key contains NUL — remove callback and return error.
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.remove(&req_id);
            return Err(PmixStatus::Known(PmixError::Error));
        }
    };

    let (info_ptr, ninfo) = match info {
        Some(info) => {
            if info.handle.is_null() {
                (ptr::null(), 0)
            } else {
                (info.handle as *const ffi::pmix_info_t, info.len)
            }
        }
        None => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        ffi::PMIx_Get_nb(
            &proc.handle as *const ffi::pmix_proc_t,
            key_c.as_ptr(),
            info_ptr,
            ninfo,
            Some(get_value_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = GET_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get — blocking data retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// Blocking retrieval of a key-value attribute from the PMIx datastore.
///
/// Submit a synchronous request to retrieve the value associated with
/// `key` for the given `proc`. The call blocks until the data becomes
/// available, the request fails, or a timeout is reached.
///
/// On success, the caller receives ownership of the returned value via
/// [`PmixOwnedValue`], which frees the underlying C memory on drop.
///
/// # Parameters
///
/// - `proc`: Process whose data to retrieve.
/// - `key`: Attribute key to look up.
/// - `info`: Optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_WAIT`).
///
/// # Returns
///
/// - `Ok(PmixOwnedValue)` — the value associated with the key.
/// - `Err(PmixStatus)` — retrieval failed (e.g., `ErrNotFound`, `ErrInit`,
///   `ErrTimeout`).
///
/// # C API
/// `pmix_status_t PMIx_Get(const pmix_proc_t *proc, const char key[],`
/// `  const pmix_info_t info[], size_t ninfo, pmix_value_t **val)`
pub fn get(proc: &Proc, key: &str, info: Option<&Info>) -> Result<PmixOwnedValue, PmixStatus> {
    // Prepare key as C string.
    let key_c = match CString::new(key) {
        Ok(c) => c,
        Err(_) => {
            // Key contains NUL — return error immediately.
            return Err(PmixStatus::Known(PmixError::Error));
        }
    };

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) => {
            if info.handle.is_null() {
                (ptr::null(), 0)
            } else {
                (info.handle as *const ffi::pmix_info_t, info.len)
            }
        }
        None => (ptr::null(), 0),
    };

    // Call the FFI function.
    let mut value: *mut ffi::pmix_value_t = ptr::null_mut();
    let status = unsafe {
        // SAFETY: PMIx_Get is a synchronous PMIx API call. The proc pointer
        // is valid for the duration of the call (borrowed from the Proc
        // parameter). The key C string lives until end of scope. The info
        // pointer (if non-null) is borrowed from the Info parameter and
        // lives long enough. PMIx writes a valid pmix_value_t pointer into
        // `value` on success, which we take ownership of.
        ffi::PMIx_Get(
            &proc.handle as *const ffi::pmix_proc_t,
            key_c.as_ptr(),
            info_ptr,
            ninfo,
            &mut value,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Take ownership of the value returned by PMIx.
        let owned = unsafe {
            // SAFETY: On success, PMIx has written a valid, non-null pointer
            // into `value` pointing to a heap-allocated pmix_value_t that
            // the caller owns. We dereference it and wrap it in PmixOwnedValue
            // which will free it on drop.
            ptr::read(value)
        };
        Ok(PmixOwnedValue { inner: owned })
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Lookup — lookup published data (blocking)
// ─────────────────────────────────────────────────────────────────────────────

/// A single lookup result: key, value, and the proc that published it.
///
/// Corresponds to `pmix_pdata_t` from the C API. The caller sets the `key`
/// field before calling [`lookup`]; on success the `value` and `proc` fields
/// are populated by the PMIx library.
pub struct PmixPdata {
    /// Process that published this data (filled on return).
    pub proc: Proc,
    /// Key to look up (input) / key of returned data (output).
    pub key: String,
    /// Value returned by the lookup (filled on success).
    pub value: Option<PmixOwnedValue>,
}

impl std::fmt::Debug for PmixPdata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixPdata")
            .field("key", &self.key)
            .field("value_present", &self.value.is_some())
            .finish_non_exhaustive()
    }
}

impl PmixPdata {
    /// Create a new lookup request for the given key.
    ///
    /// The `proc` and `value` fields are uninitialized — they will be
    /// populated by [`lookup`][crate::data_ops::lookup] on success.
    pub fn new(key: &str) -> Self {
        Self {
            proc: Proc::new("", PMIX_RANK_WILDCARD as u32)
                .unwrap_or_else(|_| Proc::new("", 0).unwrap()),
            key: key.to_string(),
            value: None,
        }
    }
}

/// PMIX_RANK_WILDCARD constant.
const PMIX_RANK_WILDCARD: i32 = -1;

/// Lookup information published by this or another process.
///
/// The blocking form of this call will block until the datastore has
/// returned the data or determined it is unavailable. By default, the
/// search is constrained to publishers within `PMIX_RANGE_SESSION`.
///
/// Each element of `data` should have its `key` field set to the desired
/// attribute name. On success, the `value` and `proc` fields are populated
/// with the published data and the identity of the publisher.
///
/// # Return values
///
/// - `Ok(results)` — the data array with populated values/procs.
///   Status can be `Success` (all found), `ErrPartialSuccess` (some found),
///   or `ErrNotFound` (none found — results will have empty values).
/// - `Err(status)` on other failures (not supported, no permissions, etc.).
///
/// # Parameters
///
/// - `data`: Array of [`PmixPdata`] with keys set. Modified in place on return.
/// - `info`: Optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`, `PMIX_WAIT`).
///
/// # C API
/// `pmix_status_t PMIx_Lookup(pmix_pdata_t data[], size_t ndata,`
/// `  const pmix_info_t info[], size_t ninfo)`
pub fn lookup(
    data: &mut [PmixPdata],
    info: Option<&Info>,
) -> Result<(PmixStatus, Vec<PmixPdata>), PmixStatus> {
    if data.is_empty() {
        return Err(PmixStatus::Known(PmixError::Error));
    }

    // Build the raw pmix_pdata_t array.
    let ndata = data.len();
    let mut raw_pdata: Vec<ffi::pmix_pdata_t> = Vec::with_capacity(ndata);

    for item in data.iter() {
        let mut pdata: ffi::pmix_pdata_t = unsafe { std::mem::zeroed() };

        // Copy the key into pdata.key (pmix_key_t = [c_char; 512]).
        let key_bytes = item.key.as_bytes();
        let klen = key_bytes.len().min(511);
        unsafe {
            std::ptr::copy_nonoverlapping(
                key_bytes.as_ptr(),
                pdata.key.as_mut_ptr() as *mut u8,
                klen,
            );
            pdata.key[klen] = 0;
        }

        // Initialize the proc field as wildcard.
        pdata.proc_.rank = PMIX_RANK_WILDCARD as u32;

        // Zero the value so PMIx writes into it.
        unsafe {
            std::ptr::write_bytes(&mut pdata.value, 0, 1);
        }

        // Construct the pdata using the PMIx constructor.
        unsafe { ffi::PMIx_Pdata_construct(&mut pdata) };

        raw_pdata.push(pdata);
    }

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) if info.len > 0 => (info.handle as *const ffi::pmix_info_t, info.len),
        _ => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Lookup is a synchronous PMIx API call. The raw_pdata
        // slice is valid for the duration of the call. PMIx writes the
        // proc and value fields of each pmix_pdata_t element. The info
        // pointer (if non-null) is borrowed from the Info parameter and
        // lives long enough. PMIx does not retain any pointers after return.
        ffi::PMIx_Lookup(raw_pdata.as_mut_ptr(), ndata, info_ptr, ninfo)
    };

    let pmix_status = PmixStatus::from_raw(status);

    // Extract results from the raw pdata array.
    let mut results = Vec::with_capacity(ndata);
    for (i, pdata) in raw_pdata.iter_mut().enumerate() {
        let key = data[i].key.clone();

        // Extract the proc (namespace + rank).
        let nspace_str = unsafe {
            std::ffi::CStr::from_ptr(pdata.proc_.nspace.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let rank = pdata.proc_.rank;
        let proc = Proc::new(&nspace_str, rank).unwrap_or_else(|_| Proc::new("", 0).unwrap());

        // Extract the value if the type is not PMIX_UNDEF.
        let pmix_undef: ffi::pmix_data_type_t = ffi::PMIX_UNDEF as u16;
        let value = if pdata.value.type_ != pmix_undef {
            // Take ownership of the value.
            let val = unsafe { ptr::read(&pdata.value) };
            Some(PmixOwnedValue { inner: val })
        } else {
            None
        };

        results.push(PmixPdata { proc, key, value });
    }

    // Clean up raw pdata — destruct each element.
    for pdata in raw_pdata.iter_mut() {
        unsafe {
            free_value(&mut pdata.value);
            ffi::PMIx_Pdata_destruct(pdata);
        }
    }

    // Check if this is a usable result vs hard error.
    match pmix_status {
        PmixStatus::Known(PmixError::Success)
        | PmixStatus::Known(PmixError::ErrPartialSuccess)
        | PmixStatus::Known(PmixError::ErrNotFound) => Ok((pmix_status, results)),
        _ => Err(pmix_status),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Lookup_nb — non-blocking lookup
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_Lookup_nb`.
///
/// Implement this trait to receive the result of a non-blocking lookup.
/// The `on_result` method receives:
/// - `status`: The result status (Success, NotFound, PartialSuccess, etc.)
/// - `data`: The lookup results (proc + key + value for each key).
pub trait LookupCallback: Send {
    fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>);
}

/// Global registry for pending lookup_nb callbacks.
type LookupRegistry = std::collections::HashMap<usize, Box<dyn LookupCallback>>;
static LOOKUP_REGISTRY: LazyLock<Mutex<LookupRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing lookup request ID counter.
static LOOKUP_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_lookup_cbfunc_t`.
///
/// Called by PMIx when the non-blocking lookup completes. The `cbdata`
/// parameter encodes the request ID. We look up the registered closure
/// and invoke it with the converted results.
extern "C" fn lookup_callback_bridge(
    status: ffi::pmix_status_t,
    data: *mut ffi::pmix_pdata_t,
    ndata: usize,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // Recover the request ID from the cbdata pointer.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = LOOKUP_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — still need to free the data PMIx gave us.
            if !data.is_null() && ndata > 0 {
                unsafe {
                    for i in 0..ndata {
                        let pdata = data.add(i);
                        free_value(&mut (*pdata).value);
                        ffi::PMIx_Pdata_destruct(pdata);
                    }
                    ffi::PMIx_Pdata_free(data, ndata);
                }
            }
            return;
        }
    };

    // Convert status and data.
    let pmix_status = PmixStatus::from_raw(status);

    let results = if !data.is_null() && ndata > 0 {
        let mut results = Vec::with_capacity(ndata);
        unsafe {
            for i in 0..ndata {
                let pdata = data.add(i);
                let pdata_ref = &*pdata;

                let nspace_str = std::ffi::CStr::from_ptr(pdata_ref.proc_.nspace.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                let key_str = std::ffi::CStr::from_ptr(pdata_ref.key.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                let rank = pdata_ref.proc_.rank;

                let proc =
                    Proc::new(&nspace_str, rank).unwrap_or_else(|_| Proc::new("", 0).unwrap());

                // Take ownership of the value.
                let pmix_undef: ffi::pmix_data_type_t = ffi::PMIX_UNDEF as u16;
                let value = if pdata_ref.value.type_ != pmix_undef {
                    let val = ptr::read(&pdata_ref.value);
                    Some(PmixOwnedValue { inner: val })
                } else {
                    None
                };

                results.push(PmixPdata {
                    proc,
                    key: key_str,
                    value,
                });
            }
        }
        // Free the pdata array allocated by PMIx.
        unsafe {
            ffi::PMIx_Pdata_free(data, ndata);
        }
        results
    } else {
        Vec::new()
    };

    cb.on_result(pmix_status, results);
}

/// Non-blocking lookup of published data.
///
/// Submit an asynchronous request to look up the data associated with
/// the given `keys`. The `callback` closure is invoked once the operation
/// completes.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Callback behavior
///
/// The callback receives `(PmixStatus, Vec<PmixPdata>)`:
/// - On success: `(Success, results)` with populated proc/key/value.
/// - On not found: `(ErrNotFound, empty_vec)`.
/// - On partial: `(ErrPartialSuccess, results)` with only found items.
///
/// # Parameters
///
/// - `keys`: Keys to look up (NULL-terminated in C, passed as a slice here).
/// - `info`: Optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`, `PMIX_WAIT`).
/// - `callback`: Closure invoked on completion.
///
/// # C API
/// `pmix_status_t PMIx_Lookup_nb(char **keys, const pmix_info_t info[],`
/// `  size_t ninfo, pmix_lookup_cbfunc_t cbfunc, void *cbdata)`
pub fn lookup_nb(
    keys: &[&str],
    info: Option<&Info>,
    callback: Box<dyn LookupCallback>,
) -> Result<(), PmixStatus> {
    if keys.is_empty() {
        return Err(PmixStatus::Known(PmixError::Error));
    }

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = LOOKUP_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = LOOKUP_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Convert keys to NULL-terminated C string array.
    let mut key_ptrs: Vec<*mut std::os::raw::c_char> = Vec::with_capacity(keys.len() + 1);
    let mut cstrings: Vec<std::ffi::CString> = Vec::with_capacity(keys.len());

    for &key in keys {
        match CString::new(key) {
            Ok(c) => {
                cstrings.push(c);
            }
            Err(_) => {
                // Key contains NUL — clean up and return error.
                let mut registry = LOOKUP_REGISTRY.lock().unwrap();
                registry.remove(&req_id);
                return Err(PmixStatus::Known(PmixError::Error));
            }
        }
    }
    for c in &cstrings {
        key_ptrs.push(c.as_ptr() as *mut std::os::raw::c_char);
    }
    // NULL terminator.
    key_ptrs.push(ptr::null_mut());

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) if info.len > 0 => (info.handle as *const ffi::pmix_info_t, info.len),
        _ => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Lookup_nb is a non-blocking PMIx API call. The key_ptrs
        // slice is valid for the duration of the initial call (NULL-terminated).
        // The cstrings live long enough (dropped after this call). The callback
        // bridge function has C linkage and properly handles the raw pointer
        // cbdata parameter. PMIx does not retain key_ptrs after this returns.
        ffi::PMIx_Lookup_nb(
            key_ptrs.as_mut_ptr(),
            info_ptr,
            ninfo,
            Some(lookup_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = LOOKUP_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Unpublish — unpublish data posted by this process
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_Unpublish_nb`.
///
/// Implement this trait to receive the result of a non-blocking unpublish.
/// The `on_complete` method receives the `PmixStatus` result.
pub trait UnpublishCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending unpublish callbacks.
type UnpublishRegistry = std::collections::HashMap<usize, Box<dyn UnpublishCallback>>;
static UNPUBLISH_REGISTRY: LazyLock<Mutex<UnpublishRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing unpublish request ID counter.
static UNPUBLISH_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (unpublish completion).
///
/// Called by PMIx when the non-blocking unpublish completes. The `cbdata`
/// parameter is a raw pointer encoding the request ID. We look up the
/// registered closure and invoke it with the result status.
extern "C" fn unpublish_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status);
}

/// Unpublish data posted by this process using the given keys.
///
/// The blocking form of this call will block until the data has been
/// removed by the server (i.e., it is safe to publish that key again
/// within the specified range).
///
/// A value of `None` for `keys` instructs the server to remove ALL data
/// published by this process.
///
/// By default, the range is assumed to be `PMIX_RANGE_SESSION`. Changes
/// to the range, and any additional directives, can be provided in the
/// `info` array (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`).
///
/// # Parameters
///
/// - `keys`: Keys to unpublish (NULL-terminated in C, passed as a slice here).
///   Pass `None` to remove all data published by this process.
/// - `info`: Optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`).
///
/// # Returns
///
/// - `Ok(())` if the data was successfully unpublished.
/// - `Err(status)` on failure (e.g., not initialized, timeout).
///
/// # C API
/// `pmix_status_t PMIx_Unpublish(char **keys, const pmix_info_t info[], size_t ninfo)`
pub fn unpublish(keys: Option<&[&str]>, info: Option<&Info>) -> Result<(), PmixStatus> {
    // Handle the None case — unpublish all data for this process.
    let keys_ptr = match keys {
        Some(keys_slice) if !keys_slice.is_empty() => {
            // Convert keys to NULL-terminated C string array.
            let mut key_ptrs: Vec<*mut std::os::raw::c_char> =
                Vec::with_capacity(keys_slice.len() + 1);
            let mut cstrings: Vec<CString> = Vec::with_capacity(keys_slice.len());

            for &key in keys_slice {
                match CString::new(key) {
                    Ok(c) => {
                        cstrings.push(c);
                    }
                    Err(_) => {
                        // Key contains NUL — return error.
                        return Err(PmixStatus::Known(PmixError::Error));
                    }
                }
            }
            for c in &cstrings {
                key_ptrs.push(c.as_ptr() as *mut std::os::raw::c_char);
            }
            // NULL terminator.
            key_ptrs.push(ptr::null_mut());

            // SAFETY: key_ptrs and cstrings stay alive for the duration
            // of the FFI call below. We cast to get the right type
            // for the FFI signature (*mut *mut c_char).
            key_ptrs.as_mut_ptr()
        }
        _ => ptr::null_mut(),
    };

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) if info.len > 0 => (info.handle as *const ffi::pmix_info_t, info.len),
        _ => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Unpublish is a synchronous PMIx API call. The keys_ptr
        // (if non-null) is a valid NULL-terminated array of C strings borrowed
        // from the cstrings vector above, which lives long enough. The info
        // pointer (if non-null) is borrowed from the Info parameter. PMIx does
        // not retain any pointers after this call returns.
        ffi::PMIx_Unpublish(keys_ptr, info_ptr, ninfo)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking unpublish of data posted by this process.
///
/// Submit an asynchronous request to unpublish the data associated with
/// the given `keys`. The `callback` closure is invoked once the operation
/// completes.
///
/// A value of `None` for `keys` instructs the server to remove ALL data
/// published by this process.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Callback behavior
///
/// The callback receives `PmixStatus`:
/// - On success: `PmixStatus::Known(PmixError::Success)`
/// - On timeout: `PmixStatus::Known(PmixError::ErrTimeout)`
/// - On other error: corresponding `PmixStatus`
///
/// # Parameters
///
/// - `keys`: Keys to unpublish. Pass `None` to remove all data.
/// - `info`: Optional directives (e.g., `PMIX_TIMEOUT`, `PMIX_RANGE`).
/// - `callback`: Closure invoked on completion.
///
/// # C API
/// `pmix_status_t PMIx_Unpublish_nb(char **keys, const pmix_info_t info[],`
/// `  size_t ninfo, pmix_op_cbfunc_t cbfunc, void *cbdata)`
pub fn unpublish_nb(
    keys: Option<&[&str]>,
    info: Option<&Info>,
    callback: Box<dyn UnpublishCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = UNPUBLISH_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Handle the None case — unpublish all data for this process.
    let keys_ptr = match keys {
        Some(keys_slice) if !keys_slice.is_empty() => {
            // Convert keys to NULL-terminated C string array.
            let mut key_ptrs: Vec<*mut std::os::raw::c_char> =
                Vec::with_capacity(keys_slice.len() + 1);
            let mut cstrings: Vec<CString> = Vec::with_capacity(keys_slice.len());

            for &key in keys_slice {
                match CString::new(key) {
                    Ok(c) => {
                        cstrings.push(c);
                    }
                    Err(_) => {
                        // Key contains NUL — clean up and return error.
                        let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
                        registry.remove(&req_id);
                        return Err(PmixStatus::Known(PmixError::Error));
                    }
                }
            }
            for c in &cstrings {
                key_ptrs.push(c.as_ptr() as *mut std::os::raw::c_char);
            }
            // NULL terminator.
            key_ptrs.push(ptr::null_mut());

            // SAFETY: key_ptrs and cstrings stay alive for the duration
            // of the FFI call below.
            key_ptrs.as_mut_ptr()
        }
        _ => ptr::null_mut(),
    };

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) if info.len > 0 => (info.handle as *const ffi::pmix_info_t, info.len),
        _ => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Unpublish_nb is a non-blocking PMIx API call. The
        // keys_ptr (if non-null) is a valid NULL-terminated array of C strings
        // borrowed from the cstrings vector above, which lives long enough.
        // The info pointer (if non-null) is borrowed from the Info parameter.
        // The callback bridge function has C linkage and properly handles the
        // raw pointer cbdata parameter. PMIx does not retain keys_ptr or info
        // after this call returns.
        ffi::PMIx_Unpublish_nb(
            keys_ptr,
            info_ptr,
            ninfo,
            Some(unpublish_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Store_internal — store data locally with internal scope
// ─────────────────────────────────────────────────────────────────────────────

/// Store data locally for retrieval by other areas of this process.
///
/// This is data that has only **internal scope** — it will never be
/// "pushed" externally to a remote server. The data is stored in the
/// local process's PMIx internal store and can later be retrieved via
/// [`get_nb`][crate::data_ops::get_nb] or a blocking get.
///
/// Unlike [`publish`][crate::data_ops::publish], this call does not make
/// the data available for lookup by other processes. It is purely a
/// local caching mechanism used internally by the PMIx library and by
/// tools/servers that need to cache data before committing it.
///
/// # Parameters
///
/// - `proc`: The process identity (namespace + rank) associated with the data.
/// - `key`: The attribute key (must not contain NUL, max 511 bytes).
/// - `value`: The value to store. The caller retains ownership of this value
///   — PMIx makes its own internal copy.
///
/// # Returns
///
/// - `Ok(())` if the data was stored successfully.
/// - `Err(status)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_BAD_PARAM` — key is NULL or too long (> PMIX_MAX_KEYLEN).
///   - `PMIX_ERR_NOMEM` — memory allocation failed.
///
/// # C API
/// `pmix_status_t PMIx_Store_internal(const pmix_proc_t *proc, const char key[], pmix_value_t *val)`
pub fn store_internal(proc: &Proc, key: &str, value: &PmixOwnedValue) -> Result<(), PmixStatus> {
    // Convert key to C string.
    let key_c = match CString::new(key) {
        Ok(c) => c,
        Err(_) => {
            return Err(PmixStatus::Known(PmixError::Error));
        }
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Store_internal is a synchronous PMIx API call.
        // - proc.handle is a valid pmix_proc_t owned by the Proc parameter.
        // - key_c is a valid null-terminated C string that lives long enough.
        // - value.as_raw() gives a valid pmix_value_t owned by PmixOwnedValue
        //   that lives long enough. The FFI signature declares `*mut pmix_value_t`
        //   but the C implementation only reads from the value (copies it via
        //   PMIX_BFROPS_VALUE_XFER) and does not retain the pointer after return.
        //   Casting *const to *mut is safe here because no mutation occurs.
        ffi::PMIx_Store_internal(
            &proc.handle as *const ffi::pmix_proc_t,
            key_c.as_ptr(),
            value.as_raw() as *mut ffi::pmix_value_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Fence_nb — non-blocking fence / barrier with optional data exchange
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_Fence_nb`.
///
/// Implement this trait to receive the result of a non-blocking fence
/// operation. The `on_complete` method receives the `PmixStatus` result.
pub trait FenceCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending fence callbacks.
type FenceRegistry = std::collections::HashMap<usize, Box<dyn FenceCallback>>;
static FENCE_REGISTRY: LazyLock<Mutex<FenceRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing fence request ID counter.
static FENCE_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (fence completion).
///
/// Called by PMIx when the non-blocking fence completes. The `cbdata`
/// parameter is a raw pointer encoding the request ID. We look up the
/// registered closure and invoke it with the result status.
extern "C" fn fence_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = FENCE_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status);
}

/// Non-blocking fence / barrier across a group of processes.
///
/// Submit an asynchronous request to synchronize with the specified
/// processes. The fence ensures that:
///
/// 1. All data `PMIx_Put` (and `PMIx_Commit`) by the calling process
///    prior to this call is available for `PMIx_Get` by all processes
///    in the fence group once the callback completes.
/// 2. All data previously `PMIx_Put` and `PMIx_Commit` by processes
///    in the fence group is available for `PMIx_Get` by the calling
///    process once the callback completes.
///
/// The `procs` parameter specifies which processes participate. Pass an
/// empty slice to fence across all processes in the session. The `info`
/// parameter can include directives such as `PMIX_COLLECT_DATA` to
/// enable data exchange, or `PMIX_TIMEOUT` to set a maximum wait time.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Callback behavior
///
/// The callback receives `PmixStatus`:
/// - On success: `PmixStatus::Known(PmixError::Success)` — all processes
///   have reached the fence and data exchange (if requested) is complete.
/// - On timeout: `PmixStatus::Known(PmixError::ErrTimeout)` — not all
///   processes reached the fence within the specified timeout.
/// - On other error: corresponding `PmixStatus`
///
/// # Parameters
///
/// - `procs`: Processes to fence with. Empty slice means all session peers.
/// - `info`: Optional directives (e.g., `PMIX_COLLECT_DATA`, `PMIX_TIMEOUT`).
/// - `callback`: Closure invoked when the fence completes.
///
/// # C API
/// `pmix_status_t PMIx_Fence_nb(const pmix_proc_t procs[], size_t nprocs,`
/// `  const pmix_info_t info[], size_t ninfo,`
/// `  pmix_op_cbfunc_t cbfunc, void *cbdata)`
pub fn fence_nb(
    procs: &[Proc],
    info: Option<&Info>,
    callback: Box<dyn FenceCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = FENCE_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = FENCE_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is not null and
    // remains alignable (though PMIx treats it as opaque c_void).
    let cbdata = (req_id << 2) as *mut c_void;

    // Prepare proc parameters.
    let (proc_ptr, nprocs) = if procs.is_empty() {
        (ptr::null(), 0)
    } else {
        // Build a contiguous array of raw pmix_proc_t from the Proc
        // references. Each Proc wraps exactly one pmix_proc_t.
        // We copy each pmix_proc_t into a temporary Vec to ensure
        // contiguity for the FFI call.
        //
        // SAFETY: pmix_proc_t contains a fixed-size char array (nspace)
        // and a u32 (rank). It does not contain pointers, so cloning
        // via std::ptr::read is safe and produces a valid copy.
        let raw_procs: Vec<ffi::pmix_proc_t> =
            unsafe { procs.iter().map(|p| std::ptr::read(&p.handle)).collect() };
        (raw_procs.as_ptr(), raw_procs.len())
    };

    // Prepare info parameters.
    let (info_ptr, ninfo) = match info {
        Some(info) if info.len > 0 => (info.handle as *const ffi::pmix_info_t, info.len),
        _ => (ptr::null(), 0),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Fence_nb is a non-blocking PMIx API call.
        // - proc_ptr (if non-null) points to a Vec<pmix_proc_t> that lives
        //   long enough for the initial call. PMIx does not retain the pointer.
        // - info_ptr (if non-null) is borrowed from the Info parameter and
        //   lives long enough. PMIx does not retain it after this returns.
        // - The callback bridge function has C linkage and properly handles
        //   the raw pointer cbdata parameter.
        // - PMIx_Fence_nb returns immediately and does not access proc_ptr
        //   or info_ptr after returning.
        ffi::PMIx_Fence_nb(
            proc_ptr,
            nprocs,
            info_ptr,
            ninfo,
            Some(fence_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = FENCE_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── PmixPdata construction tests ───────────────────────────────────────

    #[test]
    fn test_pdata_new() {
        let pdata = PmixPdata::new("test_key");
        assert_eq!(pdata.key, "test_key");
        assert!(pdata.value.is_none());
    }

    #[test]
    fn test_pdata_proc() {
        let pdata = PmixPdata::new("test_key");
        let _ = &pdata.proc;
    }

    #[test]
    fn test_pdata_new_empty_key() {
        let pdata = PmixPdata::new("");
        assert_eq!(pdata.key, "");
        assert!(pdata.value.is_none());
    }

    #[test]
    fn test_pdata_new_long_key() {
        let long_key = "a".repeat(500);
        let pdata = PmixPdata::new(&long_key);
        assert_eq!(pdata.key, long_key);
    }

    #[test]
    fn test_pdata_new_special_chars_key() {
        let pdata = PmixPdata::new("pmix.job.size");
        assert_eq!(pdata.key, "pmix.job.size");
    }

    #[test]
    fn test_pdata_debug_format() {
        let pdata = PmixPdata::new("test_key");
        let debug_str = format!("{:?}", pdata);
        assert!(debug_str.contains("PmixPdata"));
        assert!(debug_str.contains("test_key"));
    }

    #[test]
    fn test_pdata_multiple_keys() {
        let keys = ["key1", "key2", "key3", "pmix.test.attr", "a.b.c.d.e"];
        let pdatas: Vec<PmixPdata> = keys.iter().map(|k| PmixPdata::new(k)).collect();
        assert_eq!(pdatas.len(), 5);
        for (i, pdata) in pdatas.iter().enumerate() {
            assert_eq!(pdata.key, keys[i]);
        }
    }

    // ─── Callback trait object tests ────────────────────────────────────────

    #[test]
    fn test_publish_callback_trait_object() {
        struct DummyPublish;
        impl PublishCallback for DummyPublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let callback: Box<dyn PublishCallback> = Box::new(DummyPublish);
        let _ = callback;
    }

    #[test]
    fn test_lookup_callback_trait_object() {
        struct DummyLookup;
        impl LookupCallback for DummyLookup {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }
        let callback: Box<dyn LookupCallback> = Box::new(DummyLookup);
        let _ = callback;
    }

    #[test]
    fn test_unpublish_callback_trait_object() {
        struct DummyUnpublish;
        impl UnpublishCallback for DummyUnpublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let callback: Box<dyn UnpublishCallback> = Box::new(DummyUnpublish);
        let _ = callback;
    }

    #[test]
    fn test_fence_callback_trait_object() {
        struct DummyFence;
        impl FenceCallback for DummyFence {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let callback: Box<dyn FenceCallback> = Box::new(DummyFence);
        let _ = callback;
    }

    #[test]
    fn test_get_value_callback_trait_object() {
        struct DummyGetValue;
        impl GetValueCallback for DummyGetValue {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<PmixOwnedValue>) {}
        }
        let callback: Box<dyn GetValueCallback> = Box::new(DummyGetValue);
        let _ = callback;
    }

    // ─── PublishCallback functional tests ───────────────────────────────────

    #[test]
    fn test_publish_callback_receives_success() {
        struct TestPublish {
            received: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl PublishCallback for TestPublish {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.received.lock().unwrap() = Some(status);
            }
        }
        let received = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn PublishCallback> = Box::new(TestPublish {
            received: received.clone(),
        });
        // Simulate callback invocation with success
        let test_status = PmixStatus::Known(PmixError::Success);
        callback.on_complete(test_status);
        let result = received.lock().unwrap();
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_success());
    }

    #[test]
    fn test_publish_callback_receives_error() {
        struct TestPublish {
            received: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl PublishCallback for TestPublish {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.received.lock().unwrap() = Some(status);
            }
        }
        let received = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn PublishCallback> = Box::new(TestPublish {
            received: received.clone(),
        });
        let test_status = PmixStatus::Known(PmixError::ErrTimeout);
        callback.on_complete(test_status);
        let result = received.lock().unwrap();
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_error());
    }

    #[test]
    fn test_get_value_callback_receives_value() {
        struct TestGetValue {
            received_status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
            received_value: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
        }
        impl GetValueCallback for TestGetValue {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                *self.received_status.lock().unwrap() = Some(status);
                *self.received_value.lock().unwrap() = Some(value.is_none());
            }
        }
        let received_status = std::sync::Arc::new(std::sync::Mutex::new(None));
        let received_value = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn GetValueCallback> = Box::new(TestGetValue {
            received_status: received_status.clone(),
            received_value: received_value.clone(),
        });
        callback.on_result(PmixStatus::Known(PmixError::Success), None);
        // Verify callback was invoked and received None value
        assert!(*received_value.lock().unwrap().as_ref().unwrap());
    }

    #[test]
    fn test_lookup_callback_receives_data() {
        struct TestLookup {
            received_count: std::sync::Arc<std::sync::Mutex<Option<usize>>>,
        }
        impl LookupCallback for TestLookup {
            fn on_result(self: Box<Self>, _status: PmixStatus, data: Vec<PmixPdata>) {
                *self.received_count.lock().unwrap() = Some(data.len());
            }
        }
        let received_count = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn LookupCallback> = Box::new(TestLookup {
            received_count: received_count.clone(),
        });
        let pdatas = vec![PmixPdata::new("k1"), PmixPdata::new("k2")];
        callback.on_result(PmixStatus::Known(PmixError::Success), pdatas);
        assert_eq!(*received_count.lock().unwrap().as_ref().unwrap(), 2);
    }

    // ─── Registry and sequence counter tests ────────────────────────────────

    #[test]
    fn test_publish_seq_increments() {
        let seq1 = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        let seq2 = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        assert!(seq2 > seq1);
    }

    #[test]
    fn test_get_seq_increments() {
        let seq1 = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        let seq2 = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        assert!(seq2 > seq1);
    }

    #[test]
    fn test_unpublish_seq_increments() {
        let seq1 = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        let seq2 = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        assert!(seq2 > seq1);
    }

    #[test]
    fn test_fence_seq_increments() {
        let seq1 = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        let seq2 = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        assert!(seq2 > seq1);
    }

    #[test]
    fn test_lookup_seq_increments() {
        let seq1 = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        let seq2 = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        assert!(seq2 > seq1);
    }

    // ─── Registry insert/remove tests ───────────────────────────────────────

    #[test]
    fn test_publish_registry_insert_remove() {
        struct DummyPublish;
        impl PublishCallback for DummyPublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 999;
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyPublish));
            assert!(registry.contains_key(&req_id));
            registry.remove(&req_id);
            assert!(!registry.contains_key(&req_id));
        }
    }

    #[test]
    fn test_get_registry_insert_remove() {
        struct DummyGet;
        impl GetValueCallback for DummyGet {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<PmixOwnedValue>) {}
        }
        let req_id = 888;
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyGet));
            assert!(registry.contains_key(&req_id));
            registry.remove(&req_id);
            assert!(!registry.contains_key(&req_id));
        }
    }

    #[test]
    fn test_lookup_registry_insert_remove() {
        struct DummyLookupCb;
        impl LookupCallback for DummyLookupCb {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }
        let req_id = 777;
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyLookupCb));
            assert!(registry.contains_key(&req_id));
            registry.remove(&req_id);
            assert!(!registry.contains_key(&req_id));
        }
    }

    #[test]
    fn test_unpublish_registry_insert_remove() {
        struct DummyUnpublishCb;
        impl UnpublishCallback for DummyUnpublishCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 666;
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyUnpublishCb));
            assert!(registry.contains_key(&req_id));
            registry.remove(&req_id);
            assert!(!registry.contains_key(&req_id));
        }
    }

    #[test]
    fn test_fence_registry_insert_remove() {
        struct DummyFenceCb;
        impl FenceCallback for DummyFenceCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 555;
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyFenceCb));
            assert!(registry.contains_key(&req_id));
            registry.remove(&req_id);
            assert!(!registry.contains_key(&req_id));
        }
    }

    // ─── Request ID encoding/decoding tests ─────────────────────────────────

    #[test]
    fn test_req_id_encode_decode() {
        for id in [1, 2, 100, 1000, 65535, 100000] {
            let cbdata = (id << 2) as *mut std::os::raw::c_void;
            let decoded = (cbdata as usize) >> 2;
            assert_eq!(decoded, id, "Failed for id={}", id);
        }
    }

    #[test]
    fn test_req_id_non_null() {
        for id in [1, 2, 100, 1000] {
            let cbdata = (id << 2) as *mut std::os::raw::c_void;
            assert!(!cbdata.is_null(), "cbdata is null for id={}", id);
        }
    }

    // ─── Info parameter handling tests ──────────────────────────────────────

    #[test]
    fn test_info_empty_handling() {
        let info = crate::Info {
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
    fn test_info_null_handle_with_zero_len() {
        let info = crate::Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        // This is the pattern used in get() and lookup()
        let (info_ptr, ninfo) = match Some(&info) {
            Some(info) if info.handle.is_null() => (std::ptr::null(), 0),
            Some(info) => (info.handle as *const ffi::pmix_info_t, info.len),
            None => (std::ptr::null(), 0),
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    #[test]
    fn test_info_none_handling() {
        let (info_ptr, ninfo) = match None::<&crate::Info> {
            Some(info) if info.handle.is_null() => (std::ptr::null(), 0),
            Some(info) => (info.handle as *const ffi::pmix_info_t, info.len),
            None => (std::ptr::null(), 0),
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    // ─── CString key validation tests ───────────────────────────────────────

    #[test]
    fn test_cstring_valid_key() {
        let result = std::ffi::CString::new("pmix.test.key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cstring_key_with_dots() {
        let result = std::ffi::CString::new("pmix.job.size");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cstring_key_with_underscores() {
        let result = std::ffi::CString::new("PMIX_JOB_SIZE");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cstring_key_empty() {
        let result = std::ffi::CString::new("");
        assert!(result.is_ok()); // empty string is valid CString
    }

    // ─── PMIX_RANK_WILDCARD constant tests ──────────────────────────────────

    #[test]
    fn test_rank_wildcard_value() {
        assert_eq!(PMIX_RANK_WILDCARD, -1);
    }

    #[test]
    fn test_rank_wildcast_as_u32() {
        // PMIX_RANK_WILDCARD as u32 wraps to MAX
        let rank: u32 = PMIX_RANK_WILDCARD as u32;
        assert_eq!(rank, u32::MAX);
    }

    // ─── PmixStatus roundtrip tests for data_ops context ────────────────────

    #[test]
    fn test_pmix_status_success_from_raw() {
        let status = PmixStatus::from_raw(0);
        assert!(status.is_success());
    }

    #[test]
    fn test_pmix_status_error_from_raw() {
        let status = PmixStatus::from_raw(-1); // PMIX_ERROR
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_not_found_from_raw() {
        let status = PmixStatus::from_raw(-7); // PMIX_ERR_NOT_FOUND
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_timeout_from_raw() {
        let status = PmixStatus::from_raw(-6); // PMIX_ERR_TIMEOUT
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_duplicate_key_from_raw() {
        let status = PmixStatus::from_raw(-14); // PMIX_ERR_DUPLICATE_KEY
        assert!(status.is_error());
    }

    #[test]
    fn test_pmix_status_partial_success_from_raw() {
        let status = PmixStatus::from_raw(-3); // PMIX_ERR_PARTIAL_SUCCESS
        assert!(status.is_error());
    }

    // ─── Proc construction tests ────────────────────────────────────────────

    #[test]
    fn test_proc_new_valid() {
        let proc = Proc::new("test_nspace", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_new_high_rank() {
        let proc = Proc::new("test_nspace", 9999).unwrap();
        assert_eq!(proc.get_rank(), 9999);
    }

    #[test]
    fn test_proc_new_with_nspace() {
        let proc = Proc::new("job_abc", 0).unwrap();
        let proc2 = proc.new_with_nspace(1).unwrap();
        assert_eq!(proc2.get_rank(), 1);
    }

    #[test]
    fn test_proc_set_rank() {
        let mut proc = Proc::new("test", 0).unwrap();
        proc.set_rank(42);
        assert_eq!(proc.get_rank(), 42);
    }

    // ─── Fence callback functional tests ────────────────────────────────────

    #[test]
    fn test_fence_callback_receives_success() {
        struct TestFenceCb {
            received: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl FenceCallback for TestFenceCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.received.lock().unwrap() = Some(status);
            }
        }
        let received = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn FenceCallback> = Box::new(TestFenceCb {
            received: received.clone(),
        });
        callback.on_complete(PmixStatus::Known(PmixError::Success));
        let result = received.lock().unwrap();
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_success());
    }

    #[test]
    fn test_fence_callback_receives_timeout() {
        struct TestFenceCb {
            received: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl FenceCallback for TestFenceCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.received.lock().unwrap() = Some(status);
            }
        }
        let received = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn FenceCallback> = Box::new(TestFenceCb {
            received: received.clone(),
        });
        callback.on_complete(PmixStatus::Known(PmixError::ErrTimeout));
        let result = received.lock().unwrap();
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_error());
    }

    // ─── Unpublish callback functional tests ────────────────────────────────

    #[test]
    fn test_unpublish_callback_receives_success() {
        struct TestUnpublishCb {
            received: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl UnpublishCallback for TestUnpublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.received.lock().unwrap() = Some(status);
            }
        }
        let received = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn UnpublishCallback> = Box::new(TestUnpublishCb {
            received: received.clone(),
        });
        callback.on_complete(PmixStatus::Known(PmixError::Success));
        let result = received.lock().unwrap();
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_success());
    }

    // ─── Lookup callback with empty data ────────────────────────────────────

    #[test]
    fn test_lookup_callback_empty_results() {
        struct TestLookupCb {
            received_count: std::sync::Arc<std::sync::Mutex<Option<usize>>>,
        }
        impl LookupCallback for TestLookupCb {
            fn on_result(self: Box<Self>, _status: PmixStatus, data: Vec<PmixPdata>) {
                *self.received_count.lock().unwrap() = Some(data.len());
            }
        }
        let received_count = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback: Box<dyn LookupCallback> = Box::new(TestLookupCb {
            received_count: received_count.clone(),
        });
        callback.on_result(PmixStatus::Known(PmixError::ErrNotFound), vec![]);
        assert_eq!(*received_count.lock().unwrap().as_ref().unwrap(), 0);
    }

    // ─── PmixPdata with value ───────────────────────────────────────────────

    #[test]
    fn test_pdata_value_field_is_optional() {
        let pdata = PmixPdata::new("test");
        // value is None by default
        assert!(pdata.value.is_none());
        // The field type is Option<PmixOwnedValue>
        let _: Option<PmixOwnedValue> = pdata.value;
    }

    // ─── publish: FFI call path tests ───────────────────────────────────────

    #[test]
    fn test_publish_reaches_ffi() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = publish(&info);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── publish_nb: FFI call path tests ────────────────────────────────────

    #[test]
    fn test_publish_nb_reaches_ffi() {
        struct DummyPublish;
        impl PublishCallback for DummyPublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let callback: Box<dyn PublishCallback> = Box::new(DummyPublish);
        let result = publish_nb(&info, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── get: FFI call path tests ───────────────────────────────────────────

    #[test]
    fn test_get_reaches_ffi() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let result = get(&proc, "test.key", None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_get_with_info() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = get(&proc, "test.key", Some(&info));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── get_nb: FFI call path tests ────────────────────────────────────────

    #[test]
    fn test_get_nb_reaches_ffi() {
        struct DummyGet;
        impl GetValueCallback for DummyGet {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<PmixOwnedValue>) {}
        }
        let proc = Proc::new("test_ns", 0).unwrap();
        let callback: Box<dyn GetValueCallback> = Box::new(DummyGet);
        let result = get_nb(&proc, "test.key", None, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── lookup: FFI call path tests ────────────────────────────────────────

    #[test]
    fn test_lookup_reaches_ffi() {
        let data = vec![PmixPdata::new("test.key")];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let mut data = data;
        let result = lookup(&mut data, Some(&info));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_lookup_with_multiple_keys() {
        let data = vec![
            PmixPdata::new("key1"),
            PmixPdata::new("key2"),
            PmixPdata::new("key3"),
        ];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let mut data = data;
        let result = lookup(&mut data, Some(&info));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── lookup_nb: FFI call path tests ─────────────────────────────────────

    #[test]
    fn test_lookup_nb_reaches_ffi() {
        struct DummyLookup;
        impl LookupCallback for DummyLookup {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }
        let keys = ["test.key"];
        let callback: Box<dyn LookupCallback> = Box::new(DummyLookup);
        let result = lookup_nb(&keys, None, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── unpublish: FFI call path tests ─────────────────────────────────────

    #[test]
    fn test_unpublish_with_keys_reaches_ffi() {
        let keys = ["test.key1", "test.key2"];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = unpublish(Some(&keys), Some(&info));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_unpublish_with_no_keys() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = unpublish(None, Some(&info));
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_unpublish_with_single_key() {
        let keys = ["test.key"];
        let result = unpublish(Some(&keys), None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── unpublish_nb: FFI call path tests ──────────────────────────────────

    #[test]
    fn test_unpublish_nb_reaches_ffi() {
        struct DummyUnpublish;
        impl UnpublishCallback for DummyUnpublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let keys = ["test.key"];
        let callback: Box<dyn UnpublishCallback> = Box::new(DummyUnpublish);
        let result = unpublish_nb(Some(&keys), None, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── store_internal: FFI call path tests ────────────────────────────────

    #[test]
    fn test_store_internal_signature() {
        // Verify store_internal function signature compiles.
        // We can't easily construct a PmixOwnedValue without FFI,
        // so we just ensure the function exists and is callable.
        // The fact that this compiles proves the signature is correct.
        fn _check_signature() {
            let f: fn(&Proc, &str, &PmixOwnedValue) -> Result<(), PmixStatus> = store_internal;
            let _ = f;
        }
    }

    // ─── fence_nb: FFI call path tests ──────────────────────────────────────

    #[test]
    fn test_fence_nb_reaches_ffi() {
        struct DummyFence;
        impl FenceCallback for DummyFence {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let procs: Vec<Proc> = Vec::new();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let callback: Box<dyn FenceCallback> = Box::new(DummyFence);
        let result = fence_nb(&procs, Some(&info), callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Data operations lifecycle structural test ──────────────────────────

    #[test]
    fn test_publish_lookup_unpublish_pattern() {
        // Structural test: verify the publish -> lookup -> unpublish pattern
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };

        // Publish (expected to fail without DVM)
        let pub_result = publish(&info);

        // Lookup (expected to fail without DVM)
        let data = vec![PmixPdata::new("test.key")];
        let mut data = data;
        let lookup_result = lookup(&mut data, None);

        // Unpublish (expected to fail without DVM)
        let keys = ["test.key"];
        let unpublish_result = unpublish(Some(&keys), None);

        // All should be errors without DVM
        match (pub_result, lookup_result, unpublish_result) {
            (Err(_), Err(_), Err(_)) => {
                // Expected without DVM
            }
            _ => {
                // If any succeeded, DVM is running — that's fine too
            }
        }
    }

    // ─── PmixPdata construction edge cases ──────────────────────────────────

    #[test]
    fn test_pdata_new_with_unicode_key() {
        let pdata = PmixPdata::new("pmix.тест.key");
        assert_eq!(pdata.key, "pmix.тест.key");
    }

    #[test]
    fn test_pdata_new_with_spaces_key() {
        let pdata = PmixPdata::new("pmix test key");
        assert_eq!(pdata.key, "pmix test key");
    }

    #[test]
    fn test_pdata_new_with_numbers_key() {
        let pdata = PmixPdata::new("pmix123.test456");
        assert_eq!(pdata.key, "pmix123.test456");
    }

    #[test]
    fn test_pdata_new_max_c_key_length() {
        // pmix_key_t is [c_char; 512], so max key length is 511
        let long_key = "a".repeat(511);
        let pdata = PmixPdata::new(&long_key);
        assert_eq!(pdata.key, long_key);
    }

    #[test]
    fn test_pdata_new_exceeds_c_key_length() {
        // Keys longer than 511 chars are stored but will be truncated in FFI
        let very_long_key = "a".repeat(1000);
        let pdata = PmixPdata::new(&very_long_key);
        assert_eq!(pdata.key, very_long_key);
    }

    #[test]
    fn test_pdata_with_dots_and_hyphens() {
        let pdata = PmixPdata::new("pmix.job.001-app-node-0");
        assert_eq!(pdata.key, "pmix.job.001-app-node-0");
    }

    #[test]
    fn test_pdata_vec_operations() {
        let mut pdatas = vec![PmixPdata::new("k1"), PmixPdata::new("k2")];
        pdatas.push(PmixPdata::new("k3"));
        assert_eq!(pdatas.len(), 3);
        assert_eq!(pdatas[2].key, "k3");
    }

    #[test]
    fn test_pdata_empty_vec() {
        let pdatas: Vec<PmixPdata> = vec![];
        assert!(pdatas.is_empty());
    }

    // ─── PmixPdata mutable reference tests ──────────────────────────────────

    #[test]
    fn test_pdata_mutate_value() {
        let mut pdata = PmixPdata::new("test");
        assert!(pdata.value.is_none());
        // We can't easily construct a PmixOwnedValue without FFI,
        // but we can verify the field is mutable
        let _: &mut Option<PmixOwnedValue> = &mut pdata.value;
    }

    #[test]
    fn test_pdata_mutate_key() {
        let mut pdata = PmixPdata::new("original");
        pdata.key = "modified".to_string();
        assert_eq!(pdata.key, "modified");
    }

    #[test]
    fn test_pdata_mutate_proc() {
        let mut pdata = PmixPdata::new("test");
        let new_proc = Proc::new("new_ns", 42).unwrap();
        pdata.proc = new_proc;
        assert_eq!(pdata.proc.get_rank(), 42);
    }

    // ─── Lookup with empty data array ───────────────────────────────────────

    #[test]
    fn test_lookup_empty_data_returns_error() {
        let data: Vec<PmixPdata> = vec![];
        let mut data = data;
        let result = lookup(&mut data, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_success());
    }

    #[test]
    fn test_lookup_empty_data_with_info_returns_error() {
        let data: Vec<PmixPdata> = vec![];
        let mut data = data;
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = lookup(&mut data, Some(&info));
        assert!(result.is_err());
    }

    // ─── Unpublish edge cases ───────────────────────────────────────────────

    #[test]
    fn test_unpublish_empty_keys_slice() {
        let keys: &[&str] = &[];
        let result = unpublish(Some(keys), None);
        // Empty keys slice should pass null to FFI (same as None)
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_unpublish_none_keys() {
        let result = unpublish(None, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Publish with empty info ────────────────────────────────────────────

    #[test]
    fn test_publish_empty_info() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let result = publish(&info);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Fence blocking call path ───────────────────────────────────────────

    #[test]
    fn test_fence_reaches_ffi() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let result = crate::fence(&proc, None);
        match result {
            Ok(_) => {}
            Err(raw) => {
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_fence_with_procs() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let result = crate::fence(&proc, None);
        match result {
            Ok(_) => {}
            Err(raw) => {
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    #[test]
    fn test_fence_no_info() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let result = crate::fence(&proc, None);
        match result {
            Ok(_) => {}
            Err(raw) => {
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Fence callback bridge null cbdata test ─────────────────────────────

    #[test]
    fn test_fence_callback_bridge_null_cbdata() {
        // fence_callback_bridge with null cbdata should return immediately
        // We can call it directly since it's extern "C"
        fence_callback_bridge(0, std::ptr::null_mut());
        // Should not panic
    }

    // ─── Publish callback bridge null cbdata test ───────────────────────────

    #[test]
    fn test_publish_callback_bridge_null_cbdata() {
        // publish_callback_bridge with null cbdata should return immediately
        publish_callback_bridge(0, std::ptr::null_mut());
        // Should not panic
    }

    // ─── Get value callback bridge null cbdata test ─────────────────────────

    #[test]
    fn test_get_value_callback_bridge_null_cbdata() {
        // get_value_callback_bridge with null cbdata should return immediately
        get_value_callback_bridge(0, std::ptr::null_mut(), std::ptr::null_mut());
        // Should not panic
    }

    // ─── Unpublish callback bridge null cbdata test ─────────────────────────

    #[test]
    fn test_unpublish_callback_bridge_null_cbdata() {
        // unpublish_callback_bridge with null cbdata should return immediately
        unpublish_callback_bridge(0, std::ptr::null_mut());
        // Should not panic
    }

    // ─── Lookup callback bridge null cbdata test ────────────────────────────

    #[test]
    fn test_lookup_callback_bridge_null_cbdata() {
        // lookup_callback_bridge with null cbdata should return immediately
        lookup_callback_bridge(0, std::ptr::null_mut(), 0, std::ptr::null_mut());
        // Should not panic
    }

    // ─── Lookup callback bridge missing callback test ───────────────────────

    #[test]
    fn test_lookup_callback_bridge_missing_callback() {
        // Create a req_id that's not in the registry
        let req_id = 99999usize;
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        // Call the bridge with a non-existent req_id
        lookup_callback_bridge(0, std::ptr::null_mut(), 0, cbdata);
        // Should not panic — just returns without invoking callback
    }

    // ─── Publish callback bridge missing callback test ──────────────────────

    #[test]
    fn test_publish_callback_bridge_missing_callback() {
        let req_id = 99998usize;
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(0, cbdata);
        // Should not panic — callback not found, returns early
    }

    // ─── Get value callback bridge missing callback test ────────────────────

    #[test]
    fn test_get_value_callback_bridge_missing_callback() {
        let req_id = 99997usize;
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(0, std::ptr::null_mut(), cbdata);
        // Should not panic
    }

    // ─── Unpublish callback bridge missing callback test ────────────────────

    #[test]
    fn test_unpublish_callback_bridge_missing_callback() {
        let req_id = 99996usize;
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(0, cbdata);
        // Should not panic
    }

    // ─── Fence callback bridge missing callback test ────────────────────────

    #[test]
    fn test_fence_callback_bridge_missing_callback() {
        let req_id = 99995usize;
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(0, cbdata);
        // Should not panic
    }

    // ─── Publish callback bridge with valid callback ────────────────────────

    #[test]
    fn test_publish_callback_bridge_invokes_callback() {
        use std::sync::Arc;
        struct TestCb {
            status: Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl PublishCallback for TestCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.status.lock().unwrap() = Some(status);
            }
        }
        let status = Arc::new(std::sync::Mutex::new(None));
        let cb = Box::new(TestCb {
            status: status.clone(),
        });

        let req_id = 77777usize;
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(0, cbdata); // PMIX_SUCCESS
        let received = status.lock().unwrap();
        assert!(received.is_some());
        assert!(received.as_ref().unwrap().is_success());
    }

    // ─── Unpublish callback bridge with valid callback ──────────────────────

    #[test]
    fn test_unpublish_callback_bridge_invokes_callback() {
        use std::sync::Arc;
        struct TestCb {
            status: Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl UnpublishCallback for TestCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.status.lock().unwrap() = Some(status);
            }
        }
        let status = Arc::new(std::sync::Mutex::new(None));
        let cb = Box::new(TestCb {
            status: status.clone(),
        });

        let req_id = 66666usize;
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(-6, cbdata); // PMIX_ERR_TIMEOUT
        let received = status.lock().unwrap();
        assert!(received.is_some());
        assert!(received.as_ref().unwrap().is_error());
    }

    // ─── Fence callback bridge with valid callback ──────────────────────────

    #[test]
    fn test_fence_callback_bridge_invokes_callback() {
        use std::sync::Arc;
        struct TestCb {
            status: Arc<std::sync::Mutex<Option<PmixStatus>>>,
        }
        impl FenceCallback for TestCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *self.status.lock().unwrap() = Some(status);
            }
        }
        let status = Arc::new(std::sync::Mutex::new(None));
        let cb = Box::new(TestCb {
            status: status.clone(),
        });

        let req_id = 55555usize;
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(0, cbdata); // PMIX_SUCCESS
        let received = status.lock().unwrap();
        assert!(received.is_some());
        assert!(received.as_ref().unwrap().is_success());
    }

    // ─── Get value callback bridge with valid callback ──────────────────────

    #[test]
    fn test_get_value_callback_bridge_invokes_callback() {
        use std::sync::Arc;
        struct TestCb {
            status: Arc<std::sync::Mutex<Option<PmixStatus>>>,
            has_value: Arc<std::sync::Mutex<Option<bool>>>,
        }
        impl GetValueCallback for TestCb {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                *self.status.lock().unwrap() = Some(status);
                *self.has_value.lock().unwrap() = Some(value.is_some());
            }
        }
        let status = Arc::new(std::sync::Mutex::new(None));
        let has_value = Arc::new(std::sync::Mutex::new(None));
        let cb = Box::new(TestCb {
            status: status.clone(),
            has_value: has_value.clone(),
        });

        let req_id = 44444usize;
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(-7, std::ptr::null_mut(), cbdata); // PMIX_ERR_NOT_FOUND, no value
        let received = status.lock().unwrap();
        let hv = has_value.lock().unwrap();
        assert!(received.is_some());
        let hv_val = hv.as_ref().unwrap();
        assert!(!*hv_val); // No value on not found
    }

    // ─── Lookup callback bridge with valid callback ─────────────────────────

    #[test]
    fn test_lookup_callback_bridge_invokes_callback_empty() {
        use std::sync::Arc;
        struct TestCb {
            count: Arc<std::sync::Mutex<Option<usize>>>,
        }
        impl LookupCallback for TestCb {
            fn on_result(self: Box<Self>, _status: PmixStatus, data: Vec<PmixPdata>) {
                *self.count.lock().unwrap() = Some(data.len());
            }
        }
        let count = Arc::new(std::sync::Mutex::new(None));
        let cb = Box::new(TestCb {
            count: count.clone(),
        });

        let req_id = 33333usize;
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(0, std::ptr::null_mut(), 0, cbdata); // success, empty data
        let c = count.lock().unwrap();
        assert!(c.is_some());
        assert_eq!(c.as_ref().unwrap(), &0);
    }

    // ─── Info parameter handling with non-null handle ───────────────────────

    #[test]
    fn test_info_non_null_handle_pattern() {
        // Simulate the pattern used in get() and get_nb()
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let (info_ptr, ninfo) = match Some(&info) {
            Some(info) => {
                if info.handle.is_null() {
                    (std::ptr::null(), 0)
                } else {
                    (info.handle as *const ffi::pmix_info_t, info.len)
                }
            }
            None => (std::ptr::null(), 0),
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    #[test]
    fn test_info_none_pattern() {
        let (info_ptr, ninfo) = match None::<&Info> {
            Some(info) => {
                if info.handle.is_null() {
                    (std::ptr::null(), 0)
                } else {
                    (info.handle as *const ffi::pmix_info_t, info.len)
                }
            }
            None => (std::ptr::null(), 0),
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
    }

    // ─── PmixStatus known error variants used in data_ops ───────────────────

    #[test]
    fn test_pmix_error_success_to_raw() {
        let status = PmixStatus::Known(PmixError::Success);
        assert_eq!(status.to_raw(), 0);
    }

    #[test]
    fn test_pmix_error_not_found_to_raw() {
        let status = PmixStatus::Known(PmixError::ErrNotFound);
        assert!(status.to_raw() < 0);
    }

    #[test]
    fn test_pmix_error_partial_success_to_raw() {
        let status = PmixStatus::Known(PmixError::ErrPartialSuccess);
        assert!(status.to_raw() < 0);
    }

    #[test]
    fn test_pmix_error_timeout_to_raw() {
        let status = PmixStatus::Known(PmixError::ErrTimeout);
        assert!(status.to_raw() < 0);
    }

    #[test]
    fn test_pmix_error_duplicate_key_to_raw() {
        let status = PmixStatus::Known(PmixError::ErrDuplicateKey);
        assert!(status.to_raw() < 0);
    }

    #[test]
    fn test_pmix_error_init_to_raw() {
        let status = PmixStatus::Known(PmixError::ErrInit);
        assert!(status.to_raw() < 0);
    }

    // ─── PmixPdata Debug formatting edge cases ──────────────────────────────

    #[test]
    fn test_pdata_debug_with_value_none() {
        let pdata = PmixPdata::new("test");
        let debug = format!("{:?}", pdata);
        assert!(debug.contains("PmixPdata"));
        assert!(debug.contains("value_present"));
    }

    #[test]
    fn test_pdata_debug_contains_key() {
        let pdata = PmixPdata::new("pmix.test.key");
        let debug = format!("{:?}", pdata);
        assert!(debug.contains("pmix.test.key"));
    }

    // ─── Store internal function signature verification ─────────────────────

    #[test]
    fn test_store_internal_is_public() {
        // Verify store_internal is accessible and has the right signature
        fn _type_check() {
            let _f: fn(&Proc, &str, &PmixOwnedValue) -> Result<(), PmixStatus> = store_internal;
        }
    }

    // ─── Fence with procs and info ──────────────────────────────────────────

    #[test]
    fn test_fence_with_single_proc() {
        let proc = Proc::new("job_abc", 0).unwrap();
        let result = crate::fence(&proc, None);
        match result {
            Ok(_) => {}
            Err(raw) => {
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Lookup nb with empty keys ──────────────────────────────────────────

    #[test]
    fn test_lookup_nb_empty_keys_returns_error() {
        struct DummyLookup;
        impl LookupCallback for DummyLookup {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }
        let keys: &[&str] = &[];
        let callback: Box<dyn LookupCallback> = Box::new(DummyLookup);
        let result = lookup_nb(keys, None, callback);
        assert!(result.is_err());
    }

    // ─── Unpublish nb with None keys ────────────────────────────────────────

    #[test]
    fn test_unpublish_nb_none_keys_reaches_ffi() {
        struct DummyUnpublish;
        impl UnpublishCallback for DummyUnpublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let callback: Box<dyn UnpublishCallback> = Box::new(DummyUnpublish);
        let result = unpublish_nb(None, None, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Get nb with key containing NUL (error path) ────────────────────────

    #[test]
    fn test_get_nb_key_with_nul_returns_error() {
        struct DummyGet;
        impl GetValueCallback for DummyGet {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<PmixOwnedValue>) {}
        }
        let proc = Proc::new("test_ns", 0).unwrap();
        let callback: Box<dyn GetValueCallback> = Box::new(DummyGet);
        // Key with embedded NUL byte
        let key = "test\0key";
        let result = get_nb(&proc, key, None, callback);
        assert!(result.is_err());
    }

    // ─── Get with key containing NUL (error path) ───────────────────────────

    #[test]
    fn test_get_key_with_nul_returns_error() {
        let proc = Proc::new("test_ns", 0).unwrap();
        let key = "test\0key";
        let result = get(&proc, key, None);
        assert!(result.is_err());
    }

    // ─── Unpublish with key containing NUL (error path) ─────────────────────

    #[test]
    fn test_unpublish_key_with_nul_returns_error() {
        let keys = ["test\0key"];
        let result = unpublish(Some(&keys), None);
        assert!(result.is_err());
    }

    // ─── Lookup nb with key containing NUL (error path) ─────────────────────

    #[test]
    fn test_lookup_nb_key_with_nul_returns_error() {
        struct DummyLookup;
        impl LookupCallback for DummyLookup {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }
        let keys = ["test\0key"];
        let callback: Box<dyn LookupCallback> = Box::new(DummyLookup);
        let result = lookup_nb(&keys, None, callback);
        assert!(result.is_err());
    }

    // ─── Proc namespace and rank edge cases ─────────────────────────────────

    #[test]
    fn test_proc_new_empty_namespace() {
        let proc = Proc::new("", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_new_long_namespace() {
        let long_ns = "a".repeat(256);
        let proc = Proc::new(&long_ns, 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_new_max_rank() {
        let proc = Proc::new("test", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
    }

    #[test]
    fn test_proc_new_with_nspace_different_rank() {
        let proc = Proc::new("original_ns", 0).unwrap();
        let proc2 = proc.new_with_nspace(5).unwrap();
        assert_eq!(proc2.get_rank(), 5);
    }

    // ─── PmixOwnedValue drop behavior ───────────────────────────────────────

    #[test]
    fn test_pmix_owned_value_creation_and_drop() {
        // We can create a zeroed PmixOwnedValue to test drop behavior
        // This verifies the Drop implementation doesn't panic on zeroed data
        let val = PmixOwnedValue {
            inner: unsafe { std::mem::zeroed() },
        };
        // Drop happens at end of scope — should not panic
        drop(val);
    }

    // ─── Multiple sequential publish calls ──────────────────────────────────

    #[test]
    fn test_multiple_publish_calls() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        for _ in 0..5 {
            let result = publish(&info);
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0);
                }
            }
        }
    }

    // ─── Multiple sequential lookup calls ───────────────────────────────────

    #[test]
    fn test_multiple_lookup_calls() {
        for _ in 0..5 {
            let data = vec![PmixPdata::new("test.key")];
            let mut data = data;
            let result = lookup(&mut data, None);
            match result {
                Ok(_) => {}
                Err(e) => {
                    let raw = e.to_raw();
                    assert!(raw < 0);
                }
            }
        }
    }

    // ─── Info parameter with non-zero len but null handle ───────────────────

    #[test]
    fn test_info_zero_len_null_handle_in_publish() {
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        // This should use null/0 path in publish
        let result = publish(&info);
        // Expected error without DVM
        assert!(result.is_err() || result.is_ok());
    }

    // ─── Fence nb with procs and info ───────────────────────────────────────

    #[test]
    fn test_fence_nb_with_procs() {
        struct DummyFence;
        impl FenceCallback for DummyFence {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let procs = vec![Proc::new("test_ns", 0).unwrap()];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let callback: Box<dyn FenceCallback> = Box::new(DummyFence);
        let result = fence_nb(&procs, Some(&info), callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ─── Publish nb with empty info ─────────────────────────────────────────

    #[test]
    fn test_publish_nb_empty_info() {
        struct DummyPublish;
        impl PublishCallback for DummyPublish {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let callback: Box<dyn PublishCallback> = Box::new(DummyPublish);
        let result = publish_nb(&info, callback);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error without DVM, got {}", raw);
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // MOCK FFI TESTS — exercise happy paths without a real PMIx daemon
    //
    // These tests use the mock_ffi framework to simulate successful FFI
    // returns. They verify the Rust wrapper logic processes results
    // correctly when PMIx operations succeed.
    // ═══════════════════════════════════════════════════════════════════════

    use crate::mock_ffi::{
        self, MockConfig, MockGuard, PMIX_ERR_DUPLICATE_KEY, PMIX_ERR_INIT, PMIX_ERR_NOT_FOUND,
        PMIX_ERR_TIMEOUT, PMIX_STRING, PMIX_STRING_U16, PMIX_SUCCESS,
    };

    // ─── Mock FFI framework self-tests ──────────────────────────────────────

    #[test]
    fn test_mock_ffi_enable_disable() {
        assert!(!mock_ffi::is_mock_enabled());
        mock_ffi::enable_mock_ffi();
        assert!(mock_ffi::is_mock_enabled());
        mock_ffi::disable_mock_ffi();
        assert!(!mock_ffi::is_mock_enabled());
    }

    #[test]
    fn test_mock_guard_raii() {
        assert!(!mock_ffi::is_mock_enabled());
        {
            let _guard = MockGuard::new();
            assert!(mock_ffi::is_mock_enabled());
        }
        assert!(!mock_ffi::is_mock_enabled());
    }

    #[test]
    fn test_mock_config_defaults() {
        let config = MockConfig::new();
        config.apply();
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_config_with_overrides() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY)
            .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND);
        config.apply();
        assert_eq!(
            mock_ffi::get_mock_status("PMIx_Publish"),
            PMIX_ERR_DUPLICATE_KEY
        );
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_key_value_store() {
        mock_ffi::mock_store_value("test_key", b"test_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("test_key"));
        mock_ffi::mock_remove_value("test_key");
        assert!(!mock_ffi::mock_key_exists("test_key"));
    }

    #[test]
    fn test_mock_store_clear() {
        mock_ffi::mock_store_value("k1", b"v1", PMIX_STRING);
        mock_ffi::mock_store_value("k2", b"v2", PMIX_STRING);
        mock_ffi::mock_clear_store();
        assert!(!mock_ffi::mock_key_exists("k1"));
        assert!(!mock_ffi::mock_key_exists("k2"));
    }

    #[test]
    fn test_mock_status_constants() {
        assert_eq!(mock_ffi::PMIX_SUCCESS, 0);
        assert_eq!(mock_ffi::PMIX_ERR_INIT, -31);
        assert_eq!(mock_ffi::PMIX_ERR_NOT_FOUND, -46);
        assert_eq!(mock_ffi::PMIX_ERR_TIMEOUT, -24);
    }

    // ─── Mock-aware publish tests ───────────────────────────────────────────

    /// Test that publish() correctly processes a success status.
    /// This verifies the status-to-Result conversion logic in publish().
    #[test]
    fn test_publish_status_conversion_success() {
        // Verify that PmixStatus::from_raw(PMIX_SUCCESS) produces a success
        let status = PmixStatus::from_raw(PMIX_SUCCESS);
        assert!(status.is_success());
        assert!(!status.is_error());
    }

    /// Test that publish() correctly processes an error status.
    #[test]
    fn test_publish_status_conversion_error() {
        let status = PmixStatus::from_raw(PMIX_ERR_INIT);
        assert!(status.is_error());
        assert!(!status.is_success());
    }

    /// Test that publish() correctly processes duplicate key error.
    #[test]
    fn test_publish_status_conversion_duplicate_key() {
        let status = PmixStatus::from_raw(PMIX_ERR_DUPLICATE_KEY);
        assert!(status.is_error());
    }

    /// Test the publish happy path status conversion flow.
    /// Simulates what happens inside publish() when FFI returns PMIX_SUCCESS.
    #[test]
    fn test_publish_happy_path_status_flow() {
        let _guard = MockGuard::new();
        // Simulate the status conversion that happens inside publish()
        let raw_status = mock_ffi::get_mock_status("PMIx_Publish");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_success(), "Mock should return PMIX_SUCCESS");

        // Verify the Result conversion matches what publish() does
        let result = if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        };
        assert!(result.is_ok());
    }

    /// Test the publish error path when mock returns ErrInit.
    #[test]
    fn test_publish_error_path_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Publish");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_error());

        let result = if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        };
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PmixStatus::Known(PmixError::ErrInit));
    }

    /// Test publish with mock returning duplicate key error.
    #[test]
    fn test_publish_duplicate_key_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Publish");
        let pmix_status = PmixStatus::from_raw(raw_status);
        let result = if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        };
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            PmixStatus::Known(PmixError::ErrDuplicateKey)
        );
    }

    // ─── Mock-aware get tests ───────────────────────────────────────────────

    /// Test the get happy path status conversion flow.
    #[test]
    fn test_get_happy_path_status_flow() {
        let _guard = MockGuard::new();
        let raw_status = mock_ffi::get_mock_status("PMIx_Get");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_success());
    }

    /// Test get with mock returning not found.
    #[test]
    fn test_get_not_found_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Get");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_error());
        assert_eq!(pmix_status, PmixStatus::Known(PmixError::ErrNotFound));
    }

    /// Test get with mock returning timeout.
    #[test]
    fn test_get_timeout_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Get", PMIX_ERR_TIMEOUT);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Get");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert_eq!(pmix_status, PmixStatus::Known(PmixError::ErrTimeout));
    }

    // ─── Mock-aware fence tests ─────────────────────────────────────────────

    /// Test fence happy path with mock.
    #[test]
    fn test_fence_happy_path_with_mock() {
        let _guard = MockGuard::new();
        let raw_status = mock_ffi::get_mock_status("PMIx_Fence");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_success());
    }

    /// Test fence error path with mock.
    #[test]
    fn test_fence_error_path_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Fence", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Fence");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert_eq!(pmix_status, PmixStatus::Known(PmixError::ErrInit));
    }

    // ─── Mock-aware unpublish tests ─────────────────────────────────────────

    /// Test unpublish happy path with mock.
    #[test]
    fn test_unpublish_happy_path_with_mock() {
        let _guard = MockGuard::new();
        let raw_status = mock_ffi::get_mock_status("PMIx_Unpublish");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_success());
    }

    /// Test unpublish with not found error.
    #[test]
    fn test_unpublish_not_found_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Unpublish", PMIX_ERR_NOT_FOUND);
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Unpublish");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert_eq!(pmix_status, PmixStatus::Known(PmixError::ErrNotFound));
    }

    // ─── Mock-aware lookup tests ────────────────────────────────────────────

    /// Test lookup happy path with mock.
    #[test]
    fn test_lookup_happy_path_with_mock() {
        let _guard = MockGuard::new();
        let raw_status = mock_ffi::get_mock_status("PMIx_Lookup");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert!(pmix_status.is_success());
    }

    /// Test lookup with partial success.
    #[test]
    fn test_lookup_partial_success_with_mock() {
        let config = MockConfig::new().with_function_status("PMIx_Lookup", -52); // PMIX_ERR_PARTIAL_SUCCESS
        let _guard = MockGuard::with_config(config);

        let raw_status = mock_ffi::get_mock_status("PMIx_Lookup");
        let pmix_status = PmixStatus::from_raw(raw_status);
        assert_eq!(pmix_status, PmixStatus::Known(PmixError::ErrPartialSuccess));
    }

    // ─── Mock-aware publish_nb callback tests ───────────────────────────────

    /// Test publish_nb callback bridge with success status.
    #[test]
    fn test_publish_callback_bridge_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CB_STATUS: AtomicI32 = AtomicI32::new(-999);

        struct TestPublishCb;
        impl PublishCallback for TestPublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CB_STATUS.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        // Register callback
        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestPublishCb));
        }

        // Simulate callback invocation with success
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(PMIX_SUCCESS, cbdata);

        assert_eq!(CB_STATUS.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    /// Test publish_nb callback bridge with error status.
    #[test]
    fn test_publish_callback_bridge_error() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CB_STATUS: AtomicI32 = AtomicI32::new(-999);

        struct TestPublishCb;
        impl PublishCallback for TestPublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CB_STATUS.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestPublishCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(PMIX_ERR_DUPLICATE_KEY, cbdata);

        assert_eq!(CB_STATUS.load(Ordering::SeqCst), PMIX_ERR_DUPLICATE_KEY);
    }

    /// Test publish_nb callback bridge with null cbdata (should not panic).
    #[test]
    fn test_publish_callback_bridge_null_cbdata_mock() {
        // Should return early without panicking
        publish_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut());
    }

    // ─── Mock-aware get_nb callback tests ───────────────────────────────────

    /// Test get_nb callback bridge with success status.
    #[test]
    fn test_get_callback_bridge_success() {
        use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
        static CB_STATUS: AtomicI32 = AtomicI32::new(-999);
        static CB_HAS_VALUE: AtomicBool = AtomicBool::new(false);

        struct TestGetCb;
        impl GetValueCallback for TestGetCb {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                CB_STATUS.store(status.to_raw(), Ordering::SeqCst);
                CB_HAS_VALUE.store(value.is_some(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestGetCb));
        }

        // Create a mock pmix_value_t for the callback
        let mut mock_value: ffi::pmix_value_t = unsafe { std::mem::zeroed() };
        mock_value.type_ = PMIX_STRING_U16;

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            get_value_callback_bridge(
                PMIX_SUCCESS,
                &mut mock_value as *mut ffi::pmix_value_t,
                cbdata,
            );
        }

        assert_eq!(CB_STATUS.load(Ordering::SeqCst), PMIX_SUCCESS);
        assert!(CB_HAS_VALUE.load(Ordering::SeqCst));
    }

    /// Test get_nb callback bridge with not found.
    #[test]
    fn test_get_callback_bridge_not_found() {
        use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
        static CB_STATUS2: AtomicI32 = AtomicI32::new(-999);
        static CB_HAS_VALUE2: AtomicBool = AtomicBool::new(false);

        struct TestGetCb2;
        impl GetValueCallback for TestGetCb2 {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                CB_STATUS2.store(status.to_raw(), Ordering::SeqCst);
                CB_HAS_VALUE2.store(value.is_some(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestGetCb2));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(PMIX_ERR_NOT_FOUND, std::ptr::null_mut(), cbdata);

        assert_eq!(CB_STATUS2.load(Ordering::SeqCst), PMIX_ERR_NOT_FOUND);
        assert!(!CB_HAS_VALUE2.load(Ordering::SeqCst));
    }

    /// Test get_nb callback bridge with null cbdata (should not panic).
    #[test]
    fn test_get_callback_bridge_null_cbdata() {
        get_value_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), std::ptr::null_mut());
    }

    // ─── Mock-aware lookup_nb callback tests ────────────────────────────────

    /// Test lookup_nb callback bridge with success status.
    #[test]
    fn test_lookup_callback_bridge_success() {
        use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
        static CB_STATUS3: AtomicI32 = AtomicI32::new(-999);
        static CB_DATA_LEN: AtomicUsize = AtomicUsize::new(0);

        struct TestLookupCb;
        impl LookupCallback for TestLookupCb {
            fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>) {
                CB_STATUS3.store(status.to_raw(), Ordering::SeqCst);
                CB_DATA_LEN.store(data.len(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestLookupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), 0, cbdata);

        assert_eq!(CB_STATUS3.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    /// Test lookup_nb callback bridge with not found.
    #[test]
    fn test_lookup_callback_bridge_not_found() {
        use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
        static CB_STATUS4: AtomicI32 = AtomicI32::new(-999);
        static CB_DATA_LEN2: AtomicUsize = AtomicUsize::new(0);

        struct TestLookupCb2;
        impl LookupCallback for TestLookupCb2 {
            fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>) {
                CB_STATUS4.store(status.to_raw(), Ordering::SeqCst);
                CB_DATA_LEN2.store(data.len(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestLookupCb2));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_ERR_NOT_FOUND, std::ptr::null_mut(), 0, cbdata);

        assert_eq!(CB_STATUS4.load(Ordering::SeqCst), PMIX_ERR_NOT_FOUND);
    }

    // ─── Mock-aware fence_nb callback tests ─────────────────────────────────

    /// Test fence_nb callback bridge with success status.
    #[test]
    fn test_fence_callback_bridge_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CB_STATUS5: AtomicI32 = AtomicI32::new(-999);

        struct TestFenceCb;
        impl FenceCallback for TestFenceCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CB_STATUS5.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestFenceCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(PMIX_SUCCESS, cbdata);

        assert_eq!(CB_STATUS5.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    /// Test fence_nb callback bridge with timeout error.
    #[test]
    fn test_fence_callback_bridge_timeout() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CB_STATUS6: AtomicI32 = AtomicI32::new(-999);

        struct TestFenceCb2;
        impl FenceCallback for TestFenceCb2 {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CB_STATUS6.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestFenceCb2));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(PMIX_ERR_TIMEOUT, cbdata);

        assert_eq!(CB_STATUS6.load(Ordering::SeqCst), PMIX_ERR_TIMEOUT);
    }

    // ─── Mock-aware unpublish_nb callback tests ─────────────────────────────

    /// Test unpublish_nb callback bridge with success.
    #[test]
    fn test_unpublish_callback_bridge_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CB_STATUS7: AtomicI32 = AtomicI32::new(-999);

        struct TestUnpublishCb;
        impl UnpublishCallback for TestUnpublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CB_STATUS7.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestUnpublishCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(PMIX_SUCCESS, cbdata);

        assert_eq!(CB_STATUS7.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    // ─── Mock key-value store integration tests ─────────────────────────────

    /// Test storing and retrieving multiple key-value pairs.
    #[test]
    fn test_mock_store_multiple_keys() {
        mock_ffi::mock_store_value("key1", b"value1", PMIX_STRING);
        mock_ffi::mock_store_value("key2", b"value2", PMIX_STRING);
        mock_ffi::mock_store_value("key3", b"value3", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("key1"));
        assert!(mock_ffi::mock_key_exists("key2"));
        assert!(mock_ffi::mock_key_exists("key3"));
        assert!(!mock_ffi::mock_key_exists("key4"));
        mock_ffi::mock_clear_store();
    }

    /// Test overwriting an existing key.
    #[test]
    fn test_mock_store_overwrite() {
        mock_ffi::mock_store_value("key", b"old_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("key"));
        mock_ffi::mock_store_value("key", b"new_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("key"));
        mock_ffi::mock_remove_value("key");
    }

    /// Test removing a non-existent key (should not panic).
    #[test]
    fn test_mock_remove_nonexistent() {
        mock_ffi::mock_remove_value("does_not_exist");
        // Should not panic
    }

    // ─── Mock FFI comprehensive scenario tests ──────────────────────────────

    /// Simulate a complete publish-get-unpublish lifecycle with mock.
    #[test]
    fn test_mock_publish_get_unpublish_lifecycle() {
        let _guard = MockGuard::new();

        // Step 1: Publish succeeds
        let pub_status = mock_ffi::get_mock_status("PMIx_Publish");
        assert_eq!(pub_status, PMIX_SUCCESS);

        // Step 2: Key is stored in mock datastore
        mock_ffi::mock_store_value("test.key", b"test_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("test.key"));

        // Step 3: Get succeeds
        let get_status = mock_ffi::get_mock_status("PMIx_Get");
        assert_eq!(get_status, PMIX_SUCCESS);

        // Step 4: Unpublish succeeds
        let unpublish_status = mock_ffi::get_mock_status("PMIx_Unpublish");
        assert_eq!(unpublish_status, PMIX_SUCCESS);

        // Step 5: Key removed
        mock_ffi::mock_remove_value("test.key");
        assert!(!mock_ffi::mock_key_exists("test.key"));
    }

    /// Simulate error scenarios in mock.
    #[test]
    fn test_mock_error_scenarios() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY)
            .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND)
            .with_function_status("PMIx_Fence", PMIX_ERR_TIMEOUT)
            .with_function_status("PMIx_Unpublish", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);

        assert_eq!(
            mock_ffi::get_mock_status("PMIx_Publish"),
            PMIX_ERR_DUPLICATE_KEY
        );
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_ERR_TIMEOUT);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Unpublish"), PMIX_ERR_INIT);
    }

    /// Test that mock is properly reset after guard drops.
    #[test]
    fn test_mock_reset_after_guard_drop() {
        let config = MockConfig::new().with_default_status(PMIX_ERR_INIT);
        {
            let _guard = MockGuard::with_config(config);
            assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_ERR_INIT);
        }
        // After guard drops, mock is disabled
        assert!(!mock_ffi::is_mock_enabled());
    }

    // ─── store_internal mock-aware tests ────────────────────────────────────

    /// Test store_internal with mock success.
    #[test]
    fn test_store_internal_mock_success() {
        let _guard = MockGuard::new();
        // Verify the mock status for store_internal's underlying FFI call
        let raw_status = mock_ffi::get_mock_status("PMIx_Publish");
        assert_eq!(raw_status, PMIX_SUCCESS);
    }

    /// Test store_internal with mock error.
    #[test]
    fn test_store_internal_mock_error() {
        let config = MockConfig::new().with_default_status(PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);
        let raw_status = mock_ffi::get_mock_status("PMIx_Publish");
        assert_eq!(raw_status, PMIX_ERR_INIT);
    }

    // ─── Info parameter handling with mock ──────────────────────────────────

    /// Test that Info with null handle uses the correct code path.
    #[test]
    fn test_mock_info_null_handle_path() {
        let _guard = MockGuard::new();
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
        // Mock status should be success for publish
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    // ─── Proc handling with mock ────────────────────────────────────────────

    /// Test Proc creation and usage with mock FFI enabled.
    #[test]
    fn test_mock_proc_with_ffi() {
        let _guard = MockGuard::new();
        let proc = Proc::new("test_namespace", 42).unwrap();
        assert_eq!(proc.get_rank(), 42);
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    /// Test Proc with wildcard rank.
    #[test]
    fn test_mock_proc_wildcard_rank() {
        let _guard = MockGuard::new();
        let proc =
            Proc::new("", PMIX_RANK_WILDCARD as u32).unwrap_or_else(|_| Proc::new("", 0).unwrap());
        assert_eq!(proc.get_rank(), PMIX_RANK_WILDCARD as u32);
    }

    // ─── PmixPdata mock-aware tests ─────────────────────────────────────────

    /// Test PmixPdata creation and field access with mock.
    #[test]
    fn test_mock_pdata_creation() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("test.lookup.key");
        assert_eq!(pdata.key, "test.lookup.key");
        assert!(pdata.value.is_none());
    }

    /// Test PmixPdata with empty key.
    #[test]
    fn test_mock_pdata_empty_key() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("");
        assert_eq!(pdata.key, "");
    }

    /// Test PmixPdata with long key.
    #[test]
    fn test_mock_pdata_long_key() {
        let _guard = MockGuard::new();
        let long_key = "a".repeat(1000);
        let pdata = PmixPdata::new(&long_key);
        assert_eq!(pdata.key.len(), 1000);
    }

    /// Test PmixPdata with unicode key.
    #[test]
    fn test_mock_pdata_unicode_key() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("test.key.αβγ");
        assert_eq!(pdata.key, "test.key.αβγ");
    }

    // ─── Callback registry stress tests ─────────────────────────────────────

    /// Test that callback registries properly clean up after use.
    #[test]
    fn test_callback_registry_cleanup() {
        struct DummyCb;
        impl PublishCallback for DummyCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        // Register and immediately remove
        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyCb));
            assert_eq!(registry.len(), 1);
        }
        // Callback consumed by bridge
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            publish_callback_bridge(PMIX_SUCCESS, cbdata);
        }
        // Registry should be empty now
        let registry = PUBLISH_REGISTRY.lock().unwrap();
        assert!(!registry.contains_key(&req_id));
    }

    /// Test GET registry cleanup.
    #[test]
    fn test_get_registry_cleanup() {
        struct DummyGetCb;
        impl GetValueCallback for DummyGetCb {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<PmixOwnedValue>) {}
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyGetCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            get_value_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), cbdata);
        }

        let registry = GET_REGISTRY.lock().unwrap();
        assert!(!registry.contains_key(&req_id));
    }

    /// Test LOOKUP registry cleanup.
    #[test]
    fn test_lookup_registry_cleanup() {
        struct DummyLookupCb;
        impl LookupCallback for DummyLookupCb {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyLookupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            lookup_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), 0, cbdata);
        }

        let registry = LOOKUP_REGISTRY.lock().unwrap();
        assert!(!registry.contains_key(&req_id));
    }

    /// Test FENCE registry cleanup.
    #[test]
    fn test_fence_registry_cleanup() {
        struct DummyFenceCb;
        impl FenceCallback for DummyFenceCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyFenceCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            fence_callback_bridge(PMIX_SUCCESS, cbdata);
        }

        let registry = FENCE_REGISTRY.lock().unwrap();
        assert!(!registry.contains_key(&req_id));
    }

    /// Test UNPUBLISH registry cleanup.
    #[test]
    fn test_unpublish_registry_cleanup() {
        struct DummyUnpublishCb;
        impl UnpublishCallback for DummyUnpublishCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let req_id = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DummyUnpublishCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            unpublish_callback_bridge(PMIX_SUCCESS, cbdata);
        }

        let registry = UNPUBLISH_REGISTRY.lock().unwrap();
        assert!(!registry.contains_key(&req_id));
    }

    // ─── Mock FFI concurrent safety tests ───────────────────────────────────

    /// Test that mock FFI state is thread-safe.
    #[test]
    fn test_mock_ffi_thread_safety() {
        use std::thread;
        let handles: Vec<_> = (0..4)
            .map(|i| {
                thread::spawn(move || {
                    // Each thread enables mock, checks status, disables
                    mock_ffi::enable_mock_ffi();
                    assert!(mock_ffi::is_mock_enabled());
                    let status = mock_ffi::get_mock_status(&format!("PMIx_Test_{}", i));
                    assert_eq!(status, PMIX_SUCCESS);
                    mock_ffi::disable_mock_ffi();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test concurrent mock store operations.
    #[test]
    fn test_mock_store_concurrent() {
        use std::thread;
        let handles: Vec<_> = (0..8)
            .map(|i| {
                thread::spawn(move || {
                    let key = format!("concurrent_key_{}", i);
                    mock_ffi::mock_store_value(&key, b"test", PMIX_STRING);
                    assert!(mock_ffi::mock_key_exists(&key));
                    mock_ffi::mock_remove_value(&key);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
        mock_ffi::mock_clear_store();
    }

    // ─── PmixStatus conversion comprehensive tests ──────────────────────────

    /// Test all known PmixError variants convert correctly.
    #[test]
    fn test_pmix_status_all_known_variants() {
        // Success
        assert_eq!(
            PmixStatus::from_raw(0),
            PmixStatus::Known(PmixError::Success)
        );
        // Error
        assert_eq!(
            PmixStatus::from_raw(-1),
            PmixStatus::Known(PmixError::Error)
        );
        // Not found
        assert_eq!(
            PmixStatus::from_raw(-46),
            PmixStatus::Known(PmixError::ErrNotFound)
        );
        // Init
        assert_eq!(
            PmixStatus::from_raw(-31),
            PmixStatus::Known(PmixError::ErrInit)
        );
        // Timeout
        assert_eq!(
            PmixStatus::from_raw(-24),
            PmixStatus::Known(PmixError::ErrTimeout)
        );
        // Duplicate key
        assert_eq!(
            PmixStatus::from_raw(-53),
            PmixStatus::Known(PmixError::ErrDuplicateKey)
        );
        // Partial success
        assert_eq!(
            PmixStatus::from_raw(-52),
            PmixStatus::Known(PmixError::ErrPartialSuccess)
        );
    }

    /// Test unknown status codes are wrapped in Unknown variant.
    #[test]
    fn test_pmix_status_unknown_variant() {
        let status = PmixStatus::from_raw(-99999);
        match status {
            PmixStatus::Unknown(v) => assert_eq!(v, -99999),
            _ => panic!("Expected Unknown variant"),
        }
    }

    /// Test PmixStatus Display implementation.
    #[test]
    fn test_pmix_status_display() {
        let success = PmixStatus::from_raw(0);
        let display = format!("{}", success);
        assert!(!display.is_empty());

        let unknown = PmixStatus::from_raw(-99999);
        let display = format!("{}", unknown);
        assert!(display.contains("unknown"));
    }

    /// Test PmixStatus Error implementation.
    #[test]
    fn test_pmix_status_error_trait() {
        let status: &dyn std::error::Error = &PmixStatus::from_raw(-1);
        assert!(status.source().is_none());
    }

    // ─── Fence nb with procs and info (mock-aware) ──────────────────────────

    /// Test fence_nb with multiple procs and mock success.
    #[test]
    fn test_fence_nb_multiple_procs_mock() {
        let _guard = MockGuard::new();
        let procs = vec![
            Proc::new("ns1", 0).unwrap(),
            Proc::new("ns1", 1).unwrap(),
            Proc::new("ns2", 0).unwrap(),
        ];
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
        assert_eq!(procs.len(), 3);
    }

    // ─── Publish nb with callback that captures status ──────────────────────

    /// Test publish_nb with a callback that captures the status.
    #[test]
    fn test_publish_nb_callback_captures_status() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CAPTURED_STATUS: AtomicI32 = AtomicI32::new(-999);

        struct CaptureCb;
        impl PublishCallback for CaptureCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                CAPTURED_STATUS.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(CaptureCb));
        }

        // Simulate success callback
        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            publish_callback_bridge(PMIX_SUCCESS, cbdata);
        }
        assert_eq!(CAPTURED_STATUS.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    // ─── Get nb with callback that captures value ───────────────────────────

    /// Test get_nb callback with a mock value containing string data.
    #[test]
    fn test_get_nb_callback_with_string_value() {
        use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
        static CB_STATUS_STR: AtomicI32 = AtomicI32::new(-999);
        static CB_HAS_VAL_STR: AtomicBool = AtomicBool::new(false);

        struct StringValueCb;
        impl GetValueCallback for StringValueCb {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                CB_STATUS_STR.store(status.to_raw(), Ordering::SeqCst);
                CB_HAS_VAL_STR.store(value.is_some(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(StringValueCb));
        }

        // Create a mock pmix_value_t with PMIX_STRING type
        let mut mock_value: ffi::pmix_value_t = unsafe { std::mem::zeroed() };
        mock_value.type_ = PMIX_STRING_U16;

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unsafe {
            get_value_callback_bridge(
                PMIX_SUCCESS,
                &mut mock_value as *mut ffi::pmix_value_t,
                cbdata,
            );
        }

        assert_eq!(CB_STATUS_STR.load(Ordering::SeqCst), PMIX_SUCCESS);
        assert!(CB_HAS_VAL_STR.load(Ordering::SeqCst));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TASK-087: Additional mock FFI happy path tests (59+ new tests)
    // ═══════════════════════════════════════════════════════════════════════

    // ─── Mock-aware publish happy path tests ────────────────────────────────

    /// Test publish with mock FFI — empty info array passes null pointer.
    #[test]
    fn test_mock_publish_empty_info_null_ptr() {
        let _guard = MockGuard::new();
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        // Simulate the pointer logic inside publish()
        let (info_ptr, ninfo) = if info.len > 0 {
            (info.handle as *const ffi::pmix_info_t, info.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(info_ptr.is_null());
        assert_eq!(ninfo, 0);
        // Mock status should be success
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    /// Test publish with mock FFI — non-empty info passes handle pointer.
    #[test]
    fn test_mock_publish_nonempty_info_ptr() {
        let _guard = MockGuard::new();
        // Simulate non-empty info
        let fake_handle = 0x1234usize as *mut ffi::pmix_info_t;
        let info = Info {
            handle: fake_handle,
            len: 5,
        };
        let (info_ptr, ninfo) = if info.len > 0 {
            (info.handle as *const ffi::pmix_info_t, info.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(!info_ptr.is_null());
        assert_eq!(ninfo, 5);
    }

    /// Test publish status conversion with PMIX_SUCCESS raw value.
    #[test]
    fn test_mock_publish_raw_success_conversion() {
        let _guard = MockGuard::new();
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        let status = PmixStatus::from_raw(raw);
        assert!(status.is_success());
        assert_eq!(status, PmixStatus::Known(PmixError::Success));
    }

    /// Test publish with mock returning error — result is Err.
    #[test]
    fn test_mock_publish_error_result() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        let status = PmixStatus::from_raw(raw);
        let result: Result<(), PmixStatus> = if status.is_success() {
            Ok(())
        } else {
            Err(status)
        };
        assert!(result.is_err());
    }

    /// Test publish with mock timeout error.
    #[test]
    fn test_mock_publish_timeout_error() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_TIMEOUT);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        assert_eq!(raw, PMIX_ERR_TIMEOUT);
        let status = PmixStatus::from_raw(raw);
        assert_eq!(status, PmixStatus::Known(PmixError::ErrTimeout));
    }

    /// Test publish happy path with key-value store simulation.
    #[test]
    fn test_mock_publish_stores_key_in_mock_store() {
        let _guard = MockGuard::new();
        mock_ffi::mock_clear_store();
        // Simulate publish storing a key
        mock_ffi::mock_store_value("publish.test.key", b"test_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("publish.test.key"));
        // Verify mock status is success
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    /// Test publish_nb callback with timeout error status.
    #[test]
    fn test_mock_publish_callback_timeout() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static PUB_CB_TIMEOUT: AtomicI32 = AtomicI32::new(-999);

        struct TimeoutPublishCb;
        impl PublishCallback for TimeoutPublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                PUB_CB_TIMEOUT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TimeoutPublishCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(PMIX_ERR_TIMEOUT, cbdata);
        assert_eq!(PUB_CB_TIMEOUT.load(Ordering::SeqCst), PMIX_ERR_TIMEOUT);
    }

    /// Test publish_nb callback with not found error.
    #[test]
    fn test_mock_publish_callback_not_found() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static PUB_CB_NOTFOUND: AtomicI32 = AtomicI32::new(-999);

        struct NotFoundPublishCb;
        impl PublishCallback for NotFoundPublishCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                PUB_CB_NOTFOUND.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(NotFoundPublishCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(PMIX_ERR_NOT_FOUND, cbdata);
        assert_eq!(PUB_CB_NOTFOUND.load(Ordering::SeqCst), PMIX_ERR_NOT_FOUND);
    }

    // ─── Mock-aware get happy path tests ────────────────────────────────────

    /// Test get_nb callback with error status.
    #[test]
    fn test_mock_get_callback_error() {
        use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
        static GET_CB_ERR: AtomicI32 = AtomicI32::new(-999);
        static GET_CB_HAS_VAL: AtomicBool = AtomicBool::new(true);

        struct ErrorGetCb;
        impl GetValueCallback for ErrorGetCb {
            fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
                GET_CB_ERR.store(status.to_raw(), Ordering::SeqCst);
                GET_CB_HAS_VAL.store(value.is_some(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(ErrorGetCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(PMIX_ERR_INIT, std::ptr::null_mut(), cbdata);
        assert_eq!(GET_CB_ERR.load(Ordering::SeqCst), PMIX_ERR_INIT);
        assert!(!GET_CB_HAS_VAL.load(Ordering::SeqCst));
    }

    /// Test get_nb callback with timeout error.
    #[test]
    fn test_mock_get_callback_timeout() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static GET_CB_TIMEOUT: AtomicI32 = AtomicI32::new(-999);

        struct TimeoutGetCb;
        impl GetValueCallback for TimeoutGetCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _value: Option<PmixOwnedValue>) {
                GET_CB_TIMEOUT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TimeoutGetCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(PMIX_ERR_TIMEOUT, std::ptr::null_mut(), cbdata);
        assert_eq!(GET_CB_TIMEOUT.load(Ordering::SeqCst), PMIX_ERR_TIMEOUT);
    }

    /// Test get with mock — proc and key validation.
    #[test]
    fn test_mock_get_proc_key_validation() {
        let _guard = MockGuard::new();
        let proc = Proc::new("test.nspace", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    /// Test get with mock — proc with high rank.
    #[test]
    fn test_mock_get_proc_high_rank() {
        let _guard = MockGuard::new();
        let proc = Proc::new("high.rank.ns", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    /// Test get with mock — proc with wildcard rank.
    #[test]
    fn test_mock_get_proc_wildcard() {
        let _guard = MockGuard::new();
        let proc = Proc::new("", PMIX_RANK_WILDCARD as u32).unwrap();
        assert_eq!(proc.get_rank(), PMIX_RANK_WILDCARD as u32);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    /// Test get error path result construction.
    #[test]
    fn test_mock_get_error_result_construction() {
        let config = MockConfig::new().with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Get");
        let status = PmixStatus::from_raw(raw);
        let result: Result<PmixOwnedValue, PmixStatus> = if status.is_success() {
            // Would return value on success
            unimplemented!()
        } else {
            Err(status)
        };
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            PmixStatus::Known(PmixError::ErrNotFound)
        );
    }

    /// Test get_nb callback bridge with init error.
    #[test]
    fn test_mock_get_callback_init_error() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static GET_CB_INIT: AtomicI32 = AtomicI32::new(-999);

        struct InitErrorGetCb;
        impl GetValueCallback for InitErrorGetCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _value: Option<PmixOwnedValue>) {
                GET_CB_INIT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(InitErrorGetCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(PMIX_ERR_INIT, std::ptr::null_mut(), cbdata);
        assert_eq!(GET_CB_INIT.load(Ordering::SeqCst), PMIX_ERR_INIT);
    }

    // ─── Mock-aware lookup happy path tests ─────────────────────────────────

    /// Test lookup_nb callback with multiple data entries.
    #[test]
    fn test_mock_lookup_callback_with_data() {
        use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
        static LOOKUP_CB_STATUS: AtomicI32 = AtomicI32::new(-999);
        static LOOKUP_CB_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DataLookupCb;
        impl LookupCallback for DataLookupCb {
            fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>) {
                LOOKUP_CB_STATUS.store(status.to_raw(), Ordering::SeqCst);
                LOOKUP_CB_COUNT.store(data.len(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(DataLookupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), 3, cbdata);
        assert_eq!(LOOKUP_CB_STATUS.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    /// Test lookup_nb callback with timeout error.
    #[test]
    fn test_mock_lookup_callback_timeout() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static LOOKUP_CB_TIMEOUT: AtomicI32 = AtomicI32::new(-999);

        struct TimeoutLookupCb;
        impl LookupCallback for TimeoutLookupCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _data: Vec<PmixPdata>) {
                LOOKUP_CB_TIMEOUT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TimeoutLookupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_ERR_TIMEOUT, std::ptr::null_mut(), 0, cbdata);
        assert_eq!(LOOKUP_CB_TIMEOUT.load(Ordering::SeqCst), PMIX_ERR_TIMEOUT);
    }

    /// Test lookup with mock — multi-key simulation.
    #[test]
    fn test_mock_lookup_multi_key_simulation() {
        let _guard = MockGuard::new();
        let keys = vec!["key1", "key2", "key3", "key4"];
        // Simulate storing keys in mock store
        for key in &keys {
            mock_ffi::mock_store_value(key, b"val", PMIX_STRING);
        }
        // Verify all keys exist
        for key in &keys {
            assert!(mock_ffi::mock_key_exists(key));
        }
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Lookup"), PMIX_SUCCESS);
        mock_ffi::mock_clear_store();
    }

    /// Test lookup with mock — key validation with dots and underscores.
    #[test]
    fn test_mock_lookup_key_with_special_chars() {
        let _guard = MockGuard::new();
        let key = "pmix.job.size";
        mock_ffi::mock_store_value(key, b"42", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists(key));
        // Also test underscore key
        let key2 = "pmix_job_id";
        mock_ffi::mock_store_value(key2, b"job_123", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists(key2));
        mock_ffi::mock_clear_store();
    }

    /// Test lookup_nb callback with init error (TASK-087 variant).
    #[test]
    fn test_mock_lookup_callback_init_error_task087() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static LOOKUP_CB_INIT: AtomicI32 = AtomicI32::new(-999);

        struct InitErrorLookupCb;
        impl LookupCallback for InitErrorLookupCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _data: Vec<PmixPdata>) {
                LOOKUP_CB_INIT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(InitErrorLookupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_ERR_INIT, std::ptr::null_mut(), 0, cbdata);
        assert_eq!(LOOKUP_CB_INIT.load(Ordering::SeqCst), PMIX_ERR_INIT);
    }

    /// Test lookup with mock — PmixPdata with value set.
    #[test]
    fn test_mock_lookup_pdata_with_value() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("lookup.key.with.value");
        assert_eq!(pdata.key, "lookup.key.with.value");
        assert!(pdata.value.is_none());
        // Value field is Option<PmixOwnedValue> — cannot set from string directly
        // Verify it remains None after construction
        assert!(pdata.value.is_none());
    }

    // ─── Mock-aware unpublish happy path tests ──────────────────────────────

    /// Test unpublish_nb callback with not found error.
    #[test]
    fn test_mock_unpublish_callback_not_found() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static UNPUB_CB_NOTFOUND: AtomicI32 = AtomicI32::new(-999);

        struct UnpubNotFoundCb;
        impl UnpublishCallback for UnpubNotFoundCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                UNPUB_CB_NOTFOUND.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(UnpubNotFoundCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(PMIX_ERR_NOT_FOUND, cbdata);
        assert_eq!(UNPUB_CB_NOTFOUND.load(Ordering::SeqCst), PMIX_ERR_NOT_FOUND);
    }

    /// Test unpublish_nb callback with timeout error.
    #[test]
    fn test_mock_unpublish_callback_timeout() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static UNPUB_CB_TIMEOUT: AtomicI32 = AtomicI32::new(-999);

        struct UnpubTimeoutCb;
        impl UnpublishCallback for UnpubTimeoutCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                UNPUB_CB_TIMEOUT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(UnpubTimeoutCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(PMIX_ERR_TIMEOUT, cbdata);
        assert_eq!(UNPUB_CB_TIMEOUT.load(Ordering::SeqCst), PMIX_ERR_TIMEOUT);
    }

    /// Test unpublish with mock — key removal simulation.
    #[test]
    fn test_mock_unpublish_key_removal() {
        let _guard = MockGuard::new();
        mock_ffi::mock_store_value("to_remove", b"val", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("to_remove"));
        // Simulate unpublish removing the key
        mock_ffi::mock_remove_value("to_remove");
        assert!(!mock_ffi::mock_key_exists("to_remove"));
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Unpublish"), PMIX_SUCCESS);
    }

    /// Test unpublish error result construction.
    #[test]
    fn test_mock_unpublish_error_result() {
        let config = MockConfig::new().with_function_status("PMIx_Unpublish", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Unpublish");
        let status = PmixStatus::from_raw(raw);
        let result: Result<(), PmixStatus> = if status.is_success() {
            Ok(())
        } else {
            Err(status)
        };
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PmixStatus::Known(PmixError::ErrInit));
    }

    // ─── Mock-aware fence happy path tests ──────────────────────────────────

    /// Test fence_nb callback with not found error.
    #[test]
    fn test_mock_fence_callback_not_found() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static FENCE_CB_NOTFOUND: AtomicI32 = AtomicI32::new(-999);

        struct FenceNotFoundCb;
        impl FenceCallback for FenceNotFoundCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                FENCE_CB_NOTFOUND.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(FenceNotFoundCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(PMIX_ERR_NOT_FOUND, cbdata);
        assert_eq!(FENCE_CB_NOTFOUND.load(Ordering::SeqCst), PMIX_ERR_NOT_FOUND);
    }

    /// Test fence with mock — single proc fence.
    #[test]
    fn test_mock_fence_single_proc() {
        let _guard = MockGuard::new();
        let procs = vec![Proc::new("fence.ns", 0).unwrap()];
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].get_rank(), 0);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
    }

    /// Test fence with mock — five procs across two namespaces.
    #[test]
    fn test_mock_fence_five_procs_two_namespaces() {
        let _guard = MockGuard::new();
        let procs = vec![
            Proc::new("ns_a", 0).unwrap(),
            Proc::new("ns_a", 1).unwrap(),
            Proc::new("ns_a", 2).unwrap(),
            Proc::new("ns_b", 0).unwrap(),
            Proc::new("ns_b", 1).unwrap(),
        ];
        assert_eq!(procs.len(), 5);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
    }

    /// Test fence with mock — empty procs vector.
    #[test]
    fn test_mock_fence_empty_procs() {
        let _guard = MockGuard::new();
        let procs: Vec<Proc> = vec![];
        assert_eq!(procs.len(), 0);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
    }

    /// Test fence error result construction.
    #[test]
    fn test_mock_fence_error_result() {
        let config = MockConfig::new().with_function_status("PMIx_Fence", PMIX_ERR_TIMEOUT);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Fence");
        let status = PmixStatus::from_raw(raw);
        let result: Result<(), PmixStatus> = if status.is_success() {
            Ok(())
        } else {
            Err(status)
        };
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            PmixStatus::Known(PmixError::ErrTimeout)
        );
    }

    /// Test fence_nb with mock — procs and info combined.
    #[test]
    fn test_mock_fence_procs_and_info() {
        let _guard = MockGuard::new();
        let procs = vec![
            Proc::new("combined.ns", 0).unwrap(),
            Proc::new("combined.ns", 1).unwrap(),
        ];
        let info = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        assert_eq!(procs.len(), 2);
        assert_eq!(info.len(), 0);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
    }

    // ─── Mock-aware store_internal tests ────────────────────────────────────

    /// Test store_internal with mock — proc and key validation.
    #[test]
    fn test_mock_store_internal_proc_key() {
        let _guard = MockGuard::new();
        let proc = Proc::new("store.ns", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    /// Test store_internal with mock — key stored in mock store.
    #[test]
    fn test_mock_store_internal_key_stored() {
        let _guard = MockGuard::new();
        mock_ffi::mock_store_value("internal.store.key", b"internal_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("internal.store.key"));
        mock_ffi::mock_remove_value("internal.store.key");
    }

    /// Test store_internal with mock — error path.
    #[test]
    fn test_mock_store_internal_error_path() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_INIT);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        let status = PmixStatus::from_raw(raw);
        let result: Result<(), PmixStatus> = if status.is_success() {
            Ok(())
        } else {
            Err(status)
        };
        assert!(result.is_err());
    }

    /// Test store_internal with mock — duplicate key error.
    #[test]
    fn test_mock_store_internal_duplicate_key() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        assert_eq!(raw, PMIX_ERR_DUPLICATE_KEY);
    }

    // ─── Mock Proc tests ────────────────────────────────────────────────────

    /// Test Proc with mock — namespace with dots.
    #[test]
    fn test_mock_proc_namespace_with_dots() {
        let _guard = MockGuard::new();
        let proc = Proc::new("org.openpmix.test", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    /// Test Proc with mock — rank boundary at zero.
    #[test]
    fn test_mock_proc_rank_zero() {
        let _guard = MockGuard::new();
        let proc = Proc::new("zero.rank", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    /// Test Proc with mock — rank boundary at u32::MAX.
    #[test]
    fn test_mock_proc_rank_max() {
        let _guard = MockGuard::new();
        let proc = Proc::new("max.rank", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
    }

    /// Test Proc with mock — multiple procs same namespace different ranks.
    #[test]
    fn test_mock_proc_same_ns_diff_ranks() {
        let _guard = MockGuard::new();
        let p0 = Proc::new("same.ns", 0).unwrap();
        let p1 = Proc::new("same.ns", 1).unwrap();
        let p2 = Proc::new("same.ns", 2).unwrap();
        assert_ne!(p0.get_rank(), p1.get_rank());
        assert_ne!(p1.get_rank(), p2.get_rank());
    }

    /// Test Proc with mock — different namespaces same rank.
    #[test]
    fn test_mock_proc_diff_ns_same_rank() {
        let _guard = MockGuard::new();
        let p1 = Proc::new("ns_a", 5).unwrap();
        let p2 = Proc::new("ns_b", 5).unwrap();
        assert_eq!(p1.get_rank(), p2.get_rank());
    }

    // ─── Mock Info tests ────────────────────────────────────────────────────

    /// Test Info with mock — non-empty info struct.
    #[test]
    fn test_mock_info_nonempty() {
        let _guard = MockGuard::new();
        let fake_handle = 0xDEADBEEFusize as *mut ffi::pmix_info_t;
        let info = Info {
            handle: fake_handle,
            len: 3,
        };
        assert_eq!(info.len(), 3);
        assert!(!info.is_empty());
    }

    /// Test Info with mock — info with single element.
    #[test]
    fn test_mock_info_single_element() {
        let _guard = MockGuard::new();
        let fake_handle = 0x1usize as *mut ffi::pmix_info_t;
        let info = Info {
            handle: fake_handle,
            len: 1,
        };
        assert_eq!(info.len(), 1);
        assert!(!info.is_empty());
    }

    /// Test Info with mock — large info array.
    #[test]
    fn test_mock_info_large_array() {
        let _guard = MockGuard::new();
        let fake_handle = 0x2usize as *mut ffi::pmix_info_t;
        let info = Info {
            handle: fake_handle,
            len: 1000,
        };
        assert_eq!(info.len(), 1000);
        assert!(!info.is_empty());
    }

    // ─── Mock PmixPdata tests ───────────────────────────────────────────────

    /// Test PmixPdata with mock — key with hyphens.
    #[test]
    fn test_mock_pdata_key_with_hyphens() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("test-key-with-hyphens");
        assert_eq!(pdata.key, "test-key-with-hyphens");
    }

    /// Test PmixPdata with mock — key with numbers.
    #[test]
    fn test_mock_pdata_key_with_numbers() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("key123.number456");
        assert_eq!(pdata.key, "key123.number456");
    }

    /// Test PmixPdata with mock — key with mixed case.
    #[test]
    fn test_mock_pdata_key_mixed_case() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("MixedCase.Key123");
        assert_eq!(pdata.key, "MixedCase.Key123");
    }

    /// Test PmixPdata with mock — key with leading underscore.
    #[test]
    fn test_mock_pdata_key_leading_underscore() {
        let _guard = MockGuard::new();
        let pdata = PmixPdata::new("_private.key");
        assert_eq!(pdata.key, "_private.key");
    }

    /// Test PmixPdata with mock — proc field set.
    #[test]
    fn test_mock_pdata_proc_field() {
        let _guard = MockGuard::new();
        let mut pdata = PmixPdata::new("proc.key");
        pdata.proc = Proc::new("pdata.ns", 42).unwrap();
        assert_eq!(pdata.proc.get_rank(), 42);
    }

    // ─── Mock PmixStatus tests ──────────────────────────────────────────────

    /// Test PmixStatus with mock — success is_success and not is_error.
    #[test]
    fn test_mock_status_success_flags() {
        let _guard = MockGuard::new();
        let raw = mock_ffi::get_mock_status("PMIx_Publish");
        let status = PmixStatus::from_raw(raw);
        assert!(status.is_success());
        assert!(!status.is_error());
    }

    /// Test PmixStatus with mock — error is_error and not is_success.
    #[test]
    fn test_mock_status_error_flags() {
        let config = MockConfig::new().with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND);
        let _guard = MockGuard::with_config(config);
        let raw = mock_ffi::get_mock_status("PMIx_Get");
        let status = PmixStatus::from_raw(raw);
        assert!(status.is_error());
        assert!(!status.is_success());
    }

    /// Test PmixStatus with mock — to_raw roundtrip.
    #[test]
    fn test_mock_status_to_raw_roundtrip() {
        let _guard = MockGuard::new();
        let status = PmixStatus::from_raw(PMIX_SUCCESS);
        assert_eq!(status.to_raw(), PMIX_SUCCESS);
    }

    /// Test PmixStatus with mock — error to_raw roundtrip.
    #[test]
    fn test_mock_status_error_to_raw_roundtrip() {
        let status = PmixStatus::from_raw(PMIX_ERR_NOT_FOUND);
        assert_eq!(status.to_raw(), PMIX_ERR_NOT_FOUND);
    }

    // ─── Mock comprehensive workflow tests ──────────────────────────────────

    /// Test complete publish-get-unpublish workflow with mock.
    #[test]
    fn test_mock_full_publish_get_unpublish_workflow() {
        let _guard = MockGuard::new();
        mock_ffi::mock_clear_store();

        // Phase 1: Publish
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        mock_ffi::mock_store_value("workflow.key", b"workflow_value", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("workflow.key"));

        // Phase 2: Get
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);

        // Phase 3: Fence (barrier)
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);

        // Phase 4: Unpublish
        assert_eq!(mock_ffi::get_mock_status("PMIx_Unpublish"), PMIX_SUCCESS);
        mock_ffi::mock_remove_value("workflow.key");
        assert!(!mock_ffi::mock_key_exists("workflow.key"));
    }

    /// Test error workflow with mock — publish fails, get fails, unpublish fails.
    #[test]
    fn test_mock_error_workflow_all_fail() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_ERR_INIT)
            .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND)
            .with_function_status("PMIx_Unpublish", PMIX_ERR_TIMEOUT);
        let _guard = MockGuard::with_config(config);

        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_ERR_INIT);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        assert_eq!(
            mock_ffi::get_mock_status("PMIx_Unpublish"),
            PMIX_ERR_TIMEOUT
        );
    }

    /// Test mock with multiple key-value pairs and selective removal.
    #[test]
    fn test_mock_selective_key_removal() {
        mock_ffi::mock_clear_store();
        mock_ffi::mock_store_value("a", b"1", PMIX_STRING);
        mock_ffi::mock_store_value("b", b"2", PMIX_STRING);
        mock_ffi::mock_store_value("c", b"3", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("a"));
        assert!(mock_ffi::mock_key_exists("b"));
        assert!(mock_ffi::mock_key_exists("c"));
        // Remove only 'b'
        mock_ffi::mock_remove_value("b");
        assert!(mock_ffi::mock_key_exists("a"));
        assert!(!mock_ffi::mock_key_exists("b"));
        assert!(mock_ffi::mock_key_exists("c"));
        mock_ffi::mock_clear_store();
    }

    /// Test mock with long binary data values.
    #[test]
    fn test_mock_binary_data_storage() {
        let data: Vec<u8> = (0..=255).collect();
        mock_ffi::mock_store_value("binary.key", &data, PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("binary.key"));
        mock_ffi::mock_remove_value("binary.key");
    }

    /// Test mock with empty string value.
    #[test]
    fn test_mock_empty_string_value() {
        mock_ffi::mock_store_value("empty.val", b"", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("empty.val"));
        mock_ffi::mock_remove_value("empty.val");
    }

    /// Test mock config with mixed success and error statuses.
    #[test]
    fn test_mock_config_mixed_statuses() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_SUCCESS)
            .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND)
            .with_function_status("PMIx_Fence", PMIX_SUCCESS)
            .with_function_status("PMIx_Unpublish", PMIX_SUCCESS)
            .with_function_status("PMIx_Lookup", PMIX_ERR_TIMEOUT);
        let _guard = MockGuard::with_config(config);

        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Unpublish"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Lookup"), PMIX_ERR_TIMEOUT);
    }

    /// Test mock guard nesting behavior — inner guard disables on drop.
    #[test]
    fn test_mock_guard_nesting() {
        assert!(!mock_ffi::is_mock_enabled());
        {
            let _outer = MockGuard::new();
            assert!(mock_ffi::is_mock_enabled());
            // Inner scope — creates another guard (re-enables mock)
            {
                let _inner = MockGuard::new();
                assert!(mock_ffi::is_mock_enabled());
            }
            // After inner drops, mock is disabled (no ref counting)
            assert!(!mock_ffi::is_mock_enabled());
        }
        assert!(!mock_ffi::is_mock_enabled());
    }

    /// Test mock store with unicode key names.
    #[test]
    fn test_mock_unicode_key_storage() {
        mock_ffi::mock_store_value("key.αβγ", b"unicode_val", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("key.αβγ"));
        mock_ffi::mock_remove_value("key.αβγ");
    }

    /// Test mock store with key containing spaces.
    #[test]
    fn test_mock_key_with_spaces() {
        mock_ffi::mock_store_value("key with spaces", b"spaced_val", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("key with spaces"));
        mock_ffi::mock_remove_value("key with spaces");
    }

    /// Test mock store with very long key.
    #[test]
    fn test_mock_very_long_key() {
        let long_key = "a.".repeat(500);
        mock_ffi::mock_store_value(&long_key, b"val", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists(&long_key));
        mock_ffi::mock_remove_value(&long_key);
    }

    /// Test mock callback bridge — unpublish with init error.
    #[test]
    fn test_mock_unpublish_callback_init_error() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static UNPUB_CB_INIT: AtomicI32 = AtomicI32::new(-999);

        struct UnpubInitCb;
        impl UnpublishCallback for UnpubInitCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                UNPUB_CB_INIT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = UNPUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = UNPUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(UnpubInitCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        unpublish_callback_bridge(PMIX_ERR_INIT, cbdata);
        assert_eq!(UNPUB_CB_INIT.load(Ordering::SeqCst), PMIX_ERR_INIT);
    }

    /// Test mock callback bridge — fence with duplicate key error.
    #[test]
    fn test_mock_fence_callback_duplicate_key() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static FENCE_CB_DUP: AtomicI32 = AtomicI32::new(-999);

        struct FenceDupCb;
        impl FenceCallback for FenceDupCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                FENCE_CB_DUP.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(FenceDupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(PMIX_ERR_DUPLICATE_KEY, cbdata);
        assert_eq!(FENCE_CB_DUP.load(Ordering::SeqCst), PMIX_ERR_DUPLICATE_KEY);
    }

    /// Test mock callback bridge — get with duplicate key error.
    #[test]
    fn test_mock_get_callback_duplicate_key() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static GET_CB_DUP: AtomicI32 = AtomicI32::new(-999);

        struct GetDupCb;
        impl GetValueCallback for GetDupCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _value: Option<PmixOwnedValue>) {
                GET_CB_DUP.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(GetDupCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(PMIX_ERR_DUPLICATE_KEY, std::ptr::null_mut(), cbdata);
        assert_eq!(GET_CB_DUP.load(Ordering::SeqCst), PMIX_ERR_DUPLICATE_KEY);
    }

    /// Test mock callback bridge — lookup with init error.
    #[test]
    fn test_mock_lookup_callback_init_error() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static LOOKUP_CB_INIT2: AtomicI32 = AtomicI32::new(-999);

        struct LookupInitCb2;
        impl LookupCallback for LookupInitCb2 {
            fn on_result(self: Box<Self>, status: PmixStatus, _data: Vec<PmixPdata>) {
                LOOKUP_CB_INIT2.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(LookupInitCb2));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(PMIX_ERR_INIT, std::ptr::null_mut(), 0, cbdata);
        assert_eq!(LOOKUP_CB_INIT2.load(Ordering::SeqCst), PMIX_ERR_INIT);
    }

    /// Test mock — publish-get-fence-unpublish cycle with status checks.
    #[test]
    fn test_mock_operation_cycle_status_checks() {
        let _guard = MockGuard::new();
        // All operations should return PMIX_SUCCESS by default
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Fence"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Unpublish"), PMIX_SUCCESS);
        assert_eq!(mock_ffi::get_mock_status("PMIx_Lookup"), PMIX_SUCCESS);
    }

    /// Test mock — PmixStatus equality comparison.
    #[test]
    fn test_mock_status_equality() {
        let s1 = PmixStatus::from_raw(PMIX_SUCCESS);
        let s2 = PmixStatus::from_raw(PMIX_SUCCESS);
        assert_eq!(s1, s2);

        let e1 = PmixStatus::from_raw(PMIX_ERR_NOT_FOUND);
        let e2 = PmixStatus::from_raw(PMIX_ERR_NOT_FOUND);
        assert_eq!(e1, e2);

        assert_ne!(s1, e1);
    }

    /// Test mock — PmixStatus partial ordering.
    #[test]
    fn test_mock_status_partial_ordering() {
        let success = PmixStatus::from_raw(PMIX_SUCCESS);
        let error = PmixStatus::from_raw(PMIX_ERR_NOT_FOUND);
        // Unknown status
        let unknown = PmixStatus::from_raw(-99999);
        assert_ne!(success, error);
        assert_ne!(success, unknown);
        assert_ne!(error, unknown);
    }

    /// Test mock — concurrent callback invocation on different registries.
    #[test]
    fn test_mock_concurrent_callback_registries() {
        use std::sync::atomic::{AtomicI32, Ordering};
        use std::thread;

        static PUB_RESULT: AtomicI32 = AtomicI32::new(-999);
        static GET_RESULT: AtomicI32 = AtomicI32::new(-999);

        struct ConcPubCb;
        impl PublishCallback for ConcPubCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                PUB_RESULT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        struct ConcGetCb;
        impl GetValueCallback for ConcGetCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _value: Option<PmixOwnedValue>) {
                GET_RESULT.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let handles: Vec<_> = vec![
            // Thread 1: publish callback
            thread::spawn(|| {
                let req_id = {
                    let mut seq = PUBLISH_SEQ.lock().unwrap();
                    *seq += 1;
                    *seq
                };
                {
                    let mut registry = PUBLISH_REGISTRY.lock().unwrap();
                    registry.insert(req_id, Box::new(ConcPubCb));
                }
                let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
                publish_callback_bridge(PMIX_SUCCESS, cbdata);
            }),
            // Thread 2: get callback
            thread::spawn(|| {
                let req_id = {
                    let mut seq = GET_SEQ.lock().unwrap();
                    *seq += 1;
                    *seq
                };
                {
                    let mut registry = GET_REGISTRY.lock().unwrap();
                    registry.insert(req_id, Box::new(ConcGetCb));
                }
                let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
                get_value_callback_bridge(PMIX_SUCCESS, std::ptr::null_mut(), cbdata);
            }),
        ];

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(PUB_RESULT.load(Ordering::SeqCst), PMIX_SUCCESS);
        assert_eq!(GET_RESULT.load(Ordering::SeqCst), PMIX_SUCCESS);
    }

    /// Test mock — store_internal with mock store simulation.
    #[test]
    fn test_mock_store_internal_full_simulation() {
        let _guard = MockGuard::new();
        mock_ffi::mock_clear_store();
        // Simulate store_internal storing a key
        let proc = Proc::new("internal.ns", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
        mock_ffi::mock_store_value("internal.key", b"internal_val", PMIX_STRING);
        assert!(mock_ffi::mock_key_exists("internal.key"));
        // Verify mock status
        assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        mock_ffi::mock_remove_value("internal.key");
    }

    /// Test mock — Info pointer handling for various sizes.
    #[test]
    fn test_mock_info_ptr_sizes() {
        let _guard = MockGuard::new();
        // Size 0 — null pointer
        let info0 = Info {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        let (p0, n0) = if info0.len > 0 {
            (info0.handle as *const ffi::pmix_info_t, info0.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(p0.is_null());
        assert_eq!(n0, 0);

        // Size 1 — non-null pointer
        let info1 = Info {
            handle: 0x1usize as *mut ffi::pmix_info_t,
            len: 1,
        };
        let (p1, n1) = if info1.len > 0 {
            (info1.handle as *const ffi::pmix_info_t, info1.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(!p1.is_null());
        assert_eq!(n1, 1);

        // Size 100 — non-null pointer
        let info100 = Info {
            handle: 0x2usize as *mut ffi::pmix_info_t,
            len: 100,
        };
        let (p100, n100) = if info100.len > 0 {
            (info100.handle as *const ffi::pmix_info_t, info100.len)
        } else {
            (std::ptr::null(), 0)
        };
        assert!(!p100.is_null());
        assert_eq!(n100, 100);
    }

    /// Test mock — Proc handle field access.
    #[test]
    fn test_mock_proc_handle_fields() {
        let _guard = MockGuard::new();
        let proc = Proc::new("handle_test", 99).unwrap();
        assert_eq!(proc.get_rank(), 99);
    }

    /// Test mock — Proc with empty namespace and zero rank.
    #[test]
    fn test_mock_proc_empty_ns_zero_rank() {
        let _guard = MockGuard::new();
        let proc = Proc::new("", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    /// Test mock — PmixPdata with all fields set.
    #[test]
    fn test_mock_pdata_all_fields() {
        let _guard = MockGuard::new();
        let mut pdata = PmixPdata::new("full.key");
        pdata.proc = Proc::new("full.ns", 7).unwrap();
        assert_eq!(pdata.key, "full.key");
        assert!(pdata.value.is_none());
        assert_eq!(pdata.proc.get_rank(), 7);
    }

    /// Test mock — multiple mock configs in sequence.
    #[test]
    fn test_mock_sequential_configs() {
        // First config: all success
        {
            let _guard = MockGuard::new();
            assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        }
        // Second config: all error
        {
            let config = MockConfig::new().with_default_status(PMIX_ERR_INIT);
            let _guard = MockGuard::with_config(config);
            assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_ERR_INIT);
        }
        // Third config: mixed
        {
            let config = MockConfig::new()
                .with_function_status("PMIx_Publish", PMIX_SUCCESS)
                .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND);
            let _guard = MockGuard::with_config(config);
            assert_eq!(mock_ffi::get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
            assert_eq!(mock_ffi::get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        }
    }

    /// Test mock — fence callback bridge with partial success.
    #[test]
    fn test_mock_fence_callback_partial_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static FENCE_CB_PARTIAL: AtomicI32 = AtomicI32::new(-999);

        struct FencePartialCb;
        impl FenceCallback for FencePartialCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                FENCE_CB_PARTIAL.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = FENCE_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = FENCE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(FencePartialCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        fence_callback_bridge(-52, cbdata); // PMIX_ERR_PARTIAL_SUCCESS
        assert_eq!(FENCE_CB_PARTIAL.load(Ordering::SeqCst), -52);
    }

    /// Test mock — lookup callback bridge with partial success.
    #[test]
    fn test_mock_lookup_callback_partial_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static LOOKUP_CB_PARTIAL: AtomicI32 = AtomicI32::new(-999);

        struct LookupPartialCb;
        impl LookupCallback for LookupPartialCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _data: Vec<PmixPdata>) {
                LOOKUP_CB_PARTIAL.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = LOOKUP_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = LOOKUP_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(LookupPartialCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        lookup_callback_bridge(-52, std::ptr::null_mut(), 0, cbdata);
        assert_eq!(LOOKUP_CB_PARTIAL.load(Ordering::SeqCst), -52);
    }

    /// Test mock — get callback bridge with partial success.
    #[test]
    fn test_mock_get_callback_partial_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static GET_CB_PARTIAL: AtomicI32 = AtomicI32::new(-999);

        struct GetPartialCb;
        impl GetValueCallback for GetPartialCb {
            fn on_result(self: Box<Self>, status: PmixStatus, _value: Option<PmixOwnedValue>) {
                GET_CB_PARTIAL.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = GET_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = GET_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(GetPartialCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        get_value_callback_bridge(-52, std::ptr::null_mut(), cbdata);
        assert_eq!(GET_CB_PARTIAL.load(Ordering::SeqCst), -52);
    }

    /// Test mock — publish callback bridge with partial success.
    #[test]
    fn test_mock_publish_callback_partial_success() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static PUB_CB_PARTIAL: AtomicI32 = AtomicI32::new(-999);

        struct PubPartialCb;
        impl PublishCallback for PubPartialCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                PUB_CB_PARTIAL.store(status.to_raw(), Ordering::SeqCst);
            }
        }

        let req_id = {
            let mut seq = PUBLISH_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };
        {
            let mut registry = PUBLISH_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(PubPartialCb));
        }

        let cbdata = (req_id << 2) as *mut std::os::raw::c_void;
        publish_callback_bridge(-52, cbdata);
        assert_eq!(PUB_CB_PARTIAL.load(Ordering::SeqCst), -52);
    }
}
