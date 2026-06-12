//! Data operations — non-blocking data retrieval.
//!
//! This module provides safe Rust wrappers for PMIx non-blocking data
//! operations that use callback-based completion.

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_nb — non-blocking data retrieval
// ─────────────────────────────────────────────────────────────────────────────

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::{LazyLock, Mutex};

use crate::ffi;
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc};

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
static GET_REGISTRY: LazyLock<Mutex<GetRegistry>> = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

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
