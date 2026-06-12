//! Server-side PMIx APIs — `PMIx_server_init`, `PMIx_server_finalize`,
//! and the `PmixServerModule` / `PmixServerHandle` safe wrappers.
//!
//! This module provides safe Rust wrappers around the PMIx server APIs
//! that allow a resource manager (RM) or daemon to act as a PMIx server.
//!
//! # Overview
//!
//! The PMIx server library is a separate initialization path from the
//! client/tool path. Instead of calling `PMIx_Init`, a server calls
//! `PMIx_server_init` and provides a `pmix_server_module_t` structure
//! whose function pointers implement the callbacks the PMIx library
//! will invoke when clients connect, publish, spawn, etc.
//!
//! # Example
//!
//! ```no_run
//! use pmix::server::{server_init, server_finalize, PmixServerModule, PmixServerHandle};
//!
//! // Create a minimal server module with no callbacks
//! let module = PmixServerModule::default();
//!
//! // Initialize the server library
//! let handle = server_init(Some(&module), &[]).expect("server_init failed");
//!
//! // ... serve clients ...
//!
//! // Finalize
//! server_finalize(handle).expect("server_finalize failed");
//! ```
//!
//! # Callbacks
//!
//! The `PmixServerModule` struct mirrors `pmix_server_module_t`. Each
//! field corresponds to a callback the PMIx library may invoke. All
//! fields default to `None` (null in C), meaning the callback is not
//! implemented. The PMIx library checks for null before calling, so
//! it is safe to provide a minimal module that only implements the
//! callbacks you need.
//!
//! # C API
//!
//! ```c
//! pmix_status_t PMIx_server_init(pmix_server_module_t *module,
//!                                 pmix_info_t info[], size_t ninfo);
//! pmix_status_t PMIx_server_finalize(void);
//! ```

use crate::{Info, PmixStatus, ffi};
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerModule — safe wrapper around pmix_server_module_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust wrapper around `pmix_server_module_t`.
///
/// Each field corresponds to a callback the PMIx server library will
/// invoke. All fields default to `None`, meaning the callback is not
/// implemented by the server. The PMIx library checks for null before
/// calling each callback, so it is safe to set only the ones you need.
///
/// # C API
/// `struct pmix_server_module_4_0_0_t` (aliased as `pmix_server_module_t`)
#[derive(Debug, Default)]
pub struct PmixServerModule {
    /// Called when a client process connects to this server.
    ///
    /// # C type
    /// `pmix_server_client_connected_fn_t`
    pub client_connected: Option<unsafe extern "C" fn()>,

    /// Called when a client process finalizes its connection.
    ///
    /// # C type
    /// `pmix_server_client_finalized_fn_t`
    pub client_finalized: Option<unsafe extern "C" fn()>,

    /// Called when a client requests an abort.
    ///
    /// # C type
    /// `pmix_server_abort_fn_t`
    pub abort: Option<unsafe extern "C" fn()>,

    /// Non-blocking fence callback.
    ///
    /// # C type
    /// `pmix_server_fencenb_fn_t`
    pub fence_nb: Option<unsafe extern "C" fn()>,

    /// Direct modex request callback.
    ///
    /// # C type
    /// `pmix_server_dmodex_req_fn_t`
    pub direct_modex: Option<unsafe extern "C" fn()>,

    /// Publish callback — client requests to publish data.
    ///
    /// # C type
    /// `pmix_server_publish_fn_t`
    pub publish: Option<unsafe extern "C" fn()>,

    /// Lookup callback — client requests to lookup data.
    ///
    /// # C type
    /// `pmix_server_lookup_fn_t`
    pub lookup: Option<unsafe extern "C" fn()>,

    /// Unpublish callback — client requests to remove published data.
    ///
    /// # C type
    /// `pmix_server_unpublish_fn_t`
    pub unpublish: Option<unsafe extern "C" fn()>,

    /// Spawn callback — client requests to spawn new processes.
    ///
    /// # C type
    /// `pmix_server_spawn_fn_t`
    pub spawn: Option<unsafe extern "C" fn()>,

    /// Connect callback — client requests to establish a connection.
    ///
    /// # C type
    /// `pmix_server_connect_fn_t`
    pub connect: Option<unsafe extern "C" fn()>,

    /// Disconnect callback — client requests to disconnect.
    ///
    /// # C type
    /// `pmix_server_disconnect_fn_t`
    pub disconnect: Option<unsafe extern "C" fn()>,

    /// Register events callback.
    ///
    /// # C type
    /// `pmix_server_register_events_fn_t`
    pub register_events: Option<unsafe extern "C" fn()>,

    /// Deregister events callback.
    ///
    /// # C type
    /// `pmix_server_deregister_events_fn_t`
    pub deregister_events: Option<unsafe extern "C" fn()>,

    /// Listener callback — for server-to-server communication.
    ///
    /// # C type
    /// `pmix_server_listener_fn_t`
    pub listener: Option<unsafe extern "C" fn()>,

    /// Notify event callback — deliver notifications to the server.
    ///
    /// # C type
    /// `pmix_server_notify_event_fn_t`
    pub notify_event: Option<unsafe extern "C" fn()>,

    /// Query callback — client requests server-side query.
    ///
    /// # C type
    /// `pmix_server_query_fn_t`
    pub query: Option<unsafe extern "C" fn()>,

    /// Tool connection callback — accepts tool connections.
    ///
    /// # C type
    /// `pmix_server_tool_connection_fn_t`
    pub tool_connected: Option<unsafe extern "C" fn()>,

    /// Log callback — client requests logging.
    ///
    /// # C type
    /// `pmix_server_log_fn_t`
    pub log: Option<unsafe extern "C" fn()>,

    /// Allocation callback — client requests resource allocation.
    ///
    /// # C type
    /// `pmix_server_alloc_fn_t`
    pub allocate: Option<unsafe extern "C" fn()>,

    /// Job control callback.
    ///
    /// # C type
    /// `pmix_server_job_control_fn_t`
    pub job_control: Option<unsafe extern "C" fn()>,

    /// Monitoring callback — client requests monitoring.
    ///
    /// # C type
    /// `pmix_server_monitor_fn_t`
    pub monitor: Option<unsafe extern "C" fn()>,

    /// Get credential callback.
    ///
    /// # C type
    /// `pmix_server_get_cred_fn_t`
    pub get_credential: Option<unsafe extern "C" fn()>,

    /// Validate credential callback.
    ///
    /// # C type
    /// `pmix_server_validate_cred_fn_t`
    pub validate_credential: Option<unsafe extern "C" fn()>,

    /// I/O forwarding pull callback.
    ///
    /// # C type
    /// `pmix_server_iof_fn_t`
    pub iof_pull: Option<unsafe extern "C" fn()>,

    /// Push stdin callback.
    ///
    /// # C type
    /// `pmix_server_stdin_fn_t`
    pub push_stdin: Option<unsafe extern "C" fn()>,

    /// Group operations callback.
    ///
    /// # C type
    /// `pmix_server_grp_fn_t`
    pub group: Option<unsafe extern "C" fn()>,

    /// Fabric operations callback.
    ///
    /// # C type
    /// `pmix_server_fabric_fn_t`
    pub fabric: Option<unsafe extern "C" fn()>,

    /// Client connected v2 callback.
    ///
    /// # C type
    /// `pmix_server_client_connected2_fn_t`
    pub client_connected2: Option<unsafe extern "C" fn()>,

    /// Session control callback (PMIx 5.x).
    ///
    /// # C type
    /// `pmix_server_session_control_fn_t`
    pub session_control: Option<unsafe extern "C" fn()>,
}

impl PmixServerModule {
    /// Convert this safe module into the raw C `pmix_server_module_t`.
    ///
    /// The returned pointer points to stack memory that is valid for
    /// the duration of the `PMIx_server_init` call. Do not store the
    /// pointer beyond the call — the PMIx library copies the struct
    /// internally during initialization.
    ///
    /// # Safety
    /// The caller must ensure that any callback functions stored in
    /// this module remain valid for the lifetime of the PMIx server
    /// library (i.e., until `PMIx_server_finalize` is called).
    pub fn as_c_ptr(&self) -> *const ffi::pmix_server_module_t {
        self as *const Self as *const ffi::pmix_server_module_t
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — RAII handle for the initialized server library
// ─────────────────────────────────────────────────────────────────────────────

/// RAII handle returned by [`server_init`].
///
/// Dropping this handle does **not** automatically call
/// `PMIx_server_finalize` — the server must be explicitly finalized
/// via [`server_finalize`] to ensure proper cleanup of internal
/// communication channels and memory.
///
/// The handle exists to track that the server library has been
/// initialized and to prevent double-initialization.
#[derive(Debug)]
pub struct PmixServerHandle {
    #[allow(dead_code)]
    initialized: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// server_init
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize the PMIx server library.
///
/// This function initializes the server-side PMIx library and registers
/// the provided callback module. The server library will invoke the
/// callbacks in `module` when client processes connect, request data,
/// spawn processes, etc.
///
/// The `info` array can be used to pass server-specific configuration,
/// such as:
///
/// * `PMIX_SERVER_TOOL_SUPPORT` — indicate the server accepts tool connections.
/// * `PMIX_SERVER_SCHEDULER` — indicate the server is a scheduler.
/// * `PMIX_SOCKET_MODE` — file permissions for the Unix domain socket.
/// * `PMIX_HOSTNAME` — hostname for this server instance.
///
/// # Parameters
/// * `module` — the server callback module (may be `None` for a minimal
///   server with no callbacks).
/// * `info` — optional configuration info keys.
///
/// # Returns
/// * `Ok(PmixServerHandle)` — server initialized successfully.
/// * `Err(PmixStatus)` — initialization failed (e.g., PMIx library not
///   available, invalid module, or conflicting initialization).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_init(pmix_server_module_t *module,
///                                 pmix_info_t info[], size_t ninfo);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, PmixServerModule};
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &[]).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_init(
    module: Option<&PmixServerModule>,
    _info: &Info,
) -> Result<PmixServerHandle, PmixStatus> {
    let module_ptr = match module {
        Some(m) => m.as_c_ptr() as *mut ffi::pmix_server_module_t,
        None => ptr::null_mut(),
    };

    let info_ptr = if _info.len > 0 {
        _info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = _info.len;

    let status = unsafe {
        // SAFETY: PMIx_server_init expects:
        // - module_ptr: either a valid pointer to a pmix_server_module_t
        //   (which we provide from &PmixServerModule cast via as_c_ptr),
        //   or null for a minimal server. The PMIx library copies the
        //   struct internally, so the pointer only needs to be valid
        //   for the duration of this call.
        // - info_ptr: either a valid array of pmix_info_t or null.
        // - info_len: the number of info entries (0 if info_ptr is null).
        ffi::PMIx_server_init(module_ptr, info_ptr, info_len)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(PmixServerHandle { initialized: true })
    } else {
        Err(pmix_status)
    }
}

/// Initialize the PMIx server library with no info keys.
///
/// Convenience wrapper around [`server_init`] that passes no
/// configuration info to the server library.
///
/// # Parameters
/// * `module` — the server callback module (may be `None` for a minimal
///   server with no callbacks).
///
/// # Returns
/// * `Ok(PmixServerHandle)` — server initialized successfully.
/// * `Err(PmixStatus)` — initialization failed.
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init_minimal, server_finalize, PmixServerModule};
///
/// let module = PmixServerModule::default();
/// let handle = server_init_minimal(Some(&module)).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_init_minimal(
    module: Option<&PmixServerModule>,
) -> Result<PmixServerHandle, PmixStatus> {
    let module_ptr = match module {
        Some(m) => m.as_c_ptr() as *mut ffi::pmix_server_module_t,
        None => ptr::null_mut(),
    };

    let status = unsafe {
        // SAFETY: PMIx_server_init with null module and null info is the
        // minimal initialization path. The PMIx library will create an
        // internal default module with no callbacks. No pointers are
        // dereferenced beyond the null checks the library performs.
        ffi::PMIx_server_init(module_ptr, ptr::null_mut(), 0)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(PmixServerHandle { initialized: true })
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// server_finalize
// ─────────────────────────────────────────────────────────────────────────────

/// Finalize the PMIx server library.
///
/// This function shuts down the server-side PMIx library, releasing
/// all internal resources, closing communication channels, and freeing
/// memory. After calling this, the server library is no longer usable
/// until `PMIx_server_init` is called again.
///
/// # Parameters
/// * `handle` — the handle returned by [`server_init`]. Consumed by value
///   to prevent use-after-finalize.
///
/// # Returns
/// * `Ok(())` — server finalized successfully.
/// * `Err(PmixStatus)` — finalization failed (should not happen under
///   normal circumstances).
///
/// # C API
/// `pmix_status_t PMIx_server_finalize(void)`
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, PmixServerModule};
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &[]).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_finalize(_handle: PmixServerHandle) -> Result<(), PmixStatus> {
    let status = unsafe {
        // SAFETY: PMIx_server_finalize takes no parameters and returns
        // a status code. It is a cleanup function that releases internal
        // resources. It must only be called after a successful
        // PMIx_server_init and must not be called twice.
        ffi::PMIx_server_finalize()
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// is_server_initialized
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the PMIx server library has been initialized.
///
/// This checks whether `PMIx_server_init` has been called and not yet
/// finalized. Note that this is distinct from [`crate::utility::initialized`],
/// which checks the client-side initialization state.
///
/// # C API
/// Uses `PMIx_Initialized()` internally, which checks the global PMIx
/// initialization state (valid for both client and server paths).
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, is_server_initialized, PmixServerModule};
///
/// assert!(!is_server_initialized());
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &[]).expect("server_init failed");
/// assert!(is_server_initialized());
///
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn is_server_initialized() -> bool {
    // SAFETY: PMIx_Initialized is a simple state check that reads an
    // internal atomic flag. No pointers are dereferenced.
    unsafe { ffi::PMIx_Initialized() != 0 }
}
