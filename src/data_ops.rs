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
use crate::{free_value, Info, PmixError, PmixOwnedValue, PmixStatus, Proc};

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
            proc: Proc::new("", PMIX_RANK_WILDCARD as u32).unwrap_or_else(|_| {
                Proc::new("", 0).unwrap()
            }),
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
        ffi::PMIx_Lookup(
            raw_pdata.as_mut_ptr(),
            ndata,
            info_ptr,
            ninfo,
        )
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
        let proc =
            Proc::new(&nspace_str, rank).unwrap_or_else(|_| Proc::new("", 0).unwrap());

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

                let proc = Proc::new(&nspace_str, rank)
                    .unwrap_or_else(|_| Proc::new("", 0).unwrap());

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
pub fn unpublish(
    keys: Option<&[&str]>,
    info: Option<&Info>,
) -> Result<(), PmixStatus> {
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
            key_ptrs.as_mut_ptr() as *mut *mut std::os::raw::c_char
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
            key_ptrs.as_mut_ptr() as *mut *mut std::os::raw::c_char
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
        ffi::PMIx_Unpublish_nb(keys_ptr, info_ptr, ninfo, Some(unpublish_callback_bridge), cbdata)
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
