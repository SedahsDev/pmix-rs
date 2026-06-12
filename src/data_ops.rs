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
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc};

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
