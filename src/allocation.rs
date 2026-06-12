//! Safe Rust wrappers for PMIx Allocation and Job Control APIs.
//!
//! This module provides safe, idiomatic Rust bindings for the PMIx
//! allocation request and job control functions:
//!
//! - [`allocation_request`] — blocking allocation request
//! - [`allocation_request_nb`] — non-blocking allocation request
//! - [`job_control`] — blocking job control action (pause, resume, kill, etc.)
//! - [`job_control_nb`] — non-blocking job control action
//!
//! # C API
//! ```text
//! pmix_status_t PMIx_Allocation_request(pmix_alloc_directive_t directive,
//!                                       pmix_info_t info[], size_t ninfo,
//!                                       pmix_info_t **results, size_t *nresults);
//! pmix_status_t PMIx_Allocation_request_nb(pmix_alloc_directive_t directive,
//!                                          pmix_info_t info[], size_t ninfo,
//!                                          pmix_info_cbfunc_t cbfunc, void *cbdata);
//! ```
//!
//! # Overview
//!
//! Allocation requests allow an application to interact with the host
//! resource manager for dynamic resource management. Several broad
//! categories are supported:
//!
//! - **New allocation** — request additional resources (memory, bandwidth,
//!   compute) that are disjoint from the current allocation.
//! - **Extend** — extend the reservation on currently allocated resources,
//!   either in time or as additional resources.
//! - **Release** — return no-longer-required resources to the scheduler,
//!   including "lending" resources with the expectation of reacquiring them.
//! - **Reacquire** — reacquire resources that were previously "lent".
//!
//! # Examples
//!
//! ```ignore
//! use pmix::{allocation::*, Info, PmixAllocDirective};
//!
//! // Request a new allocation of 4 nodes with 8 CPUs each for 1 hour
//! let info = InfoBuilder::new()
//!     .build();  // add PMIX_ALLOC_NUM_NODES, PMIX_ALLOC_NUM_CPUS, etc.
//!
//! match allocation_request(PmixAllocDirective::AllocNew, &info) {
//!     Ok(results) => println!("Got {} result entries", results.len()),
//!     Err(e) => eprintln!("Allocation failed: {:?}", e),
//! }
//! ```

use std::ffi::c_int;
use std::ptr;
use std::sync::Mutex;

use std::sync::LazyLock;

use crate::ffi;
use crate::{Info, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// PmixAllocDirective — allocation directive enum
// ─────────────────────────────────────────────────────────────────────────────

/// Allocation directive that defines the behavior of an allocation request.
///
/// Corresponds to `pmix_alloc_directive_t` (`uint8_t`) from the PMIx spec.
///
/// # Spec Reference
/// PMIx Standard v4.1, Section 12.1.4 (Job Allocation Directives)
///
/// # Values
/// - [`AllocNew`] (1) — request a new, disjoint allocation.
/// - [`AllocExtend`] (2) — extend the existing allocation.
/// - [`AllocRelease`] (3) — release part of the existing allocation.
/// - [`AllocReacquire`] (4) — reacquire previously "lent" resources.
/// - [`AllocExternal`] (128) — boundary above which implementers define values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PmixAllocDirective {
    /// `PMIX_ALLOC_NEW` — A new allocation is being requested. The resulting
    /// allocation will be disjoint (i.e., not connected in a job sense) from
    /// the requesting allocation.
    AllocNew,

    /// `PMIX_ALLOC_EXTEND` — Extend the existing allocation, either in time
    /// or as additional resources.
    AllocExtend,

    /// `PMIX_ALLOC_RELEASE` — Release part of the existing allocation.
    /// Attributes in the accompanying `pmix_info_t` array may be used to
    /// specify permanent release of the identified resources, or "lending"
    /// of those resources for some period of time.
    AllocRelease,

    /// `PMIX_ALLOC_REAQUIRE` — Reacquire resources that were previously
    /// "lent" back to the scheduler.
    AllocReacquire,

    /// `PMIX_ALLOC_EXTERNAL` (128) — A value boundary above which
    /// implementers are free to define their own directive values.
    AllocExternal,

    /// An unrecognized or future directive value.
    Unknown(u8),
}

impl PmixAllocDirective {
    /// Convert a raw `pmix_alloc_directive_t` (`u8`) into a `PmixAllocDirective`.
    pub fn from_raw(directive: u8) -> Self {
        match directive {
            1 => Self::AllocNew,
            2 => Self::AllocExtend,
            3 => Self::AllocRelease,
            4 => Self::AllocReacquire,
            128 => Self::AllocExternal,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::AllocNew => 1,
            Self::AllocExtend => 2,
            Self::AllocRelease => 3,
            Self::AllocReacquire => 4,
            Self::AllocExternal => 128,
            Self::Unknown(v) => v,
        }
    }
}

impl std::fmt::Display for PmixAllocDirective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllocNew => write!(f, "ALLOC_NEW"),
            Self::AllocExtend => write!(f, "ALLOC_EXTEND"),
            Self::AllocRelease => write!(f, "ALLOC_RELEASE"),
            Self::AllocReacquire => write!(f, "ALLOC_REAQUIRE"),
            Self::AllocExternal => write!(f, "ALLOC_EXTERNAL"),
            Self::Unknown(v) => write!(f, "UNKNOWN_DIRECTIVE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AllocationResults — owned wrapper for the output info array
// ─────────────────────────────────────────────────────────────────────────────

/// Owned wrapper around the `pmix_info_t` array returned by
/// `PMIx_Allocation_request`. Automatically frees the array via
/// `PMIx_Info_free` on drop.
///
/// The results contain information about the allocation outcome, such as
/// the resource manager's allocation identifier (`PMIX_ALLOC_ID`) that
/// can be used in subsequent calls (e.g., `PMIx_Spawn`).
#[derive(Debug)]
pub struct AllocationResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl AllocationResults {
    /// Number of info entries in this result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for AllocationResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx_Allocation_request as an
                // allocated pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

// Prevent accidental use-after-free across threads.
// The raw pointer makes this type unsound to share across threads
// because the C-allocated memory it points to may be freed by another thread.

// ─────────────────────────────────────────────────────────────────────────────
// allocation_request — blocking
// ─────────────────────────────────────────────────────────────────────────────

/// Request an allocation operation from the host resource manager (blocking).
///
/// This function sends an allocation request to the PMIx server / host RM
/// and blocks until the request is processed. The `directive` parameter
/// specifies the type of operation (new, extend, release, reacquire).
/// The `info` array carries the request attributes (e.g., number of nodes,
/// CPUs, time limit).
///
/// On success, returns [`AllocationResults`] containing the response info
/// array. The results may include:
/// - `PMIX_ALLOC_ID` — the RM's identifier for the new/modified allocation.
/// - Additional attributes describing the allocated resources.
///
/// # Parameters
/// - `directive`: The allocation directive (new, extend, release, reacquire).
/// - `info`: Array of [`Info`] entries specifying request attributes.
///
/// # Returns
/// - `Ok(AllocationResults)` with the response info array on success.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host RM does not support this function.
///   - `PMIX_ERR_BAD_PARAM` — invalid directive or info array.
///   - `PMIX_ERR_RESOURCE_UNAVAILABLE` — the requested resources are not available.
///
/// # C API
/// `pmix_status_t PMIx_Allocation_request(pmix_alloc_directive_t directive,`
/// `  pmix_info_t info[], size_t ninfo,`
/// `  pmix_info_t **results, size_t *nresults);`
pub fn allocation_request(
    directive: PmixAllocDirective,
    info: &[Info],
) -> Result<AllocationResults, PmixStatus> {
    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    // Convert the Info slice to C pointers.
    let info_ptr = if info.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: info is a non-empty slice of Info objects that remain
        // alive for the duration of this call. We take the address of the
        // first element's handle field.
        &info[0] as *const Info as *mut ffi::pmix_info_t
    };

    let status = unsafe {
        // SAFETY:
        // - info_ptr is either null or points to a valid array of `Info`
        //   objects whose handles are valid pmix_info_t structs.
        // - results and nresults are valid mutable references that PMIx
        //   will write into on success.
        // - PMIx_Allocation_request is a thread-safe blocking call.
        ffi::PMIx_Allocation_request(
            directive.to_raw(),
            info_ptr,
            info.len(),
            &mut results,
            &mut nresults,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    Ok(AllocationResults { handle: results, len: nresults })
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait & registry for non-blocking variant
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for non-blocking allocation requests.
///
/// Implement this trait to receive the result of an asynchronous
/// allocation request. The `on_complete` method is called exactly once
/// when the operation finishes, with the status and results.
pub trait AllocationCallback: Send + 'static {
    /// Called when the allocation request completes.
    ///
    /// - `status`: The result status (success or error).
    /// - `results`: The allocation results (owned, freed on drop).
    fn on_complete(&self, status: PmixStatus, results: AllocationResults);
}

/// Monotonically increasing allocation request ID counter.
static ALLOCATION_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// Global registry of pending allocation callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static ALLOCATION_REGISTRY: LazyLock<Mutex<std::collections::HashMap<usize, Box<dyn AllocationCallback>>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// C bridge for `pmix_info_cbfunc_t` (allocation completion).
///
/// Called by PMIx when the non-blocking allocation request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered closure and invoke it with the result status and info array.
extern "C" fn allocation_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    _release_cbdata: *mut std::ffi::c_void,
    release_fn: ffi::pmix_release_cbfunc_t,
    cbdata: *mut std::ffi::c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = ALLOCATION_REGISTRY.lock().unwrap();
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
    let results = AllocationResults {
        handle: info,
        len: ninfo,
    };
    cb.on_complete(pmix_status, results);
    // release_fn is unused — we manage our own memory via AllocationResults Drop.
    let _ = release_fn;
}

// ─────────────────────────────────────────────────────────────────────────────
// allocation_request_nb — non-blocking
// ─────────────────────────────────────────────────────────────────────────────

/// Request an allocation operation from the host resource manager (non-blocking).
///
/// Submit an asynchronous allocation request. The `callback` closure is
/// invoked once the operation completes, receiving both the status and
/// the results.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Parameters
/// - `directive`: The allocation directive (new, extend, release, reacquire).
/// - `info`: Array of [`Info`] entries specifying request attributes.
/// - `callback`: A boxed callback that will be invoked on completion.
///
/// # C API
/// `pmix_status_t PMIx_Allocation_request_nb(pmix_alloc_directive_t directive,`
/// `  pmix_info_t info[], size_t ninfo,`
/// `  pmix_info_cbfunc_t cbfunc, void *cbdata);`
pub fn allocation_request_nb(
    directive: PmixAllocDirective,
    info: &[Info],
    callback: Box<dyn AllocationCallback>,
) -> Result<(), PmixStatus> {
    // Assign a unique request ID and register the callback.
    let req_id = {
        let mut seq = ALLOCATION_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };

    // SAFETY: We shift the request ID left by 2 bits to ensure cbdata
    // is never null (req_id starts at 1, so shifted value >= 4).
    let cbdata = (req_id << 2) as *mut std::ffi::c_void;

    {
        let mut registry = ALLOCATION_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Convert the Info slice to C pointers.
    let info_ptr = if info.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: info is a non-empty slice of Info objects that remain
        // alive for the duration of this call. The callback bridge takes
        // ownership of the result, not the input info.
        &info[0] as *const Info as *mut ffi::pmix_info_t
    };

    let status = unsafe {
        // SAFETY:
        // - info_ptr is either null or points to a valid array of Info
        //   objects whose handles are valid pmix_info_t structs.
        // - allocation_callback_bridge is a valid extern "C" function
        //   matching the pmix_info_cbfunc_t signature.
        // - cbdata encodes the request ID and is guaranteed non-null.
        // - The callback registered in ALLOCATION_REGISTRY outlives this
        //   call and will be removed when the callback fires.
        ffi::PMIx_Allocation_request_nb(
            directive.to_raw(),
            info_ptr,
            info.len(),
            Some(allocation_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request was rejected — remove the callback so it doesn't leak.
        let mut registry = ALLOCATION_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Job_control — job control APIs
// ─────────────────────────────────────────────────────────────────────────────

/// Job control action directive.
///
/// Corresponds to the directives passed via `pmix_info_t` keys to
/// `PMIx_Job_control` / `PMIx_Job_control_nb`. This enum represents
/// the set of required attributes defined by the PMIx spec.
///
/// # Spec Reference
/// PMIx Standard v4.1, Section 12.2 (Job Control)
///
/// # Required Attributes
/// - [`Pause`] — pause the specified processes.
/// - [`Resume`] — resume (un-pause) the specified processes.
/// - [`Kill`] — forcibly terminate the specified processes.
/// - [`Signal`] — send a signal to the specified processes.
/// - [`Terminate`] — politely terminate the specified processes.
///
/// # Optional Attributes
/// - [`Cancel`] — cancel a previous job control request by ID.
/// - [`Restart`] — restart processes using a given checkpoint ID.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PmixJobCtrlAction {
    /// `PMIX_JOB_CTRL_PAUSE` — pause the specified processes.
    Pause,
    /// `PMIX_JOB_CTRL_RESUME` — resume (un-pause) the specified processes.
    Resume,
    /// `PMIX_JOB_CTRL_KILL` — forcibly terminate the specified processes and cleanup.
    Kill,
    /// `PMIX_JOB_CTRL_SIGNAL` — send the given signal to the specified processes.
    Signal(c_int),
    /// `PMIX_JOB_CTRL_TERMINATE` — politely terminate the specified processes.
    Terminate,
    /// `PMIX_JOB_CTRL_CANCEL` — cancel a previous job control request by ID.
    Cancel(String),
    /// `PMIX_JOB_CTRL_RESTART` — restart processes using the given checkpoint ID.
    Restart(String),
}

impl PmixJobCtrlAction {
    /// The PMIx info key for this action.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Pause => "pmix.jctrl.pause",
            Self::Resume => "pmix.jctrl.resume",
            Self::Kill => "pmix.jctrl.kill",
            Self::Signal(_) => "pmix.jctrl.sig",
            Self::Terminate => "pmix.jctrl.term",
            Self::Cancel(_) => "pmix.jctrl.cancel",
            Self::Restart(_) => "pmix.jctrl.restart",
        }
    }
}

impl std::fmt::Display for PmixJobCtrlAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pause => write!(f, "PAUSE"),
            Self::Resume => write!(f, "RESUME"),
            Self::Kill => write!(f, "KILL"),
            Self::Signal(sig) => write!(f, "SIGNAL({sig})"),
            Self::Terminate => write!(f, "TERMINATE"),
            Self::Cancel(id) => write!(f, "CANCEL({id})"),
            Self::Restart(id) => write!(f, "RESTART({id})"),
        }
    }
}

/// Owned wrapper around the `pmix_info_t` array returned by
/// `PMIx_Job_control`. Automatically frees the array via
/// `PMIx_Info_free` on drop.
///
/// The results contain information about the job control outcome,
/// such as confirmation of the action taken or error details.
#[derive(Debug)]
pub struct JobControlResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl JobControlResults {
    /// Number of info entries in this result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create an empty `JobControlResults`.
    ///
    /// Useful for testing and for constructing a no-op result.
    pub fn new_empty() -> Self {
        Self {
            handle: ptr::null_mut(),
            len: 0,
        }
    }
}

impl Drop for JobControlResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx_Job_control as an
                // allocated pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

/// Request a job control action on target processes (blocking).
///
/// This function sends a job control directive to the PMIx server / host
/// resource manager and blocks until the request is processed. Supported
/// actions include pause, resume, kill, signal, terminate, cancel, and
/// restart.
///
/// The `targets` array specifies which processes the action should apply to.
/// Pass an empty slice to apply to the caller's own job.
///
/// The `directives` parameter carries the job control action(s) plus optional
/// attributes such as `PMIX_JOB_CTRL_ID` (request identifier), cleanup
/// registration (`PMIX_REGISTER_CLEANUP`), and checkpoint methods.
///
/// On success, returns [`JobControlResults`] containing the response info
/// array with details about the result.
///
/// # Parameters
/// - `targets`: Processes to which the job control action applies.
/// - `directives`: Array of [`Info`] entries specifying the action and options.
///
/// # Returns
/// - `Ok(JobControlResults)` with the response info array on success.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host RM does not support this function.
///   - `PMIX_ERR_BAD_PARAM` — invalid targets or directives.
///
/// # C API
/// `pmix_status_t PMIx_Job_control(const pmix_proc_t targets[], size_t ntargets,`
/// `  const pmix_info_t directives[], size_t ndirs,`
/// `  pmix_info_t **results, size_t *nresults);`
pub fn job_control(
    targets: &[Proc],
    directives: &[Info],
) -> Result<JobControlResults, PmixStatus> {
    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    // Convert targets slice to C pointer.
    let targets_ptr = if targets.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: targets is a non-empty slice of Proc objects whose handle
        // fields are valid pmix_proc_t structs that remain alive for this call.
        unsafe {
            std::ptr::addr_of!((*(&targets[0] as *const Proc)).handle) as *mut ffi::pmix_proc_t
        }
    };

    // Convert directives slice to C pointer.
    let directives_ptr = if directives.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: directives is a non-empty slice of Info objects whose handles
        // are valid pmix_info_t pointers that remain alive for this call.
        unsafe {
            std::ptr::addr_of!((*(&directives[0] as *const Info)).handle) as *mut ffi::pmix_info_t
        }
    };

    let status = unsafe {
        // SAFETY:
        // - targets_ptr is either null or points to a valid array of pmix_proc_t.
        // - directives_ptr is either null or points to a valid array of pmix_info_t.
        // - results and nresults are valid mutable references that PMIx will write.
        // - PMIx_Job_control is a thread-safe blocking call per the spec.
        ffi::PMIx_Job_control(
            targets_ptr as *const ffi::pmix_proc_t,
            targets.len(),
            directives_ptr as *const ffi::pmix_info_t,
            directives.len(),
            &mut results,
            &mut nresults,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    Ok(JobControlResults { handle: results, len: nresults })
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait & registry for non-blocking job control
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for non-blocking job control requests.
///
/// Implement this trait to receive the result of an asynchronous
/// job control request. The `on_complete` method is called exactly once
/// when the operation finishes, with the status and results.
pub trait JobControlCallback: Send + 'static {
    /// Called when the job control request completes.
    ///
    /// - `status`: The result status (success or error).
    /// - `results`: The job control results (owned, freed on drop).
    fn on_complete(&self, status: PmixStatus, results: JobControlResults);
}

/// Monotonically increasing job control request ID counter.
static JOB_CTRL_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// Global registry of pending job control callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static JOB_CTRL_REGISTRY: LazyLock<Mutex<std::collections::HashMap<usize, Box<dyn JobControlCallback>>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// C bridge for `pmix_info_cbfunc_t` (job control completion).
///
/// Called by PMIx when the non-blocking job control request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered closure and invoke it with the result status and info array.
extern "C" fn job_control_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    _release_cbdata: *mut std::ffi::c_void,
    release_fn: ffi::pmix_release_cbfunc_t,
    cbdata: *mut std::ffi::c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = JOB_CTRL_REGISTRY.lock().unwrap();
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
    let results = JobControlResults {
        handle: info,
        len: ninfo,
    };
    cb.on_complete(pmix_status, results);
    // release_fn is unused — we manage our own memory via JobControlResults Drop.
    let _ = release_fn;
}

// ─────────────────────────────────────────────────────────────────────────────
// job_control_nb — non-blocking
// ─────────────────────────────────────────────────────────────────────────────

/// Request a job control action on target processes (non-blocking).
///
/// Submit an asynchronous job control request. The `callback` closure is
/// invoked once the operation completes, receiving both the status and
/// the results.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # Parameters
/// - `targets`: Processes to which the job control action applies.
/// - `directives`: Array of [`Info`] entries specifying the action and options.
/// - `callback`: A boxed callback that will be invoked on completion.
///
/// # C API
/// `pmix_status_t PMIx_Job_control_nb(const pmix_proc_t targets[], size_t ntargets,`
/// `  const pmix_info_t directives[], size_t ndirs,`
/// `  pmix_info_cbfunc_t cbfunc, void *cbdata);`
pub fn job_control_nb(
    targets: &[Proc],
    directives: &[Info],
    callback: Box<dyn JobControlCallback>,
) -> Result<(), PmixStatus> {
    // Assign a unique request ID and register the callback.
    let req_id = {
        let mut seq = JOB_CTRL_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };

    // SAFETY: We shift the request ID left by 2 bits to ensure cbdata
    // is never null (req_id starts at 1, so shifted value >= 4).
    let cbdata = (req_id << 2) as *mut std::ffi::c_void;

    {
        let mut registry = JOB_CTRL_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Convert targets slice to C pointer.
    let targets_ptr = if targets.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: targets is a non-empty slice of Proc objects whose handle
        // fields are valid pmix_proc_t structs that remain alive for this call.
        unsafe {
            std::ptr::addr_of!((*(&targets[0] as *const Proc)).handle) as *mut ffi::pmix_proc_t
        }
    };

    // Convert directives slice to C pointer.
    let directives_ptr = if directives.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: directives is a non-empty slice of Info objects whose handles
        // are valid pmix_info_t pointers that remain alive for this call.
        unsafe {
            std::ptr::addr_of!((*(&directives[0] as *const Info)).handle) as *mut ffi::pmix_info_t
        }
    };

    let status = unsafe {
        // SAFETY:
        // - targets_ptr is either null or points to a valid array of pmix_proc_t.
        // - directives_ptr is either null or points to a valid array of pmix_info_t.
        // - job_control_callback_bridge is a valid extern "C" function matching
        //   the pmix_info_cbfunc_t signature.
        // - cbdata encodes the request ID and is guaranteed non-null.
        // - The callback registered in JOB_CTRL_REGISTRY outlives this call
        //   and will be removed when the callback fires.
        ffi::PMIx_Job_control_nb(
            targets_ptr as *const ffi::pmix_proc_t,
            targets.len(),
            directives_ptr as *const ffi::pmix_info_t,
            directives.len(),
            Some(job_control_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request was rejected — remove the callback so it doesn't leak.
        let mut registry = JOB_CTRL_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── PmixAllocDirective conversions ──

    #[test]
    fn test_alloc_directive_new_roundtrip() {
        let d = PmixAllocDirective::AllocNew;
        assert_eq!(d.to_raw(), 1);
        assert_eq!(PmixAllocDirective::from_raw(1), PmixAllocDirective::AllocNew);
    }

    #[test]
    fn test_alloc_directive_extend_roundtrip() {
        let d = PmixAllocDirective::AllocExtend;
        assert_eq!(d.to_raw(), 2);
        assert_eq!(PmixAllocDirective::from_raw(2), PmixAllocDirective::AllocExtend);
    }

    #[test]
    fn test_alloc_directive_release_roundtrip() {
        let d = PmixAllocDirective::AllocRelease;
        assert_eq!(d.to_raw(), 3);
        assert_eq!(PmixAllocDirective::from_raw(3), PmixAllocDirective::AllocRelease);
    }

    #[test]
    fn test_alloc_directive_reacquire_roundtrip() {
        let d = PmixAllocDirective::AllocReacquire;
        assert_eq!(d.to_raw(), 4);
        assert_eq!(PmixAllocDirective::from_raw(4), PmixAllocDirective::AllocReacquire);
    }

    #[test]
    fn test_alloc_directive_external_roundtrip() {
        let d = PmixAllocDirective::AllocExternal;
        assert_eq!(d.to_raw(), 128);
        assert_eq!(PmixAllocDirective::from_raw(128), PmixAllocDirective::AllocExternal);
    }

    #[test]
    fn test_alloc_directive_unknown() {
        let d = PmixAllocDirective::from_raw(42);
        assert!(matches!(d, PmixAllocDirective::Unknown(42)));
        assert_eq!(d.to_raw(), 42);
    }

    #[test]
    fn test_alloc_directive_display() {
        assert_eq!(format!("{}", PmixAllocDirective::AllocNew), "ALLOC_NEW");
        assert_eq!(format!("{}", PmixAllocDirective::AllocExtend), "ALLOC_EXTEND");
        assert_eq!(format!("{}", PmixAllocDirective::AllocRelease), "ALLOC_RELEASE");
        assert_eq!(format!("{}", PmixAllocDirective::AllocReacquire), "ALLOC_REAQUIRE");
        assert_eq!(format!("{}", PmixAllocDirective::AllocExternal), "ALLOC_EXTERNAL");
        assert_eq!(
            format!("{}", PmixAllocDirective::Unknown(99)),
            "UNKNOWN_DIRECTIVE (99)"
        );
    }

    #[test]
    fn test_alloc_directive_derives() {
        // Debug
        let d = PmixAllocDirective::AllocNew;
        let s = format!("{:?}", d);
        assert!(!s.is_empty());

        // Clone
        let d2 = d.clone();
        assert_eq!(d2.to_raw(), d.to_raw());

        // Copy
        let d3 = d;
        assert_eq!(d3.to_raw(), d.to_raw());

        // PartialEq
        assert_eq!(PmixAllocDirective::AllocNew, PmixAllocDirective::AllocNew);
        assert_ne!(PmixAllocDirective::AllocNew, PmixAllocDirective::AllocExtend);

        // Eq + Hash (compile-time check)
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(PmixAllocDirective::AllocNew);
        set.insert(PmixAllocDirective::AllocExtend);
        assert_eq!(set.len(), 2);
        assert!(!set.insert(PmixAllocDirective::AllocNew));
    }

    // ── AllocationResults ──

    #[test]
    fn test_allocation_results_empty() {
        let results = AllocationResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_allocation_results_debug() {
        let results = AllocationResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        let s = format!("{:?}", results);
        assert!(s.contains("AllocationResults"));
    }

    // ── allocation_request (requires PMIx init — ignored) ──

    #[test]
    fn test_allocation_request_requires_init() {
        // Without PMIx_Init, the call should return an error.
        // This test verifies the FFI call path works even without a daemon.
        let result = allocation_request(PmixAllocDirective::AllocNew, &[]);
        // We expect an error because PMIx is not initialized.
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_success());
    }

    #[test]
    fn test_allocation_request_directive_values() {
        // Verify that all directive values are correctly mapped.
        assert_eq!(PmixAllocDirective::AllocNew.to_raw(), 1);
        assert_eq!(PmixAllocDirective::AllocExtend.to_raw(), 2);
        assert_eq!(PmixAllocDirective::AllocRelease.to_raw(), 3);
        assert_eq!(PmixAllocDirective::AllocReacquire.to_raw(), 4);
        assert_eq!(PmixAllocDirective::AllocExternal.to_raw(), 128);
    }

    #[test]
    fn test_allocation_request_empty_info() {
        // Calling with empty info array should still make the FFI call
        // and return an error (PMIx not initialized).
        let result = allocation_request(PmixAllocDirective::AllocExtend, &[]);
        assert!(result.is_err());
    }
}
