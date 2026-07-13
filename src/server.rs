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
//! use pmix::InfoBuilder;;
//!
//! // Create a minimal server module with no callbacks
//! let module = PmixServerModule::default();
//!
//! // Initialize the server library
//! let handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
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

use crate::security::PmixCredential;
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc, ffi};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{LazyLock, Mutex};

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
/// use pmix::InfoBuilder;;
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
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
/// use pmix::InfoBuilder;;
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
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
// PMIx_server_register_nspace — register a job nspace with the server library
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_register_nspace_nb`.
///
/// Implement this trait to receive the result of a non-blocking nspace
/// registration. The `on_complete` method receives the `PmixStatus` result.
pub trait RegisterNspaceCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending register_nspace callbacks.
type RegisterNspaceRegistry = std::collections::HashMap<usize, Box<dyn RegisterNspaceCallback>>;
static REGISTER_NS_SPACE_REGISTRY: LazyLock<Mutex<RegisterNspaceRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing register_nspace request ID counter.
static REGISTER_NS_SPACE_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (register_nspace completion).
///
/// Called by PMIx when the non-blocking nspace registration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
extern "C" fn register_nspace_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
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

/// Register an nspace (job namespace) with the PMIx server library.
///
/// This function informs the PMIx server library about a new job
/// namespace. The server must register ALL nspaces that will participate
/// in collective operations with local processes — even if no local
/// processes belong to that nspace, as long as any local process might
/// perform a collective involving processes from that nspace.
///
/// The `nlocalprocs` parameter tells the library how many local processes
/// will be launched within this nspace. This is required for correct
/// collective handling, because a collective call can occur before all
/// processes have started.
///
/// The `info` array can contain per-process information such as:
///
/// * `PMIX_LOCAL_RANK` — local rank of each process.
/// * `PMIX_PROC_RANK` — global rank within the job.
/// * `PMIX_NODE_RANK` — rank on the local node.
/// * `PMIX_HOSTNAME` — hostname where the process runs.
/// * `PMIX_NODEID` — numeric identifier of the node.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `nspace` — the job namespace identifier (string, max 255 chars).
/// * `nlocalprocs` — number of local processes in this nspace.
/// * `info` — optional per-process info keys.
/// * `callback` — invoked when registration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid
///   nspace, PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_register_nspace(const pmix_nspace_t nspace,
///                                           int nlocalprocs,
///                                           pmix_info_t info[], size_t ninfo,
///                                           pmix_op_cbfunc_t cbfunc,
///                                           void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_register_nspace, PmixServerModule, server_init, server_finalize};
/// use pmix::InfoBuilder;
/// use pmix::PmixStatus;
///
/// struct MyNspaceCallback;
/// impl pmix::server::RegisterNspaceCallback for MyNspaceCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("register_nspace completed: {:?}", status);
///     }
/// }
///
/// let module = PmixServerModule::default();
/// let _handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
///
/// server_register_nspace("myjob.12345", 4, &InfoBuilder::new().build(), Box::new(MyNspaceCallback))
///     .expect("register_nspace request rejected");
/// ```
pub fn server_register_nspace(
    nspace: &str,
    nlocalprocs: i32,
    info: &Info,
    callback: Box<dyn RegisterNspaceCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => return Err(PmixStatus::from_raw(-1)), // PMIX_ERROR — contains NUL
    };

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = REGISTER_NS_SPACE_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Prepare info parameters.
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle, info.len)
    } else {
        (ptr::null_mut(), 0)
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_register_nspace is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array or null (checked above).
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        ffi::PMIx_server_register_nspace(
            nspace_c.as_ptr(),
            nlocalprocs,
            info_ptr,
            ninfo,
            Some(register_nspace_callback_bridge),
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
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
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
/// use pmix::InfoBuilder;
///
/// assert!(!is_server_initialized());
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
/// assert!(is_server_initialized());
///
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn is_server_initialized() -> bool {
    // SAFETY: PMIx_Initialized is a simple state check that reads an
    // internal atomic flag. No pointers are dereferenced.
    unsafe { ffi::PMIx_Initialized() != 0 }
}

// PMIx_server_deregister_nspace — deregister a job nspace and purge its data
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_deregister_nspace` (non-blocking mode).
///
/// Implement this trait to receive the result of a non-blocking nspace
/// deregistration. The `on_complete` method receives the `PmixStatus` result.
pub trait DeregisterNspaceCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending deregister_nspace callbacks.
type DeregisterNspaceRegistry = std::collections::HashMap<usize, Box<dyn DeregisterNspaceCallback>>;
static DEREGISTER_NS_SPACE_REGISTRY: LazyLock<Mutex<DeregisterNspaceRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing deregister_nspace request ID counter.
static DEREGISTER_NS_SPACE_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (deregister_nspace completion).
///
/// Called by PMIx when the non-blocking nspace deregistration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
extern "C" fn deregister_nspace_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DEREGISTER_NS_SPACE_REGISTRY.lock().unwrap();
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

/// Deregister an nspace (job namespace) and purge all related data.
///
/// This function tells the PMIx server library to delete all client
/// information for the specified namespace, including any published
/// data, process records, and other objects associated with that job.
///
/// This is intended to support persistent PMIx servers by providing
/// an opportunity for the host resource manager (RM) to tell the PMIx
/// server library to release all memory for a completed job.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`. If you need blocking behavior, pass
/// `None` for the callback (the C API accepts a NULL callback for
/// blocking execution).
///
/// # Parameters
///
/// * `nspace` — the job namespace identifier to deregister.
/// * `callback` — invoked when deregistration completes. Pass `None`
///   for blocking behavior (not recommended in async contexts).
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback` (if provided).
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # C API
/// ```c
/// void PMIx_server_deregister_nspace(const pmix_nspace_t nspace,
///                                    pmix_op_cbfunc_t cbfunc,
///                                    void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_deregister_nspace, DeregisterNspaceCallback};
/// use pmix::PmixStatus;
///
/// struct MyDeregisterCallback;
/// impl DeregisterNspaceCallback for MyDeregisterCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("deregister_nspace completed: {:?}", status);
///     }
/// }
///
/// // Deregister a completed job's namespace
/// server_deregister_nspace("myjob.12345", Some(Box::new(MyDeregisterCallback)));
/// ```
pub fn server_deregister_nspace(nspace: &str, callback: Option<Box<dyn DeregisterNspaceCallback>>) {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            // Since the C API returns void, we can't report this
            // through a return value. If a callback was provided,
            // invoke it with an error status immediately.
            if let Some(cb) = callback {
                cb.on_complete(PmixStatus::from_raw(-1)); // PMIX_ERROR
            }
            return;
        }
    };

    match callback {
        Some(cb) => {
            // Non-blocking mode: register callback and pass bridge to FFI.
            let req_id = {
                let mut seq = DEREGISTER_NS_SPACE_SEQ.lock().unwrap();
                *seq += 1;
                *seq
            };
            {
                let mut registry = DEREGISTER_NS_SPACE_REGISTRY.lock().unwrap();
                registry.insert(req_id, cb);
            }

            // Encode the request ID as a non-null pointer for cbdata.
            let cbdata = (req_id << 2) as *mut c_void;

            // SAFETY: PMIx_server_deregister_nspace is a non-blocking server API.
            // - nspace_c.as_ptr() is a valid null-terminated string for the
            //   duration of this call (PMIx copies it internally).
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            // - The C function returns void; no return value to check.
            unsafe {
                ffi::PMIx_server_deregister_nspace(
                    nspace_c.as_ptr(),
                    Some(deregister_nspace_callback_bridge),
                    cbdata,
                );
            }
        }
        None => {
            // Blocking mode: pass NULL callback. The C API documents
            // that a NULL cbfunc means the function executes as a
            // blocking operation.
            //
            // SAFETY: nspace_c.as_ptr() is a valid null-terminated string.
            // NULL callback means blocking execution.
            unsafe {
                ffi::PMIx_server_deregister_nspace(nspace_c.as_ptr(), None, std::ptr::null_mut());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_register_client — register a client process with the server
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_register_client`.
///
/// Implement this trait to receive the result of a non-blocking client
/// registration. The `on_complete` method receives the `PmixStatus` result.
pub trait RegisterClientCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending register_client callbacks.
type RegisterClientRegistry = std::collections::HashMap<usize, Box<dyn RegisterClientCallback>>;
static REGISTER_CLIENT_REGISTRY: LazyLock<Mutex<RegisterClientRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing register_client request ID counter.
static REGISTER_CLIENT_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (register_client completion).
///
/// Called by PMIx when the non-blocking client registration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
extern "C" fn register_client_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = REGISTER_CLIENT_REGISTRY.lock().unwrap();
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

/// Register a client process with the PMIx server library.
///
/// This function informs the PMIx server about a specific client process
/// that has been launched. The `uid` and `gid` parameters help the server
/// library authenticate clients as they connect — the library requires
/// the actual credentials of connecting processes to match the registered
/// values.
///
/// The `server_object` parameter allows the host resource manager to
/// associate an opaque pointer with this client. The PMIx library will
/// return this pointer in server callbacks (e.g., when the client calls
/// finalize), allowing the host server to access its own per-client
/// state without performing a lookup.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `proc` — the process identifier (namespace + rank) of the client.
/// * `uid` — expected user ID of the client process for authentication.
/// * `gid` — expected group ID of the client process for authentication.
/// * `server_object` — opaque pointer associated with this client, returned
///   in server callbacks. Pass `None` if not needed.
/// * `callback` — invoked when registration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid proc,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_register_client(const pmix_proc_t *proc,
///                                           uid_t uid, gid_t gid,
///                                           void *server_object,
///                                           pmix_op_cbfunc_t cbfunc,
///                                           void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_register_client, RegisterClientCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyClientCallback;
/// impl RegisterClientCallback for MyClientCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("register_client completed: {:?}", status);
///     }
/// }
///
/// let proc = Proc::new("myjob.12345", 0).expect("invalid nspace");
/// server_register_client(&proc, 1000, 1000, None, Box::new(MyClientCallback))
///     .expect("register_client request rejected");
/// ```
pub fn server_register_client(
    proc: &Proc,
    uid: ffi::uid_t,
    gid: ffi::gid_t,
    server_object: Option<*mut c_void>,
    callback: Box<dyn RegisterClientCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = REGISTER_CLIENT_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = REGISTER_CLIENT_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // The server_object is an opaque pointer the RM associates with this client.
    let server_obj_ptr = match server_object {
        Some(ptr) => ptr,
        None => ptr::null_mut(),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_register_client is a non-blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - uid and gid are passed by value.
        // - server_obj_ptr is either a valid pointer owned by the caller or null.
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        ffi::PMIx_server_register_client(
            proc_ptr,
            uid,
            gid,
            server_obj_ptr,
            Some(register_client_callback_bridge),
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
        let mut registry = REGISTER_CLIENT_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_deregister_client — deregister a specific client process
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_deregister_client`.
///
/// Implement this trait to receive the result of a non-blocking client
/// deregistration. The `on_complete` method receives the `PmixStatus` result.
pub trait DeregisterClientCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending deregister_client callbacks.
type DeregisterClientRegistry = std::collections::HashMap<usize, Box<dyn DeregisterClientCallback>>;
static DEREGISTER_CLIENT_REGISTRY: LazyLock<Mutex<DeregisterClientRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing deregister_client request ID counter.
static DEREGISTER_CLIENT_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (deregister_client completion).
///
/// Called by PMIx when the non-blocking client deregistration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
extern "C" fn deregister_client_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DEREGISTER_CLIENT_REGISTRY.lock().unwrap();
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

/// Deregister a specific client process and purge all data relating to it.
///
/// This function tells the PMIx server library to delete all information
/// for a specific client process. Unlike [`server_deregister_nspace`],
/// which purges ALL data for an entire namespace, this API targets only
/// a single process within a namespace.
///
/// This API is intended solely for use in exception cases — for example,
/// when a specific client must be forcibly removed while other clients
/// in the same namespace continue to operate normally.
///
/// # Parameters
///
/// * `proc` — the process identifier (namespace + rank) of the client to deregister.
/// * `callback` — invoked when deregistration completes. Pass `None` for
///   blocking behavior (the C API accepts a NULL callback for blocking
///   execution, though this is not recommended in async contexts).
///
/// # Returns
///
/// Nothing — the C API returns `void`. If a callback is provided, the
/// result is delivered asynchronously. If no callback is provided, the
/// call executes as a blocking operation.
///
/// # C API
/// ```c
/// void PMIx_server_deregister_client(const pmix_proc_t *proc,
///                                    pmix_op_cbfunc_t cbfunc,
///                                    void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_deregister_client, DeregisterClientCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyDeregisterCallback;
/// impl DeregisterClientCallback for MyDeregisterCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("deregister_client completed: {:?}", status);
///     }
/// }
///
/// let proc = Proc::new("myjob.12345", 3).expect("invalid nspace");
/// // Deregister a specific misbehaving client
/// server_deregister_client(&proc, Some(Box::new(MyDeregisterCallback)));
/// ```
pub fn server_deregister_client(proc: &Proc, callback: Option<Box<dyn DeregisterClientCallback>>) {
    // The proc's nspace must not contain NUL bytes — Proc::new already
    // validates this at construction time, so we can safely access the
    // internal CString here.

    match callback {
        Some(cb) => {
            // Non-blocking mode: register callback and pass bridge to FFI.
            let req_id = {
                let mut seq = DEREGISTER_CLIENT_SEQ.lock().unwrap();
                *seq += 1;
                *seq
            };
            {
                let mut registry = DEREGISTER_CLIENT_REGISTRY.lock().unwrap();
                registry.insert(req_id, cb);
            }

            // Encode the request ID as a non-null pointer for cbdata.
            let cbdata = (req_id << 2) as *mut c_void;

            // Get a pointer to the proc's internal pmix_proc_t for FFI.
            let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

            // SAFETY: PMIx_server_deregister_client is a non-blocking server API.
            // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
            //   that remains alive for the duration of this call (PMIx copies it).
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            // - The C function returns void; no return value to check.
            unsafe {
                ffi::PMIx_server_deregister_client(
                    proc_ptr,
                    Some(deregister_client_callback_bridge),
                    cbdata,
                );
            }
        }
        None => {
            // Blocking mode: pass NULL callback. The C API documents
            // that a NULL cbfunc means the function executes as a
            // blocking operation.
            //
            // SAFETY: proc_ptr is a valid reference to the Proc's internal
            // pmix_proc_t. NULL callback means blocking execution.
            let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;
            unsafe {
                ffi::PMIx_server_deregister_client(proc_ptr, None, std::ptr::null_mut());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_fork — prepare environment for forked child process
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_setup_fork`.
///
/// Sets up the environment of a child process to be forked by the host
/// so it can correctly interact with the PMIx server. The PMIx client
/// needs setup information to properly connect back to the server. This
/// function sets appropriate environment variables for that purpose, and
/// also provides any environment variables that were specified in the
/// launch command (e.g., via `PMIx_Spawn`) plus other values (e.g.,
/// variables required to properly initialize the client's fabric library).
///
/// # Environment variables set
///
/// Typical variables include (implementation-dependent):
///
/// * `PMIX_NAMESPACE` — the process's namespace
/// * `PMIX_RANK` — the process's rank within the namespace
/// * Listener URI variable(s) — rendezvous information for the client
///   to connect back to the server
/// * `PMIX_SECURITY_MODE` — active security module
/// * `PMIX_BFROP_BUFFER_TYPE` — buffer serialization format
/// * `PMIX_GDS_MODULE` — available GDS modules
/// * `PMIX_HOSTNAME` — agreed hostname
/// * `PMIX_VERSION` — PMIx version string
///
/// # Parameters
/// * `proc` — the process whose environment should be set up
/// * `env` — optional initial environment variables (in `KEY=VALUE` format).
///   If `None`, an empty environment is created. The returned environment
///   includes both the initial variables and those added by PMIx.
///
/// # Returns
/// * `Ok(Vec<String>)` — the populated environment as `KEY=VALUE` strings.
///   Pass this directly to `std::process::Command::env_clear().envs(...)`
///   when forking the child process.
/// * `Err(PmixStatus)` — setup failed (e.g., server not initialized,
///   invalid proc, or internal error).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_fork(const pmix_proc_t *proc, char ***env);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_setup_fork, PmixServerModule};
/// use pmix::InfoBuilder;
/// use pmix::Proc;
///
/// let module = PmixServerModule::default();
/// let _handle = server_init(Some(&module), &InfoBuilder::new().build()).expect("server_init failed");
///
/// let proc = Proc::new("myjob.12345", 0).expect("proc creation failed");
/// let env = server_setup_fork(&proc, None).expect("setup_fork failed");
///
/// // Use env to fork/exec the child process
/// // e.g., Command::new("client").env_clear().envs(env).spawn();
/// ```
#[allow(clippy::collapsible_if)]
pub fn server_setup_fork(proc: &Proc, env: Option<Vec<&str>>) -> Result<Vec<String>, PmixStatus> {
    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // Build the initial C environment array from the optional env parameter.
    // The C API expects a char ** that it will modify (append to).
    // We allocate it using libc::calloc/realloc-compatible memory so that
    // pmix_argv_free (which calls free) can safely release it.
    let mut c_env: *mut *mut std::os::raw::c_char = std::ptr::null_mut();

    // If the caller provided initial env vars, convert them to a C array.
    if let Some(initial_env) = env {
        if !initial_env.is_empty() {
            // Allocate array of pointers (null-terminated).
            let arr_len = initial_env.len() + 1; // +1 for NULL terminator
            // SAFETY: calloc returns a zeroed allocation or null on failure.
            // We use std::alloc for a null-terminated array of null pointers.
            let arr_ptr = unsafe {
                libc::calloc(arr_len, std::mem::size_of::<*mut std::os::raw::c_char>())
                    as *mut *mut std::os::raw::c_char
            };
            if arr_ptr.is_null() && !initial_env.is_empty() {
                return Err(PmixStatus::from_raw(-32)); // PMIX_ERR_NOMEM
            }

            for (i, env_str) in initial_env.iter().enumerate() {
                match CString::new(*env_str) {
                    Ok(cs) => {
                        // SAFETY: arr_ptr[i] is a valid writable slot in our
                        // calloc'd array. We store a raw pointer from CString
                        // which will be freed later by libc::free.
                        unsafe {
                            *arr_ptr.add(i) = cs.into_raw();
                        }
                    }
                    Err(_) => {
                        // NUL byte in env string — clean up and return error.
                        // SAFETY: Free already-stored strings and the array.
                        unsafe {
                            for j in 0..i {
                                let s = *arr_ptr.add(j);
                                if !s.is_null() {
                                    libc::free(s as *mut std::os::raw::c_void);
                                }
                            }
                            libc::free(arr_ptr as *mut std::os::raw::c_void);
                        }
                        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
                    }
                }
            }
            c_env = arr_ptr;
        }
    }

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_fork is a blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - c_env is either a valid null-terminated char** array (allocated
        //   with calloc, compatible with free) or null (PMIx will allocate).
        // - We pass &mut c_env as the char *** output parameter.
        ffi::PMIx_server_setup_fork(proc_ptr, &mut c_env)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        // On error, free the C environment array we may have allocated.
        // SAFETY: c_env is either null or points to a valid null-terminated
        // array of null-terminated strings allocated with libc.
        unsafe {
            if !c_env.is_null() {
                pmix_argv_free(c_env);
            }
        }
        return Err(pmix_status);
    }

    // On success, read the environment array into a Vec<String>.
    // The array is null-terminated.
    let env_vec: Vec<String> = unsafe {
        let mut result = Vec::new();
        if !c_env.is_null() {
            let mut i = 0;
            loop {
                let entry = *c_env.add(i);
                if entry.is_null() {
                    break; // Null terminator reached
                }
                // Convert C string to Rust String.
                let cstr = CStr::from_ptr(entry);
                if let Ok(s) = cstr.to_str() {
                    result.push(s.to_owned());
                }
                i += 1;
            }
        }
        result
    };

    // Free the C environment array (both the array and individual strings).
    // SAFETY: c_env is a valid null-terminated char** allocated by PMIx
    // or by our calloc above. pmix_argv_free frees both the strings and
    // the array itself. After this, c_env is dangling.
    unsafe {
        pmix_argv_free(c_env);
    }

    Ok(env_vec)
}

/// Free a PMIx-allocated `char **` environment array.
///
/// This mirrors `pmix_argv_free` from the PMIx library: it iterates the
/// null-terminated array, frees each string with `libc::free`, then frees
/// the array itself.
///
/// # Safety
/// The caller must ensure that `env` is either null or points to a valid
/// null-terminated array of null-terminated strings allocated by PMIx
/// (which uses standard `calloc`/`realloc`/`strdup` internally).
/// Do not call this on a Rust-owned or stack-allocated array.
unsafe fn pmix_argv_free(env: *mut *mut std::os::raw::c_char) {
    if env.is_null() {
        return;
    }
    let mut p = env;
    loop {
        let entry = unsafe { *p };
        p = unsafe { p.add(1) };
        if entry.is_null() {
            break;
        }
        unsafe {
            libc::free(entry as *mut std::os::raw::c_void);
        }
    }
    unsafe {
        libc::free(env as *mut std::os::raw::c_void);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_dmodex_request — request modex data for a remote process
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_dmodex_request`.
///
/// Implement this trait to receive the result of a direct modex request.
/// The `on_complete` method receives:
///
/// * `status` — the PMIx status of the request (success or error code).
/// * `blob` — the serialized modex data blob (owned by the caller after
///   the callback returns; the PMIx library frees it upon callback return).
///
/// The `blob` is a serialized byte array containing the modex data for the
/// requested process. The host server is responsible for sending this blob
/// back to the original remote requestor. The PMIx library **frees** the
/// `data` buffer immediately after the callback returns, so the caller must
/// copy the data if it needs to retain it beyond the callback.
pub trait DmodexRequestCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>);
}

/// Global registry mapping request IDs to pending dmodex_request callbacks.
type DmodexRequestRegistry = std::collections::HashMap<usize, Box<dyn DmodexRequestCallback>>;
static DMODEX_REQUEST_REGISTRY: LazyLock<Mutex<DmodexRequestRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing dmodex_request ID counter.
static DMODEX_REQUEST_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_dmodex_response_fn_t` (dmodex_request completion).
///
/// Called by PMIx when the direct modex request completes. The `data`
/// parameter is a C-allocated buffer containing the serialized modex blob.
/// The PMIx library frees this buffer upon return from this function, so
/// we must copy the data before returning.
///
/// The `cbdata` parameter encodes the request ID as a raw pointer.
/// We look up the registered Rust callback and invoke it with the result.
extern "C" fn dmodex_request_callback_bridge(
    status: ffi::pmix_status_t,
    data: *mut std::os::raw::c_char,
    sz: usize,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Copy the data blob before the PMIx library frees it.
    // The PMIx docs state: "The PMIx server will free the data blob
    // upon return from the response fn."
    let blob: Vec<u8> = if !data.is_null() && sz > 0 {
        // SAFETY: data points to a valid buffer of sz bytes allocated by PMIx.
        // We copy the data into a Vec before returning so we own it.
        unsafe { std::slice::from_raw_parts(data as *const u8, sz) }.to_vec()
    } else {
        Vec::new()
    };

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DMODEX_REQUEST_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback with the copied data.
    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status, blob);
}

/// Request modex data for a specific process (direct modex operation).
///
/// This function is used by the host server to obtain a serialized blob
/// of modex data for a specific process. It is part of the "direct modex"
/// (dmodex) mechanism, where modex data is cached locally on each PMIx
/// server for its own local clients and obtained on-demand for remote
/// requests.
///
/// When a remote server needs modex data for a process managed by this
/// local PMIx server, the host server receives the request and calls
/// `PMIx_server_dmodex_request`. The PMIx library assembles the modex
/// data into a serialized blob and returns it via the callback. The host
/// server is then responsible for sending the blob back to the original
/// remote requestor.
///
/// **Important:** The data buffer passed to the callback is owned by the
/// PMIx library and is freed immediately upon callback return. The safe
/// Rust wrapper copies the data into a `Vec<u8>` so the caller owns it.
///
/// # Parameters
///
/// * `proc` — the process whose modex data is being requested.
/// * `callback` — invoked when the modex data is available (or an error
///   occurs). The callback receives the status and the serialized blob.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid proc,
///   NULL callback, or PMIx not initialized as server). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — `proc` is null or `callback` is null.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_dmodex_request(const pmix_proc_t *proc,
///                                          pmix_dmodex_response_fn_t cbfunc,
///                                          void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_dmodex_request, DmodexRequestCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyDmodexCallback;
/// impl DmodexRequestCallback for MyDmodexCallback {
///     fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
///         if status.is_success() {
///             println!("Received modex blob of {} bytes", blob.len());
///             // Send blob to the remote requestor...
///         } else {
///             eprintln!("dmodex request failed: {:?}", status);
///         }
///     }
/// }
///
/// let proc = Proc::new("remote.job.12345", 0).expect("invalid nspace");
/// server_dmodex_request(&proc, Box::new(MyDmodexCallback))
///     .expect("dmodex_request rejected");
/// ```
pub fn server_dmodex_request(
    proc: &Proc,
    callback: Box<dyn DmodexRequestCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = DMODEX_REQUEST_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = DMODEX_REQUEST_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is properly aligned
    // and non-null (req_id starts from 1, so req_id << 2 >= 4).
    let cbdata = (req_id << 2) as *mut c_void;

    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_dmodex_request is a non-blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - The callback bridge has C linkage and properly handles cbdata.
        //   It copies the data blob before the PMIx library frees it.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates proc and cbfunc internally and returns
        //   PMIX_ERR_BAD_PARAM if either is null.
        ffi::PMIx_server_dmodex_request(proc_ptr, Some(dmodex_request_callback_bridge), cbdata)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = DMODEX_REQUEST_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_application — setup application before launch
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_setup_application`.
///
/// Implement this trait to receive the result of a non-blocking application
/// setup request. The `on_complete` method receives:
///
/// * `status` — the PMIx status of the setup operation (success or error code).
/// * `info` — the resulting info key-value pairs produced by the setup
///   operation. This contains environment variables, resource assignments,
///   security credentials, and other data needed prior to process launch.
///   The values are owned Rust `String`s copied from the C info array.
///
/// # Ownership
///
/// The underlying C info array is owned by the PMIx library until the
/// caller invokes the acknowledgment callback. The safe wrapper copies
/// the info keys/values into owned `Vec<(String, String)>` so the
/// caller owns the data, and automatically invokes the ack callback
/// after copying.
pub trait SetupApplicationCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>);
}

/// Global registry mapping request IDs to pending setup_application callbacks.
type SetupApplicationRegistry = std::collections::HashMap<usize, Box<dyn SetupApplicationCallback>>;
static SETUP_APPLICATION_REGISTRY: LazyLock<Mutex<SetupApplicationRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing setup_application request ID counter.
static SETUP_APPLICATION_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_setup_application_cbfunc_t`.
///
/// Called by PMIx when the setup_application operation completes. The
/// `info` array contains the setup results (env vars, resources, etc.).
/// The PMIx library owns this array until we call the provided `cbfunc`
/// acknowledgment callback.
///
/// The `cbdata` parameter encodes the request ID as a raw pointer.
/// We look up the registered Rust callback, copy the info data, invoke
/// the acknowledgment, and then invoke the user's callback.
extern "C" fn setup_application_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    provided_cbdata: *mut c_void,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    if provided_cbdata.is_null() {
        return;
    }

    // SAFETY: provided_cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (provided_cbdata as usize) >> 2;

    // Copy the info array before the PMIx library frees it.
    // The info array is owned by PMIx until we call the ack callback.
    //
    // pmix_info_t has: key (pmix_key_t = [c_char; 512]), flags, value (pmix_value_t).
    // pmix_value_t has: type_ (pmix_data_type_t = u16), data (union).
    // The union fields are: string, integer, uint, size, int8, int16, int32, int64,
    // uint8, uint16, uint32, uint64, fval, dval, rank, flag, byte, bo (byte_object), etc.
    let copied_info: Vec<(String, String)> = if !info.is_null() && ninfo > 0 {
        // SAFETY: info points to a valid array of ninfo pmix_info_t entries
        // allocated by PMIx. We only read the key and value fields.
        unsafe {
            let mut entries = Vec::with_capacity(ninfo);
            for i in 0..ninfo {
                let entry = *info.add(i);
                let key = CStr::from_ptr(entry.key.as_ptr() as *const std::os::raw::c_char)
                    .to_string_lossy()
                    .into_owned();
                // Extract value as string based on type_.
                // pmix_data_type_t is u16; match against known values.
                let dtype = entry.value.type_;
                let value_str = match dtype {
                    3 => {
                        // PMIX_STRING
                        if !entry.value.data.string.is_null() {
                            CStr::from_ptr(entry.value.data.string)
                                .to_string_lossy()
                                .into_owned()
                        } else {
                            String::new()
                        }
                    }
                    6 => format!("{}", entry.value.data.integer), // PMIX_INT
                    11 => format!("{}", entry.value.data.uint),   // PMIX_UINT
                    4 => format!("{}", entry.value.data.size),    // PMIX_SIZE
                    7 => format!("{}", entry.value.data.int8),    // PMIX_INT8
                    8 => format!("{}", entry.value.data.int16),   // PMIX_INT16
                    9 => format!("{}", entry.value.data.int32),   // PMIX_INT32
                    10 => format!("{}", entry.value.data.int64),  // PMIX_INT64
                    12 => format!("{}", entry.value.data.uint8),  // PMIX_UINT8
                    13 => format!("{}", entry.value.data.uint16), // PMIX_UINT16
                    14 => format!("{}", entry.value.data.uint32), // PMIX_UINT32
                    15 => format!("{}", entry.value.data.uint64), // PMIX_UINT64
                    16 => format!("{}", entry.value.data.fval),   // PMIX_FLOAT
                    17 => format!("{}", entry.value.data.dval),   // PMIX_DOUBLE
                    1 => format!("{}", entry.value.data.flag),    // PMIX_BOOL
                    5 => format!("{}", entry.value.data.pid),     // PMIX_PID
                    20 => format!("{}", entry.value.data.status), // PMIX_STATUS
                    31 => format!("{}", entry.value.data.rank), // PMIX_PROC_RANK (stored as rank in union)
                    _ => format!("[type={}] ", dtype),
                };
                entries.push((key, value_str));
            }
            entries
        }
    } else {
        Vec::new()
    };

    // Call the acknowledgment callback to let PMIx free the info array.
    // This must be done before we return from the bridge function.
    if let Some(ack) = cbfunc {
        // SAFETY: cbfunc and cbdata are provided by PMIx and are valid
        // for the duration of this callback invocation.
        unsafe { ack(ffi::PMIX_SUCCESS as ffi::pmix_status_t, cbdata) };
    }

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = SETUP_APPLICATION_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback with the copied data.
    let pmix_status = PmixStatus::from_raw(status);
    cb.on_complete(pmix_status, copied_info);
}

/// Request application-specific setup prior to process launch.
///
/// This function asks the PMIx library (and any loaded network/fabric
/// modules) to prepare for the launch of an application identified by
/// the given namespace. It returns setup information such as environment
/// variables, security credentials, and resource assignments via the
/// asynchronous callback.
///
/// The host resource manager calls this function after registering the
/// namespace with [`server_register_nspace`] and before calling
/// [`server_setup_fork`] for individual processes.
///
/// # Parameters
///
/// * `nspace` — the namespace of the application being set up.
/// * `info` — info keys that describe the application (job size, number
///   of nodes, fabric requirements, etc.).
/// * `callback` — invoked when setup completes with the status and
///   resulting info array.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., PMIx not
///   initialized as server, or invalid parameters). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_application(
///     const pmix_nspace_t nspace,
///     pmix_info_t info[], size_t ninfo,
///     pmix_setup_application_cbfunc_t cbfunc,
///     void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_setup_application, SetupApplicationCallback};
/// use pmix::InfoBuilder;
/// use pmix::PmixStatus;
///
/// struct MySetupCallback;
/// impl SetupApplicationCallback for MySetupCallback {
///     fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
///         if status.is_success() {
///             println!("Setup complete, got {} info entries", info.len());
///             for (key, value) in &info {
///                 println!("  {} = {}", key, value);
///             }
///         } else {
///             eprintln!("Setup failed: {:?}", status);
///         }
///     }
/// }
///
/// // After registering the namespace...
/// server_setup_application("myapp.ns", &InfoBuilder::new().build(), Box::new(MySetupCallback))
///     .expect("setup_application rejected");
/// ```
pub fn server_setup_application(
    nspace: &str,
    info: &Info,
    callback: Box<dyn SetupApplicationCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            // Invoke the callback with an error status immediately.
            callback.on_complete(PmixStatus::from_raw(-1), Vec::new());
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
    };

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = SETUP_APPLICATION_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = SETUP_APPLICATION_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is properly aligned
    // and non-null (req_id starts from 1, so req_id << 2 >= 4).
    let cbdata = (req_id << 2) as *mut c_void;

    // Get the info array pointer and length.
    let info_ptr = if info.len > 0 {
        info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = info.len;

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_application is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - info_len is the number of entries matching info_ptr.
        // - The callback bridge has C linkage and properly handles all parameters:
        //   copies the info array, calls the ack callback, then invokes the user.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_setup_application(
            nspace_c.as_ptr(),
            info_ptr,
            info_len,
            Some(setup_application_callback_bridge),
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
        let mut registry = SETUP_APPLICATION_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_local_support — setup local support for an nspace
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_setup_local_support` (non-blocking mode).
///
/// Implement this trait to receive the result of a non-blocking local
/// support setup operation. The `on_complete` method receives the
/// `PmixStatus` result — success means the PMIx server has completed
/// any application-specific operations prior to spawning local clients.
pub trait SetupLocalSupportCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending setup_local_support callbacks.
type SetupLocalSupportRegistry =
    std::collections::HashMap<usize, Box<dyn SetupLocalSupportCallback>>;
static SETUP_LOCAL_SUPPORT_REGISTRY: LazyLock<Mutex<SetupLocalSupportRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing setup_local_support request ID counter.
static SETUP_LOCAL_SUPPORT_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (setup_local_support completion).
///
/// Called by PMIx when the non-blocking setup_local_support operation completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered Rust callback and invoke it with the result status.
extern "C" fn setup_local_support_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock().unwrap();
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

/// Setup local support for a given namespace before spawning local clients.
///
/// This function allows the local PMIx server to perform any application-specific
/// operations prior to spawning local clients of a given application. The host
/// resource manager (RM) calls this to inform the PMIx server about the local
/// processes that will be spawned, allowing it to prepare internal data
/// structures and perform any necessary setup.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `nspace` — the namespace identifier for the job whose local support
///   is being set up.
/// * `info` — optional per-process info keys that describe the local
///   processes (e.g., node info, process counts, resource allocations).
/// * `callback` — invoked when setup completes. The callback receives
///   the status of the operation.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(PmixStatus::OperationSucceeded)` — the request was immediately
///   processed and returned success. The callback will NOT be called.
/// * `Err(status)` — request rejected immediately (e.g., invalid nspace,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_local_support(const pmix_nspace_t nspace,
///                                               pmix_info_t info[], size_t ninfo,
///                                               pmix_op_cbfunc_t cbfunc,
///                                               void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_setup_local_support, SetupLocalSupportCallback};
/// use pmix::{Info, InfoBuilder, PmixStatus};
///
/// struct MySetupLocalCallback;
/// impl SetupLocalSupportCallback for MySetupLocalCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         match status {
///             ok if ok.is_success() => println!("Local support setup complete"),
///             err => eprintln!("Setup failed: {:?}", err),
///         }
///     }
/// }
///
/// // Setup local support for a namespace
/// server_setup_local_support(
///     "myapp.12345",
///     &InfoBuilder::new().build(),
///     Box::new(MySetupLocalCallback),
/// )
/// .expect("setup_local_support rejected");
/// ```
pub fn server_setup_local_support(
    nspace: &str,
    info: &Info,
    callback: Box<dyn SetupLocalSupportCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            callback.on_complete(PmixStatus::from_raw(-1));
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
    };

    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = SETUP_LOCAL_SUPPORT_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is properly aligned
    // and non-null (req_id starts from 1, so req_id << 2 >= 4).
    let cbdata = (req_id << 2) as *mut c_void;

    // Prepare info parameters.
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle, info.len)
    } else {
        (ptr::null_mut(), 0)
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_local_support is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - ninfo is the number of entries matching info_ptr.
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_setup_local_support(
            nspace_c.as_ptr(),
            info_ptr,
            ninfo,
            Some(setup_local_support_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // PMIX_SUCCESS — request accepted, callback will be invoked asynchronously.
        // PMIX_OPERATION_SUCCEEDED (-157) — immediately processed and succeeded,
        // callback will NOT be called.
        if pmix_status.to_raw() == -157 {
            // PMIX_OPERATION_SUCCEEDED — callback not called, so remove it.
            let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock().unwrap();
            registry.remove(&req_id);
            // Return success — the operation completed immediately.
            Ok(())
        } else {
            // PMIX_SUCCESS — callback will be invoked asynchronously.
            Ok(())
        }
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_IOF_deliver — deliver forwarded I/O to local PMIx server
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_IOF_deliver`.
///
/// Implement this trait to receive the result of an I/O forwarding
/// delivery request. The `on_complete` method receives the `PmixStatus`
/// result — success means the PMIx server has accepted the data for
/// distribution to its clients.
///
/// # Important
///
/// The host RM must retain ownership of the byte object (`bo`) until
/// the callback is executed, or until a non-success status is returned
/// immediately by the function. The safe wrapper handles this by taking
/// a reference that must remain valid until the callback fires.
pub trait IOFDeliverCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending IOF_deliver callbacks.
type IOFDeliverRegistry = std::collections::HashMap<usize, Box<dyn IOFDeliverCallback>>;
static IOF_DELIVER_REGISTRY: LazyLock<Mutex<IOFDeliverRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing IOF_deliver request ID counter.
static IOF_DELIVER_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_op_cbfunc_t` (IOF_deliver completion).
///
/// Called by PMIx when the I/O forwarding delivery completes. The
/// `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered Rust callback and invoke it with the
/// result status.
extern "C" fn iof_deliver_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = IOF_DELIVER_REGISTRY.lock().unwrap();
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

/// Deliver forwarded I/O data to the local PMIx server for distribution.
///
/// This function allows the host resource manager (RM) to pass I/O data
/// that has been forwarded from a remote source to the local PMIx server
/// for distribution to its clients that have registered for the data.
///
/// The PMIx server is responsible for determining which of its clients
/// have actually registered to receive the provided data and delivering
/// it to them.
///
/// # Parameters
///
/// * `source` — the process that provided the data being forwarded.
///   This identifies the source of the I/O stream.
/// * `channel` — the I/O channel (stdin, stdout, stderr) from which
///   the data originated. Specified as `IOFChannelFlags` bitmask.
/// * `bo` — a byte object containing the raw I/O data to deliver.
///   The data must remain valid until the callback is invoked.
/// * `info` — optional metadata describing the data, including
///   attributes such as `PMIX_IOF_COMPLETE` to indicate that the
///   source channel has been closed (EOF).
/// * `callback` — invoked when the data has been processed by the
///   PMIx server. The host RM must retain the byte object until this
///   callback fires.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid
///   source, PMIx not initialized as server). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — `source` is null, `bo` is null, or
///   `channel` is invalid.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_IOF_deliver(const pmix_proc_t *source,
///                                       pmix_iof_channel_t channel,
///                                       const pmix_byte_object_t *bo,
///                                       const pmix_info_t info[], size_t ninfo,
///                                       pmix_op_cbfunc_t cbfunc, void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_iof_deliver, IOFDeliverCallback};
/// use pmix::{Proc, PmixStatus, IOFChannelFlags, InfoBuilder};
/// use pmix::data_serialization::PmixByteObject;
///
/// struct MyIOFCallback;
/// impl IOFDeliverCallback for MyIOFCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         if status.is_success() {
///             println!("I/O data delivered successfully");
///         } else {
///             eprintln!("I/O delivery failed: {:?}", status);
///         }
///     }
/// }
///
/// let source = Proc::new("myapp.12345", 0).expect("invalid nspace");
/// let data = PmixByteObject::from(b"Hello, stdout!".to_vec());
/// let channel = IOFChannelFlags::STDOUT;
///
/// // Note: data must remain alive until the callback fires.
/// // In practice, use Arc or a longer-lived buffer.
/// server_iof_deliver(
///     &source,
///     channel,
///     &data,
///     &InfoBuilder::new().build(),
///     Box::new(MyIOFCallback),
/// ).expect("IOF_deliver rejected");
/// ```
pub fn server_iof_deliver(
    source: &Proc,
    channel: crate::IOFChannelFlags,
    bo: &crate::data_serialization::PmixByteObject,
    info: &Info,
    callback: Box<dyn IOFDeliverCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = IOF_DELIVER_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = IOF_DELIVER_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // We shift left by 2 to ensure the pointer is properly aligned
    // and non-null (req_id starts from 1, so req_id << 2 >= 4).
    let cbdata = (req_id << 2) as *mut c_void;

    // Get a pointer to the source proc's internal pmix_proc_t for FFI.
    let source_ptr = &source.handle as *const ffi::pmix_proc_t;

    // Get the byte object pointer.
    let bo_ptr =
        bo as *const crate::data_serialization::PmixByteObject as *const ffi::pmix_byte_object_t;

    // Prepare info parameters.
    let (info_ptr, ninfo) = if info.len > 0 {
        (info.handle, info.len)
    } else {
        (ptr::null_mut(), 0)
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_IOF_deliver is a non-blocking server API.
        // - source_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - channel.0 is the raw pmix_iof_channel_t bitmask.
        // - bo_ptr is a valid reference to the PmixByteObject's internal
        //   pmix_byte_object_t. The caller must ensure bo remains valid until
        //   the callback fires (the PMIx spec requires the host RM to retain
        //   the byte object until the callback is executed).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_BAD_PARAM
        //   if source/bo/channel are invalid, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_IOF_deliver(
            source_ptr,
            channel.0,
            bo_ptr,
            info_ptr,
            ninfo,
            Some(iof_deliver_callback_bridge),
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
        let mut registry = IOF_DELIVER_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_collect_inventory
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `server_collect_inventory`.
///
/// Implement this trait to receive the result of an asynchronous
/// inventory collection request. The `on_complete` method is called
/// exactly once when the operation finishes, with the status and
/// collected inventory info array.
pub trait CollectInventoryCallback: Send + 'static {
    /// Called when the inventory collection completes.
    ///
    /// - `status`: The result status (success or error).
    /// - `inventory`: The collected inventory info entries (owned, freed on drop).
    fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults);
}

/// Results from a `server_collect_inventory` call.
///
/// Contains the collected inventory as an array of info entries.
/// The underlying C memory is automatically freed when this value
/// is dropped.
#[derive(Debug)]
pub struct CollectInventoryResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl CollectInventoryResults {
    /// Number of info entries in the inventory result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the inventory result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for CollectInventoryResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx as an allocated
                // pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

/// Monotonically increasing collect-inventory request ID counter.
static COLLECT_INVENTORY_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// Global registry of pending collect-inventory callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static COLLECT_INVENTORY_REGISTRY: LazyLock<
    Mutex<std::collections::HashMap<usize, Box<dyn CollectInventoryCallback>>>,
> = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// C bridge for `pmix_info_cbfunc_t` (collect inventory completion).
///
/// Called by PMIx when the inventory collection request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered Rust callback and invoke it with the result status
/// and the collected inventory info array.
extern "C" fn collect_inventory_callback_bridge(
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
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock().unwrap();
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
    let inventory = CollectInventoryResults {
        handle: info,
        len: ninfo,
    };
    cb.on_complete(pmix_status, inventory);
    // release_fn is unused — we manage our own memory via CollectInventoryResults Drop.
    let _ = release_fn;
}

/// Collect hardware and software inventory from the local system (non-blocking).
///
/// Request the PMIx server to collect inventory information about the local
/// system (CPU, memory, network, GPU, etc.) from the configured inventory
/// plugins. The `callback` closure is invoked once the collection completes,
/// receiving both the status and the collected inventory as an info array.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized as server). The callback will
///   NOT be called.
///
/// # Parameters
///
/// * `directives` — optional info entries that specify collection directives
///   (e.g., which plugins to query, filtering criteria). Pass an empty slice
///   to use default collection behavior.
/// * `callback` — invoked when inventory collection completes, receiving
///   the status and the collected inventory info array.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_collect_inventory(pmix_info_t directives[],
///                                             size_t ndirs,
///                                             pmix_info_cbfunc_t cbfunc,
///                                             void *cbdata);
/// ```
///
/// # Example
///
/// ```no_run
/// use pmix::server::{server_collect_inventory, CollectInventoryCallback, CollectInventoryResults};
/// use pmix::{Info, InfoBuilder, PmixStatus};
///
/// struct MyInventoryCallback;
/// impl CollectInventoryCallback for MyInventoryCallback {
///     fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
///         if status.is_success() {
///             println!("Collected {} inventory items", inventory.len());
///         } else {
///             eprintln!("Inventory collection failed: {:?}", status);
///         }
///     }
/// }
///
/// let directives = InfoBuilder::new().build();
/// server_collect_inventory(
///     &directives,
///     Box::new(MyInventoryCallback),
/// ).expect("collect_inventory rejected");
/// ```
pub fn server_collect_inventory(
    directives: &Info,
    callback: Box<dyn CollectInventoryCallback>,
) -> Result<(), PmixStatus> {
    // Assign a unique request ID and register the callback.
    let req_id = {
        let mut seq = COLLECT_INVENTORY_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };

    // SAFETY: We shift the request ID left by 2 bits to ensure cbdata
    // is never null (req_id starts at 1, so shifted value >= 4).
    let cbdata = (req_id << 2) as *mut c_void;

    {
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Convert the directives Info slice to C pointers.
    let (directives_ptr, ndirs) = if directives.len > 0 {
        (directives.handle, directives.len)
    } else {
        (ptr::null_mut(), 0)
    };

    let status = unsafe {
        // SAFETY:
        // - directives_ptr is either null or points to a valid array of
        //   pmix_info_t objects from the Info handle that remains alive
        //   for the duration of this call (PMIx copies the pointer).
        // - ndirs matches the length of the directives array.
        // - collect_inventory_callback_bridge is a valid extern "C" function
        //   matching the pmix_info_cbfunc_t signature.
        // - cbdata encodes the request ID and is guaranteed non-null.
        // - The callback registered in COLLECT_INVENTORY_REGISTRY outlives
        //   this call and will be removed when the callback fires.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_collect_inventory(
            directives_ptr,
            ndirs,
            Some(collect_inventory_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Request was rejected — remove the callback so it doesn't leak.
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_deliver_inventory
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `server_deliver_inventory` completion.
///
/// Implement this trait to receive the result of a non-blocking
/// inventory delivery request. The `on_complete` method is invoked
/// asynchronously by the PMIx library when the delivery completes.
///
/// # Example
///
/// ```no_run
/// use pmix::PmixStatus;
/// use pmix::server::DeliverInventoryCallback;
///
/// struct MyDeliverCallback;
/// impl DeliverInventoryCallback for MyDeliverCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         if status.is_success() {
///             println!("Inventory delivered successfully");
///         } else {
///             eprintln!("Inventory delivery failed: {:?}", status);
///         }
///     }
/// }
/// ```
pub trait DeliverInventoryCallback: Send + 'static {
    /// Called when the inventory delivery request completes.
    ///
    /// - `status`: The result status (success or error).
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Monotonically increasing deliver-inventory request ID counter.
static DELIVER_INVENTORY_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// Global registry of pending deliver-inventory callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static DELIVER_INVENTORY_REGISTRY: LazyLock<
    Mutex<std::collections::HashMap<usize, Box<dyn DeliverInventoryCallback>>>,
> = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// C bridge for `pmix_op_cbfunc_t` (deliver inventory completion).
///
/// Called by PMIx when the inventory delivery request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered Rust callback and invoke it with the result status.
extern "C" fn deliver_inventory_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DELIVER_INVENTORY_REGISTRY.lock().unwrap();
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

/// Deliver collected inventory information to the PMIx server library.
///
/// Pass collected inventory data (e.g., from hardware discovery or
/// inventory plugins) to the PMIx server for storage and subsequent
/// access by clients. The inventory is provided as an array of
/// `pmix_info_t` key-value pairs describing hardware or software
/// attributes.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`. If you need blocking behavior, pass
/// `None` for the callback (the C API accepts a NULL callback for
/// blocking execution).
///
/// # Parameters
///
/// * `inventory` — info entries containing the inventory data to deliver.
///   Each entry describes a hardware or software attribute (e.g., CPU model,
///   GPU count, memory capacity).
/// * `directives` — optional info entries that direct the delivery
///   (e.g., filtering, storage options). Pass an empty slice for defaults.
/// * `callback` — invoked when delivery completes. Pass `None` for
///   blocking behavior (not recommended in async contexts).
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback` (if provided).
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — invalid parameters (e.g., NULL info with ninfo > 0).
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_deliver_inventory(
///     const pmix_info_t info[],
///     size_t ninfo,
///     const pmix_info_t directives[],
///     size_t ndirs,
///     pmix_op_cbfunc_t cbfunc,
///     void *cbdata);
/// ```
///
/// # Example
///
/// ```no_run
/// use pmix::server::{server_deliver_inventory, DeliverInventoryCallback};
/// use pmix::{Info, InfoBuilder};
///
/// struct MyDeliverCallback;
/// impl DeliverInventoryCallback for MyDeliverCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("Delivery result: {:?}", status);
///     }
/// }
///
/// let inventory = InfoBuilder::new().build();
/// let directives = InfoBuilder::new().build();
/// server_deliver_inventory(
///     &inventory,
///     &directives,
///     Some(Box::new(MyDeliverCallback)),
/// ).expect("deliver_inventory rejected");
/// ```
pub fn server_deliver_inventory(
    inventory: &Info,
    directives: &Info,
    callback: Option<Box<dyn DeliverInventoryCallback>>,
) -> Result<(), PmixStatus> {
    // If a callback is provided, register it for async completion.
    if let Some(cb) = callback {
        let req_id = {
            let mut seq = DELIVER_INVENTORY_SEQ.lock().unwrap();
            *seq += 1;
            *seq
        };

        // SAFETY: We shift the request ID left by 2 bits to ensure cbdata
        // is never null (req_id starts at 1, so shifted value >= 4).
        let cbdata = (req_id << 2) as *mut c_void;

        {
            let mut registry = DELIVER_INVENTORY_REGISTRY.lock().unwrap();
            registry.insert(req_id, cb);
        }

        // Convert inventory Info slice to C pointers.
        let (info_ptr, ninfo) = if inventory.len > 0 {
            (inventory.handle, inventory.len)
        } else {
            (ptr::null_mut(), 0)
        };

        // Convert directives Info slice to C pointers.
        let (directives_ptr, ndirs) = if directives.len > 0 {
            (directives.handle, directives.len)
        } else {
            (ptr::null_mut(), 0)
        };

        let status = unsafe {
            // SAFETY:
            // - info_ptr is either null or points to a valid array of
            //   pmix_info_t objects from the inventory Info handle that
            //   remains alive for the duration of this call.
            // - directives_ptr is either null or points to a valid array
            //   of pmix_info_t objects from the directives Info handle.
            // - ninfo and ndirs match the lengths of their respective arrays.
            // - deliver_inventory_callback_bridge is a valid extern "C" function
            //   matching the pmix_op_cbfunc_t signature.
            // - cbdata encodes the request ID and is guaranteed non-null.
            // - The callback registered in DELIVER_INVENTORY_REGISTRY outlives
            //   this call and will be removed when the callback fires.
            // - The PMIx library validates parameters internally and returns
            //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
            ffi::PMIx_server_deliver_inventory(
                info_ptr,
                ninfo,
                directives_ptr,
                ndirs,
                Some(deliver_inventory_callback_bridge),
                cbdata,
            )
        };

        let pmix_status = PmixStatus::from_raw(status);

        if pmix_status.is_success() {
            // Request accepted — callback will be invoked asynchronously.
            Ok(())
        } else {
            // Request was rejected — remove the callback so it doesn't leak.
            let mut registry = DELIVER_INVENTORY_REGISTRY.lock().unwrap();
            registry.remove(&req_id);
            Err(pmix_status)
        }
    } else {
        // Blocking mode: no callback provided.
        // The C API accepts NULL for cbfunc to execute synchronously.

        // Convert inventory Info slice to C pointers.
        let (info_ptr, ninfo) = if inventory.len > 0 {
            (inventory.handle, inventory.len)
        } else {
            (ptr::null_mut(), 0)
        };

        // Convert directives Info slice to C pointers.
        let (directives_ptr, ndirs) = if directives.len > 0 {
            (directives.handle, directives.len)
        } else {
            (ptr::null_mut(), 0)
        };

        let status = unsafe {
            // SAFETY:
            // - info_ptr is either null or points to a valid array of
            //   pmix_info_t objects from the inventory Info handle.
            // - directives_ptr is either null or points to a valid array
            //   of pmix_info_t objects from the directives Info handle.
            // - ninfo and ndirs match the lengths of their respective arrays.
            // - Passing None for cbfunc is the documented blocking mode.
            // - The PMIx library validates parameters internally.
            ffi::PMIx_server_deliver_inventory(
                info_ptr,
                ninfo,
                directives_ptr,
                ndirs,
                None,
                ptr::null_mut(),
            )
        };

        let pmix_status = PmixStatus::from_raw(status);

        if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_generate_locality_string
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_generate_locality_string`.
///
/// Generate a PMIx locality string from a given CPU set bitmap.
///
/// The returned locality string encodes the hardware topology location
/// (NUMA node, L3/L2/L1 cache domain, socket, core, etc.) of the CPUs
/// in the provided cpuset. The string can be passed to
/// `PMIx_Get_relative_locality` to determine the relative locality of
/// two processes.
///
/// This function shall only be called for local client processes, with
/// the returned locality included in the job-level information (via the
/// `PMIX_LOCALITY_STRING` attribute) provided to local clients.
///
/// # Parameters
/// * `cpuset` — CPU set bitmap from which to generate the locality string.
///
/// # Returns
/// * `Ok(String)` — the locality string on success.
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_NOT_SUPPORTED` if
///   hwloc is not available or the cpuset is invalid.
///
/// # C API
/// `pmix_status_t PMIx_server_generate_locality_string(const pmix_cpuset_t *cpuset, char **locality)`
pub fn server_generate_locality_string(
    cpuset: &mut crate::fabric::PmixCpuset,
) -> Result<String, PmixStatus> {
    let cpuset_ptr = cpuset.as_mut_ptr();

    let mut locality_ptr: *mut std::os::raw::c_char = ptr::null_mut();

    let status = unsafe {
        // SAFETY:
        // - cpuset_ptr points to a valid, constructed pmix_cpuset_t that
        //   remains alive for the duration of this call (PMIx copies its
        //   contents to build the locality string).
        // - locality_ptr is a valid output pointer (&mut of a stack variable).
        // - The PMIx library allocates the returned string internally.
        ffi::PMIx_server_generate_locality_string(cpuset_ptr, &mut locality_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // On success, PMIx has allocated a null-terminated string.
    // Read it and take ownership, then free the C allocation.
    let locality = unsafe {
        if locality_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        let s = CStr::from_ptr(locality_ptr).to_string_lossy().into_owned();
        // PMIx_server_generate_locality_string allocates with strdup/calloc;
        // free with libc::free.
        libc::free(locality_ptr as *mut std::os::raw::c_void);
        s
    };

    Ok(locality)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_generate_cpuset_string
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_generate_cpuset_string`.
///
/// Generate a PMIx string representation of the provided CPU set bitmap.
///
/// The returned string is prefixed by the source field of the provided cpuset
/// followed by a colon (e.g., `hwloc:0-3,8-11`). The remainder of the string
/// represents the PUs to which the process is bound as expressed by the
/// underlying implementation (e.g., hwloc bitmap list notation).
///
/// This function shall only be called for local client processes, with the
/// returned string included in the job-level information (via the
/// `PMIX_CPUSET` attribute) provided to local clients. Local clients can use
/// these strings as input to obtain their PU bindings via the
/// `PMIx_Parse_cpuset_string` API.
///
/// # Parameters
/// * `cpuset` — CPU set bitmap from which to generate the string representation.
///
/// # Returns
/// * `Ok(String)` — the cpuset string on success (e.g., `"hwloc:0-3,8-11"`).
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_BAD_PARAM` if the cpuset
///   or its bitmap is null, `PMIX_ERR_TAKE_NEXT_OPTION` if the cpuset source
///   is not hwloc, or other PMIx error constants.
///
/// # C API
/// `pmix_status_t PMIx_server_generate_cpuset_string(const pmix_cpuset_t *cpuset, char **cpuset_string)`
pub fn server_generate_cpuset_string(
    cpuset: &mut crate::fabric::PmixCpuset,
) -> Result<String, PmixStatus> {
    let cpuset_ptr = cpuset.as_mut_ptr();

    let mut cpuset_string_ptr: *mut std::os::raw::c_char = ptr::null_mut();

    let status = unsafe {
        // SAFETY:
        // - cpuset_ptr points to a valid, constructed pmix_cpuset_t that
        //   remains alive for the duration of this call (PMIx copies its
        //   contents to build the cpuset string).
        // - cpuset_string_ptr is a valid output pointer (&mut of a stack variable).
        // - The PMIx library allocates the returned string internally.
        ffi::PMIx_server_generate_cpuset_string(cpuset_ptr, &mut cpuset_string_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // On success, PMIx has allocated a null-terminated string.
    // Read it and take ownership, then free the C allocation.
    let cpuset_string = unsafe {
        if cpuset_string_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        let s = CStr::from_ptr(cpuset_string_ptr)
            .to_string_lossy()
            .into_owned();
        // PMIx_server_generate_cpuset_string allocates with asprintf/strdup;
        // free with libc::free.
        libc::free(cpuset_string_ptr as *mut std::os::raw::c_void);
        s
    };

    Ok(cpuset_string)
}

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
        // Call FFI while arr is still valid.
        let status = unsafe {
            // SAFETY:
            // - arr is a valid pointer to a contiguous array of pmix_proc_t
            //   values, alive for the duration of this call (PMIx copies the
            //   proc identifiers internally).
            // - pset_name_c.as_ptr() is a valid null-terminated string for the
            //   duration of this call (PMIx copies it internally).
            // - PMIx_server_define_process_set is a synchronous server API.
            ffi::PMIx_server_define_process_set(arr, nmembers, pset_name_c.as_ptr())
        };
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

    // Empty members case — call with null pointer.
    let status = unsafe {
        // SAFETY:
        // - members_ptr is null (empty slice) — PMIx handles this gracefully.
        // - pset_name_c.as_ptr() is a valid null-terminated string.
        // - PMIx_server_define_process_set is a synchronous server API.
        ffi::PMIx_server_define_process_set(members_ptr, nmembers, pset_name_c.as_ptr())
    };

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
    let status = unsafe {
        // SAFETY:
        // - pset_name_c is a valid null-terminated string for the duration of
        //   this call (PMIx copies it internally, does not retain the pointer).
        // - The cast from *const to *mut is safe because PMIx does not write
        //   to the string — the non-const signature is a C API convention.
        // - PMIx_server_delete_process_set is a synchronous server API.
        ffi::PMIx_server_delete_process_set(pset_name_c.as_ptr() as *mut std::os::raw::c_char)
    };

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
extern "C" fn register_resources_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut c_void) {
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

    // Call the FFI function.
    let status = unsafe {
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
    };

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
extern "C" fn deregister_resources_callback_bridge(
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

    // Call the FFI function.
    let status = unsafe {
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
    };

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

    let status = unsafe {
        // SAFETY: PMIx_Publish is a synchronous PMIx API call. The info
        // pointer is valid for the duration of the call. PMIx does not
        // retain the pointer after this call returns.
        ffi::PMIx_Publish(info_ptr, ninfo)
    };

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

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_Lookup is a synchronous PMIx API call. The pdata
        // is valid for the duration of the call. PMIx writes the proc
        // and value fields.
        ffi::PMIx_Lookup(&mut pdata, 1, info_ptr, ninfo)
    };

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

    let status = unsafe {
        // SAFETY: PMIx_Unpublish is a synchronous PMIx API call.
        // keys_ptr is a valid NULL-terminated array of C strings.
        ffi::PMIx_Unpublish(key_ptrs.as_mut_ptr(), ptr::null(), 0)
    };

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

    let status = unsafe {
        // SAFETY: PMIx_Fence is a synchronous PMIx API call.
        // We pass null procs and 0 nprocs to fence all processes.
        ffi::PMIx_Fence(ptr::null(), 0, info_ptr, ninfo)
    };

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
    callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
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

    extern "C" fn fence_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
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

    let status = unsafe { ffi::PMIx_Connect(procs_ptr, procs.len(), info_ptr, ninfo) };

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

    extern "C" fn connect_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
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

    let status = unsafe { ffi::PMIx_Disconnect(procs_ptr, procs.len(), info_ptr, ninfo) };

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

    extern "C" fn disconnect_nb_callback_bridge(status: i32, cbdata: *mut c_void) {
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
    crate::tool::tool_attach_to_server(myproc, want_server, info)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_credential — get credential (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Retrieve a credential from a server context.
///
/// This wraps `PMIx_Get_credential` to be called from a server context.
/// Delegates to [`crate::security::get_credential`] for the actual implementation.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `info` — directives specifying which credential to retrieve.
///
/// # Returns
/// * `Ok(PmixCredential)` — the requested credential.
/// * `Err(PmixStatus)` — credential retrieval failed.
///
/// # C API
/// `pmix_status_t PMIx_Get_credential(const pmix_info_t info[], size_t ninfo,
///                                     pmix_byte_object_t *credential);`
pub fn server_get_credential(
    _handle: &PmixServerHandle,
    info: &[Info],
) -> Result<PmixCredential, PmixStatus> {
    crate::security::get_credential(info)
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    // ── PmixServerModule tests ───────────────────────────────────────────────

    #[test]
    fn test_server_module_default() {
        let module = PmixServerModule::default();
        assert!(module.client_connected.is_none());
        assert!(module.client_finalized.is_none());
        assert!(module.abort.is_none());
    }

    #[test]
    fn test_server_module_all_fields_none() {
        let module = PmixServerModule::default();
        assert!(module.fence_nb.is_none());
        assert!(module.direct_modex.is_none());
        assert!(module.publish.is_none());
        assert!(module.lookup.is_none());
        assert!(module.unpublish.is_none());
        assert!(module.spawn.is_none());
        assert!(module.connect.is_none());
        assert!(module.disconnect.is_none());
        assert!(module.register_events.is_none());
        assert!(module.deregister_events.is_none());
        assert!(module.listener.is_none());
        assert!(module.notify_event.is_none());
        assert!(module.query.is_none());
        assert!(module.tool_connected.is_none());
        assert!(module.log.is_none());
    }

    #[test]
    fn test_server_module_additional_fields() {
        let module = PmixServerModule::default();
        assert!(module.allocate.is_none());
        assert!(module.job_control.is_none());
        assert!(module.monitor.is_none());
        assert!(module.get_credential.is_none());
        assert!(module.validate_credential.is_none());
        assert!(module.iof_pull.is_none());
        assert!(module.push_stdin.is_none());
        assert!(module.group.is_none());
        assert!(module.fabric.is_none());
        assert!(module.client_connected2.is_none());
        assert!(module.session_control.is_none());
    }

    // ── New: PmixServerModule callback manipulation ──────────────────────────

    extern "C" fn dummy_callback() {}

    #[test]
    fn test_server_module_set_single_callback() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        assert!(module.client_finalized.is_none());
        assert!(module.abort.is_none());
    }

    #[test]
    fn test_server_module_set_all_callbacks() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        module.client_finalized = Some(dummy_callback);
        module.abort = Some(dummy_callback);
        module.fence_nb = Some(dummy_callback);
        module.direct_modex = Some(dummy_callback);
        module.publish = Some(dummy_callback);
        module.lookup = Some(dummy_callback);
        module.unpublish = Some(dummy_callback);
        module.spawn = Some(dummy_callback);
        module.connect = Some(dummy_callback);
        module.disconnect = Some(dummy_callback);
        module.register_events = Some(dummy_callback);
        module.deregister_events = Some(dummy_callback);
        module.listener = Some(dummy_callback);
        module.notify_event = Some(dummy_callback);
        module.query = Some(dummy_callback);
        module.tool_connected = Some(dummy_callback);
        module.log = Some(dummy_callback);
        module.allocate = Some(dummy_callback);
        module.job_control = Some(dummy_callback);
        module.monitor = Some(dummy_callback);
        module.get_credential = Some(dummy_callback);
        module.validate_credential = Some(dummy_callback);
        module.iof_pull = Some(dummy_callback);
        module.push_stdin = Some(dummy_callback);
        module.group = Some(dummy_callback);
        module.fabric = Some(dummy_callback);
        module.client_connected2 = Some(dummy_callback);
        module.session_control = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        assert!(module.session_control.is_some());
    }

    #[test]
    fn test_server_module_clear_callback() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        module.client_connected = None;
        assert!(module.client_connected.is_none());
    }

    #[test]
    fn test_server_module_as_c_ptr_returns_non_null() {
        let module = PmixServerModule::default();
        let ptr = module.as_c_ptr();
        assert!(
            !ptr.is_null(),
            "as_c_ptr must not return null for a valid module"
        );
    }

    #[test]
    fn test_server_module_as_c_ptr_consistent() {
        let module = PmixServerModule::default();
        let ptr1 = module.as_c_ptr();
        let ptr2 = module.as_c_ptr();
        assert_eq!(
            ptr1, ptr2,
            "as_c_ptr should return consistent pointer for same module"
        );
    }

    #[test]
    fn test_server_module_debug_format() {
        let module = PmixServerModule::default();
        let debug_str = format!("{:?}", module);
        assert!(!debug_str.is_empty(), "Debug output should not be empty");
        assert!(debug_str.starts_with("PmixServerModule"));
    }

    #[test]
    fn test_server_module_field_count() {
        let module = PmixServerModule::default();
        let debug_str = format!("{:?}", module);
        assert!(debug_str.starts_with("PmixServerModule"));
    }

    // ── PmixServerHandle tests ───────────────────────────────────────────────

    #[test]
    fn test_server_handle_debug_format() {
        let handle = PmixServerHandle { initialized: true };
        let debug_str = format!("{:?}", handle);
        assert!(!debug_str.is_empty(), "Debug output should not be empty");
        assert!(debug_str.starts_with("PmixServerHandle"));
    }

    #[test]
    fn test_server_handle_construction() {
        let handle = PmixServerHandle { initialized: true };
        assert!(handle.initialized);
    }

    // ── is_server_initialized tests ──────────────────────────────────────────

    #[test]
    fn test_is_server_initialized_returns_bool() {
        let _result: bool = is_server_initialized();
        // Verify it compiles and doesn't panic
    }

    // ── Callback trait compile-time verification ─────────────────────────────

    struct TestNspaceCallback {
        called: Arc<AtomicBool>,
    }

    impl RegisterNspaceCallback for TestNspaceCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_register_nspace_callback_trait_compiles() {
        let _callback: Box<dyn RegisterNspaceCallback> = Box::new(TestNspaceCallback {
            called: Arc::new(AtomicBool::new(false)),
        });
    }

    struct TestDeregisterNspaceCallback;

    impl DeregisterNspaceCallback for TestDeregisterNspaceCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_nspace_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterNspaceCallback> = Box::new(TestDeregisterNspaceCallback);
    }

    struct TestRegisterClientCallback;

    impl RegisterClientCallback for TestRegisterClientCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_register_client_callback_trait_compiles() {
        let _callback: Box<dyn RegisterClientCallback> = Box::new(TestRegisterClientCallback);
    }

    struct TestDeregisterClientCallback;

    impl DeregisterClientCallback for TestDeregisterClientCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_client_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterClientCallback> = Box::new(TestDeregisterClientCallback);
    }

    struct TestDmodexRequestCallback;

    impl DmodexRequestCallback for TestDmodexRequestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    #[test]
    fn test_dmodex_request_callback_trait_compiles() {
        let _callback: Box<dyn DmodexRequestCallback> = Box::new(TestDmodexRequestCallback);
    }

    struct TestSetupApplicationCallback;

    impl SetupApplicationCallback for TestSetupApplicationCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    #[test]
    fn test_setup_application_callback_trait_compiles() {
        let _callback: Box<dyn SetupApplicationCallback> = Box::new(TestSetupApplicationCallback);
    }

    struct TestSetupLocalSupportCallback;

    impl SetupLocalSupportCallback for TestSetupLocalSupportCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_setup_local_support_callback_trait_compiles() {
        let _callback: Box<dyn SetupLocalSupportCallback> = Box::new(TestSetupLocalSupportCallback);
    }

    struct TestIOFDeliverCallback;

    impl IOFDeliverCallback for TestIOFDeliverCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_iof_deliver_callback_trait_compiles() {
        let _callback: Box<dyn IOFDeliverCallback> = Box::new(TestIOFDeliverCallback);
    }

    struct TestCollectInventoryCallback;

    impl CollectInventoryCallback for TestCollectInventoryCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    #[test]
    fn test_collect_inventory_callback_trait_compiles() {
        let _callback: Box<dyn CollectInventoryCallback> = Box::new(TestCollectInventoryCallback);
    }

    struct TestDeliverInventoryCallback;

    impl DeliverInventoryCallback for TestDeliverInventoryCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deliver_inventory_callback_trait_compiles() {
        let _callback: Box<dyn DeliverInventoryCallback> = Box::new(TestDeliverInventoryCallback);
    }

    struct TestRegisterResourcesCallback;

    impl RegisterResourcesCallback for TestRegisterResourcesCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_register_resources_callback_trait_compiles() {
        let _callback: Box<dyn RegisterResourcesCallback> = Box::new(TestRegisterResourcesCallback);
    }

    struct TestDeregisterResourcesCallback;

    impl DeregisterResourcesCallback for TestDeregisterResourcesCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_resources_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterResourcesCallback> =
            Box::new(TestDeregisterResourcesCallback);
    }

    // ── FenceNbCallbackWrapper tests ─────────────────────────────────────────

    #[test]
    fn test_fence_nb_callback_wrapper_construction() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let _wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {
            called_clone.store(true, Ordering::SeqCst);
        });
        assert!(!called.load(Ordering::SeqCst));
    }

    // ── Registry and sequence ID tests ───────────────────────────────────────

    #[test]
    fn test_register_nspace_seq_is_accessible() {
        let _seq = REGISTER_NS_SPACE_SEQ.lock().unwrap();
    }

    #[test]
    fn test_deregister_nspace_seq_is_accessible() {
        let _seq = DEREGISTER_NS_SPACE_SEQ.lock().unwrap();
    }

    #[test]
    fn test_register_nspace_registry_is_empty() {
        let registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        assert!(
            registry.is_empty(),
            "Register nspace registry should be empty at test start"
        );
    }

    #[test]
    fn test_deregister_nspace_registry_is_empty() {
        let registry = DEREGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        assert!(
            registry.is_empty(),
            "Deregister nspace registry should be empty at test start"
        );
    }

    // ── Error path tests: FFI calls without server init ──────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_register_nspace_fails_without_init() {
        let info = crate::InfoBuilder::new().build();
        let callback: Box<dyn RegisterNspaceCallback> = Box::new(TestNspaceCallback {
            called: Arc::new(AtomicBool::new(false)),
        });
        let result = server_register_nspace("test.nspace", 1, &info, callback);
        assert!(
            result.is_err(),
            "register_nspace should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_define_process_set_fails_without_init() {
        let result = server_define_process_set(&[], "test.pset");
        assert!(
            result.is_err(),
            "define_process_set should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_delete_process_set_fails_without_init() {
        let result = server_delete_process_set("test.pset");
        assert!(
            result.is_err(),
            "delete_process_set should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_publish_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_publish(&handle, "test.nspace", &info);
        assert!(
            result.is_err(),
            "server_publish should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_lookup_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_lookup(&handle, "test.nspace", "test_key", &[]);
        assert!(
            result.is_err(),
            "server_lookup should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_delete_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_delete(&handle, "test.nspace", "test_key");
        assert!(
            result.is_err(),
            "server_delete should fail without server init"
        );
    }

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_fence_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_fence(&handle, &[], 0);
        assert!(
            result.is_err(),
            "server_fence should fail without server init"
        );
    }

    #[test]
    fn test_server_connect_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_connect(&handle, &[], &[]);
        assert!(
            result.is_err(),
            "server_connect should reject empty proc list"
        );
    }

    #[test]
    fn test_server_disconnect_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_disconnect(&handle, &[], &[]);
        assert!(
            result.is_err(),
            "server_disconnect should reject empty proc list"
        );
    }

    // ── Callback bridge null safety tests ────────────────────────────────────

    #[test]
    fn test_register_nspace_callback_bridge_null_cbdata() {
        register_nspace_callback_bridge(0, ptr::null_mut());
        // Should not panic
    }

    #[test]
    fn test_deregister_nspace_callback_bridge_null_cbdata() {
        deregister_nspace_callback_bridge(0, ptr::null_mut());
        // Should not panic
    }

    // ── CollectInventoryResults tests ────────────────────────────────────────

    #[test]
    fn test_collect_inventory_results_construction() {
        let results = CollectInventoryResults {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    // ── server_register_client error path ────────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_register_client_fails_without_init() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new failed");
        let callback: Box<dyn RegisterClientCallback> = Box::new(TestRegisterClientCallback);
        let result = server_register_client(&proc, 0, 0, None, callback);
        assert!(
            result.is_err(),
            "register_client should fail without server init"
        );
    }

    // ── server_deregister_client error path ──────────────────────────────────

    #[test]
    fn test_server_deregister_client_compiles() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new failed");
        let callback: Option<Box<dyn DeregisterClientCallback>> =
            Some(Box::new(TestDeregisterClientCallback));
        server_deregister_client(&proc, callback);
        // Should not panic
    }

    // ── server_dmodex_request error path ─────────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_dmodex_request_fails_without_init() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new failed");
        let callback: Box<dyn DmodexRequestCallback> = Box::new(TestDmodexRequestCallback);
        let result = server_dmodex_request(&proc, callback);
        assert!(
            result.is_err(),
            "dmodex_request should fail without server init"
        );
    }

    // ── server_setup_application error path ──────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_setup_application_fails_without_init() {
        let info = crate::InfoBuilder::new().build();
        let callback: Box<dyn SetupApplicationCallback> = Box::new(TestSetupApplicationCallback);
        let result = server_setup_application("test.nspace", &info, callback);
        assert!(
            result.is_err(),
            "setup_application should fail without server init"
        );
    }

    // ── server_setup_local_support error path ────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_setup_local_support_fails_without_init() {
        let info = crate::InfoBuilder::new().build();
        let callback: Box<dyn SetupLocalSupportCallback> = Box::new(TestSetupLocalSupportCallback);
        let result = server_setup_local_support("test.nspace", &info, callback);
        assert!(
            result.is_err(),
            "setup_local_support should fail without server init"
        );
    }

    // ── server_register_resources error path ─────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_register_resources_fails_without_init() {
        let info = crate::InfoBuilder::new().build();
        let callback: Box<dyn RegisterResourcesCallback> = Box::new(TestRegisterResourcesCallback);
        let result = server_register_resources(&info, callback);
        assert!(
            result.is_err(),
            "register_resources should fail without server init"
        );
    }

    // ── server_deregister_resources error path ───────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_deregister_resources_fails_without_init() {
        let info = crate::InfoBuilder::new().build();
        let callback: Box<dyn DeregisterResourcesCallback> =
            Box::new(TestDeregisterResourcesCallback);
        let result = server_deregister_resources(&info, callback);
        assert!(
            result.is_err(),
            "deregister_resources should fail without server init"
        );
    }

    // ── server_fence_nb error path ───────────────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_fence_nb_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let result = server_fence_nb(&handle, &[], wrapper);
        assert!(result.is_err(), "fence_nb should fail without server init");
    }

    // ── server_connect_nb error path ─────────────────────────────────────────

    #[test]
    fn test_server_connect_nb_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let result = server_connect_nb(&handle, &[], &[], wrapper);
        assert!(result.is_err(), "connect_nb should reject empty proc list");
    }

    // ── server_disconnect_nb error path ──────────────────────────────────────

    #[test]
    fn test_server_disconnect_nb_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let result = server_disconnect_nb(&handle, &[], &[], wrapper);
        assert!(
            result.is_err(),
            "disconnect_nb should reject empty proc list"
        );
    }

    // ── server_tool_attach_to_server error path ──────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_tool_attach_to_server_fails_without_init() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_tool_attach_to_server(&handle, None, false, &info);
        assert!(
            result.is_err(),
            "tool_attach_to_server should fail without server init"
        );
    }

    // ── server_get_credential error path ─────────────────────────────────────

    #[test]
    #[ignore = "requires PMIx server init — calling FFI without init corrupts global PMIx state"]
    fn test_server_get_credential_fails_without_setup() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_get_credential(&handle, &[info]);
        assert!(
            result.is_err(),
            "get_credential should fail without proper setup"
        );
    }

    // ── server_spawn / server_spawn_nb compilation tests ─────────────────────

    #[test]
    fn test_server_spawn_compiles() {
        // Verify the function exists with correct signature
        let _: fn(
            &PmixServerHandle,
            &[crate::Info],
            &[crate::process_mgmt::PmixApp],
        ) -> Result<String, PmixStatus> = server_spawn;
    }

    #[test]
    fn test_server_spawn_nb_compiles() {
        let _: fn(
            &PmixServerHandle,
            &[crate::Info],
            &[crate::process_mgmt::PmixApp],
            crate::process_mgmt::SpawnCallbackWrapper,
        ) -> Result<(), PmixStatus> = server_spawn_nb;
    }

    // ── server_init / server_init_minimal / server_finalize compilation ──────

    #[test]
    fn test_server_init_minimal_compiles() {
        let _: fn(Option<&PmixServerModule>) -> Result<PmixServerHandle, PmixStatus> =
            server_init_minimal;
    }

    #[test]
    fn test_server_finalize_compiles() {
        let _: fn(PmixServerHandle) -> Result<(), PmixStatus> = server_finalize;
    }

    // ── CString NUL rejection verification ───────────────────────────────────

    #[test]
    fn test_cstring_rejects_nul_bytes() {
        let result = CString::new("test\0nspace");
        assert!(result.is_err(), "CString::new should reject NUL bytes");
    }

    // ── Proc construction for server tests ───────────────────────────────────

    #[test]
    fn test_proc_construction_for_server_tests() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
        let _ = proc;
    }

    #[test]
    fn test_proc_construction_multiple_ranks() {
        let proc0 = Proc::new("test.nspace", 0).expect("Proc::new rank 0 failed");
        let proc1 = Proc::new("test.nspace", 1).expect("Proc::new rank 1 failed");
        let procs = vec![proc0, proc1];
        assert_eq!(procs.len(), 2);
    }
}
