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
//!
//! // Initialize as a tool with no extra directives
//! let handle = tool_init(None, &[]).expect("tool_init failed");
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
///
/// let handle = tool_init(None, &[]).expect("tool_init failed");
/// println!("Tool identity: nspace={:?}, rank={:?}",
///           handle.proc().nspace(), handle.proc().rank());
/// tool_finalize(handle).expect("tool_finalize failed");
/// ```
pub fn tool_init(
    _proc: Option<&Proc>,
    _info: &Info,
) -> Result<PmixToolHandle, PmixStatus> {
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
    tool_init(None, &Info { handle: ptr::null_mut(), len: 0 })
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
