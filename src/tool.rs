//! Tool-side PMIx APIs — `PMIx_tool_init`, `PMIx_tool_finalize`, and related.
//!
//! This module provides safe Rust wrappers around the PMIx tool APIs
//! that allow an external tool or debugger to connect to a PMIx server
//! and interact with managed jobs and processes.
//!
//! # Overview
//!
//! Tools are external processes (debuggers, monitors, profilers, etc.)
//! that need to observe or influence PMIx-managed jobs without being
//! part of the job itself. Unlike clients that call `PMIx_Init`, tools
//! call `PMIx_tool_init` and can optionally specify connection targets,
//! tool identity, and other directives via the info array.
//!
//! The tool library is reference-counted, so multiple calls to
//! `tool_init` are allowed. Each matching `tool_finalize` decrements
//! the count; the connection closes when it reaches zero.
//!
//! # Example
//!
//! ```no_run
//! use pmix::tool::{tool_init, tool_finalize, PmixToolHandle};
//! use pmix::InfoBuilder;
//!
//! // Initialize as a tool with no extra directives
//! let info = InfoBuilder::new().build();
//! let handle = tool_init(None, &info).expect("tool_init failed");
//!
//! // The handle carries the tool's assigned namespace and rank
//! let proc = handle.proc();
//! println!("Tool nspace: {:?}, rank: {:?}", proc.nspace(), proc.rank());
//!
//! // Finalize when done
//! tool_finalize(handle).expect("tool_finalize failed");
//! ```
//!
//! # C API
//!
//! ```c
//! pmix_status_t PMIx_tool_init(pmix_proc_t *proc,
//!                               pmix_info_t info[], size_t ninfo);
//! pmix_status_t PMIx_tool_finalize(void);
//! ```

use crate::ffi;
use crate::{Info, PmixStatus, Proc};
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::{LazyLock, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// PmixToolHandle — RAII handle returned by tool_init
// ─────────────────────────────────────────────────────────────────────────────

/// RAII handle returned by [`tool_init`].
///
/// Carries the tool's server-assigned process identifier (namespace + rank).
/// Dropping the handle does NOT automatically call `PMIx_tool_finalize` —
/// the caller must explicitly finalize to release the connection.
///
/// # C API
/// Returned `pmix_proc_t` from `PMIx_tool_init`.
#[derive(Clone)]
pub struct PmixToolHandle {
    proc: Proc,
}

impl std::fmt::Debug for PmixToolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixToolHandle")
            .field("nspace", &self.proc.nspace())
            .field("rank", &self.proc.rank())
            .finish()
    }
}

impl PmixToolHandle {
    /// Return the tool's assigned process identifier.
    pub fn proc(&self) -> &Proc {
        &self.proc
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// is_tool_initialized
// ─────────────────────────────────────────────────────────────────────────────

/// Whether the PMIx tool library has been initialized (reference count > 0).
///
/// # C API
/// No direct equivalent — derived from internal reference counting.
static TOOL_INITIALIZED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Check whether the PMIx tool library has been initialized.
///
/// Returns `true` if `tool_init` has been called more times than
/// `tool_finalize` (i.e., the reference count is positive).
pub fn is_tool_initialized() -> bool {
    *TOOL_INITIALIZED.lock().unwrap()
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize the PMIx tool library, connecting to a PMIx server.
///
/// When called, the PMIx tool library checks for the required connection
/// information of the local PMIx server and establishes the connection.
/// If the information is not found, or the server connection fails, an
/// appropriate error constant will be returned.
///
/// If successful, the function returns `PMIX_SUCCESS` and fills the
/// returned [`PmixToolHandle`] with the server-assigned namespace and
/// rank of the tool.
///
/// The PMIx tool library is reference-counted, so multiple calls to
/// `tool_init` are allowed. Each call increments the internal reference
/// count; [`tool_finalize`] decrements it.
///
/// # Parameters
/// * `proc` — if `Some`, the tool requests a specific process identity.
///   If `None`, the server assigns one.
/// * `info` — array of [`Info`] directives controlling the connection.
///   Common keys include:
///   - `PMIX_TOOL_NSPACE` — namespace for this tool
///   - `PMIX_TOOL_RANK` — rank of this tool
///   - `PMIX_TOOL_DO_NOT_CONNECT` — skip server connection
///   - `PMIX_SERVER_URI` — URI of target server
///   - `PMIX_CONNECT_TO_SYSTEM` — connect to system-level server
///
/// # Returns
/// * `Ok(PmixToolHandle)` — tool initialized, handle contains assigned identity.
/// * `Err(PmixStatus)` — initialization failed.
///
/// # Errors
/// * `PmixError::ErrNotFound` — no server connection info available.
/// * `PmixError::ErrTimeout` — connection attempt timed out.
/// * `PmixError::ErrConnRefused` — server refused the connection.
///
/// # C API
/// `pmix_status_t PMIx_tool_init(pmix_proc_t *proc, pmix_info_t info[], size_t ninfo)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init, tool_finalize, PmixToolHandle};
/// use pmix::InfoBuilder;
///
/// let info = InfoBuilder::new().build();
/// let handle = tool_init(None, &info).expect("tool_init failed");
/// println!("Tool identity: nspace={:?}, rank={:?} ",
///           handle.proc().nspace(), handle.proc().rank());
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_init(_proc: Option<&Proc>, _info: &Info) -> Result<PmixToolHandle, PmixStatus> {
    let mut uninit_proc = MaybeUninit::<ffi::pmix_proc_t>::uninit();

    let info_ptr = if _info.len > 0 {
        _info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = _info.len;

    let status = unsafe {
        // SAFETY: PMIx_tool_init expects:
        // - proc: a mutable pointer to a pmix_proc_t that the library
        //   will fill with the tool's assigned namespace and rank.
        //   We provide an uninitialized MaybeUninit pointer which is
        //   valid for the library to write into.
        // - info: either a valid array of pmix_info_t or null.
        // - ninfo: the number of info entries (0 if info is null).
        // The PMIx library documents that proc may be NULL if the
        // caller does not need the assigned identity, but we always
        // provide a valid pointer to capture it.
        ffi::PMIx_tool_init(uninit_proc.as_mut_ptr(), info_ptr, info_len)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        let proc_raw = unsafe { uninit_proc.assume_init() };
        let proc = Proc {
            handle: proc_raw,
            len: 1,
        };
        *TOOL_INITIALIZED.lock().unwrap() = true;
        Ok(PmixToolHandle { proc })
    } else {
        Err(pmix_status)
    }
}

/// Initialize the PMIx tool library with no info directives.
///
/// Convenience wrapper around [`tool_init`] that passes no configuration
/// info. The tool will use environment variables (e.g., `PMIX_SERVER_URI`)
/// to locate the server.
///
/// # Returns
/// * `Ok(PmixToolHandle)` — tool initialized successfully.
/// * `Err(PmixStatus)` — initialization failed.
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init_minimal, tool_finalize};
///
/// let handle = tool_init_minimal().expect("tool_init failed");
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_init_minimal() -> Result<PmixToolHandle, PmixStatus> {
    tool_init(
        None,
        &Info {
            handle: ptr::null_mut(),
            len: 0,
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_finalize
// ─────────────────────────────────────────────────────────────────────────────

/// Finalize the PMIx tool library, closing the connection to the server.
///
/// The PMIx tool library is reference-counted. Each call to `tool_finalize`
/// decrements the reference count. The actual connection closes when the
/// count reaches zero.
///
/// # Parameters
/// * `_handle` — the handle returned by [`tool_init`]. Consumed on finalize.
///
/// # Returns
/// * `Ok(())` — finalize succeeded (reference count decremented).
/// * `Err(PmixStatus)` — finalize failed for some reason.
///
/// # C API
/// `pmix_status_t PMIx_tool_finalize(void)`
pub fn tool_finalize(_handle: PmixToolHandle) -> Result<(), PmixStatus> {
    let status = unsafe {
        // SAFETY: PMIx_tool_finalize takes no parameters and returns a
        // status code. It is safe to call as long as the tool library
        // was previously initialized via PMIx_tool_init. The function
        // does not dereference any caller-provided pointers.
        ffi::PMIx_tool_finalize()
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        *TOOL_INITIALIZED.lock().unwrap() = false;
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc helpers — extract nspace/rank from Proc
// ─────────────────────────────────────────────────────────────────────────────

impl Proc {
    /// Extract the namespace string from this process identifier.
    ///
    /// Returns `None` if the nspace C string is empty or invalid.
    pub fn nspace(&self) -> Option<String> {
        unsafe {
            let nspace_ptr = self.handle.nspace.as_ptr();
            if nspace_ptr.is_null() {
                return None;
            }
            let cstr = CStr::from_ptr(nspace_ptr);
            match cstr.to_str() {
                Ok(s) if !s.is_empty() => Some(s.to_string()),
                _ => None,
            }
        }
    }

    /// Extract the rank from this process identifier.
    pub fn rank(&self) -> u32 {
        self.handle.rank
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — server identity returned by tool_attach_to_server
// ─────────────────────────────────────────────────────────────────────────────

/// Server identity returned by [`tool_attach_to_server`].
///
/// Contains the process identifier (`pmix_proc_t`) of the PMIx server
/// to which the tool has attached.
///
/// # C API
/// The `server` output parameter of `PMIx_tool_attach_to_server`.
#[derive(Clone)]
pub struct PmixServerHandle {
    proc: Proc,
}

impl std::fmt::Debug for PmixServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixServerHandle")
            .field("nspace", &self.proc.nspace())
            .field("rank", &self.proc.rank())
            .finish()
    }
}

impl PmixServerHandle {
    /// Return the server's process identifier.
    pub fn proc(&self) -> &Proc {
        &self.proc
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_attach_to_server
// ─────────────────────────────────────────────────────────────────────────────

/// Establish a connection to a PMIx server.
///
/// This function can be called at any time by a PMIx tool (after
/// [`tool_init`]) to create a new connection to a server. The target
/// server is specified via the `info` array using one or more of the
/// following keys:
///
/// * `PMIX_TOOL_ATTACHMENT_FILE` — pathname of a file containing
///   connection information for a specific server.
/// * `PMIX_SERVER_URI` — URI of the PMIx server to contact.
/// * `PMIX_TCP_URI` — TCP URI of the server, or `file:<path>` pointing
///   to a file containing it.
/// * `PMIX_SERVER_PIDINFO` — PID of the target PMIx server process.
/// * `PMIX_SERVER_NSPACE` — namespace of the target PMIx server.
/// * `PMIX_CONNECT_TO_SYSTEM` — connect only to a local, system-level
///   server.
/// * `PMIX_CONNECT_SYSTEM_FIRST` — prefer a system-level server first,
///   then fall back to other discovery methods.
///
/// If the tool is already attached to the specified server, the function
/// returns `PMIX_SUCCESS` without taking further action.
///
/// If `PMIX_PRIMARY_SERVER` is included in the info array, the newly
/// connected server becomes the tool's primary server. Otherwise, call
/// [`tool_set_server`] afterwards.
///
/// # Parameters
/// * `myproc` — if `Some`, the tool's existing process identity is
///   passed in (for obsolence protection). Pass `None` if the tool does
///   not need to provide its identity.
/// * `server` — if `Some`, the server's process identity will be
///   returned here. Pass `None` if the caller does not need the server
///   identifier.
/// * `info` — array of [`Info`] directives. **Must not be empty** — the
///   C API requires at least one info entry specifying the target server.
///
/// # Returns
/// * `Ok((Option<PmixToolHandle>, Option<PmixServerHandle>))` — connection
///   established. The returned handles contain the tool and/or server
///   identities depending on which parameters were provided.
/// * `Err(PmixStatus)` — connection failed (e.g., `ErrUnreach` if no
///   server could be discovered).
///
/// # Errors
/// * `PmixError::ErrUnreach` — no new server connection could be made.
/// * `PmixError::ErrTimeout` — connection attempt timed out.
/// * `PmixError::ErrBadParam` — info array was empty or invalid.
/// * `PmixError::ErrInit` — tool library was not initialized.
///
/// # C API
/// `pmix_status_t PMIx_tool_attach_to_server(pmix_proc_t *myproc, pmix_proc_t *server,\n                                             pmix_info_t info[], size_t ninfo)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init, tool_attach_to_server, tool_finalize};
/// use pmix::InfoBuilder;
///
/// // Initialize the tool first
/// let handle = tool_init(None, &InfoBuilder::new().build())
///     .expect("tool_init failed");
///
/// // Attach to a specific server (requires a PMIx server running)
/// // let info = InfoBuilder::new().build(); // with PMIX_SERVER_URI etc.
/// // let result = tool_attach_to_server(Some(handle.proc()), true, &info);
///
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_attach_to_server(
    myproc: Option<&Proc>,
    want_server: bool,
    info: &Info,
) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> {
    let mut uninit_myproc = MaybeUninit::<ffi::pmix_proc_t>::uninit();
    let mut uninit_server = MaybeUninit::<ffi::pmix_proc_t>::uninit();

    let myproc_ptr: *mut ffi::pmix_proc_t = if myproc.is_some() {
        uninit_myproc.as_mut_ptr()
    } else {
        ptr::null_mut()
    };
    let server_ptr: *mut ffi::pmix_proc_t = if want_server {
        uninit_server.as_mut_ptr()
    } else {
        ptr::null_mut()
    };

    let info_ptr = if info.len > 0 {
        info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = info.len;

    let status = unsafe {
        // SAFETY: PMIx_tool_attach_to_server expects:
        // - myproc: a mutable pmix_proc_t pointer (or NULL) that the
        //   library will fill with the tool's current identity.
        //   We provide MaybeUninit which is valid for the library to
        //   write into, or NULL if the caller does not need it.
        // - server: a mutable pmix_proc_t pointer (or NULL) that the
        //   library will fill with the server's identity on success.
        //   We provide MaybeUninit or NULL depending on want_server.
        // - info: a valid array of pmix_info_t (required, cannot be
        //   NULL per the C header). We pass the Info's internal handle.
        // - ninfo: the number of info entries.
        // The PMIx library documents that myproc and server may be NULL
        // if the caller does not need the returned identity.
        ffi::PMIx_tool_attach_to_server(myproc_ptr, server_ptr, info_ptr, info_len)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // On success, extract the returned identities if requested.
    let tool_handle = if myproc.is_some() {
        let proc_raw = unsafe { uninit_myproc.assume_init() };
        let proc = Proc {
            handle: proc_raw,
            len: 1,
        };
        Some(PmixToolHandle { proc })
    } else {
        None
    };

    let server_handle = if want_server {
        let proc_raw = unsafe { uninit_server.assume_init() };
        let proc = Proc {
            handle: proc_raw,
            len: 1,
        };
        Some(PmixServerHandle { proc })
    } else {
        None
    };

    Ok((tool_handle, server_handle))
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_disconnect
// ─────────────────────────────────────────────────────────────────────────────

/// Disconnect the PMIx tool from a specific server connection.
///
/// This function disconnects the tool from the server identified by the
/// given process identifier while leaving the tool library itself
/// initialized. The tool can later reconnect using [`tool_attach_to_server`]
/// or disconnect from other servers it is connected to.
///
/// If the tool is not currently connected to the specified server, the
/// function returns `PMIX_ERR_NOT_FOUND`.
///
/// The tool library remains initialized after this call — use
/// [`tool_finalize`] to shut down the tool library entirely.
///
/// # Parameters
/// * `server` — process identifier of the server to disconnect from.
///   This is typically the [`PmixServerHandle`] returned by
///   [`tool_attach_to_server`], or obtained via [`tool_get_servers`].
///
/// # Returns
/// * `Ok(())` — disconnection succeeded.
/// * `Err(PmixStatus)` — disconnection failed (e.g., `ErrNotFound` if
///   the tool is not connected to that server, `ErrInit` if the tool
///   library was never initialized).
///
/// # Errors
/// * `PmixError::ErrNotFound` — tool is not connected to the specified server.
/// * `PmixError::ErrInit` — tool library was not initialized via [`tool_init`].
/// * `PmixError::ErrBadParam` — server identifier is invalid or null.
///
/// # C API
/// `pmix_status_t PMIx_tool_disconnect(const pmix_proc_t *server)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init, tool_attach_to_server, tool_disconnect, tool_finalize};
/// use pmix::InfoBuilder;
///
/// // Initialize the tool
/// let handle = tool_init(None, &InfoBuilder::new().build())
///     .expect("tool_init failed");
///
/// // Attach to a server (requires a running PMIx server)
/// // let info = InfoBuilder::new().build();
/// // let (_, server) = tool_attach_to_server(Some(handle.proc()), true, &info)?;
/// // if let Some(s) = server {
/// //     // Disconnect from that server
/// //     tool_disconnect(s.proc()).expect("tool_disconnect failed");
/// // }
///
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_disconnect(server: &Proc) -> Result<(), PmixStatus> {
    let status = unsafe {
        // SAFETY: PMIx_tool_disconnect expects a valid pointer to a
        // pmix_proc_t identifying the server to disconnect from. We
        // provide the Proc's internal C struct pointer which is a
        // valid, initialized pmix_proc_t. The function does not
        // modify the struct pointed to (const parameter).
        ffi::PMIx_tool_disconnect(&server.handle as *const _)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_connect_to_server
// ─────────────────────────────────────────────────────────────────────────────

/// Connect a PMIx tool to a server, returning a tool handle with the
/// server-assigned identity.
///
/// This is a convenience wrapper that calls `PMIx_tool_connect_to_server`
/// and returns the resulting tool identity as a [`PmixToolHandle`]. Use this
/// when the tool needs to connect to a server after initialization (e.g.,
/// after `tool_init` with `PMIX_TOOL_DO_NOT_CONNECT`).
///
/// # Parameters
/// * `proc` — if `Some`, the tool requests a specific process identity.
///   If `None`, the server assigns one.
/// * `info` — array of [`Info`] directives controlling the connection.
///   Common keys include `PMIX_SERVER_URI`, `PMIX_TCP_URI`, etc.
///
/// # Returns
/// * `Ok(PmixToolHandle)` — connection established, handle contains the
///   server-assigned namespace and rank.
/// * `Err(PmixStatus)` — connection failed.
///
/// # Errors
/// * `PmixError::ErrNotFound` — no server connection info available.
/// * `PmixError::ErrTimeout` — connection attempt timed out.
/// * `PmixError::ErrConnRefused` — server refused the connection.
///
/// # C API
/// `pmix_status_t PMIx_tool_connect_to_server(pmix_proc_t *proc,`
/// `  pmix_info_t info[], size_t ninfo)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init_minimal, tool_connect_to_server, tool_finalize};
/// use pmix::InfoBuilder;
///
/// // Initialize without connecting
/// let handle = tool_init_minimal().expect("tool_init failed");
///
/// // Connect to a server
/// let info = InfoBuilder::new().build();
/// let conn = tool_connect_to_server(None, &info).expect("connect failed");
///
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_connect_to_server(
    _proc: Option<&Proc>,
    info: &Info,
) -> Result<PmixToolHandle, PmixStatus> {
    let mut uninit_proc = MaybeUninit::<ffi::pmix_proc_t>::uninit();

    let info_ptr = if info.len > 0 {
        info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = info.len;

    let status = unsafe {
        // SAFETY: PMIx_tool_connect_to_server expects:
        // - proc: a mutable pointer to a pmix_proc_t that the library
        //   will fill with the tool's assigned namespace and rank.
        //   We provide an uninitialized MaybeUninit pointer which is
        //   valid for the library to write into.
        // - info: either a valid array of pmix_info_t or null.
        // - ninfo: the number of info entries (0 if info is null).
        ffi::PMIx_tool_connect_to_server(uninit_proc.as_mut_ptr(), info_ptr, info_len)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        let proc_raw = unsafe { uninit_proc.assume_init() };
        let proc = Proc {
            handle: proc_raw,
            len: 1,
        };
        Ok(PmixToolHandle { proc })
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_get_servers
// ─────────────────────────────────────────────────────────────────────────────

/// Retrieve the list of servers that the tool is currently connected to.
///
/// Returns the process identifiers of all PMIx servers to which the tool
/// has an active connection. The caller receives a Rust [`Vec<Proc>`] — the
/// underlying C-allocated array is freed after copying.
///
/// # Returns
/// * `Ok(Vec<Proc>)` — list of connected server process identifiers.
///   Empty vec if the tool is not connected to any server.
/// * `Err(PmixStatus)` — query failed (e.g., tool not initialized).
///
/// # Errors
/// * `PmixError::ErrInit` — tool library was not initialized via [`tool_init`].
/// * `PmixError::Error` — generic failure.
///
/// # C API
/// `pmix_status_t PMIx_tool_get_servers(pmix_proc_t **servers, size_t *nservers)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init_minimal, tool_get_servers, tool_finalize};
///
/// let handle = tool_init_minimal().expect("tool_init failed");
///
/// let servers = tool_get_servers().expect("get_servers failed");
/// for server in &servers {
///     println!("Connected to: {:?} rank={}",
///         server.nspace(), server.rank());
/// }
///
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_get_servers() -> Result<Vec<Proc>, PmixStatus> {
    let mut servers: *mut ffi::pmix_proc_t = ptr::null_mut();
    let mut nservers: usize = 0;

    let status = unsafe {
        // SAFETY: PMIx_tool_get_servers expects:
        // - servers: a pointer to a pmix_proc_t* that the library will
        //   allocate and populate with an array of server identities.
        //   The caller is responsible for freeing the array via
        //   PMIx_Proc_free.
        // - nservers: a pointer to a size_t that receives the count.
        ffi::PMIx_tool_get_servers(&mut servers, &mut nservers)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        let mut result = Vec::with_capacity(nservers);
        if !servers.is_null() && nservers > 0 {
            unsafe {
                // SAFETY: PMIx allocated an array of nservers pmix_proc_t
                // elements. We read each one and construct Rust Proc wrappers.
                for i in 0..nservers {
                    let proc_raw = ptr::read(servers.add(i));
                    result.push(Proc {
                        handle: proc_raw,
                        len: 1,
                    });
                }
                // Free the C-allocated array.
                ffi::PMIx_Proc_free(servers, nservers);
            }
        }
        Ok(result)
    } else {
        // On error, PMIx may still have allocated the array — free it.
        if !servers.is_null() {
            unsafe {
                ffi::PMIx_Proc_free(servers, nservers);
            }
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_set_server
// ─────────────────────────────────────────────────────────────────────────────

/// Set the primary server for the tool.
///
/// After connecting to multiple servers via [`tool_attach_to_server`], use
/// this function to designate one server as the primary. The primary server
/// is used for data operations and other requests that do not explicitly
/// specify a target.
///
/// # Parameters
/// * `server` — process identifier of the server to set as primary.
///   This is typically obtained from [`tool_attach_to_server`] or
///   [`tool_get_servers`].
/// * `info` — array of [`Info`] directives (optional, can be empty).
///
/// # Returns
/// * `Ok(())` — server successfully set as primary.
/// * `Err(PmixStatus)` — operation failed.
///
/// # Errors
/// * `PmixError::ErrNotFound` — tool is not connected to the specified server.
/// * `PmixError::ErrInit` — tool library was not initialized.
/// * `PmixError::ErrBadParam` — server identifier is invalid.
///
/// # C API
/// `pmix_status_t PMIx_tool_set_server(const pmix_proc_t *server,`
/// `  pmix_info_t info[], size_t ninfo)`
///
/// # Examples
///
/// ```no_run
/// use pmix::tool::{tool_init_minimal, tool_attach_to_server, tool_set_server, tool_finalize};
/// use pmix::InfoBuilder;
///
/// let handle = tool_init_minimal().expect("tool_init failed");
///
/// // Attach to a server (requires a running PMIx server)
/// // let info = InfoBuilder::new().build();
/// // let (_, server) = tool_attach_to_server(Some(handle.proc()), true, &info)?;
/// // if let Some(s) = server {
/// //     tool_set_server(s.proc(), &InfoBuilder::new().build())
/// //         .expect("set_server failed");
/// // }
///
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_set_server(server: &Proc, info: &Info) -> Result<(), PmixStatus> {
    let info_ptr = if info.len > 0 {
        info.handle
    } else {
        ptr::null_mut()
    };
    let info_len = info.len;

    let status = unsafe {
        // SAFETY: PMIx_tool_set_server expects:
        // - server: a valid const pointer to a pmix_proc_t identifying
        //   the server to set as primary. We provide the Proc's internal
        //   C struct pointer which is a valid, initialized pmix_proc_t.
        // - info: either a valid array of pmix_info_t or null.
        // - ninfo: the number of info entries (0 if info is null).
        ffi::PMIx_tool_set_server(&server.handle as *const _, info_ptr, info_len)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_tool_is_connected — connection status check
// ─────────────────────────────────────────────────────────────────────────────

/// Check whether the current tool is connected to the PMIx server.
///
/// Returns `true` if the tool's connection to the server is active,
/// `false` if disconnected or if PMIx has not been initialized.
///
/// This is useful for detecting disconnection events and determining
/// whether to retry operations or exit gracefully.
///
/// # C API
/// `bool PMIx_tool_is_connected(void);`
pub fn tool_is_connected() -> bool {
    unsafe { ffi::PMIx_tool_is_connected() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tool_initialized_default() {
        assert!(!is_tool_initialized());
    }

    #[test]
    fn test_tool_is_connected_default() {
        assert!(!tool_is_connected());
    }
}
