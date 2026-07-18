//! Server submodule: pset

use super::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_define_process_set
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_define_process_set`.
///
/// Define a PMIx process set — a named group of processes.
///
/// The PMIx server will alert all local clients of the new process set
/// (including process set name and membership) via the
/// `PMIX_PROCESS_SET_DEFINE` event.
///
/// # Parameters
/// * `members` — array of processes that belong to the process set.
/// * `pset_name` — string name of the process set being defined.
///
/// # Returns
/// * `Ok(())` — process set defined successfully.
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_BAD_PARAM` if the
///   pset_name or members array is null/empty, or other PMIx error constants.
///
/// # Host environment responsibilities
/// The host environment is responsible for ensuring:
/// - Consistent knowledge of process set membership across all involved PMIx servers.
/// - That process set names do not conflict with system-assigned namespaces
///   within the scope of the set.
///
/// # C API
/// `pmix_status_t PMIx_server_define_process_set(const pmix_proc_t members[], size_t nmembers, char *pset_name)`
pub fn server_define_process_set(members: &[Proc], pset_name: &str) -> Result<(), PmixStatus> {
    // Convert pset_name to CString for FFI.
    let pset_name_c = match CString::new(pset_name) {
        Ok(cs) => cs,
        Err(_) => return Err(PmixStatus::from_raw(-1)), // PMIX_ERROR — contains NUL
    };

    // Proc wraps pmix_proc_t as its first field, but also has a `len` field,
    // so we cannot pass &[Proc] directly as *const pmix_proc_t. Instead,
    // allocate a contiguous C array of pmix_proc_t and copy the handles.
    let nmembers = members.len();
    let members_ptr: *const ffi::pmix_proc_t = if nmembers == 0 {
        ptr::null()
    } else {
        // SAFETY: calloc returns a zeroed allocation or null on failure.
        // We allocate space for nmembers pmix_proc_t structs.
        let arr = unsafe {
            libc::calloc(nmembers, std::mem::size_of::<ffi::pmix_proc_t>()) as *mut ffi::pmix_proc_t
        };
        if arr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR — allocation failed
        }
        // Copy each Proc's handle into the C array.
        for (i, proc) in members.iter().enumerate() {
            unsafe {
                std::ptr::write(arr.add(i), proc.handle);
            }
        }
        // Call FFI while arr is still valid (or mock).
                let status;
        #[cfg(any(test, feature = "mock_ffi"))]
        {
            status = if mock_ffi::is_mock_enabled() {
                unsafe {
                mock_ffi::mock_server_define_process_set(
                    arr as *const std::ffi::c_void,
                    nmembers,
                    pset_name_c.as_ptr(),
                )
            }
            } else {
                unsafe {
                // SAFETY:
                // - arr is a valid pointer to a contiguous array of pmix_proc_t
                //   values, alive for the duration of this call (PMIx copies the
                //   proc identifiers internally).
                // - pset_name_c.as_ptr() is a valid null-terminated string for the
                //   duration of this call (PMIx copies it internally).
                // - PMIx_server_define_process_set is a synchronous server API.
                ffi::PMIx_server_define_process_set(arr, nmembers, pset_name_c.as_ptr())
            }
            };
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            status = {
                unsafe {
                // SAFETY:
                // - arr is a valid pointer to a contiguous array of pmix_proc_t
                //   values, alive for the duration of this call (PMIx copies the
                //   proc identifiers internally).
                // - pset_name_c.as_ptr() is a valid null-terminated string for the
                //   duration of this call (PMIx copies it internally).
                // - PMIx_server_define_process_set is a synchronous server API.
                ffi::PMIx_server_define_process_set(arr, nmembers, pset_name_c.as_ptr())
            }
            };
        }
        // Free the temporary C array.
        unsafe {
            libc::free(arr as *mut std::os::raw::c_void);
        }
        let pmix_status = PmixStatus::from_raw(status);
        return if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        };
    };

    // Empty members case — call with null pointer (or mock).
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_define_process_set(
                ptr::null(),
                nmembers,
                pset_name_c.as_ptr(),
            )
        }
        } else {
            unsafe {
            // SAFETY:
            // - members_ptr is null (empty slice) — PMIx handles this gracefully.
            // - pset_name_c.as_ptr() is a valid null-terminated string.
            // - PMIx_server_define_process_set is a synchronous server API.
            ffi::PMIx_server_define_process_set(members_ptr, nmembers, pset_name_c.as_ptr())
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY:
            // - members_ptr is null (empty slice) — PMIx handles this gracefully.
            // - pset_name_c.as_ptr() is a valid null-terminated string.
            // - PMIx_server_define_process_set is a synchronous server API.
            ffi::PMIx_server_define_process_set(members_ptr, nmembers, pset_name_c.as_ptr())
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_delete_process_set
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_delete_process_set`.
///
/// Delete a PMIx process set — a named group of processes.
///
/// The PMIx server will alert all local clients of the process set name
/// being deleted via the `PMIX_PROCESS_SET_DELETE` event. Deletion of
/// the name has no impact on the member processes.
///
/// # Parameters
/// * `pset_name` — string name of the process set being deleted.
///
/// # Returns
/// * `Ok(())` — process set deleted successfully.
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_BAD_PARAM` if the
///   pset_name is null, `PMIX_ERR_NOT_FOUND` if the process set does not
///   exist, or other PMIx error constants.
///
/// # Host environment responsibilities
/// The host environment is responsible for ensuring consistent knowledge
/// of process set membership across all involved PMIx servers.
///
/// # C API
/// `pmix_status_t PMIx_server_delete_process_set(char *pset_name)`
pub fn server_delete_process_set(pset_name: &str) -> Result<(), PmixStatus> {
    // Convert pset_name to CString for FFI.
    let pset_name_c = match CString::new(pset_name) {
        Ok(cs) => cs,
        Err(_) => return Err(PmixStatus::from_raw(-1)), // PMIX_ERROR — contains NUL
    };

    // The C API takes `char *` (non-const) even though it doesn't modify the
    // string. We use `as_ptr() as *mut` to match the FFI signature; this is
    // safe because PMIx only reads the string and copies it internally.
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_delete_process_set(pset_name_c.as_ptr() as *mut std::os::raw::c_char)
        }
        } else {
            unsafe {
            // SAFETY:
            // - pset_name_c is a valid null-terminated string for the duration of
            //   this call (PMIx copies it internally, does not retain the pointer).
            // - The cast from *const to *mut is safe because PMIx does not write
            //   to the string — the non-const signature is a C API convention.
            // - PMIx_server_delete_process_set is a synchronous server API.
            ffi::PMIx_server_delete_process_set(pset_name_c.as_ptr() as *mut std::os::raw::c_char)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY:
            // - pset_name_c is a valid null-terminated string for the duration of
            //   this call (PMIx copies it internally, does not retain the pointer).
            // - The cast from *const to *mut is safe because PMIx does not write
            //   to the string — the non-const signature is a C API convention.
            // - PMIx_server_delete_process_set is a synchronous server API.
            ffi::PMIx_server_delete_process_set(pset_name_c.as_ptr() as *mut std::os::raw::c_char)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_register_resources — register non-namespace resource information
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_register_resources`.
///
/// Implement this trait to receive the result of a non-blocking resource
/// registration. The `on_complete` method receives the `PmixStatus` result.
pub trait RegisterResourcesCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending register_resources callbacks.
type RegisterResourcesRegistry =
    std::collections::HashMap<usize, Box<dyn RegisterResourcesCallback>>;
static REGISTER_RESOURCES_REGISTRY: LazyLock<Mutex<RegisterResourcesRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing register_resources request ID counter.
static REGISTER_RESOURCES_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (register_resources completion).
///
/// Called by PMIx when the non-blocking resource registration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn register_resources_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = REGISTER_RESOURCES_REGISTRY.lock().unwrap();
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

/// Register non-namespace related resource information with the PMIx server.
///
/// This function passes information about resources not associated with a
/// specific namespace to the PMIx server library for distribution to local
/// client processes. This includes information on fabric devices, GPUs, and
/// other hardware resources. All information provided through this API shall
/// be made available to each job as part of its job-level information.
///
/// Duplicate information provided with `PMIx_server_register_nspace` shall
/// override any information provided by this function for that namespace,
/// but only for that specific namespace.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `info` — array of info structures describing resources (e.g., GPU
///   counts, fabric topology, node-local resources).
/// * `callback` — invoked when registration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid info,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_register_resources(pmix_info_t info[], size_t ninfo,
///                                              pmix_op_cbfunc_t cbfunc,
///                                              void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_register_resources, RegisterResourcesCallback};
/// use pmix::InfoBuilder;
///
/// struct MyResourceCallback;
/// impl RegisterResourcesCallback for MyResourceCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("register_resources completed: {:?}", status);
///     }
/// }
///
/// // Register with no info keys (e.g., to clear previous registration)
/// let info = InfoBuilder::new().build();
/// server_register_resources(&info, Box::new(MyResourceCallback))
///     .expect("register_resources request rejected");
/// ```
pub fn server_register_resources(
    info: &Info,
    callback: Box<dyn RegisterResourcesCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = REGISTER_RESOURCES_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = REGISTER_RESOURCES_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Get a pointer to the info array for FFI.
    let info_ptr = if info.len > 0 {
        info.handle as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };
    let info_len = info.len;

    // Call the FFI function (or mock).
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_register_resources(
                info_ptr as *mut std::ffi::c_void,
                info_len,
                Some(register_resources_callback_bridge),
                cbdata,
            )
        }
        } else {
            unsafe {
            // SAFETY: PMIx_server_register_resources is a non-blocking server API.
            // - info_ptr is either a valid pointer to an array of pmix_info_t
            //   (owned by the Info handle, which remains alive for the duration
            //   of this call — PMIx copies the info internally), or null when
            //   info_len is 0.
            // - info_len is the number of elements in the info array.
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            ffi::PMIx_server_register_resources(
                info_ptr as *mut ffi::pmix_info_t,
                info_len,
                Some(register_resources_callback_bridge),
                cbdata,
            )
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_server_register_resources is a non-blocking server API.
            // - info_ptr is either a valid pointer to an array of pmix_info_t
            //   (owned by the Info handle, which remains alive for the duration
            //   of this call — PMIx copies the info internally), or null when
            //   info_len is 0.
            // - info_len is the number of elements in the info array.
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            ffi::PMIx_server_register_resources(
                info_ptr as *mut ffi::pmix_info_t,
                info_len,
                Some(register_resources_callback_bridge),
                cbdata,
            )
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = REGISTER_RESOURCES_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_deregister_resources — deregister non-namespace resource information
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_deregister_resources`.
///
/// Implement this trait to receive the result of a non-blocking resource
/// deregistration. The `on_complete` method receives the `PmixStatus` result.
pub trait DeregisterResourcesCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending deregister_resources callbacks.
type DeregisterResourcesRegistry =
    std::collections::HashMap<usize, Box<dyn DeregisterResourcesCallback>>;
static DEREGISTER_RESOURCES_REGISTRY: LazyLock<Mutex<DeregisterResourcesRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing deregister_resources request ID counter.
static DEREGISTER_RESOURCES_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (deregister_resources completion).
///
/// Called by PMIx when the non-blocking resource deregistration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn deregister_resources_callback_bridge(
    status: ffi::pmix_status_t,
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
        let mut registry = DEREGISTER_RESOURCES_REGISTRY.lock().unwrap();
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

/// Deregister non-namespace related resource information from the PMIx server.
///
/// This function removes information about resources not associated with a
/// specific namespace from the PMIx server library. This includes information
/// on fabric devices, GPUs, and other hardware resources that were previously
/// registered via [`server_register_resources`].
///
/// The deregister operation allows the host resource manager (RM) to update
/// or remove resource information when the underlying hardware state changes
/// (e.g., a GPU becomes unavailable, a fabric device is replaced).
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `info` — array of info structures identifying which resources to
///   deregister. If empty, all previously registered non-namespace
///   resources are removed.
/// * `callback` — invoked when deregistration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid info,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_deregister_resources(pmix_info_t info[], size_t ninfo,
///                                                pmix_op_cbfunc_t cbfunc,
///                                                void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_deregister_resources, DeregisterResourcesCallback};
/// use pmix::InfoBuilder;
///
/// struct MyResourceCallback;
/// impl DeregisterResourcesCallback for MyResourceCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("deregister_resources completed: {:?}", status);
///     }
/// }
///
/// // Deregister all previously registered non-namespace resources
/// let info = InfoBuilder::new().build();
/// server_deregister_resources(&info, Box::new(MyResourceCallback))
///     .expect("deregister_resources request rejected");
/// ```
pub fn server_deregister_resources(
    info: &Info,
    callback: Box<dyn DeregisterResourcesCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = DEREGISTER_RESOURCES_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = DEREGISTER_RESOURCES_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Get a pointer to the info array for FFI.
    let info_ptr = if info.len > 0 {
        info.handle as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };
    let info_len = info.len;

    // Call the FFI function (or mock).
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_server_deregister_resources(
                info_ptr as *mut std::ffi::c_void,
                info_len,
                Some(deregister_resources_callback_bridge),
                cbdata,
            )
        }
        } else {
            unsafe {
            // SAFETY: PMIx_server_deregister_resources is a non-blocking server API.
            // - info_ptr is either a valid pointer to an array of pmix_info_t
            //   (owned by the Info handle, which remains alive for the duration
            //   of this call — PMIx copies the info internally), or null when
            //   info_len is 0.
            // - info_len is the number of elements in the info array.
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            ffi::PMIx_server_deregister_resources(
                info_ptr as *mut ffi::pmix_info_t,
                info_len,
                Some(deregister_resources_callback_bridge),
                cbdata,
            )
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            // SAFETY: PMIx_server_deregister_resources is a non-blocking server API.
            // - info_ptr is either a valid pointer to an array of pmix_info_t
            //   (owned by the Info handle, which remains alive for the duration
            //   of this call — PMIx copies the info internally), or null when
            //   info_len is 0.
            // - info_len is the number of elements in the info array.
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            ffi::PMIx_server_deregister_resources(
                info_ptr as *mut ffi::pmix_info_t,
                info_len,
                Some(deregister_resources_callback_bridge),
                cbdata,
            )
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = DEREGISTER_RESOURCES_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}


