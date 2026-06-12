//! Process management — `PMIx_Abort`.
//!
//! This module provides safe Rust wrappers around the PMIx process
//! management APIs:
//!
//! * **Abort** — request that the host resource manager abort the
//!   specified processes with a given error code and message.
//!
//! # Example
//!
//! ```no_run
//! use pmix::process_mgmt::abort;
//! use pmix::{PmixError, PmixStatus, Proc};
//!
//! // Abort all processes in the caller's namespace
//! let result = abort(
//!     PmixStatus::Known(PmixError::Error),
//!     Some("something went wrong"),
//!     None, // NULL procs = abort all in caller's namespace
//! );
//! // Note: if the caller is included in the abort, the function
//! // will not return unless the host is unable to execute it.
//! ```

use crate::ffi;
use crate::{PmixStatus, Proc};
use std::ffi::CString;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Abort
// ─────────────────────────────────────────────────────────────────────────────

/// Request that the host resource manager abort the specified processes.
///
/// Instructs the PMIx server to print the provided message (if any) and
/// abort the given array of processes. The `status` argument is the error
/// code to return to the invoking environment.
///
/// * If `procs` is `None`, all processes in the caller's namespace are
///   aborted, including the caller itself — equivalent to passing a single
///   `pmix_proc_t` with `PMIX_RANK_WILDCARD`.
/// * The function is **blocking**: it does not return until the host
///   environment has carried out the operation on the specified processes.
/// * If the caller is included in the abort targets, the function will
///   **not return** unless the host is unable to execute the operation.
/// * Passing `None` for `msg` is allowed (no message printed).
///
/// # Returns
/// * `Ok(())` — the abort request was accepted. If the caller's own
///   process was included, the function will not return with success.
/// * `Err(PmixStatus::Known(PmixError::ErrParamValueNotSupported))` — the
///   host environment cannot abort the requested processes (e.g., subsets
///   from another namespace).
/// * `Err(PmixStatus)` — another error in the request.
///
/// # Thread Safety
/// Race conditions caused by multiple processes calling `PMIx_Abort`
/// simultaneously are left to the server implementation to resolve.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Abort(int status, const char msg[],
///                          pmix_proc_t procs[], size_t nprocs);
/// ```
pub fn abort(
    status: PmixStatus,
    msg: Option<&str>,
    procs: Option<&[Proc]>,
) -> Result<(), PmixStatus> {
    // Convert the optional message to a C string pointer.
    let (msg_ptr, _msg_cstring) = match msg {
        Some(m) => {
            let cs = CString::new(m).expect("abort message must not contain interior NUL bytes");
            (cs.as_ptr(), Some(cs))
        }
        None => (ptr::null(), None),
    };

    // Convert the optional proc array to a raw pointer + length.
    let (procs_ptr, nprocs) = match procs {
        Some(procs) if !procs.is_empty() => (
            &procs[0].handle as *const ffi::pmix_proc_t as *mut ffi::pmix_proc_t,
            procs.len(),
        ),
        _ => (ptr::null_mut(), 0),
    };

    // SAFETY: FFI call into PMIx library.
    // - `status.to_raw()` converts our type-safe PmixStatus to a raw
    //   `pmix_status_t` (i32) that the C API expects.
    // - `msg_ptr` is either null or a valid NUL-terminated C string
    //   whose lifetime (`_msg_cstring`) is kept alive until after this
    //   call returns.
    // - `procs_ptr` is either null or points to a valid slice of
    //   `pmix_proc_t` handles that remain valid for the duration of
    //   this call. PMIx does not retain these pointers after return.
    // - Note: if the caller's own process is included in `procs`, this
    //   function may not return. That is documented PMIx behavior.
    let raw_status = unsafe {
        ffi::PMIx_Abort(status.to_raw() as std::os::raw::c_int, msg_ptr, procs_ptr, nprocs)
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}
