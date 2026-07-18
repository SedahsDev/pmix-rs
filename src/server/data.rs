//! Server submodule: data

use super::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_publish — publish key-value data through the server
// ─────────────────────────────────────────────────────────────────────────────

/// Publish key-value data to the PMIx key-value store.
///
/// This wraps `PMIx_Publish` to be called from a server context. The data
/// is published under the given namespace and key, making it available for
/// lookup by other processes.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `nspace` — the namespace to publish under.
/// * `key_val` — the key-value pair to publish (wrapped in an `Info`).
///
/// # Returns
/// * `Ok(PmixStatus)` — data published successfully.
/// * `Err(PmixStatus)` — publish failed.
///
/// # C API
/// `pmix_status_t PMIx_Publish(const pmix_info_t info[], size_t ninfo)`
pub fn server_publish(
    _handle: &PmixServerHandle,
    _nspace: &str,
    key_val: &Info,
) -> Result<PmixStatus, PmixStatus> {
    let (info_ptr, ninfo) = if key_val.len > 0 {
        (key_val.handle as *const ffi::pmix_info_t, key_val.len)
    } else {
        (ptr::null(), 0)
    };

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_publish(
                ptr::null(),
                ptr::null(),
                ptr::null(),
                info_ptr as *mut std::ffi::c_void,
                ninfo,
            )
        }
        } else {
            unsafe {
            // SAFETY: PMIx_Publish is a synchronous PMIx API call. The info
            // pointer is valid for the duration of the call. PMIx does not
            // retain the pointer after this call returns.
            ffi::PMIx_Publish(info_ptr, ninfo)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_Publish is a synchronous PMIx API call. The info
            // pointer is valid for the duration of the call. PMIx does not
            // retain the pointer after this call returns.
            ffi::PMIx_Publish(info_ptr, ninfo)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(pmix_status)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_lookup — lookup published key-value data
// ─────────────────────────────────────────────────────────────────────────────

/// Lookup a published key in the PMIx key-value store.
///
/// This wraps `PMIx_Lookup` to be called from a server context.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `nspace` — the namespace to look up in.
/// * `key` — the key to look up.
/// * `info` — optional directives (e.g., timeout).
///
/// # Returns
/// * `Ok(PmixOwnedValue)` — the value associated with the key.
/// * `Err(PmixStatus)` — lookup failed (e.g., key not found).
///
/// # C API
/// `pmix_status_t PMIx_Lookup(pmix_pdata_t data[], size_t ndata, ...)`
pub fn server_lookup(
    _handle: &PmixServerHandle,
    _nspace: &str,
    key: &str,
    _info: &[Info],
) -> Result<PmixOwnedValue, PmixStatus> {
    // Build a single pmix_pdata_t for the lookup.
    let mut pdata: ffi::pmix_pdata_t = unsafe { std::mem::zeroed() };

    // Copy the key into pdata.key.
    let key_bytes = key.as_bytes();
    let klen = key_bytes.len().min(511);
    unsafe {
        std::ptr::copy_nonoverlapping(key_bytes.as_ptr(), pdata.key.as_mut_ptr() as *mut u8, klen);
        pdata.key[klen] = 0;
    }

    // Initialize the proc field as wildcard.
    pdata.proc_.rank = ffi::PMIX_RANK_WILDCARD;

    // Zero the value so PMIx writes into it.
    unsafe {
        std::ptr::write_bytes(&mut pdata.value, 0, 1);
    }

    // Construct using the PMIx constructor.
    unsafe { ffi::PMIx_Pdata_construct(&mut pdata) };

    // Prepare info parameters.
    let (info_ptr, ninfo) = if !_info.is_empty() && _info[0].len > 0 {
        (_info[0].handle as *const ffi::pmix_info_t, _info[0].len)
    } else {
        (ptr::null(), 0)
    };

    // Call the FFI function (or mock).
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_lookup(ptr::null(), ptr::null(), ptr::null(), 0, ptr::null_mut(), 0, ptr::null_mut())
        }
        } else {
            unsafe {
            // SAFETY: PMIx_Lookup is a synchronous PMIx API call. The pdata
            // is valid for the duration of the call. PMIx writes the proc
            // and value fields.
            ffi::PMIx_Lookup(&mut pdata, 1, info_ptr, ninfo)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_Lookup is a synchronous PMIx API call. The pdata
            // is valid for the duration of the call. PMIx writes the proc
            // and value fields.
            ffi::PMIx_Lookup(&mut pdata, 1, info_ptr, ninfo)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);

    // Check for success or not-found.
    if pmix_status == PmixStatus::Known(PmixError::Success) {
        // Extract the value.
        let pmix_undef: ffi::pmix_data_type_t = ffi::PMIX_UNDEF as u16;
        if pdata.value.type_ != pmix_undef {
            // Take ownership of the value.
            let val = unsafe { ptr::read(&pdata.value) };
            // Destruct the pdata.
            unsafe { ffi::PMIx_Pdata_destruct(&mut pdata) };
            Ok(PmixOwnedValue { inner: val })
        } else {
            unsafe { ffi::PMIx_Pdata_destruct(&mut pdata) };
            Err(PmixStatus::Known(PmixError::ErrNotFound))
        }
    } else {
        // Clean up.
        unsafe {
            crate::free_value(&mut pdata.value);
            ffi::PMIx_Pdata_destruct(&mut pdata);
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_delete — delete (unpublish) key-value data
// ─────────────────────────────────────────────────────────────────────────────

/// Delete (unpublish) a key from the PMIx key-value store.
///
/// This wraps `PMIx_Unpublish` to be called from a server context.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `nspace` — the namespace to delete from.
/// * `key` — the key to delete.
///
/// # Returns
/// * `Ok(PmixStatus)` — key deleted successfully.
/// * `Err(PmixStatus)` — delete failed.
///
/// # C API
/// `pmix_status_t PMIx_Unpublish(char **keys, ...)`
pub fn server_delete(
    _handle: &PmixServerHandle,
    _nspace: &str,
    key: &str,
) -> Result<PmixStatus, PmixStatus> {
    // Convert key to C string.
    let cstring = std::ffi::CString::new(key).map_err(|_| PmixStatus::Known(PmixError::Error))?;

    // Build NULL-terminated array.
    let mut key_ptrs: Vec<*mut std::os::raw::c_char> = Vec::with_capacity(2);
    key_ptrs.push(cstring.as_ptr() as *mut std::os::raw::c_char);
    key_ptrs.push(ptr::null_mut());

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_delete(ptr::null(), cstring.as_ptr(), ptr::null_mut(), 0)
        }
        } else {
            unsafe {
            // SAFETY: PMIx_Unpublish is a synchronous PMIx API call.
            // keys_ptr is a valid NULL-terminated array of C strings.
            ffi::PMIx_Unpublish(key_ptrs.as_mut_ptr(), ptr::null(), 0)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_Unpublish is a synchronous PMIx API call.
            // keys_ptr is a valid NULL-terminated array of C strings.
            ffi::PMIx_Unpublish(key_ptrs.as_mut_ptr(), ptr::null(), 0)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(pmix_status)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_fence — synchronization fence
// ─────────────────────────────────────────────────────────────────────────────

/// Execute a synchronization fence operation.
///
/// This wraps `PMIx_Fence` to be called from a server context. The fence
/// ensures that all prior publish/lookup operations are visible before
/// the fence returns.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `info` — optional directives (e.g., timeout).
/// * `timeout` — timeout in seconds (0 means no timeout).
///
/// # Returns
/// * `Ok(PmixStatus)` — fence completed successfully.
/// * `Err(PmixStatus)` — fence failed or timed out.
///
/// # C API
/// `pmix_status_t PMIx_Fence(const pmix_proc_t procs[], size_t nprocs, ...)`
pub fn server_fence(
    _handle: &PmixServerHandle,
    _info: &[Info],
    _timeout: i32,
) -> Result<PmixStatus, PmixStatus> {
    // Prepare info parameters.
    let (info_ptr, ninfo) = if !_info.is_empty() && _info[0].len > 0 {
        (_info[0].handle as *const ffi::pmix_info_t, _info[0].len)
    } else {
        (ptr::null(), 0)
    };

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_fence(ptr::null(), 0, info_ptr as *mut std::ffi::c_void, ninfo, ptr::null_mut(), ptr::null_mut())
        }
        } else {
            unsafe {
            // SAFETY: PMIx_Fence is a synchronous PMIx API call.
            // We pass null procs and 0 nprocs to fence all processes.
            ffi::PMIx_Fence(ptr::null(), 0, info_ptr, ninfo)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_Fence is a synchronous PMIx API call.
            // We pass null procs and 0 nprocs to fence all processes.
            ffi::PMIx_Fence(ptr::null(), 0, info_ptr, ninfo)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(pmix_status)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Fence_nb — non-blocking synchronization fence (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Callback wrapper for [`server_fence_nb`].
///
/// Wraps a Rust closure so it can be called from the C FFI callback.
/// The closure receives `PmixStatus` — the result of the fence operation.
pub struct FenceNbCallbackWrapper {
    pub(crate) callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
}

impl FenceNbCallbackWrapper {
    /// Create a new wrapper around a Rust closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// Execute a non-blocking synchronization fence operation from a server context.
///
/// This wraps `PMIx_Fence_nb` to be called from a server context. Unlike
/// [`server_fence`], this returns immediately and invokes the provided
/// callback when the fence completes.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `info` — optional directives (e.g., timeout).
/// * `callback` — a [`FenceNbCallbackWrapper`] containing a closure that
///   receives `PmixStatus` when the fence completes.
///
/// # Returns
/// * `Ok(())` — the fence request was accepted (async, result in callback).
/// * `Err(PmixStatus)` — the fence request itself failed synchronously.
///
/// # C API
/// `pmix_status_t PMIx_Fence_nb(const pmix_proc_t procs[], size_t nprocs,
///                               const pmix_info_t info[], size_t ninfo,
///                               pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn server_fence_nb(
    _handle: &PmixServerHandle,
    _info: &[Info],
    callback: FenceNbCallbackWrapper,
) -> Result<(), PmixStatus> {
    let cb_box: *mut FenceNbCallbackWrapper = Box::into_raw(Box::new(callback));

    pub(crate) extern "C" fn fence_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut FenceNbCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
    }

    let (info_ptr, ninfo) = if !_info.is_empty() && _info[0].len > 0 {
        (_info[0].handle as *const ffi::pmix_info_t, _info[0].len)
    } else {
        (ptr::null(), 0)
    };

    let status = unsafe {
        ffi::PMIx_Fence_nb(
            ptr::null(),
            0,
            info_ptr,
            ninfo,
            Some(fence_nb_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe {
            let _ = Box::from_raw(cb_box);
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Connect — connect processes (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Connect a set of processes from a server context.
///
/// This wraps `PMIx_Connect` to be called from a server context. The connect
/// operation establishes a communication channel between the specified
/// processes, enabling them to exchange data via PMIx.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `procs` — the set of processes to connect.
/// * `info` — optional directives for the connect operation.
///
/// # Returns
/// * `Ok(())` — all processes connected successfully.
/// * `Err(PmixStatus)` — connect failed.
///
/// # C API
/// `pmix_status_t PMIx_Connect(const pmix_proc_t procs[], size_t nprocs,
///                              const pmix_info_t info[], size_t ninfo);`
pub fn server_connect(
    _handle: &PmixServerHandle,
    procs: &[Proc],
    info: &[Info],
) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_fence(
                procs_ptr as *const std::ffi::c_void,
                procs.len(),
                info_ptr as *mut std::ffi::c_void,
                ninfo,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        } else {
            unsafe { ffi::PMIx_Connect(procs_ptr, procs.len(), info_ptr, ninfo) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Connect(procs_ptr, procs.len(), info_ptr, ninfo) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking connect from a server context.
///
/// This wraps `PMIx_Connect_nb` to be called from a server context.
///
/// # C API
/// `pmix_status_t PMIx_Connect_nb(const pmix_proc_t procs[], size_t nprocs,
///                                 const pmix_info_t info[], size_t ninfo,
///                                 pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn server_connect_nb(
    _handle: &PmixServerHandle,
    procs: &[Proc],
    info: &[Info],
    callback: FenceNbCallbackWrapper,
) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let cb_box: *mut FenceNbCallbackWrapper = Box::into_raw(Box::new(callback));

    pub(crate) extern "C" fn connect_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut FenceNbCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
    }

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let status = unsafe {
        ffi::PMIx_Connect_nb(
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            Some(connect_nb_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe {
            let _ = Box::from_raw(cb_box);
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Disconnect — disconnect processes (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Disconnect a set of processes from a server context.
///
/// This wraps `PMIx_Disconnect` to be called from a server context.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `procs` — the set of processes to disconnect.
/// * `info` — optional directives for the disconnect operation.
///
/// # Returns
/// * `Ok(())` — all processes disconnected successfully.
/// * `Err(PmixStatus)` — disconnect failed.
///
/// # C API
/// `pmix_status_t PMIx_Disconnect(const pmix_proc_t procs[], size_t nprocs,
///                                 const pmix_info_t info[], size_t ninfo);`
pub fn server_disconnect(
    _handle: &PmixServerHandle,
    procs: &[Proc],
    info: &[Info],
) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_fence(
                procs_ptr as *const std::ffi::c_void,
                procs.len(),
                info_ptr as *mut std::ffi::c_void,
                ninfo,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        } else {
            unsafe { ffi::PMIx_Disconnect(procs_ptr, procs.len(), info_ptr, ninfo) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Disconnect(procs_ptr, procs.len(), info_ptr, ninfo) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking disconnect from a server context.
///
/// # C API
/// `pmix_status_t PMIx_Disconnect_nb(const pmix_proc_t ranges[], size_t nprocs,
///                                    const pmix_info_t info[], size_t ninfo,
///                                    pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn server_disconnect_nb(
    _handle: &PmixServerHandle,
    procs: &[Proc],
    info: &[Info],
    callback: FenceNbCallbackWrapper,
) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let cb_box: *mut FenceNbCallbackWrapper = Box::into_raw(Box::new(callback));

    pub(crate) extern "C" fn disconnect_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut FenceNbCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
    }

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let status = unsafe {
        ffi::PMIx_Disconnect_nb(
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            Some(disconnect_nb_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe {
            let _ = Box::from_raw(cb_box);
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Spawn — spawn processes (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn processes from a server context.
///
/// This wraps `PMIx_Spawn` to be called from a server context.
/// Delegates to [`crate::process_mgmt::spawn`] for the actual implementation.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `job_info` — job-level directives.
/// * `apps` — applications to spawn.
///
/// # Returns
/// * `Ok(String)` — the namespace of the spawned job.
/// * `Err(PmixStatus)` — spawn failed.
///
/// # C API
/// `pmix_status_t PMIx_Spawn(const pmix_info_t job_info[], size_t ninfo,
///                            const pmix_app_t apps[], size_t napps,
///                            char nspace[]);`
pub fn server_spawn(
    _handle: &PmixServerHandle,
    job_info: &[Info],
    apps: &[crate::process_mgmt::PmixApp],
) -> Result<String, PmixStatus> {
    crate::process_mgmt::spawn(job_info, apps)
}

/// Non-blocking spawn from a server context.
///
/// Delegates to [`crate::process_mgmt::spawn_nb`] for the actual implementation.
///
/// # C API
/// `pmix_status_t PMIx_Spawn_nb(const pmix_info_t job_info[], size_t ninfo,
///                               const pmix_app_t apps[], size_t napps,
///                               pmix_spawn_cbfunc_t cbfunc, void *cbdata);`
pub fn server_spawn_nb(
    _handle: &PmixServerHandle,
    job_info: &[Info],
    apps: &[crate::process_mgmt::PmixApp],
    callback: crate::process_mgmt::SpawnCallbackWrapper,
) -> Result<(), PmixStatus> {
    crate::process_mgmt::spawn_nb(job_info, apps, callback)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_tool_attach_to_server — tool attach (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Attach a tool to a server from a server context.
///
/// This wraps `PMIx_tool_attach_to_server` to be called from a server context.
/// Delegates to [`crate::tool::tool_attach_to_server`] for the actual implementation.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `myproc` — optional process identity for the tool.
/// * `want_server` — whether to request the server's identity.
/// * `info` — directives for the attach operation.
///
/// # Returns
/// * `Ok((Option<PmixToolHandle>, Option<PmixServerHandle>))` — tool and/or server handles.
/// * `Err(PmixStatus)` — attach failed.
///
/// # C API
/// `pmix_status_t PMIx_tool_attach_to_server(pmix_proc_t *myproc, pmix_proc_t *server,
///                                            pmix_info_t info[], size_t ninfo);`
pub fn server_tool_attach_to_server(
    _handle: &PmixServerHandle,
    myproc: Option<&Proc>,
    want_server: bool,
    info: &Info,
) -> Result<
    (
        Option<crate::tool::PmixToolHandle>,
        Option<crate::tool::PmixServerHandle>,
    ),
    PmixStatus,
> {
        #[cfg(any(test, feature = "mock_ffi"))]
    {
        if mock_ffi::is_mock_enabled() {
            let status = unsafe {
            mock_ffi::mock_server_tool_attach_to_server(
                0u32,
                0u32,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
            )
        };
        let pmix_status = PmixStatus::from_raw(status);
        if pmix_status.is_success() {
            Ok((None, None))
        } else {
            Err(pmix_status)
        }
        } else {
            crate::tool::tool_attach_to_server(myproc, want_server, info)
        }
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        crate::tool::tool_attach_to_server(myproc, want_server, info)
    }
}


