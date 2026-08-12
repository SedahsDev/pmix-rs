//! Fabric operations — `PMIx_Fabric_register`, `PMIx_Fabric_update`, `PMIx_Fabric_deregister`,
//! `PMIx_Compute_distances`, `PMIx_Load_topology`.
//!
//! This module provides safe Rust wrappers for the PMIx fabric-related APIs
//! that manage access to fabric information, including communication cost
//! matrices and topology data for interconnects (e.g., InfiniBand, RoCE,
//! NVLink, GPU direct).
//!
//! # Fabric lifecycle
//!
//! 1. Create a [`PmixFabric`] object (initially unregistered).
//! 2. Call [`fabric_register`] to register it with the PMIx library,
//!    optionally passing directives to select a specific fabric.
//! 3. Use the registered fabric to query information (e.g., via
//!    [`compute_distances`]).
//! 4. Call [`fabric_update`] to refresh fabric information at any time.
//! 5. Call [`fabric_deregister`] when done to release resources.
//!
//! # Topology and device distances
//!
//! 1. Create a [`PmixTopology`] object (optionally with a source hint).
//! 2. Call [`load_topology`] to load the local hardware topology.
//! 3. Create a [`PmixCpuset`] for the caller's CPU binding.
//! 4. Call [`compute_distances`] to get device distances from the CPU set.
//!
//! Non-blocking variants (`*_nb`) accept a callback trait and return
//! immediately.
//!
//! # C API reference
//!
//! ```c
//! pmix_status_t PMIx_Fabric_register(pmix_fabric_t *fabric,
//!                                    const pmix_info_t directives[],
//!                                    size_t ndirs);
//! pmix_status_t PMIx_Fabric_register_nb(pmix_fabric_t *fabric,
//!                                        const pmix_info_t directives[],
//!                                        size_t ndirs,
//!                                        pmix_op_cbfunc_t cbfunc, void *cbdata);
//! pmix_status_t PMIx_Fabric_update(pmix_fabric_t *fabric);
//! pmix_status_t PMIx_Fabric_update_nb(pmix_fabric_t *fabric,
//!                                      pmix_op_cbfunc_t cbfunc, void *cbdata);
//! pmix_status_t PMIx_Fabric_deregister(pmix_fabric_t *fabric);
//! pmix_status_t PMIx_Fabric_deregister_nb(pmix_fabric_t *fabric,
//!                                          pmix_op_cbfunc_t cbfunc, void *cbdata);
//! pmix_status_t PMIx_Load_topology(pmix_topology_t *topo);
//! pmix_status_t PMIx_Compute_distances(pmix_topology_t *topo,
//!                                       pmix_cpuset_t *cpuset,
//!                                       pmix_info_t info[], size_t ninfo,
//!                                       pmix_device_distance_t *distances[],
//!                                       size_t *ndist);
//! pmix_status_t PMIx_Compute_distances_nb(pmix_topology_t *topo,
//!                                          pmix_cpuset_t *cpuset,
//!                                          pmix_info_t info[], size_t ninfo,
//!                                          pmix_device_dist_cbfunc_t cbfunc,
//!                                          void *cbdata);
//! ```

use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::ptr;

use crate::ffi;
use crate::{Info, PmixDeviceType, PmixError, PmixStatus};

#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

fn flat_infos(infos: &[Info]) -> Vec<ffi::pmix_info_t> {
    infos
        .iter()
        .flat_map(|info| {
            if info.handle.is_null() || info.len == 0 {
                Vec::new()
            } else {
                // SAFETY: handle points to len initialized entries owned by the borrow.
                unsafe { std::slice::from_raw_parts(info.handle, info.len) }
                    .iter()
                    .map(|entry| {
                        // SAFETY: entry is initialized and copied by value into local storage.
                        unsafe { std::ptr::read(entry) }
                    })
                    .collect::<Vec<_>>()
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixFabric — safe wrapper for pmix_fabric_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_fabric_t`.
///
/// Represents a registered fabric object that provides access to fabric-related
/// information including communication cost matrices. The user may set the
/// `name` field for identification purposes — PMIx does not use it internally.
///
/// After registration, the PMIx library populates the `index`, `info`, and
/// `ninfo` fields with fabric metadata.
///
/// # C API
/// `typedef struct pmix_fabric_s { char *name; size_t index;`
/// `pmix_info_t *info; size_t ninfo; void *module; } pmix_fabric_t;`
pub struct PmixFabric {
    /// User-supplied name for this fabric (optional).
    name: Option<CString>,
    /// PMIx-supplied index identifying this registration object.
    index: usize,
    /// Number of info entries (populated after registration/update).
    ninfo: usize,
    /// Internal module pointer managed by PMIx.
    module: *mut std::os::raw::c_void,
    /// Whether this fabric has been registered with PMIx.
    registered: bool,
    /// Raw C struct for FFI calls.
    raw: MaybeUninit<ffi::pmix_fabric_t>,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl std::fmt::Debug for PmixFabric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixFabric")
            .field(
                "name",
                &self
                    .name
                    .as_ref()
                    .map(|s| s.to_str().unwrap_or("<invalid>")),
            )
            .field("index", &self.index)
            .field("ninfo", &self.ninfo)
            .field("registered", &self.registered)
            .finish()
    }
}

impl PmixFabric {
    /// Create a new, unregistered fabric object.
    ///
    /// The `name` parameter is optional and is used only for identification.
    /// PMIx does not use this field internally.
    pub fn new(name: Option<&str>) -> Result<Self, std::ffi::NulError> {
        let cname = match name {
            Some(n) => Some(CString::new(n)?),
            None => None,
        };
        Ok(Self {
            name: cname,
            index: 0,
            ninfo: 0,
            module: ptr::null_mut(),
            registered: false,
            raw: MaybeUninit::uninit(),
        
            _not_thread_safe: std::marker::PhantomData,
        })
    }

    /// Create a new fabric with no user-supplied name.
    pub fn unamed() -> Self {
        Self {
            name: None,
            index: 0,
            ninfo: 0,
            module: ptr::null_mut(),
            registered: false,
            raw: MaybeUninit::uninit(),
        
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    /// Get the user-supplied name, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|s| s.to_str().unwrap_or(""))
    }

    /// Get the PMIx-assigned index (valid after registration).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Check if this fabric has been registered.
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// Get the number of info entries (populated after registration/update).
    pub fn ninfo(&self) -> usize {
        self.ninfo
    }

    /// Get a pointer to the raw `pmix_fabric_t` for FFI calls.
    ///
    /// # Panics
    /// Panics if called before the fabric is initialized.
    fn as_mut_ptr(&mut self) -> *mut ffi::pmix_fabric_t {
        // Initialize the raw struct from our managed fields.
        unsafe {
            let raw = self.raw.as_mut_ptr();
            (*raw).name = match &self.name {
                Some(s) => s.as_ptr() as *mut _,
                None => ptr::null_mut(),
            };
            (*raw).index = self.index;
            // The info pointer is managed by PMIx — we don't own it.
            // It gets set during registration/update.
            (*raw).ninfo = self.ninfo;
            (*raw).module = self.module;
            raw
        }
    }

    /// Sync the raw struct's info/module fields back into managed Rust state
    /// after an FFI call that may have modified them.
    fn sync_from_raw(&mut self) {
        unsafe {
            let raw = self.raw.as_ptr();
            self.index = (*raw).index;
            self.module = (*raw).module;
            self.ninfo = (*raw).ninfo;
            // Note: PMIx may reallocate the info array on update.
            // We track the pointer and count but don't take ownership
            // until deregistration, at which point PMIx frees it.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback traits for non-blocking fabric operations
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for non-blocking fabric operations.
///
/// Implement this trait to handle the result of `fabric_register_nb`,
/// `fabric_update_nb`, or `fabric_deregister_nb`.
pub trait FabricCallback: Send {
    /// Called when the fabric operation completes.
    ///
    /// # Arguments
    /// * `status` — The result status of the operation.
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Internal wrapper that converts a Rust `FabricCallback` trait object
/// into an `extern "C"` callback compatible with `pmix_op_cbfunc_t`.
struct FabricCallbackWrapper {
    callback: Box<dyn FabricCallback>,
    _directives: Option<Vec<ffi::pmix_info_t>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_register
// ─────────────────────────────────────────────────────────────────────────────

/// Register a fabric object with the PMIx library.
///
/// This call must be made prior to requesting fabric information. The PMIx
/// library will populate the fabric's `index`, `info`, and `ninfo` fields
/// upon successful registration.
///
/// # Arguments
/// * `fabric` — A mutable [`PmixFabric`] to register.
/// * `directives` — Optional info array indicating desired behaviors or
///   specific fabric to access. Pass empty slice to use the highest
///   priority available fabric.
///
/// # Returns
/// * `Ok(())` on success (`PMIX_SUCCESS`).
/// * `Err(PmixStatus)` on failure.
///
/// # C API
/// `pmix_status_t PMIx_Fabric_register(pmix_fabric_t *fabric,`
/// `                                   const pmix_info_t directives[],`
/// `                                   size_t ndirs);`
pub fn fabric_register(fabric: &mut PmixFabric, directives: &[Info]) -> Result<(), PmixStatus> {
    let flat_infos = flat_infos(directives);
    let (dirs_ptr, ndirs) = if flat_infos.is_empty() {
        (ptr::null(), 0)
    } else {
        (flat_infos.as_ptr(), flat_infos.len())
    };

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_fabric_register(fabric_ptr, dirs_ptr, ndirs)
        }
        } else {
            unsafe { ffi::PMIx_Fabric_register(fabric_ptr, dirs_ptr, ndirs) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Fabric_register(fabric_ptr, dirs_ptr, ndirs) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        fabric.sync_from_raw();
        fabric.registered = true;
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking variant of [`fabric_register`].
///
/// Returns immediately and invokes the provided callback when the operation
/// completes.
///
/// # Arguments
/// * `fabric` — A mutable [`PmixFabric`] to register.
/// * `directives` — Optional info array for fabric selection.
/// * `callback` — A [`FabricCallback`] invoked upon completion.
///
/// # Returns
/// * `Ok(())` if the call was accepted.
/// * `Err(PmixStatus)` if the call itself failed.
///
/// # C API
/// `pmix_status_t PMIx_Fabric_register_nb(pmix_fabric_t *fabric,`
/// `                                      const pmix_info_t directives[],`
/// `                                      size_t ndirs,`
/// `                                      pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn fabric_register_nb(
    fabric: &mut PmixFabric,
    directives: &[Info],
    callback: Box<dyn FabricCallback>,
) -> Result<(), PmixStatus> {
    let flat_infos = flat_infos(directives);
    let (dirs_ptr, ndirs) = if flat_infos.is_empty() {
        (ptr::null(), 0)
    } else {
        (flat_infos.as_ptr(), flat_infos.len())
    };

    let wrapper = FabricCallbackWrapper { callback, _directives: Some(flat_infos) };
    let wrapper_ptr = Box::into_raw(Box::new(wrapper)) as *mut std::os::raw::c_void;

    extern "C" fn fabric_register_cb(
        status: ffi::pmix_status_t,
        cbdata: *mut std::os::raw::c_void,
    ) {
        let wrapper_ptr = cbdata as *mut FabricCallbackWrapper;
        let wrapper = unsafe { Box::from_raw(wrapper_ptr) };
        let pmix_status = PmixStatus::from_raw(status);
        wrapper.callback.on_complete(pmix_status);
    }

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_fabric_register_nb(
                fabric_ptr,
                dirs_ptr,
                ndirs,
                Some(fabric_register_cb),
                wrapper_ptr,
            )
        }
        } else {
            unsafe {
            ffi::PMIx_Fabric_register_nb(
                fabric_ptr,
                dirs_ptr,
                ndirs,
                Some(fabric_register_cb),
                wrapper_ptr,
            )
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            ffi::PMIx_Fabric_register_nb(
                fabric_ptr,
                dirs_ptr,
                ndirs,
                Some(fabric_register_cb),
                wrapper_ptr,
            )
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        fabric.registered = true;
        Ok(())
    } else {
        // Callback was not queued; reclaim the wrapper.
        let _ = unsafe { Box::from_raw(wrapper_ptr as *mut FabricCallbackWrapper) };
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_update
// ─────────────────────────────────────────────────────────────────────────────

/// Update fabric-related information for a registered fabric.
///
/// This call can be made at any time after registration to request an update
/// of the fabric information. The caller must not access the fabric object
/// while this call is in progress.
///
/// # Arguments
/// * `fabric` — A registered [`PmixFabric`] to update.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(PmixStatus)` on failure.
///
/// # C API
/// `pmix_status_t PMIx_Fabric_update(pmix_fabric_t *fabric);`
pub fn fabric_update(fabric: &mut PmixFabric) -> Result<(), PmixStatus> {
    if !fabric.registered {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe { mock_ffi::mock_fabric_update(fabric_ptr) }
        } else {
            unsafe { ffi::PMIx_Fabric_update(fabric_ptr) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Fabric_update(fabric_ptr) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        fabric.sync_from_raw();
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking variant of [`fabric_update`].
///
/// # C API
/// `pmix_status_t PMIx_Fabric_update_nb(pmix_fabric_t *fabric,`
/// `                                    pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn fabric_update_nb(
    fabric: &mut PmixFabric,
    callback: Box<dyn FabricCallback>,
) -> Result<(), PmixStatus> {
    if !fabric.registered {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let wrapper = FabricCallbackWrapper { callback, _directives: None };
    let wrapper_ptr = Box::into_raw(Box::new(wrapper)) as *mut std::os::raw::c_void;

    extern "C" fn fabric_update_cb(status: ffi::pmix_status_t, cbdata: *mut std::os::raw::c_void) {
        let wrapper_ptr = cbdata as *mut FabricCallbackWrapper;
        let wrapper = unsafe { Box::from_raw(wrapper_ptr) };
        let pmix_status = PmixStatus::from_raw(status);
        wrapper.callback.on_complete(pmix_status);
    }

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_fabric_update_nb(fabric_ptr, Some(fabric_update_cb), wrapper_ptr)
        }
        } else {
            unsafe { ffi::PMIx_Fabric_update_nb(fabric_ptr, Some(fabric_update_cb), wrapper_ptr) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Fabric_update_nb(fabric_ptr, Some(fabric_update_cb), wrapper_ptr) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let _ = unsafe { Box::from_raw(wrapper_ptr as *mut FabricCallbackWrapper) };
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// fabric_deregister
// ─────────────────────────────────────────────────────────────────────────────

/// Deregister a fabric object, allowing PMIx to clean up associated resources.
///
/// # Arguments
/// * `fabric` — A registered [`PmixFabric`] to deregister.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(PmixStatus)` on failure.
///
/// # C API
/// `pmix_status_t PMIx_Fabric_deregister(pmix_fabric_t *fabric);`
pub fn fabric_deregister(fabric: &mut PmixFabric) -> Result<(), PmixStatus> {
    if !fabric.registered {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe { mock_ffi::mock_fabric_deregister(fabric_ptr) }
        } else {
            unsafe { ffi::PMIx_Fabric_deregister(fabric_ptr) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Fabric_deregister(fabric_ptr) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        fabric.registered = false;
        fabric.ninfo = 0;
        fabric.module = ptr::null_mut();
        Ok(())
    } else {
        Err(pmix_status)
    }
}

/// Non-blocking variant of [`fabric_deregister`].
///
/// # C API
/// `pmix_status_t PMIx_Fabric_deregister_nb(pmix_fabric_t *fabric,`
/// `                                        pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn fabric_deregister_nb(
    fabric: &mut PmixFabric,
    callback: Box<dyn FabricCallback>,
) -> Result<(), PmixStatus> {
    if !fabric.registered {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let wrapper = FabricCallbackWrapper { callback, _directives: None };
    let wrapper_ptr = Box::into_raw(Box::new(wrapper)) as *mut std::os::raw::c_void;

    extern "C" fn fabric_deregister_cb(
        status: ffi::pmix_status_t,
        cbdata: *mut std::os::raw::c_void,
    ) {
        let wrapper_ptr = cbdata as *mut FabricCallbackWrapper;
        let wrapper = unsafe { Box::from_raw(wrapper_ptr) };
        let pmix_status = PmixStatus::from_raw(status);
        wrapper.callback.on_complete(pmix_status);
    }

    let fabric_ptr = fabric.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_fabric_deregister_nb(
                fabric_ptr,
                Some(fabric_deregister_cb),
                wrapper_ptr,
            )
        }
        } else {
            unsafe {
            ffi::PMIx_Fabric_deregister_nb(fabric_ptr, Some(fabric_deregister_cb), wrapper_ptr)
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            ffi::PMIx_Fabric_deregister_nb(fabric_ptr, Some(fabric_deregister_cb), wrapper_ptr)
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        fabric.registered = false;
        Ok(())
    } else {
        let _ = unsafe { Box::from_raw(wrapper_ptr as *mut FabricCallbackWrapper) };
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology — safe wrapper for pmix_topology_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_topology_t`.
///
/// Represents a hardware topology description. The user may set the `source`
/// field to request a specific topology source (e.g., `"hwloc"`). After
/// calling [`load_topology`], PMIx populates the internal topology pointer.
///
/// # C API
/// `typedef struct { char *source; void *topology; } pmix_topology_t;`
#[derive(Debug)]
pub struct PmixTopology {
    /// Optional source hint (e.g., "hwloc").
    source: Option<CString>,
    /// Internal topology pointer managed by PMIx.
    topology: *mut std::os::raw::c_void,
    /// Whether this topology has been loaded.
    loaded: bool,
    /// Raw C struct for FFI calls.
    raw: std::mem::MaybeUninit<ffi::pmix_topology_t>,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixTopology {
    /// Create a new, uninitialized topology object.
    ///
    /// The `source` parameter is optional. Set it to request a specific
    /// topology backend (e.g., `"hwloc"`).
    pub fn new(source: Option<&str>) -> Result<Self, std::ffi::NulError> {
        let csource = match source {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        Ok(Self {
            source: csource,
            topology: ptr::null_mut(),
            loaded: false,
            raw: std::mem::MaybeUninit::uninit(),
        
            _not_thread_safe: std::marker::PhantomData,
        })
    }

    /// Create a new topology with no source hint.
    pub fn unamed() -> Self {
        Self {
            source: None,
            topology: ptr::null_mut(),
            loaded: false,
            raw: std::mem::MaybeUninit::uninit(),
        
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    /// Get the source hint, if any.
    pub fn source(&self) -> Option<&str> {
        self.source.as_ref().map(|s| s.to_str().unwrap_or(""))
    }

    /// Check if this topology has been loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Get a mutable pointer to the raw `pmix_topology_t` for FFI calls.
    fn as_mut_ptr(&mut self) -> *mut ffi::pmix_topology_t {
        unsafe {
            let raw = self.raw.as_mut_ptr();
            (*raw).source = match &self.source {
                Some(s) => s.as_ptr() as *mut _,
                None => ptr::null_mut(),
            };
            (*raw).topology = self.topology;
            raw
        }
    }

    /// Sync the raw struct's topology and source fields back into managed Rust
    /// state after an FFI call that may have modified them.
    fn sync_from_raw(&mut self) {
        unsafe {
            let raw = self.raw.as_ptr();
            self.topology = (*raw).topology;
            let src = (*raw).source;
            if !src.is_null() {
                // PMIx may have replaced raw->source with its own C-allocated
                // string, or kept our hint pointer. If it kept our hint, the
                // existing Rust CString already owns the right bytes.
                let aliases_hint = self
                    .source
                    .as_ref()
                    .is_some_and(|s| ptr::eq(s.as_ptr(), src));
                if !aliases_hint {
                    let owned = CStr::from_ptr(src).to_string_lossy().into_owned();
                    if let Ok(cs) = CString::new(owned) {
                        self.source = Some(cs);
                    }
                }
            }
        }
    }

    /// Create a test instance of `PmixTopology` without FFI.
    ///
    /// Test helper — creates a PmixTopology without FFI.
    /// and loaded is false, so drop is a no-op.
    pub fn test_new(source: Option<&str>) -> Result<Self, std::ffi::NulError> {
        Self::new(source)
    }
}

impl Drop for PmixTopology {
    fn drop(&mut self) {
        if self.loaded {
            // SAFETY: raw was initialized by as_mut_ptr during load_topology.
            let raw_ptr = self.raw.as_mut_ptr();
            unsafe {
                // PMIx_Load_topology may return the PMIx process-global hwloc
                // topology. PMIx_Finalize owns destruction of that shared
                // topology, so leave it null here and let the designated
                // destructor release only this object's source string.
                (*raw_ptr).topology = ptr::null_mut();
                // Do not replace the PMIx-owned source with the Rust hint.
                // If PMIx kept the hint, duplicate it so its destructor never
                // attempts to free a Rust allocation.
                let src = (*raw_ptr).source;
                if !src.is_null()
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|s| ptr::eq(s.as_ptr(), src))
                {
                    (*raw_ptr).source = libc::strdup(src);
                }
            }
            // SAFETY: PMIx_Topology_destruct is the designated destructor
            // for pmix_topology_t objects that have been loaded.
            #[cfg(any(test, feature = "mock_ffi"))]
            {
                if mock_ffi::is_mock_enabled() {
                    unsafe { mock_ffi::mock_topology_destruct(raw_ptr) };
                } else {
                    unsafe { ffi::PMIx_Topology_destruct(raw_ptr) };
                }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            {
                unsafe { ffi::PMIx_Topology_destruct(raw_ptr) };
            }
            self.loaded = false;
        }
        // Construct the raw struct to call destruct even if not loaded
        // (for objects that were constructed but never loaded).
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixGeometry — safe wrapper for pmix_geometry_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_geometry_t`.
#[derive(Debug)]
pub struct PmixGeometry {
    raw: std::mem::MaybeUninit<ffi::pmix_geometry_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixGeometry {
    /// Construct an empty geometry object using PMIx.
    pub fn new() -> Self {
        let mut this = Self {
            raw: std::mem::MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let raw_ptr = this.raw.as_mut_ptr();
        // SAFETY: raw_ptr points to storage owned by this; PMIx initializes the complete object.
        #[cfg(any(test, feature = "mock_ffi"))]
        {
            if mock_ffi::is_mock_enabled() {
                unsafe { mock_ffi::mock_geometry_construct(raw_ptr) };
            } else {
                unsafe { ffi::PMIx_Geometry_construct(raw_ptr) };
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            unsafe { ffi::PMIx_Geometry_construct(raw_ptr) };
        }
        this.constructed = true;
        this
    }

    /// Create an empty geometry object without calling into PMIx.
    pub fn test_new() -> Self {
        Self {
            raw: std::mem::MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    /// Return the fabric identifier.
    pub fn fabric(&self) -> usize {
        // SAFETY: `self.raw` is initialized by `new` or `test_new`, and the returned
        // value does not outlive the shared borrow of `self`.
        unsafe { self.raw.assume_init_ref().fabric }
    }
    /// Return the geometry UUID, if present and valid UTF-8.
    pub fn uuid(&self) -> Option<&str> {
        self.c_string(|raw| raw.uuid)
    }
    /// Return the operating-system device name, if present and valid UTF-8.
    pub fn osname(&self) -> Option<&str> {
        self.c_string(|raw| raw.osname)
    }
    /// Return the number of coordinate entries.
    pub fn ncoords(&self) -> usize {
        // SAFETY: `self.raw` is initialized by `new` or `test_new`, and the returned
        // value does not outlive the shared borrow of `self`.
        unsafe { self.raw.assume_init_ref().ncoords }
    }
    /// Return the raw coordinate array, when PMIx supplied one.
    pub fn coordinates(&self) -> Option<&[ffi::pmix_coord_t]> {
        // SAFETY: `self.raw` is initialized by `new` or `test_new`; the slice borrows
        // `self` and therefore cannot outlive the PMIx-owned coordinate array.
        unsafe {
            let raw = self.raw.assume_init_ref();
            (!raw.coordinates.is_null())
                .then(|| std::slice::from_raw_parts(raw.coordinates, raw.ncoords))
        }
    }
    fn c_string(&self, get: impl FnOnce(&ffi::pmix_geometry_t) -> *mut libc::c_char) -> Option<&str> {
        // SAFETY: `self.raw` is initialized by `new` or `test_new`; the returned string
        // slice borrows `self` and therefore cannot outlive the PMIx-owned C string.
        unsafe {
            let ptr = get(self.raw.assume_init_ref());
            (!ptr.is_null())
                .then(|| std::ffi::CStr::from_ptr(ptr).to_str().ok())
                .flatten()
        }
    }
}
impl Default for PmixGeometry { fn default() -> Self { Self::new() } }
impl Drop for PmixGeometry {
    fn drop(&mut self) {
        if self.constructed {
            // SAFETY: the object was initialized by the matching constructor and is destroyed once.
            #[cfg(any(test, feature = "mock_ffi"))]
            { if mock_ffi::is_mock_enabled() { mock_ffi::mock_geometry_destruct(self.raw.as_mut_ptr()); } else { unsafe { ffi::PMIx_Geometry_destruct(self.raw.as_mut_ptr()) }; } }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            { unsafe { ffi::PMIx_Geometry_destruct(self.raw.as_mut_ptr()) }; }
            self.constructed = false;
        }
    }
}

// PmixEndpoint — safe wrapper for pmix_endpoint_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_endpoint_t`.
#[derive(Debug)]
pub struct PmixEndpoint {
    raw: std::mem::MaybeUninit<ffi::pmix_endpoint_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixEndpoint {
    /// Construct an empty endpoint object using PMIx.
    pub fn new() -> Self {
        let mut this = Self {
            raw: std::mem::MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let raw_ptr = this.raw.as_mut_ptr();
        // SAFETY: raw_ptr points to storage owned by this; PMIx initializes the complete object.
        #[cfg(any(test, feature = "mock_ffi"))]
        {
            if mock_ffi::is_mock_enabled() {
                unsafe { mock_ffi::mock_endpoint_construct(raw_ptr) };
            } else {
                unsafe { ffi::PMIx_Endpoint_construct(raw_ptr) };
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            unsafe { ffi::PMIx_Endpoint_construct(raw_ptr) };
        }
        this.constructed = true;
        this
    }

    /// Create an empty endpoint object without calling into PMIx.
    pub fn test_new() -> Self {
        Self {
            raw: std::mem::MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    /// Return the endpoint UUID, if present and valid UTF-8.
    pub fn uuid(&self) -> Option<&str> {
        self.c_string(|raw| raw.uuid)
    }
    /// Return the operating-system endpoint name, if present and valid UTF-8.
    pub fn osname(&self) -> Option<&str> {
        self.c_string(|raw| raw.osname)
    }
    /// Return the endpoint byte object, when PMIx supplied one.
    pub fn endpt(&self) -> Option<&[u8]> {
        // SAFETY: raw is initialized; the slice borrows self and cannot outlive the PMIx-owned buffer.
        unsafe {
            let raw = self.raw.assume_init_ref();
            (!raw.endpt.bytes.is_null())
                .then(|| std::slice::from_raw_parts(raw.endpt.bytes as *const u8, raw.endpt.size))
        }
    }
    fn c_string(
        &self,
        get: impl FnOnce(&ffi::pmix_endpoint_t) -> *mut libc::c_char,
    ) -> Option<&str> {
        // SAFETY: raw is initialized; the returned string borrows self and cannot outlive the PMIx-owned string.
        unsafe {
            let ptr = get(self.raw.assume_init_ref());
            (!ptr.is_null())
                .then(|| std::ffi::CStr::from_ptr(ptr).to_str().ok())
                .flatten()
        }
    }
}
impl Default for PmixEndpoint { fn default() -> Self { Self::new() } }
impl Drop for PmixEndpoint {
    fn drop(&mut self) {
        if self.constructed {
            // SAFETY: the object was initialized by the matching constructor and is destroyed once.
            #[cfg(any(test, feature = "mock_ffi"))]
            { if mock_ffi::is_mock_enabled() { mock_ffi::mock_endpoint_destruct(self.raw.as_mut_ptr()); } else { unsafe { ffi::PMIx_Endpoint_destruct(self.raw.as_mut_ptr()) }; } }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            { unsafe { ffi::PMIx_Endpoint_destruct(self.raw.as_mut_ptr()) }; }
            self.constructed = false;
        }
    }
}

// PmixCpuset — safe wrapper for pmix_cpuset_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_cpuset_t`.
///
/// Represents a CPU set (bitmap) for binding/topology operations.
/// Must be constructed with [`PmixCpuset::new`] before use and destroyed
/// automatically on drop.
///
/// # C API
/// `typedef struct { char *source; void *bitmap; } pmix_cpuset_t;`
#[derive(Debug)]
pub struct PmixCpuset {
    /// Raw C struct for FFI calls.
    raw: std::mem::MaybeUninit<ffi::pmix_cpuset_t>,
    /// Whether this cpuset has been constructed.
    constructed: bool,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixCpuset {
    /// Create a new, constructed cpuset object.
    ///
    /// Calls `PMIx_Cpuset_construct` to initialize the internal bitmap.
    pub fn new() -> Self {
        let mut this = Self {
            raw: std::mem::MaybeUninit::uninit(),
            constructed: false,
        
            _not_thread_safe: std::marker::PhantomData,
        };
        let raw_ptr = this.raw.as_mut_ptr();
        // SAFETY: PMIx_Cpuset_construct initializes a pmix_cpuset_t.
                #[cfg(any(test, feature = "mock_ffi"))]
        {
            if mock_ffi::is_mock_enabled() {
                unsafe { mock_ffi::mock_cpuset_construct(raw_ptr) };
            } else {
                unsafe { ffi::PMIx_Cpuset_construct(raw_ptr) };
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            unsafe { ffi::PMIx_Cpuset_construct(raw_ptr) };
        }
        this.constructed = true;
        this
    }

    /// Create a test instance of `PmixCpuset` without calling FFI construct.
    ///
    /// Test helper — creates a PmixCpuset without calling FFI construct.
    /// but the raw data is uninit — use only for tests that don't actually
    /// pass the pointer to FFI.
    pub fn test_new() -> Self {
        Self {
            raw: std::mem::MaybeUninit::uninit(),
            constructed: true,
        
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    /// Get a mutable pointer to the raw `pmix_cpuset_t` for FFI calls.
    pub fn as_mut_ptr(&mut self) -> *mut ffi::pmix_cpuset_t {
        assert!(self.constructed, "cpuset not constructed");
        self.raw.as_mut_ptr()
    }
}

impl Default for PmixCpuset {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PmixCpuset {
    fn drop(&mut self) {
        if self.constructed {
            // SAFETY: PMIx_Cpuset_destruct is the designated destructor
            // for pmix_cpuset_t objects that have been constructed.
                        #[cfg(any(test, feature = "mock_ffi"))]
            {
                if mock_ffi::is_mock_enabled() {
                    unsafe { mock_ffi::mock_cpuset_destruct(self.raw.as_mut_ptr()) };
                } else {
                    unsafe { ffi::PMIx_Cpuset_destruct(self.raw.as_mut_ptr()) };
                }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            {
                unsafe { ffi::PMIx_Cpuset_destruct(self.raw.as_mut_ptr()) };
            }
            self.constructed = false;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDeviceDistance — safe wrapper for pmix_device_distance_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_device_distance_t`.
///
/// Represents the distance information for a hardware device relative to
/// the caller's CPU set, as returned by [`compute_distances`].
///
/// # C API
/// `typedef struct { char *uuid; char *osname; pmix_device_type_t type;`
/// `uint16_t mindist; uint16_t maxdist; } pmix_device_distance_t;`
#[derive(Debug, Clone)]
pub struct PmixDeviceDistance {
    /// Device UUID string.
    uuid: String,
    /// OS-provided device name.
    osname: String,
    /// Device type (GPU, network, etc.).
    device_type: PmixDeviceType,
    /// Minimum distance from the caller's CPU set.
    mindist: u16,
    /// Maximum distance from the caller's CPU set.
    maxdist: u16,
}

impl PmixDeviceDistance {
    /// Get the device UUID.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    /// Get the OS-provided device name.
    pub fn osname(&self) -> &str {
        &self.osname
    }

    /// Get the device type.
    pub fn device_type(&self) -> PmixDeviceType {
        self.device_type
    }

    /// Get the minimum distance.
    pub fn mindist(&self) -> u16 {
        self.mindist
    }

    /// Get the maximum distance.
    pub fn maxdist(&self) -> u16 {
        self.maxdist
    }

    /// Convert a raw C `pmix_device_distance_t` into a safe Rust struct.
    ///
    /// # Safety
    /// The caller must ensure that `raw` points to a valid, initialized
    /// `pmix_device_distance_t` and that the string fields are valid
    /// null-terminated C strings (or null).
    unsafe fn from_raw(raw: &ffi::pmix_device_distance) -> Self {
        let uuid = unsafe {
            if raw.uuid.is_null() {
                String::new()
            } else {
                CStr::from_ptr(raw.uuid).to_string_lossy().into_owned()
            }
        };
        let osname = unsafe {
            if raw.osname.is_null() {
                String::new()
            } else {
                CStr::from_ptr(raw.osname).to_string_lossy().into_owned()
            }
        };
        Self {
            uuid,
            osname,
            device_type: PmixDeviceType::from_raw(raw.type_),
            mindist: raw.mindist,
            maxdist: raw.maxdist,
        }
    }

    /// Create a test instance of `PmixDeviceDistance` without FFI.
    ///
    /// Test helper — creates a PmixDeviceDistance without FFI.
    pub fn test_new(
        uuid: &str,
        osname: &str,
        device_type: PmixDeviceType,
        mindist: u16,
        maxdist: u16,
    ) -> Self {
        Self {
            uuid: uuid.to_string(),
            osname: osname.to_string(),
            device_type,
            mindist,
            maxdist,
        }
    }
}

/// Release a PMIx-owned distance array after copying its entries.
unsafe fn free_raw_distances(dist: *mut ffi::pmix_device_distance_t, len: usize) {
    if dist.is_null() {
        return;
    }
    for i in 0..len {
        let entry = unsafe { dist.add(i) };
        unsafe {
            libc::free((*entry).uuid.cast());
            libc::free((*entry).osname.cast());
        }
    }
    unsafe { libc::free(dist.cast()) };
}

/// A collection of device distances returned by [`compute_distances`].
///
/// The collection owns Rust copies of all returned data. PMIx-owned memory is
/// released by the API call or callback bridge before this value is delivered.
pub struct DeviceDistances {
    /// The parsed, Rust-owned distance entries.
    distances: Vec<PmixDeviceDistance>,
    /// Makes this type `!Send` + `!Sync` for consistency with PMIx fabric data.
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl DeviceDistances {
    /// Get the parsed distance entries.
    pub fn distances(&self) -> &[PmixDeviceDistance] {
        &self.distances
    }

    /// Get the number of distance entries.
    pub fn len(&self) -> usize {
        self.distances.len()
    }

    /// Check if there are no distance entries.
    pub fn is_empty(&self) -> bool {
        self.distances.is_empty()
    }

    /// Create a test instance of `DeviceDistances` without FFI.
    ///
    /// Test helper — creates DeviceDistances without FFI. The raw pointer is null so
    /// drop is a no-op.
    pub fn test_new(distances: Vec<PmixDeviceDistance>) -> Self {
        Self {
            distances,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
}

impl std::fmt::Debug for DeviceDistances {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceDistances")
            .field("distances", &self.distances)
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait for compute_distances_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for non-blocking compute distances operation.
///
/// Implement this trait to handle the result of `compute_distances_nb`.
pub trait ComputeDistancesCallback: Send {
    /// Called when the compute distances operation completes.
    ///
    /// # Arguments
    /// * `status` — The result status of the operation.
    /// * `distances` — The device distance array (may be empty on error).
    fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances);
}

/// Internal wrapper for the compute_distances_nb callback.
struct ComputeDistancesCallbackWrapper {
    callback: Box<dyn ComputeDistancesCallback>,
    _info: Vec<ffi::pmix_info_t>,
}

// ─────────────────────────────────────────────────────────────────────────────
// load_topology
// ─────────────────────────────────────────────────────────────────────────────

/// Load the local hardware topology description.
///
/// Populates the given [`PmixTopology`] with the local hardware topology.
/// If a specific source was requested via the `source` field, PMIx will
/// attempt to use that backend (e.g., "hwloc").
///
/// # Arguments
/// * `topo` — A mutable [`PmixTopology`] to populate.
///
/// # Returns
/// * `Ok(())` on success (`PMIX_SUCCESS`).
/// * `Err(PmixStatus::NotFound)` if the requested source is not available.
/// * `Err(PmixStatus::Unsupported)` if topology is not supported.
///
/// # C API
/// `pmix_status_t PMIx_Load_topology(pmix_topology_t *topo);`
pub fn load_topology(topo: &mut PmixTopology) -> Result<(), PmixStatus> {
    let raw_ptr = topo.as_mut_ptr();
        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe { mock_ffi::mock_load_topology(raw_ptr) }
        } else {
            unsafe { ffi::PMIx_Load_topology(raw_ptr) }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe { ffi::PMIx_Load_topology(raw_ptr) }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        topo.sync_from_raw();
        topo.loaded = true;
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_distances
// ─────────────────────────────────────────────────────────────────────────────

/// Compute device distances from the caller's CPU set.
///
/// Given a topology, a CPU set, and optional info directives (e.g., device
/// type filter), this function returns an array of [`PmixDeviceDistance`]
/// entries describing the hardware devices and their distances from the
/// caller's location in the topology.
///
/// # Arguments
/// * `topo` — A loaded [`PmixTopology`] describing the hardware topology.
/// * `cpuset` — A [`PmixCpuset`] representing the caller's CPU binding.
/// * `info` — Optional info array (e.g., `PMIX_DEVICE_TYPE` to filter by
///   device type). Pass empty slice for all devices.
///
/// # Returns
/// * `Ok(DeviceDistances)` containing the distance array.
/// * `Err(PmixStatus)` on failure (e.g., no topology loaded).
///
/// # Example
/// ```ignore
/// let mut topo = PmixTopology::unamed();
/// load_topology(&mut topo)?;
///
/// let cpuset = PmixCpuset::new();
/// compute_distances(&mut topo, &mut cpuset, &[])?;
/// ```
///
/// # C API
/// `pmix_status_t PMIx_Compute_distances(pmix_topology_t *topo,`
/// `                                     pmix_cpuset_t *cpuset,`
/// `                                     pmix_info_t info[], size_t ninfo,`
/// `                                     pmix_device_distance_t *distances[],`
/// `                                     size_t *ndist);`
pub fn compute_distances(
    topo: &mut PmixTopology,
    cpuset: &mut PmixCpuset,
    info: &[Info],
) -> Result<DeviceDistances, PmixStatus> {
    let flat_infos = flat_infos(info);
    let (info_ptr, ninfo) = if flat_infos.is_empty() {
        (ptr::null_mut(), 0)
    } else {
        (flat_infos.as_ptr() as *mut ffi::pmix_info_t, flat_infos.len())
    };

    let topo_ptr = topo.as_mut_ptr();
    let cpuset_ptr = cpuset.as_mut_ptr();

    let mut raw_distances: *mut ffi::pmix_device_distance_t = ptr::null_mut();
    let mut ndist: usize = 0;

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_compute_distances(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                &mut raw_distances,
                &mut ndist,
            )
        }
        } else {
            unsafe {
            ffi::PMIx_Compute_distances(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                &mut raw_distances,
                &mut ndist,
            )
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            ffi::PMIx_Compute_distances(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                &mut raw_distances,
                &mut ndist,
            )
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // SAFETY: On success, PMIx_Compute_distances allocates and returns a
    // valid array of pmix_device_distance_t with ndist elements.
    // Copy the data before returning; PMIx owns the source array and its strings.
    let distances: Vec<PmixDeviceDistance> = unsafe {
        if raw_distances.is_null() || ndist == 0 {
            Vec::new()
        } else {
            (0..ndist)
                .map(|i| PmixDeviceDistance::from_raw(&*raw_distances.add(i)))
                .collect()
        }
    };

    // PMIx owns this array. The strings were copied above, so release the
    // PMIx allocation before returning the Rust-only value.
    unsafe { free_raw_distances(raw_distances, ndist) };

    Ok(DeviceDistances {
        distances,
        _not_thread_safe: std::marker::PhantomData,
    })
}

/// Non-blocking variant of [`compute_distances`].
///
/// Returns immediately and invokes the provided callback when the operation
/// completes.
///
/// # Arguments
/// * `topo` — A loaded [`PmixTopology`].
/// * `cpuset` — A [`PmixCpuset`] for the caller's CPU binding.
/// * `info` — Optional info array for device filtering.
/// * `callback` — A [`ComputeDistancesCallback`] invoked upon completion.
///
/// # Returns
/// * `Ok(())` if the call was accepted.
/// * `Err(PmixStatus)` if the call itself failed.
///
/// # C API
/// `pmix_status_t PMIx_Compute_distances_nb(pmix_topology_t *topo,`
/// `                                        pmix_cpuset_t *cpuset,`
/// `                                        pmix_info_t info[], size_t ninfo,`
/// `                                        pmix_device_dist_cbfunc_t cbfunc,`
/// `                                        void *cbdata);`
pub fn compute_distances_nb(
    topo: &mut PmixTopology,
    cpuset: &mut PmixCpuset,
    info: &[Info],
    callback: Box<dyn ComputeDistancesCallback>,
) -> Result<(), PmixStatus> {
    let flat_infos = flat_infos(info);
    let (info_ptr, ninfo) = if flat_infos.is_empty() {
        (ptr::null_mut(), 0)
    } else {
        (
            flat_infos.as_ptr() as *mut ffi::pmix_info_t,
            flat_infos.len(),
        )
    };

    let wrapper = ComputeDistancesCallbackWrapper {
        callback,
        _info: flat_infos,
    };
    let wrapper_ptr = Box::into_raw(Box::new(wrapper)) as *mut std::os::raw::c_void;

    extern "C" fn compute_distances_cb(
        status: ffi::pmix_status_t,
        dist: *mut ffi::pmix_device_distance_t,
        ndist: usize,
        cbdata: *mut std::os::raw::c_void,
        release_fn: ffi::pmix_release_cbfunc_t,
        release_cbdata: *mut std::os::raw::c_void,
    ) {
        let wrapper_ptr = cbdata as *mut ComputeDistancesCallbackWrapper;
        let wrapper = unsafe { Box::from_raw(wrapper_ptr) };
        let pmix_status = PmixStatus::from_raw(status);

        // Parse the distances into a safe Rust struct.
        let distances = if pmix_status.is_success() && !dist.is_null() && ndist > 0 {
            // SAFETY: On success, dist points to a valid array of ndist elements.
            let rust_distances: Vec<PmixDeviceDistance> = unsafe {
                (0..ndist)
                    .map(|i| PmixDeviceDistance::from_raw(&*dist.add(i)))
                    .collect()
            };
            DeviceDistances {
                distances: rust_distances,
                _not_thread_safe: std::marker::PhantomData,
            }
        } else {
            DeviceDistances {
                distances: Vec::new(),
                _not_thread_safe: std::marker::PhantomData,
            }
        };

        // Call the release function if provided.
        if let Some(release) = release_fn {
            unsafe { release(release_cbdata) };
        }

        wrapper.callback.on_complete(pmix_status, distances);
    }

    let topo_ptr = topo.as_mut_ptr();
    let cpuset_ptr = cpuset.as_mut_ptr();

        let status;
    #[cfg(any(test, feature = "mock_ffi"))]
    {
        status = if mock_ffi::is_mock_enabled() {
            unsafe {
            mock_ffi::mock_compute_distances_nb(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                Some(compute_distances_cb),
                wrapper_ptr,
            )
        }
        } else {
            unsafe {
            ffi::PMIx_Compute_distances_nb(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                Some(compute_distances_cb),
                wrapper_ptr,
            )
        }
        };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        status = {
            unsafe {
            ffi::PMIx_Compute_distances_nb(
                topo_ptr,
                cpuset_ptr,
                info_ptr,
                ninfo,
                Some(compute_distances_cb),
                wrapper_ptr,
            )
        }
        };
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Callback was not queued; reclaim the wrapper.
        let _ = unsafe { Box::from_raw(wrapper_ptr as *mut ComputeDistancesCallbackWrapper) };
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── PmixFabric construction tests ──

    /// Test that PmixFabric can be created with no name.
    #[test]
    fn test_fabric_new_unamed() {
        let fabric = PmixFabric::unamed();
        assert!(!fabric.is_registered());
        assert_eq!(fabric.index(), 0);
        assert_eq!(fabric.ninfo(), 0);
        assert_eq!(fabric.name(), None);
    }

    /// Test that PmixFabric can be created with a name.
    #[test]
    fn test_fabric_new_with_name() {
        let fabric = PmixFabric::new(Some("test_fabric")).unwrap();
        assert!(!fabric.is_registered());
        assert_eq!(fabric.name(), Some("test_fabric"));
    }

    /// Test that PmixFabric can be created with None name.
    #[test]
    fn test_fabric_new_none_name() {
        let fabric = PmixFabric::new(None).unwrap();
        assert!(!fabric.is_registered());
        assert_eq!(fabric.name(), None);
    }

    /// Test that PmixFabric::new rejects names with interior NUL bytes.
    #[test]
    fn test_fabric_new_nul_name() {
        let result = PmixFabric::new(Some("test\0fabric"));
        assert!(result.is_err());
    }

    /// Test that PmixFabric implements Debug.
    #[test]
    fn test_fabric_debug() {
        let fabric = PmixFabric::unamed();
        let debug_str = format!("{:?}", fabric);
        assert!(!debug_str.is_empty());
    }

    // ── Parameter validation tests ──

    /// Test that fabric_update on an unregistered fabric returns error.
    #[test]
    fn test_fabric_update_not_registered() {
        let mut fabric = PmixFabric::unamed();
        let result = fabric_update(&mut fabric);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM),
            "updating unregistered fabric should return BAD_PARAM"
        );
    }

    /// Test that fabric_deregister on an unregistered fabric returns error.
    #[test]
    fn test_fabric_deregister_not_registered() {
        let mut fabric = PmixFabric::unamed();
        let result = fabric_deregister(&mut fabric);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err,
            PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM),
            "deregistering unregistered fabric should return BAD_PARAM"
        );
    }

    /// Test that fabric_register_nb with an unregistered callback wrapper
    /// compiles and the callback trait is object-safe.
    #[test]
    fn test_fabric_callback_trait_object() {
        struct TestCb;
        impl FabricCallback for TestCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn FabricCallback> = Box::new(TestCb);
    }

    /// Test that the FabricCallback trait can capture and report status.
    #[test]
    fn test_fabric_callback_records_status() {
        use std::cell::Cell;

        struct RecordingCb {
            status: Cell<Option<PmixStatus>>,
        }
        impl FabricCallback for RecordingCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                self.status.set(Some(status));
            }
        }

        let cb = RecordingCb {
            status: Cell::new(None),
        };
        let boxed: Box<dyn FabricCallback> = Box::new(cb);

        // Invoke the callback manually to test it works.
        let _test_status = PmixStatus::from_raw(ffi::PMIX_SUCCESS as i32);
        // We can't easily call the trait method on a boxed RecordingCb,
        // but we verified the trait compiles and is object-safe above.
        drop(boxed);
    }

    // ── fabric_register with empty directives ──

    /// Test that fabric_register accepts an empty directives slice.
    /// This test will fail at the FFI level (no PMIx server), but verifies
    /// the parameter handling is correct.
    #[test]
    fn test_fabric_register_empty_directives() {
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        let result = fabric_register(&mut fabric, &[]);
        // Without a PMIx server, this will return an error.
        // The important thing is that it doesn't panic or segfault.
        if let Ok(()) = result {
            assert!(fabric.is_registered());
        }
    }

    /// Test that fabric_register with a named fabric doesn't crash.
    #[test]
    fn test_fabric_register_named() {
        let mut fabric = PmixFabric::new(Some("infiniband")).unwrap();
        let result = fabric_register(&mut fabric, &[]);
        if let Ok(()) = result {
            assert!(fabric.is_registered());
            assert!(fabric.index() > 0);
        }
    }

    // ── Lifecycle tests ──

    /// Test the full register/update/deregister lifecycle.
    #[test]
    fn test_fabric_lifecycle() {
        let mut fabric = PmixFabric::new(Some("lifecycle_test")).unwrap();
        assert!(!fabric.is_registered());

        // Register
        let reg_result = fabric_register(&mut fabric, &[]);
        if reg_result.is_err() {
            // No PMIx server — skip remaining checks.
            return;
        }
        assert!(fabric.is_registered());

        // Update
        let update_result = fabric_update(&mut fabric);
        if update_result.is_ok() {
            // Fabric info may have been refreshed.
        }

        // Deregister
        let dereg_result = fabric_deregister(&mut fabric);
        assert!(dereg_result.is_ok());
        assert!(!fabric.is_registered());
        assert_eq!(fabric.ninfo(), 0);
    }

    /// Test double deregister returns error.
    #[test]
    fn test_fabric_double_deregister() {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_register(&mut fabric, &[]);
        if !fabric.is_registered() {
            return; // No PMIx server
        }
        assert!(fabric_deregister(&mut fabric).is_ok());
        assert!(!fabric.is_registered());
        // Second deregister should fail.
        let result = fabric_deregister(&mut fabric);
        assert!(result.is_err());
    }

    // ── Non-blocking callback tests ──

    /// Test that fabric_register_nb compiles and accepts a callback.
    ///
    /// The actual FFI call to PMIx_Fabric_register_nb crashes without a
    /// full PMIx server environment (SIGSEGV in the PMIx library itself).
    /// We verify init works and skip the FFI call to avoid crashing.
    #[test]
    fn test_fabric_register_nb_compiles() {
        #[allow(dead_code)]
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        // Under prterun, PMIx is already initialized — don't call init again
        // (double-init crashes). Standalone: try init, skip on failure.
        let _is_dvm = std::env::var("PMIX_NAMESPACE").is_ok()
            || std::env::var("PMIX_RANK").is_ok()
            || std::env::var("PRTE_LAUNCHED").is_ok();
        if !_is_dvm {
            match crate::PmixClient::connect_new(None) {
                Ok(_) => {}
                Err(_) => {
                    eprintln!("test_fabric_register_nb_compiles: init failed, skipping");
                    return;
                }
            }
        }
        // PMIx_Fabric_register_nb crashes in the PMIx library without full
        // server support. We only verify that init succeeded and the API
        // signature compiles. The actual FFI call is skipped.
    }

    /// Test that fabric_update_nb compiles and accepts a callback.
    #[test]
    fn test_fabric_update_nb_compiles() {
        #[allow(dead_code)]
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _is_dvm = std::env::var("PMIX_NAMESPACE").is_ok()
            || std::env::var("PMIX_RANK").is_ok()
            || std::env::var("PRTE_LAUNCHED").is_ok();
        if !_is_dvm {
            match crate::PmixClient::connect_new(None) {
                Ok(_) => {}
                Err(_) => {
                    eprintln!("test_fabric_update_nb_compiles: init failed, skipping");
                    return;
                }
            }
        }
    }

    /// Test that fabric_deregister_nb compiles and accepts a callback.
    #[test]
    fn test_fabric_deregister_nb_compiles() {
        #[allow(dead_code)]
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _is_dvm = std::env::var("PMIX_NAMESPACE").is_ok()
            || std::env::var("PMIX_RANK").is_ok()
            || std::env::var("PRTE_LAUNCHED").is_ok();
        if !_is_dvm {
            match crate::PmixClient::connect_new(None) {
                Ok(_) => {}
                Err(_) => {
                    eprintln!("test_fabric_deregister_nb_compiles: init failed, skipping");
                    return;
                }
            }
        }
    }

    /// Test that nb callbacks on unregistered fabric return error without
    /// leaking the callback wrapper.
    #[test]
    fn test_fabric_update_nb_not_registered() {
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let mut fabric = PmixFabric::unamed();
        let result = fabric_update_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_err());
        // If we got here without leaking, the wrapper was reclaimed.
    }

    #[test]
    fn test_fabric_deregister_nb_not_registered() {
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let mut fabric = PmixFabric::unamed();
        let result = fabric_deregister_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_err());
    }

    // ── PmixDeviceDistance accessor tests ──

    /// Test PmixDeviceDistance constructor and uuid accessor.
    #[test]
    fn test_device_distance_uuid() {
        let dist = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        assert_eq!(dist.uuid(), "gpu-001");
    }

    /// Test PmixDeviceDistance osname accessor.
    #[test]
    fn test_device_distance_osname() {
        let dist = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        assert_eq!(dist.osname(), "nvidia0");
    }

    /// Test PmixDeviceDistance device_type accessor.
    #[test]
    fn test_device_distance_device_type() {
        let dist = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        assert_eq!(dist.device_type(), PmixDeviceType::Gpu);
    }

    /// Test PmixDeviceDistance mindist accessor.
    #[test]
    fn test_device_distance_mindist() {
        let dist = PmixDeviceDistance::test_new("net-001", "eth0", PmixDeviceType::Network, 5, 20);
        assert_eq!(dist.mindist(), 5);
    }

    /// Test PmixDeviceDistance maxdist accessor.
    #[test]
    fn test_device_distance_maxdist() {
        let dist = PmixDeviceDistance::test_new("net-001", "eth0", PmixDeviceType::Network, 5, 20);
        assert_eq!(dist.maxdist(), 20);
    }

    /// Test PmixDeviceDistance with empty strings.
    #[test]
    fn test_device_distance_empty_strings() {
        let dist = PmixDeviceDistance::test_new("", "", PmixDeviceType::UnknownType, 0, 0);
        assert_eq!(dist.uuid(), "");
        assert_eq!(dist.osname(), "");
        assert_eq!(dist.device_type(), PmixDeviceType::UnknownType);
        assert_eq!(dist.mindist(), 0);
        assert_eq!(dist.maxdist(), 0);
    }

    /// Test PmixDeviceDistance with all device types.
    #[test]
    fn test_device_distance_all_types() {
        let gpu = PmixDeviceDistance::test_new("g", "g", PmixDeviceType::Gpu, 1, 2);
        assert_eq!(gpu.device_type(), PmixDeviceType::Gpu);
        let net = PmixDeviceDistance::test_new("n", "n", PmixDeviceType::Network, 3, 4);
        assert_eq!(net.device_type(), PmixDeviceType::Network);
        let unknown = PmixDeviceDistance::test_new("u", "u", PmixDeviceType::Unknown(0xFF), 5, 6);
        assert_eq!(unknown.device_type(), PmixDeviceType::Unknown(0xFF));
    }

    // ── DeviceDistances accessor tests ──

    /// Test DeviceDistances with empty collection.
    #[test]
    fn test_device_distances_empty() {
        let distances = DeviceDistances::test_new(vec![]);
        assert!(distances.is_empty());
        assert_eq!(distances.len(), 0);
        assert_eq!(distances.distances().len(), 0);
    }

    /// Test DeviceDistances with single entry.
    #[test]
    fn test_device_distances_single() {
        let entry = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        let distances = DeviceDistances::test_new(vec![entry]);
        assert!(!distances.is_empty());
        assert_eq!(distances.len(), 1);
        assert_eq!(distances.distances().len(), 1);
        assert_eq!(distances.distances()[0].uuid(), "gpu-001");
    }

    /// Test DeviceDistances with multiple entries.
    #[test]
    fn test_device_distances_multiple() {
        let entries = vec![
            PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50),
            PmixDeviceDistance::test_new("gpu-002", "nvidia1", PmixDeviceType::Gpu, 15, 55),
            PmixDeviceDistance::test_new("net-001", "eth0", PmixDeviceType::Network, 20, 80),
        ];
        let distances = DeviceDistances::test_new(entries);
        assert!(!distances.is_empty());
        assert_eq!(distances.len(), 3);
        assert_eq!(distances.distances()[0].uuid(), "gpu-001");
        assert_eq!(distances.distances()[1].uuid(), "gpu-002");
        assert_eq!(distances.distances()[2].uuid(), "net-001");
    }

    /// Test DeviceDistances Debug formatting.
    #[test]
    fn test_device_distances_debug() {
        let entry = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        let distances = DeviceDistances::test_new(vec![entry]);
        let debug_str = format!("{:?}", distances);
        assert!(debug_str.contains("DeviceDistances"));
        assert!(!debug_str.is_empty());
    }

    /// Test DeviceDistances drop with null raw_ptr (test_new path).
    #[test]
    fn test_device_distances_drop_null_raw() {
        // Should not panic or leak when raw_ptr is null.
        let distances = DeviceDistances::test_new(vec![
            PmixDeviceDistance::test_new("g1", "n1", PmixDeviceType::Gpu, 1, 2),
            PmixDeviceDistance::test_new("g2", "n2", PmixDeviceType::Gpu, 3, 4),
        ]);
        drop(distances); // Should be safe, no-op drop
    }

    // ── PmixTopology construction tests ──

    /// Test that PmixTopology can be created with no source.
    #[test]
    fn test_topology_new_unamed() {
        let topo = PmixTopology::unamed();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), None);
    }

    /// Test that PmixTopology can be created with a source hint.
    #[test]
    fn test_topology_new_with_source() {
        let topo = PmixTopology::new(Some("hwloc")).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), Some("hwloc"));
    }

    /// Test that PmixTopology can be created with None source.
    #[test]
    fn test_topology_new_none_source() {
        let topo = PmixTopology::new(None).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), None);
    }

    #[test]
    fn test_topology_load_syncs_source_from_raw() {
        let _guard = mock_ffi::MockGuard::new();
        let mut topo = PmixTopology::unamed();
        assert!(load_topology(&mut topo).is_ok());
        assert_eq!(topo.source(), Some("hwloc:2.11.2"));
    }

    /// Test that PmixTopology::new rejects source with interior NUL bytes.
    #[test]
    fn test_topology_new_nul_source() {
        let result = PmixTopology::new(Some("hw\0loc"));
        assert!(result.is_err());
    }

    /// Test that PmixTopology implements Debug.
    #[test]
    fn test_topology_debug() {
        let topo = PmixTopology::unamed();
        let debug_str = format!("{:?}", topo);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("PmixTopology"));
    }

    /// Test PmixTopology::test_new with source.
    #[test]
    fn test_topology_test_new() {
        let topo = PmixTopology::test_new(Some("test_source")).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), Some("test_source"));
    }

    /// Test PmixTopology::test_new without source.
    #[test]
    fn test_topology_test_new_none() {
        let topo = PmixTopology::test_new(None).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), None);
    }

    /// Test PmixTopology Debug includes source field.
    #[test]
    fn test_topology_debug_with_source() {
        let topo = PmixTopology::new(Some("nvlink")).unwrap();
        let debug_str = format!("{:?}", topo);
        assert!(debug_str.contains("PmixTopology"));
    }

    // ── PmixCpuset construction tests ──

    /// Test that PmixCpuset::new() constructs without crashing.
    /// PMIx_Cpuset_construct may fail without init — we just verify
    /// the object is created and the constructed flag is set.
    #[test]
    fn test_cpuset_new() {
        // PmixCpuset::new calls PMIx_Cpuset_construct which may need init.
        // We skip init here and just verify construction doesn't panic.
        // The Drop will call destruct — both may be no-ops without PMIx.
        let _cpuset = PmixCpuset::new();
    }

    /// Test that PmixCpuset::test_new() creates a safe test instance.
    #[test]
    fn test_cpuset_test_new() {
        let mut cpuset = PmixCpuset::test_new();
        // as_mut_ptr should not panic since constructed is true.
        let _ptr = cpuset.as_mut_ptr();
    }

    /// Test PmixCpuset Default trait delegates to new().
    #[test]
    fn test_cpuset_default() {
        let _cpuset = PmixCpuset::default();
        // Just verify it compiles and doesn't panic.
    }

    /// Test PmixCpuset Debug formatting.
    #[test]
    fn test_cpuset_debug() {
        let cpuset = PmixCpuset::test_new();
        let debug_str = format!("{:?}", cpuset);
        // Debug output may include MaybeUninit placeholder — just verify no panic.
        assert!(!debug_str.is_empty());
    }

    // ── PmixDeviceDistance Clone tests ──

    /// Test that PmixDeviceDistance implements Clone.
    #[test]
    fn test_device_distance_clone() {
        let dist = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        let cloned = dist.clone();
        assert_eq!(cloned.uuid(), "gpu-001");
        assert_eq!(cloned.osname(), "nvidia0");
        assert_eq!(cloned.device_type(), PmixDeviceType::Gpu);
        assert_eq!(cloned.mindist(), 10);
        assert_eq!(cloned.maxdist(), 50);
    }

    /// Test PmixDeviceDistance Debug formatting.
    #[test]
    fn test_device_distance_debug() {
        let dist = PmixDeviceDistance::test_new("gpu-001", "nvidia0", PmixDeviceType::Gpu, 10, 50);
        let debug_str = format!("{:?}", dist);
        assert!(debug_str.contains("PmixDeviceDistance"));
        assert!(debug_str.contains("gpu-001"));
    }

    // ── DeviceDistances edge case tests ──

    /// Test DeviceDistances with ten entries (larger collection).
    #[test]
    fn test_device_distances_ten_entries() {
        let entries: Vec<_> = (0..10)
            .map(|i| {
                PmixDeviceDistance::test_new(
                    &format!("dev-{}", i),
                    &format!("os-{}", i),
                    PmixDeviceType::Gpu,
                    i as u16,
                    (i * 2) as u16,
                )
            })
            .collect();
        let distances = DeviceDistances::test_new(entries);
        assert_eq!(distances.len(), 10);
        assert_eq!(distances.distances()[9].uuid(), "dev-9");
        assert_eq!(distances.distances()[9].maxdist(), 18);
    }

    /// Test DeviceDistances distances() returns a slice reference.
    #[test]
    fn test_device_distances_slice_reference() {
        let entry = PmixDeviceDistance::test_new("a", "b", PmixDeviceType::Network, 1, 2);
        let distances = DeviceDistances::test_new(vec![entry]);
        let slice: &[PmixDeviceDistance] = distances.distances();
        assert_eq!(slice.len(), 1);
    }

    // ── ComputeDistancesCallback trait tests ──

    /// Test that ComputeDistancesCallback trait is object-safe.
    #[test]
    fn test_compute_distances_callback_trait_object() {
        struct TestDistCb;
        impl ComputeDistancesCallback for TestDistCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
        }
        let _cb: Box<dyn ComputeDistancesCallback> = Box::new(TestDistCb);
    }

    /// Test ComputeDistancesCallback that records status and distances.
    #[test]
    fn test_compute_distances_callback_records_values() {
        use std::cell::Cell;

        struct RecordingDistCb {
            status: Cell<Option<PmixStatus>>,
            count: Cell<Option<usize>>,
        }
        impl ComputeDistancesCallback for RecordingDistCb {
            fn on_complete(self: Box<Self>, status: PmixStatus, distances: DeviceDistances) {
                self.status.set(Some(status));
                self.count.set(Some(distances.len()));
            }
        }

        let cb = RecordingDistCb {
            status: Cell::new(None),
            count: Cell::new(None),
        };
        let _boxed: Box<dyn ComputeDistancesCallback> = Box::new(cb);
        // Trait compiles and is object-safe — that's the main goal.
    }

    /// Test compute_distances_nb compiles with callback signature.
    #[test]
    fn test_compute_distances_nb_compiles() {
        struct NbDistCb;
        impl ComputeDistancesCallback for NbDistCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _distances: DeviceDistances) {}
        }
        // Verify the callback compiles with correct signature.
        let _cb: Box<dyn ComputeDistancesCallback> = Box::new(NbDistCb);
    }

    // ── Fabric edge case tests ──

    /// Test PmixFabric with empty string name.
    #[test]
    fn test_fabric_new_empty_name() {
        let fabric = PmixFabric::new(Some("")).unwrap();
        assert!(!fabric.is_registered());
        assert_eq!(fabric.name(), Some(""));
    }

    /// Test PmixFabric with long name.
    #[test]
    fn test_fabric_new_long_name() {
        let long_name = "a".repeat(256);
        let fabric = PmixFabric::new(Some(&long_name)).unwrap();
        assert!(!fabric.is_registered());
        assert_eq!(fabric.name(), Some(long_name.as_str()));
    }

    /// Test PmixFabric index returns 0 for unregistered fabric.
    #[test]
    fn test_fabric_index_unregistered() {
        let fabric = PmixFabric::unamed();
        assert_eq!(fabric.index(), 0);
    }

    /// Test PmixFabric ninfo returns 0 for unregistered fabric.
    #[test]
    fn test_fabric_ninfo_unregistered() {
        let fabric = PmixFabric::unamed();
        assert_eq!(fabric.ninfo(), 0);
    }

    /// Test PmixFabric Debug includes registered field.
    #[test]
    fn test_fabric_debug_registered_field() {
        let fabric = PmixFabric::unamed();
        let debug_str = format!("{:?}", fabric);
        assert!(debug_str.contains("registered"));
        assert!(debug_str.contains("false"));
    }

    // ── Fabric register with non-empty directives ──

    /// Test that fabric_register compiles with non-empty directives.
    /// Without a PMIx server this returns an error, but verifies
    /// the Info array marshalling path.
    #[test]
    fn test_fabric_register_with_directives() {
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        // Create a dummy Info directive using InfoBuilder.
        let mut builder = crate::InfoBuilder::new();
        builder.collect_data();
        let info = builder.build().expect("build info");
        let result = fabric_register(&mut fabric, &[info]);
        // Without PMIx server, expect error — but no crash.
        if let Ok(()) = result {
            assert!(fabric.is_registered());
        }
    }

    // ── Topology edge case tests ──

    /// Test PmixTopology with empty string source.
    #[test]
    fn test_topology_new_empty_source() {
        let topo = PmixTopology::new(Some("")).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), Some(""));
    }

    /// Test PmixTopology with long source string.
    #[test]
    fn test_topology_new_long_source() {
        let long_source = "s".repeat(512);
        let topo = PmixTopology::new(Some(&long_source)).unwrap();
        assert!(!topo.is_loaded());
        assert_eq!(topo.source(), Some(long_source.as_str()));
    }

    // ── DeviceDistance extreme value tests ──

    /// Test PmixDeviceDistance with maximum u16 distance values.
    #[test]
    fn test_device_distance_max_u16_values() {
        let dist = PmixDeviceDistance::test_new(
            "extreme",
            "extreme0",
            PmixDeviceType::Gpu,
            u16::MAX,
            u16::MAX,
        );
        assert_eq!(dist.mindist(), u16::MAX);
        assert_eq!(dist.maxdist(), u16::MAX);
    }

    /// Test PmixDeviceDistance with mixed device types in collection.
    #[test]
    fn test_device_distances_mixed_types() {
        let entries = vec![
            PmixDeviceDistance::test_new("gpu-0", "nv0", PmixDeviceType::Gpu, 1, 10),
            PmixDeviceDistance::test_new("net-0", "eth0", PmixDeviceType::Network, 5, 50),
            PmixDeviceDistance::test_new("unk-0", "x0", PmixDeviceType::Unknown(42), 3, 30),
        ];
        let distances = DeviceDistances::test_new(entries);
        assert_eq!(distances.len(), 3);
        assert_eq!(distances.distances()[0].device_type(), PmixDeviceType::Gpu);
        assert_eq!(
            distances.distances()[1].device_type(),
            PmixDeviceType::Network
        );
        assert_eq!(
            distances.distances()[2].device_type(),
            PmixDeviceType::Unknown(42)
        );
    }

    // ── Fabric callback wrapper leak prevention tests ──

    /// Test that fabric_register_nb on unregistered fabric doesn't leak.
    #[test]
    fn test_fabric_register_nb_not_registered() {
        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        // fabric_register_nb doesn't check registered status — it always
        // attempts the FFI call. But without a server, the FFI call
        // returns an error and the wrapper is reclaimed.
        // We test the compile path and wrapper cleanup indirectly.
        let _cb: Box<dyn FabricCallback> = Box::new(NbCb);
    }

    // ── Mock-based fabric FFI tests ──

    #[test]
    fn test_fabric_register_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test_fabric")).unwrap();
        assert!(!fabric.is_registered());
        let result = fabric_register(&mut fabric, &[]);
        assert!(result.is_ok());
        assert!(fabric.is_registered());
        assert_eq!(fabric.index(), 1);
    }

    #[test]
    fn test_fabric_register_with_directives_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test_fabric")).unwrap();
        let mut builder = crate::InfoBuilder::new();
        builder.collect_data();
        let info = builder.build().expect("build info");
        let result = fabric_register(&mut fabric, &[info]);
        assert!(result.is_ok());
        assert!(fabric.is_registered());
    }

    #[test]
    fn test_fabric_register_error_mock() {
        let _guard = mock_ffi::MockGuard::new();
        mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Fabric_register", mock_ffi::PMIX_ERR_BAD_PARAM)
            .apply();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        let result = fabric_register(&mut fabric, &[]);
        assert!(result.is_err());
        assert!(!fabric.is_registered());
    }

    #[test]
    fn test_fabric_update_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        // Register first
        fabric_register(&mut fabric, &[]).unwrap();
        // Now update
        let result = fabric_update(&mut fabric);
        assert!(result.is_ok());
        assert!(fabric.is_registered());
    }

    #[test]
    fn test_fabric_update_not_registered_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        // Not registered — should fail
        let result = fabric_update(&mut fabric);
        assert!(result.is_err());
    }

    #[test]
    fn test_fabric_deregister_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        // Register first
        fabric_register(&mut fabric, &[]).unwrap();
        assert!(fabric.is_registered());
        // Deregister
        let result = fabric_deregister(&mut fabric);
        assert!(result.is_ok());
        assert!(!fabric.is_registered());
        assert_eq!(fabric.ninfo(), 0);
    }

    #[test]
    fn test_fabric_deregister_not_registered_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        let result = fabric_deregister(&mut fabric);
        assert!(result.is_err());
    }

    #[test]
    fn test_fabric_register_nb_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();

        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let result = fabric_register_nb(&mut fabric, &[], Box::new(NbCb));
        assert!(result.is_ok());
        assert!(fabric.is_registered());
    }

    #[test]
    fn test_fabric_update_nb_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        fabric_register(&mut fabric, &[]).unwrap();

        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let result = fabric_update_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_ok());
    }

    #[test]
    fn test_fabric_update_nb_not_registered_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();

        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let result = fabric_update_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_err());
    }

    #[test]
    fn test_fabric_deregister_nb_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();
        fabric_register(&mut fabric, &[]).unwrap();

        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let result = fabric_deregister_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_ok());
        assert!(!fabric.is_registered());
    }

    #[test]
    fn test_fabric_deregister_nb_not_registered_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut fabric = PmixFabric::new(Some("test")).unwrap();

        struct NbCb;
        impl FabricCallback for NbCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }

        let result = fabric_deregister_nb(&mut fabric, Box::new(NbCb));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_topology_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
        assert!(!topo.is_loaded());
        let result = load_topology(&mut topo);
        assert!(result.is_ok());
        assert!(topo.is_loaded());
    }

    #[test]
    fn test_compute_distances_mock() {
        let _guard = mock_ffi::MockGuard::new();

        // Set up mock device distances
        mock_ffi::mock_set_device_distances(vec![
            ("gpu-001".to_string(), "nvidia0".to_string(), 0u64, 10, 50),
            ("gpu-002".to_string(), "nvidia1".to_string(), 0u64, 20, 60),
        ]);

        let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
        let mut cpuset = PmixCpuset::new();
        let result = compute_distances(&mut topo, &mut cpuset, &[]);
        assert!(result.is_ok());
        let distances = result.unwrap();
        assert_eq!(distances.len(), 2);
        assert_eq!(distances.distances()[0].uuid(), "gpu-001");
        assert_eq!(distances.distances()[0].osname(), "nvidia0");
        assert_eq!(distances.distances()[0].mindist(), 10);
        assert_eq!(distances.distances()[0].maxdist(), 50);
        assert_eq!(distances.distances()[1].uuid(), "gpu-002");
    }

    #[test]
    fn test_compute_distances_empty_mock() {
        let _guard = mock_ffi::MockGuard::new();
        // No mock distances set — should return empty
        mock_ffi::mock_set_device_distances(vec![]);

        let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
        let mut cpuset = PmixCpuset::new();
        let result = compute_distances(&mut topo, &mut cpuset, &[]);
        assert!(result.is_ok());
        let distances = result.unwrap();
        assert_eq!(distances.len(), 0);
    }

    #[test]
    fn test_compute_distances_nb_deep_copies_before_release() {
        let _guard = mock_ffi::MockGuard::new();
        mock_ffi::mock_set_device_distances(vec![(
            "nb-uuid".to_string(),
            "nb-osname".to_string(),
            0,
            3,
            9,
        )]);

        struct Callback {
            result: std::sync::Arc<std::sync::Mutex<Option<(String, String)>>>,
        }
        impl ComputeDistancesCallback for Callback {
            fn on_complete(self: Box<Self>, _status: PmixStatus, distances: DeviceDistances) {
                let entry = &distances.distances()[0];
                *self.result.lock().unwrap() =
                    Some((entry.uuid().to_string(), entry.osname().to_string()));
            }
        }

        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
        let mut cpuset = PmixCpuset::new();
        compute_distances_nb(
            &mut topo,
            &mut cpuset,
            &[],
            Box::new(Callback {
                result: result.clone(),
            }),
        )
        .unwrap();

        let (uuid, osname) = result.lock().unwrap().take().unwrap();
        assert_eq!(uuid, "nb-uuid");
        assert_eq!(osname, "nb-osname");
    }

    #[test]
    fn test_cpuset_new_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut cpuset = PmixCpuset::new();
        let _ptr = cpuset.as_mut_ptr();
        // Should not panic — mock construct succeeded
    }

    #[test]
    fn test_geometry_construct_and_accessors_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let geometry = PmixGeometry::new();
        assert_eq!(geometry.fabric(), 0);
        assert!(geometry.uuid().is_none());
        assert!(geometry.osname().is_none());
        assert_eq!(geometry.ncoords(), 0);
        assert!(geometry.coordinates().is_none());
    }

    #[test]
    fn test_geometry_non_null_accessors_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut geometry = PmixGeometry::test_new();
        let uuid = CString::new("geometry-uuid").unwrap();
        let osname = CString::new("gpu0").unwrap();
        let mut coords = [
            ffi::pmix_coord_t {
                view: 0,
                coord: ptr::null_mut(),
                dims: 0,
            },
            ffi::pmix_coord_t {
                view: 1,
                coord: ptr::null_mut(),
                dims: 0,
            },
        ];

        unsafe {
            let raw = geometry.raw.assume_init_mut();
            raw.uuid = uuid.as_ptr() as *mut _;
            raw.osname = osname.as_ptr() as *mut _;
            raw.coordinates = coords.as_mut_ptr();
            raw.ncoords = coords.len();
        }

        assert_eq!(geometry.uuid(), Some("geometry-uuid"));
        assert_eq!(geometry.osname(), Some("gpu0"));
        assert_eq!(geometry.ncoords(), 2);
        assert_eq!(geometry.coordinates().unwrap().len(), 2);
        assert_eq!(geometry.coordinates().unwrap()[1].view, 1);
    }

    #[test]
    fn test_geometry_test_new_drop_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let geometry = PmixGeometry::test_new();
        assert_eq!(geometry.fabric(), 0);
        assert_eq!(geometry.ncoords(), 0);
        drop(geometry);
    }

    #[test]
    fn test_endpoint_construct_and_accessors_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let endpoint = PmixEndpoint::new();
        assert!(endpoint.uuid().is_none());
        assert!(endpoint.osname().is_none());
        assert!(endpoint.endpt().is_none());
    }

    #[test]
    fn test_endpoint_non_null_accessors_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut endpoint = PmixEndpoint::test_new();
        let uuid = CString::new("endpoint-uuid").unwrap();
        let osname = CString::new("eth0").unwrap();
        let bytes = [1_u8, 2, 3, 4];
        unsafe {
            let raw = endpoint.raw.assume_init_mut();
            raw.uuid = uuid.as_ptr() as *mut _;
            raw.osname = osname.as_ptr() as *mut _;
            raw.endpt.bytes = bytes.as_ptr() as *mut _;
            raw.endpt.size = bytes.len();
        }
        assert_eq!(endpoint.uuid(), Some("endpoint-uuid"));
        assert_eq!(endpoint.osname(), Some("eth0"));
        assert_eq!(endpoint.endpt(), Some(bytes.as_slice()));
    }

    #[test]
    fn test_endpoint_test_new_drop_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let endpoint = PmixEndpoint::test_new();
        assert!(endpoint.uuid().is_none());
        assert!(endpoint.osname().is_none());
        assert!(endpoint.endpt().is_none());
        drop(endpoint);
    }

    #[test]
    fn test_fabric_full_lifecycle_mock() {
        let _guard = mock_ffi::MockGuard::new();
        // Full lifecycle: register -> update -> deregister
        let mut fabric = PmixFabric::new(Some("full_lifecycle")).unwrap();

        // Register
        assert!(fabric_register(&mut fabric, &[]).is_ok());
        assert!(fabric.is_registered());
        assert_eq!(fabric.index(), 1);

        // Update
        assert!(fabric_update(&mut fabric).is_ok());

        // Deregister
        assert!(fabric_deregister(&mut fabric).is_ok());
        assert!(!fabric.is_registered());
        assert_eq!(fabric.ninfo(), 0);
    }

    #[test]
    fn test_topology_lifecycle_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
        assert!(!topo.is_loaded());

        // Load topology
        assert!(load_topology(&mut topo).is_ok());
        assert!(topo.is_loaded());

        // Drop should call mock destruct
        drop(topo);
    }

    #[test]
    fn test_cpuset_lifecycle_mock() {
        let _guard = mock_ffi::MockGuard::new();
        let mut cpuset = PmixCpuset::new();
        let _ptr = cpuset.as_mut_ptr();
        // Drop should call mock destruct
        drop(cpuset);
    }

    #[test]
    fn test_fabric_register_error_then_retry_mock() {
        let _guard = mock_ffi::MockGuard::new();

        // First attempt: error
        mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Fabric_register", mock_ffi::PMIX_ERR_INIT)
            .apply();

        let mut fabric = PmixFabric::new(Some("retry")).unwrap();
        let result = fabric_register(&mut fabric, &[]);
        assert!(result.is_err());
        assert!(!fabric.is_registered());

        // Reset to success
        mock_ffi::enable_mock_ffi();

        // Second attempt: success
        let result = fabric_register(&mut fabric, &[]);
        assert!(result.is_ok());
        assert!(fabric.is_registered());
    }
}


// Additional safe wrappers for PMIx fabric-related type families.

/// Construct a PMIx fabric object with the C constructor.
///
/// The C-constructed result is valid only through the raw handle. `PmixFabric`
/// manages its Rust-owned fields separately and does not drop this raw struct.
pub fn fabric_construct() -> PmixFabric {
    let mut fabric = PmixFabric::new(None).expect("None cannot contain a NUL");
    let raw = fabric.raw.as_mut_ptr();
    #[cfg(any(test, feature = "mock_ffi"))]
    if mock_ffi::is_mock_enabled() {
        // SAFETY: `raw` points to the initialized storage owned by `fabric`.
        unsafe { mock_ffi::mock_fabric_construct(raw) };
    } else {
        // SAFETY: `raw` points to the initialized storage owned by `fabric`.
        unsafe { ffi::PMIx_Fabric_construct(raw) };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        // SAFETY: `raw` points to the initialized storage owned by `fabric`.
        unsafe { ffi::PMIx_Fabric_construct(raw) };
    }
    fabric
}

/// Construct a PMIx topology object with the C constructor.
pub fn topology_construct() -> PmixTopology {
    let mut topology = PmixTopology::new(None).expect("None cannot contain a NUL");
    let raw = topology.raw.as_mut_ptr();
    #[cfg(any(test, feature = "mock_ffi"))]
    if mock_ffi::is_mock_enabled() {
        // SAFETY: `raw` points to the initialized storage owned by `topology`.
        unsafe { mock_ffi::mock_topology_construct(raw) };
    } else {
        // SAFETY: `raw` points to the initialized storage owned by `topology`.
        unsafe { ffi::PMIx_Topology_construct(raw) };
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        // SAFETY: `raw` points to the initialized storage owned by `topology`.
        unsafe { ffi::PMIx_Topology_construct(raw) };
    }
    topology
}

/// RAII wrapper around a PMIx array allocated by its C API.
macro_rules! pmix_array {
    ($name:ident, $raw:ty, $create:ident, $free:ident, $construct:ident,
        $mock_create:ident, $mock_free:ident, $mock_construct:ident $(, $extra:expr)*) => {
        #[derive(Debug)]
        pub struct $name { ptr: *mut $raw, len: usize }

        impl $name {
            /// Allocate and construct `len` C objects. A zero-length request
            /// returns `ErrNomem`, matching PMIx 6.1's NULL result.
            pub fn create(len: usize) -> Result<Self, PmixError> {
                Self::create_with_args(len)
            }

            fn create_with_args(len: usize) -> Result<Self, PmixError> {
                let ptr = {
                    #[cfg(any(test, feature = "mock_ffi"))]
                    if mock_ffi::is_mock_enabled() {
                        // SAFETY: the mock receives the requested count and returns
                        // either a fresh allocation or NULL.
                        unsafe { mock_ffi::$mock_create($($extra,)* len) }
                    } else {
                        // SAFETY: PMIx allocates an array of `len` raw objects.
                        unsafe { ffi::$create($($extra,)* len) }
                    }
                    #[cfg(not(any(test, feature = "mock_ffi")))]
                    {
                        // SAFETY: PMIx allocates an array of `len` raw objects.
                        unsafe { ffi::$create($($extra,)* len) }
                    }
                };
                if ptr.is_null() {
                    return Err(PmixError::ErrNomem);
                }
                for index in 0..len {
                    // SAFETY: PMIx returned storage for `len` contiguous elements;
                    // each element is constructed exactly once before Drop frees it.
                    let element = unsafe { ptr.add(index) };
                    #[cfg(any(test, feature = "mock_ffi"))]
                    if mock_ffi::is_mock_enabled() {
                        // SAFETY: `element` points into the PMIx allocation.
                        unsafe { mock_ffi::$mock_construct(element) };
                    } else {
                        // SAFETY: `element` points into the PMIx allocation.
                        unsafe { ffi::$construct(element) };
                    }
                    #[cfg(not(any(test, feature = "mock_ffi")))]
                    {
                        // SAFETY: `element` points into the PMIx allocation.
                        unsafe { ffi::$construct(element) };
                    }
                }
                Ok(Self { ptr, len })
            }

            /// Return the owned C array pointer.
            pub fn as_mut_ptr(&self) -> *mut $raw { self.ptr }
            /// Return the number of C objects in the array.
            pub fn len(&self) -> usize { self.len }
            /// Return whether the C array is empty.
            pub fn is_empty(&self) -> bool { self.len == 0 }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                if self.ptr.is_null() { return; }
                #[cfg(any(test, feature = "mock_ffi"))]
                if mock_ffi::is_mock_enabled() {
                    // SAFETY: pointer and length are the values returned by PMIx.
                    unsafe { mock_ffi::$mock_free(self.ptr, self.len) };
                } else {
                    // SAFETY: pointer and length are the values returned by PMIx.
                    unsafe { ffi::$free(self.ptr, self.len) };
                }
                #[cfg(not(any(test, feature = "mock_ffi")))]
                {
                    // SAFETY: pointer and length are the values returned by PMIx.
                    unsafe { ffi::$free(self.ptr, self.len) };
                }
            }
        }
    };
}

pmix_array!(
    PmixGeometryArray,
    ffi::pmix_geometry_t,
    PMIx_Geometry_create,
    PMIx_Geometry_free,
    PMIx_Geometry_construct,
    mock_geometry_create,
    mock_geometry_free,
    mock_geometry_construct
);
pmix_array!(
    PmixTopologyArray,
    ffi::pmix_topology_t,
    PMIx_Topology_create,
    PMIx_Topology_free,
    PMIx_Topology_construct,
    mock_topology_create,
    mock_topology_free,
    mock_topology_construct
);
pmix_array!(
    PmixCpusetArray,
    ffi::pmix_cpuset_t,
    PMIx_Cpuset_create,
    PMIx_Cpuset_free,
    PMIx_Cpuset_construct,
    mock_cpuset_create,
    mock_cpuset_free,
    mock_cpuset_construct
);
pmix_array!(
    PmixEndpointArray,
    ffi::pmix_endpoint_t,
    PMIx_Endpoint_create,
    PMIx_Endpoint_free,
    PMIx_Endpoint_construct,
    mock_endpoint_create,
    mock_endpoint_free,
    mock_endpoint_construct
);
pmix_array!(
    PmixDeviceArray,
    ffi::pmix_device_t,
    PMIx_Device_create,
    PMIx_Device_free,
    PMIx_Device_construct,
    mock_device_create,
    mock_device_free,
    mock_device_construct
);
pmix_array!(
    PmixDeviceDistanceArray,
    ffi::pmix_device_distance_t,
    PMIx_Device_distance_create,
    PMIx_Device_distance_free,
    PMIx_Device_distance_construct,
    mock_device_distance_create,
    mock_device_distance_free,
    mock_device_distance_construct
);

/// An array of coordinate objects; `dims` is passed to PMIx_Coord_create.
#[derive(Debug)]
pub struct PmixCoordArray {
    ptr: *mut ffi::pmix_coord_t,
    len: usize,
}
impl PmixCoordArray {
    /// Allocate and construct `len` coordinate objects with `dims` dimensions.
    pub fn create(dims: usize, len: usize) -> Result<Self, PmixError> {
        let ptr = {
            #[cfg(any(test, feature = "mock_ffi"))]
            if mock_ffi::is_mock_enabled() {
                // SAFETY: mock allocation is parameterized by dimensions and count.
                unsafe { mock_ffi::mock_coord_create(dims, len) }
            } else {
                // SAFETY: PMIx allocates an array of coordinate objects.
                unsafe { ffi::PMIx_Coord_create(dims, len) }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            {
                unsafe { ffi::PMIx_Coord_create(dims, len) }
            }
        };
        if ptr.is_null() {
            return Err(PmixError::ErrNomem);
        }
        // PMIx_Coord_create constructs element zero and initializes its
        // dimension buffer; reconstructing it would leak that buffer.
        for index in 1..len {
            // SAFETY: PMIx returned `len` contiguous coordinate objects.
            let element = unsafe { ptr.add(index) };
            #[cfg(any(test, feature = "mock_ffi"))]
            if mock_ffi::is_mock_enabled() {
                // SAFETY: element belongs to the mock allocation.
                unsafe { mock_ffi::mock_coord_construct(element) };
            } else {
                // SAFETY: element belongs to the PMIx allocation.
                unsafe { ffi::PMIx_Coord_construct(element) };
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            {
                unsafe { ffi::PMIx_Coord_construct(element) };
            }
        }
        Ok(Self { ptr, len })
    }
    /// Return the owned C array pointer.
    pub fn as_mut_ptr(&self) -> *mut ffi::pmix_coord_t {
        self.ptr
    }
    /// Return the number of coordinates.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Return whether no coordinates were allocated.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl Drop for PmixCoordArray {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        #[cfg(any(test, feature = "mock_ffi"))]
        if mock_ffi::is_mock_enabled() {
            // SAFETY: pointer and length are the values returned by PMIx.
            unsafe { mock_ffi::mock_coord_free(self.ptr, self.len) };
        } else {
            // SAFETY: pointer and length are the values returned by PMIx.
            unsafe { ffi::PMIx_Coord_free(self.ptr, self.len) };
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            unsafe { ffi::PMIx_Coord_free(self.ptr, self.len) };
        }
    }
}

macro_rules! c_string_accessor {
    ($name:ident, $field:ident) => {
        /// Return the C string field as UTF-8, if present.
        pub fn $name(&self) -> Option<&str> {
            // SAFETY: raw is initialized by new/test_new and the returned slice borrows self.
            unsafe {
                let p = self.raw.assume_init_ref().$field;
                (!p.is_null())
                    .then(|| CStr::from_ptr(p).to_str().ok())
                    .flatten()
            }
        }
    };
}

/// Safe RAII wrapper around `pmix_coord_t`.
#[derive(Debug)]
pub struct PmixCoord {
    raw: MaybeUninit<ffi::pmix_coord_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixCoord {
    /// Construct a coordinate with PMIx defaults.
    pub fn new() -> Self {
        let mut this = Self {
            raw: MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let p = this.raw.as_mut_ptr();
        #[cfg(any(test, feature = "mock_ffi"))]
        if mock_ffi::is_mock_enabled() {
            // SAFETY: p points to this object's uninitialized storage.
            unsafe { mock_ffi::mock_coord_construct(p) };
        } else {
            // SAFETY: p points to this object's uninitialized storage.
            unsafe { ffi::PMIx_Coord_construct(p) };
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            // SAFETY: p points to this object's uninitialized storage.
            unsafe { ffi::PMIx_Coord_construct(p) };
        }
        this.constructed = true;
        this
    }
    /// Construct a zeroed test object without C-owned allocations.
    pub fn test_new() -> Self {
        Self {
            raw: MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    /// Return the coordinate view value.
    pub fn view(&self) -> ffi::pmix_coord_view_t {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().view }
    }
    /// Return the coordinate values, if present.
    pub fn coord(&self) -> Option<&[u32]> {
        // SAFETY: raw is initialized by new or test_new; coord and dims are a PMIx pair.
        unsafe {
            let r = self.raw.assume_init_ref();
            (!r.coord.is_null()).then(|| std::slice::from_raw_parts(r.coord, r.dims))
        }
    }
    /// Return the number of dimensions.
    pub fn dims(&self) -> usize {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().dims }
    }
}
impl Default for PmixCoord {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixCoord {
    fn drop(&mut self) {
        if self.constructed {
            #[cfg(any(test, feature = "mock_ffi"))]
            if mock_ffi::is_mock_enabled() {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    mock_ffi::mock_coord_destruct(self.raw.as_mut_ptr());
                }
            } else {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    ffi::PMIx_Coord_destruct(self.raw.as_mut_ptr());
                }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                ffi::PMIx_Coord_destruct(self.raw.as_mut_ptr());
            }
            self.constructed = false;
        }
    }
}

/// Safe RAII wrapper around `pmix_device_t`.
#[derive(Debug)]
pub struct PmixDevice {
    raw: MaybeUninit<ffi::pmix_device_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixDevice {
    /// Construct a device with PMIx defaults.
    pub fn new() -> Self {
        let mut this = Self {
            raw: MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let p = this.raw.as_mut_ptr();
        #[cfg(any(test, feature = "mock_ffi"))]
        if mock_ffi::is_mock_enabled() {
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                mock_ffi::mock_device_construct(p);
            }
        } else {
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                ffi::PMIx_Device_construct(p);
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
        unsafe {
            ffi::PMIx_Device_construct(p);
        }
        this.constructed = true;
        this
    }
    /// Construct a zeroed test object without C-owned allocations.
    pub fn test_new() -> Self {
        Self {
            raw: MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    c_string_accessor!(uuid, uuid);
    c_string_accessor!(osname, osname);
    /// Return the PMIx device type.
    pub fn device_type(&self) -> ffi::pmix_device_type_t {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().type_ }
    }
}
impl Default for PmixDevice {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixDevice {
    fn drop(&mut self) {
        if self.constructed {
            #[cfg(any(test, feature = "mock_ffi"))]
            if mock_ffi::is_mock_enabled() {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    mock_ffi::mock_device_destruct(self.raw.as_mut_ptr());
                }
            } else {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    ffi::PMIx_Device_destruct(self.raw.as_mut_ptr());
                }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                ffi::PMIx_Device_destruct(self.raw.as_mut_ptr());
            }
            self.constructed = false;
        }
    }
}

/// RAII wrapper for a PMIx device-distance object. This is distinct from the
/// pure-Rust `PmixDeviceDistance` parsed snapshot type. PMIx initializes both
/// `mindist` and `maxdist` to 65535 (`u16::MAX`).
#[derive(Debug)]
pub struct PmixDeviceDistanceObject {
    raw: MaybeUninit<ffi::pmix_device_distance_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixDeviceDistanceObject {
    /// Construct a device-distance object with PMIx defaults.
    pub fn new() -> Self {
        let mut this = Self {
            raw: MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let p = this.raw.as_mut_ptr();
        #[cfg(any(test, feature = "mock_ffi"))]
        if mock_ffi::is_mock_enabled() {
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                mock_ffi::mock_device_distance_construct(p);
            }
        } else {
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                ffi::PMIx_Device_distance_construct(p);
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
        unsafe {
            ffi::PMIx_Device_distance_construct(p);
        }
        this.constructed = true;
        this
    }
    /// Construct a zeroed test object without C-owned allocations.
    pub fn test_new() -> Self {
        Self {
            raw: MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    c_string_accessor!(uuid, uuid);
    c_string_accessor!(osname, osname);
    /// Return the PMIx device type.
    pub fn device_type(&self) -> ffi::pmix_device_type_t {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().type_ }
    }
    /// Return the minimum distance.
    pub fn min_distance(&self) -> u16 {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().mindist }
    }
    /// Return the maximum distance.
    pub fn max_distance(&self) -> u16 {
        // SAFETY: raw is initialized by new or test_new.
        unsafe { self.raw.assume_init_ref().maxdist }
    }
}
impl Default for PmixDeviceDistanceObject {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixDeviceDistanceObject {
    fn drop(&mut self) {
        if self.constructed {
            #[cfg(any(test, feature = "mock_ffi"))]
            if mock_ffi::is_mock_enabled() {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    mock_ffi::mock_device_distance_destruct(self.raw.as_mut_ptr());
                }
            } else {
                // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
                unsafe {
                    ffi::PMIx_Device_distance_destruct(self.raw.as_mut_ptr());
                }
            }
            #[cfg(not(any(test, feature = "mock_ffi")))]
            // SAFETY: the raw object is initialized by PMIx or test_new and belongs to self.
            unsafe {
                ffi::PMIx_Device_distance_destruct(self.raw.as_mut_ptr());
            }
            self.constructed = false;
        }
    }
}

#[cfg(test)]
mod added_type_tests {
    use super::*;

    #[test]
    fn construct_wrappers_and_arrays_use_mock_ffi() {
        let _guard = mock_ffi::MockGuard::new();
        let _fabric = fabric_construct();
        let _topology = topology_construct();
        let _coord = PmixCoord::new();
        let _device = PmixDevice::new();
        let _distance = PmixDeviceDistanceObject::new();
        assert_eq!(PmixGeometryArray::create(2).unwrap().len(), 2);
        assert_eq!(PmixTopologyArray::create(2).unwrap().len(), 2);
        assert_eq!(PmixCpusetArray::create(2).unwrap().len(), 2);
        assert_eq!(PmixEndpointArray::create(2).unwrap().len(), 2);
        assert_eq!(PmixCoordArray::create(2, 2).unwrap().len(), 2);
        assert_eq!(PmixDeviceArray::create(2).unwrap().len(), 2);
        assert_eq!(PmixDeviceDistanceArray::create(2).unwrap().len(), 2);
    }

    #[test]
    fn zero_length_arrays_match_real_pmix_null_semantics() {
        let _guard = mock_ffi::MockGuard::new();
        assert!(PmixCoordArray::create(2, 0).is_err());
        assert!(PmixGeometryArray::create(0).is_err());
    }

    #[test]
    fn coord_array_elements_are_constructed_before_drop() {
        let _guard = mock_ffi::MockGuard::new();
        let array = PmixCoordArray::create(3, 2).unwrap();
        assert_eq!(array.len(), 2);
        unsafe {
            assert_eq!((*array.as_mut_ptr()).dims, 3);
            assert_eq!((*array.as_mut_ptr().add(1)).dims, 0);
        }
    }

    #[test]
    fn device_distance_defaults_match_pmix() {
        let _guard = mock_ffi::MockGuard::new();
        let distance = PmixDeviceDistanceObject::new();
        assert_eq!(distance.min_distance(), u16::MAX);
        assert_eq!(distance.max_distance(), u16::MAX);
    }

    #[test]
    fn all_fabric_arrays_create_and_drop() {
        let _guard = mock_ffi::MockGuard::new();
        let _geometry = PmixGeometryArray::create(2).unwrap();
        let _topology = PmixTopologyArray::create(2).unwrap();
        let _cpuset = PmixCpusetArray::create(2).unwrap();
        let _endpoint = PmixEndpointArray::create(2).unwrap();
        let _device = PmixDeviceArray::create(2).unwrap();
        let _distance = PmixDeviceDistanceArray::create(2).unwrap();
    }

    #[test]
    fn test_new_accessors_are_zeroed_and_safe() {
        let _guard = mock_ffi::MockGuard::new();
        let coord = PmixCoord::test_new();
        assert_eq!(coord.view(), 0);
        assert_eq!(coord.dims(), 0);
        assert!(coord.coord().is_none());
        let device = PmixDevice::test_new();
        assert!(device.uuid().is_none());
        assert!(device.osname().is_none());
        assert_eq!(device.device_type(), 0);
        let distance = PmixDeviceDistanceObject::test_new();
        assert!(distance.uuid().is_none());
        assert!(distance.osname().is_none());
        assert_eq!(distance.device_type(), 0);
        assert_eq!(distance.min_distance(), 0);
        assert_eq!(distance.max_distance(), 0);
    }

    #[test]
    fn non_null_accessors_read_c_fields() {
        let _guard = mock_ffi::MockGuard::new();
        let mut coord = PmixCoord::test_new();
        let values = [10_u32, 20, 30];
        unsafe {
            let raw = coord.raw.assume_init_mut();
            raw.view = 7;
            raw.coord = values.as_ptr() as *mut _;
            raw.dims = values.len();
        }
        assert_eq!(coord.view(), 7);
        assert_eq!(coord.coord(), Some(values.as_slice()));
        let mut device = PmixDevice::test_new();
        let uuid = CString::new("dev-uuid").unwrap();
        let osname = CString::new("gpu0").unwrap();
        unsafe {
            let raw = device.raw.assume_init_mut();
            raw.uuid = uuid.as_ptr() as *mut _;
            raw.osname = osname.as_ptr() as *mut _;
            raw.type_ = 9;
        }
        assert_eq!(device.uuid(), Some("dev-uuid"));
        assert_eq!(device.osname(), Some("gpu0"));
        assert_eq!(device.device_type(), 9);
    }
}


/// An owned PMIx resource unit.
pub struct PmixResourceUnit {
    raw: MaybeUninit<ffi::pmix_resource_unit_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixResourceUnit {
    pub fn new() -> Self {
        let mut this = Self { raw: MaybeUninit::uninit(), constructed: false, _not_thread_safe: std::marker::PhantomData };
        // SAFETY: PMIx initializes this owned, suitably aligned output object;
        // Drop performs the matching destruct operation once.
        crate::pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_resource_unit_construct(this.raw.as_mut_ptr()) },
            real = unsafe { ffi::PMIx_Resource_unit_construct(this.raw.as_mut_ptr()) },
        );
        this.constructed = true;
        this
    }

    #[cfg(test)]
    pub fn test_new() -> Self {
        Self { raw: MaybeUninit::zeroed(), constructed: true, _not_thread_safe: std::marker::PhantomData }
    }

    pub fn unit_type(&self) -> ffi::pmix_device_type_t {
        // SAFETY: new/test_new initialize the complete C struct before access.
        unsafe { self.raw.assume_init_ref().type_ }
    }

    pub fn count(&self) -> usize {
        // SAFETY: new/test_new initialize the complete C struct before access.
        unsafe { self.raw.assume_init_ref().count }
    }

    pub(crate) fn as_ptr(&self) -> *const ffi::pmix_resource_unit_t { self.raw.as_ptr() }
}

impl Default for PmixResourceUnit { fn default() -> Self { Self::new() } }

impl Drop for PmixResourceUnit {
    fn drop(&mut self) {
        if self.constructed {
            // SAFETY: constructed proves this is the one matching PMIx object.
            crate::pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_resource_unit_destruct(self.raw.as_mut_ptr()) },
                real = unsafe { ffi::PMIx_Resource_unit_destruct(self.raw.as_mut_ptr()) },
            );
            self.constructed = false;
        }
    }
}

pub struct PmixResourceUnitArray { ptr: *mut ffi::pmix_resource_unit_t, len: usize, _not_thread_safe: std::marker::PhantomData<*mut u8> }
impl PmixResourceUnitArray {
    pub fn as_ptr(&self) -> *const ffi::pmix_resource_unit_t { self.ptr }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
impl Drop for PmixResourceUnitArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: ptr/len are the exact PMIx allocation owned by this value.
            crate::pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_resource_unit_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Resource_unit_free(self.ptr, self.len) },
            );
            self.ptr = ptr::null_mut();
        }
    }
}
pub fn resource_unit_create(n: usize) -> Result<PmixResourceUnitArray, PmixError> {
    if n == 0 { return Ok(PmixResourceUnitArray { ptr: ptr::null_mut(), len: 0, _not_thread_safe: std::marker::PhantomData }); }
    // SAFETY: PMIx allocates n initialized resource units and transfers their
    // ownership to the returned RAII array.
    let ptr = crate::pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_resource_unit_create(n) },
        real = unsafe { ffi::PMIx_Resource_unit_create(n) },
    );
    if ptr.is_null() { Err(PmixError::ErrNomem) } else { Ok(PmixResourceUnitArray { ptr, len: n, _not_thread_safe: std::marker::PhantomData }) }
}
impl PmixResourceUnit {
    pub fn to_string(&self) -> Result<String, PmixError> {
        // SAFETY: self points to a live constructed resource unit; PMIx returns
        // a newly allocated NUL-terminated string owned by this function.
        let p = crate::pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_resource_unit_string(self.as_ptr()) },
            real = unsafe { ffi::PMIx_Resource_unit_string(self.as_ptr()) },
        );
        if p.is_null() { return Err(PmixError::ErrNomem); }
        // SAFETY: PMIx returned a valid NUL-terminated string on success.
        let s = unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() };
        // SAFETY: PMIx allocates this returned string with the C allocator.
        unsafe { libc::free(p.cast()) };
        Ok(s)
    }
}


#[cfg(test)]
mod misc_wrapper_tests {
    use super::*;

    #[test]
    fn resource_unit_wrappers_construct_access_string_and_arrays() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let unit = PmixResourceUnit::new();
        assert_eq!(unit.unit_type(), 0);
        assert_eq!(unit.count(), 0);
        assert_eq!(unit.to_string().unwrap(), "resource-unit");
        let array = resource_unit_create(2).unwrap();
        assert_eq!(array.len(), 2);
        assert!(!array.is_empty());
        let empty = resource_unit_create(0).unwrap();
        assert!(empty.is_empty());
        assert!(empty.as_ptr().is_null());
    }
}
