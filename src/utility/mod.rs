//! Utility functions — `PMIx_Initialized`, `PMIx_Error_string`, `PMIx_Proc_state_string`,
//! `PMIx_Scope_string`, `PMIx_Persistence_string`, `PMIx_Data_range_string`,
//! `PMIx_Info_directives_string`, `PMIx_Data_type_string`, `PMIx_Alloc_directive_string`,
//! `PMIx_IOF_channel_string`, `PMIx_Job_state_string`, `PMIx_Get_attribute_string`,
//! `PMIx_Get_attribute_name`, `PMIx_Link_state_string`, `PMIx_Device_type_string`,
//! `PMIx_generate_regex`, `PMIx_generate_ppn`, `PMIx_Register_attributes`,
//! `PMIx_IOF_pull`, `PMIx_IOF_deregister`, `PMIx_IOF_push`, and related helpers.
//!
//! This module provides safe Rust wrappers around PMIx utility APIs
//! that do not fit into the lifecycle, data, or event categories.

use crate::{
    IOFChannelFlags, InfoFlags, PmixAllocDirective, PmixDataRange, PmixDataType, PmixDeviceType,
    PmixJobState, PmixLinkState, PmixPersistence, PmixProcState, PmixScope, PmixStatus, ffi,
};
use std::ffi::CStr;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Initialized
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the PMIx client has been successfully initialized.
///
/// This function only reports the internal state of the PMIx client library.
/// It does **not** verify that an active connection with the server exists,
/// nor that the server is functional.
///
/// # C API
/// `int PMIx_Initialized(void)`
///
/// # Examples
/// ```no_run
/// use pmix::utility::initialized;
///
/// if initialized() {
///     println!("PMIx is initialized");
/// } else {
///     println!("PMIx has not been initialized yet");
/// }
/// ```
pub fn initialized() -> bool {
    // SAFETY: PMIx_Initialized takes no parameters and returns a plain int.
    // It is a thread-safe read of an internal atomic flag in the PMIx library.
    // No pointers are dereferenced and no memory is allocated or freed.
    unsafe { ffi::PMIx_Initialized() != 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Error_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx status code.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Error_string(pmix_status_t status)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the status code.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid PMIx status codes, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::error_string, PmixError};
///
/// let status = PmixError::Success.into();
/// let desc = error_string(status).expect("valid status");
/// assert_eq!(desc, "success");
/// ```
pub fn error_string(status: PmixStatus) -> Result<String, PmixStatus> {
    let raw = status.to_raw();
    // SAFETY: PMIx_Error_string takes a single pmix_status_t and returns
    // a pointer to a static, null-terminated string owned by the library.
    // No memory is allocated or freed by this call. The returned pointer
    // is valid for the lifetime of the process (it points to read-only
    // data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Error_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_status_t, but guard anyway.
        return Err(PmixStatus::from_raw(raw));
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Proc_state_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx process state code.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Proc_state_string(pmix_proc_state_t state)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the process state.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_proc_state_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::proc_state_string, PmixProcState};
///
/// let state = PmixProcState::Running;
/// let desc = proc_state_string(state).expect("valid state");
/// assert_eq!(desc, "PROC EXECUTING");
/// ```
pub fn proc_state_string(state: PmixProcState) -> Result<String, PmixStatus> {
    let raw = state.to_raw();
    // SAFETY: PMIx_Proc_state_string takes a single pmix_proc_state_t (u8)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Proc_state_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_proc_state_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Scope_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx scope value.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Scope_string(pmix_scope_t scope)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the scope.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_scope_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::scope_string, PmixScope};
///
/// let scope = PmixScope::Global;
/// let desc = scope_string(scope).expect("valid scope");
/// assert_eq!(desc, "GLOBAL");
/// ```
pub fn scope_string(scope: PmixScope) -> Result<String, PmixStatus> {
    let raw = scope.to_raw();
    // SAFETY: PMIx_Scope_string takes a single pmix_scope_t (u8) and returns
    // a pointer to a static, null-terminated string owned by the library.
    // No memory is allocated or freed by this call. The returned pointer
    // is valid for the lifetime of the process (it points to read-only
    // data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Scope_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_scope_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Persistence_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx persistence value.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Persistence_string(pmix_persistence_t persist)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the persistence value.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_persistence_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::persistence_string, PmixPersistence};
///
/// let persist = PmixPersistence::Indefinite;
/// let desc = persistence_string(persist).expect("valid persistence");
/// assert_eq!(desc, "INDEFINITE");
/// ```
pub fn persistence_string(persist: PmixPersistence) -> Result<String, PmixStatus> {
    let raw = persist.to_raw();
    // SAFETY: PMIx_Persistence_string takes a single pmix_persistence_t (u8)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Persistence_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_persistence_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_range_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx data range value.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Data_range_string(pmix_data_range_t range)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the data range.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_data_range_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::data_range_string, PmixDataRange};
///
/// let range = PmixDataRange::Global;
/// let desc = data_range_string(range).expect("valid range");
/// assert_eq!(desc, "GLOBAL");
/// ```
pub fn data_range_string(range: PmixDataRange) -> Result<String, PmixStatus> {
    let raw = range.to_raw();
    // SAFETY: PMIx_Data_range_string takes a single pmix_data_range_t (u8)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Data_range_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_data_range_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Info_directives_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of PMIx info directives flags.
///
/// The `pmix_info_directives_t` is a bitmask that controls how `pmix_info_t`
/// entries are processed. Common flags include:
///
/// * `PMIX_INFO_REQD` (1) — the info entry is required; fail if unsupported.
/// * `PMIX_INFO_ARRAY_END` (2) — marks the end of a variadic info array.
/// * `PMIX_INFO_REQD_PROCESSED` (4) — set by the library after processing.
/// * `PMIX_INFO_DIR_RESERVED` (0xFFFF0000) — bits reserved for implementers.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Info_directives_string(pmix_info_directives_t directives)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the directives bitmask.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_info_directives_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::info_directives_string, InfoFlags};
///
/// let flags = InfoFlags::REQD;
/// let desc = info_directives_string(flags).expect("valid directives");
/// assert!(!desc.is_empty());
/// ```
pub fn info_directives_string(directives: InfoFlags) -> Result<String, PmixStatus> {
    let raw = directives.raw();
    // SAFETY: PMIx_Info_directives_string takes a single pmix_info_directives_t
    // (u32 bitmask) and returns a pointer to a static, null-terminated string
    // owned by the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points to
    // read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Info_directives_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_info_directives_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_type_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx data type value.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Data_type_string(pmix_data_type_t type)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the data type.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_data_type_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::data_type_string, PmixDataType};
///
/// let ty = PmixDataType::String;
/// let desc = data_type_string(ty).expect("valid data type");
/// assert!(!desc.is_empty());
/// ```
pub fn data_type_string(ty: PmixDataType) -> Result<String, PmixStatus> {
    let raw = ty.to_raw();
    // SAFETY: PMIx_Data_type_string takes a single pmix_data_type_t (u16)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Data_type_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_data_type_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Alloc_directive_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx allocation directive.
///
/// The `pmix_alloc_directive_t` controls the behavior of
/// `PMIx_Allocation_request`. Currently only one value is defined:
/// `PMIX_ALLOC_DIRECTIVE` (43), indicating a hard allocation request.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Alloc_directive_string(pmix_alloc_directive_t directive)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the allocation directive.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_alloc_directive_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::alloc_directive_string, PmixAllocDirective};
///
/// let directive = PmixAllocDirective::AllocDirective;
/// let desc = alloc_directive_string(directive).expect("valid directive");
/// assert!(!desc.is_empty());
/// ```
pub fn alloc_directive_string(directive: PmixAllocDirective) -> Result<String, PmixStatus> {
    let raw = directive.to_raw();
    // SAFETY: PMIx_Alloc_directive_string takes a single pmix_alloc_directive_t
    // (u8) and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The returned
    // pointer is valid for the lifetime of the process (it points to read-only
    // data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Alloc_directive_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_alloc_directive_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_IOF_channel_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx I/O forwarding channel.
///
/// The `pmix_iof_channel_t` is a bitmask that specifies which standard
/// I/O channels are being forwarded. Common values include:
///
/// * `PMIX_FWD_STDIN_CHANNEL`   (0x0001) — standard input.
/// * `PMIX_FWD_STDOUT_CHANNEL`  (0x0002) — standard output.
/// * `PMIX_FWD_STDERR_CHANNEL`  (0x0004) — standard error.
/// * `PMIX_FWD_STDDIAG_CHANNEL` (0x0008) — diagnostic channel.
/// * `PMIX_FWD_ALL_CHANNELS`    (0x00FF) — all channels.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_IOF_channel_string(pmix_iof_channel_t channel)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the channel bitmask.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_iof_channel_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::iof_channel_string, IOFChannelFlags};
///
/// let channel = IOFChannelFlags::STDOUT;
/// let desc = iof_channel_string(channel).expect("valid channel");
/// assert!(!desc.is_empty());
/// ```
pub fn iof_channel_string(channel: IOFChannelFlags) -> Result<String, PmixStatus> {
    let raw = channel.raw();
    // SAFETY: PMIx_IOF_channel_string takes a single pmix_iof_channel_t
    // (u16 bitmask) and returns a pointer to a static, null-terminated string
    // owned by the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points to
    // read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_IOF_channel_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_iof_channel_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Job_state_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx job state code.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Job_state_string(pmix_job_state_t state)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the job state.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_job_state_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::job_state_string, PmixJobState};
///
/// let state = PmixJobState::Running;
/// let desc = job_state_string(state).expect("valid state");
/// assert_eq!(desc, "JOB RUNNING");
/// ```
pub fn job_state_string(state: PmixJobState) -> Result<String, PmixStatus> {
    let raw = state.to_raw();
    // SAFETY: PMIx_Job_state_string takes a single pmix_job_state_t (u8)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Job_state_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_job_state_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_attribute_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the canonical string representation of a PMIx attribute key.
///
/// Given an attribute key (e.g., `"pmix.host"`, `"pmix.nprocs"`), this function
/// performs a case-insensitive lookup in the PMIx library's registered attribute
/// table and returns the canonical/canonicalized name. If the attribute is not
/// found or the library has not been initialized, the input string is returned
/// unchanged.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Get_attribute_string(const char *attribute)`
///
/// # Returns
/// * `Ok(String)` — the canonical attribute string (never null from the C side,
///   but we guard against it anyway).
/// * `Err(PmixStatus)` — if the attribute string contains a NUL byte
///   (would be `NulError` in a stricter API, but we return `PmixStatus`
///   for consistency with other utility functions).
///
/// # Examples
/// ```no_run
/// use pmix::utility::get_attribute_string;
///
/// let canonical = get_attribute_string("pmix.host").expect("valid attribute");
/// assert!(!canonical.is_empty());
/// ```
pub fn get_attribute_string(attribute: &str) -> Result<String, PmixStatus> {
    // Convert the attribute string to a C string for the FFI call.
    // If the string contains a NUL byte, this is an invalid attribute key.
    let attr_c = std::ffi::CString::new(attribute).map_err(|_| PmixStatus::from_raw(-1))?;
    // SAFETY: PMIx_Get_attribute_string takes a const char* attribute key and
    // returns a pointer to a static, null-terminated string owned by the library.
    // The returned pointer is never null — if the attribute is not found or the
    // library is not initialized, it returns the input string unchanged.
    // No memory is allocated or freed by this call. The returned pointer is
    // valid for the lifetime of the process (it points to read-only data inside
    // the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Get_attribute_string(attr_c.as_ptr()) };
    if c_ptr.is_null() {
        // Should not happen — the C implementation always returns a non-null
        // pointer (either the canonical name or the input unchanged).
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_attribute_name
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the attribute key name for a given canonical attribute string.
///
/// This is the inverse of [`get_attribute_string`]. Given the canonical string
/// representation of an attribute (e.g., `"host name"`), it performs a
/// case-insensitive reverse lookup and returns the corresponding attribute key
/// (e.g., `"pmix.host"`). If the string is not found or the library has not
/// been initialized, the input string is returned unchanged.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Get_attribute_name(const char *attrstring)`
///
/// # Returns
/// * `Ok(String)` — the attribute key name (never null from the C side).
/// * `Err(PmixStatus)` — if the attribute string contains a NUL byte.
///
/// # Examples
/// ```no_run
/// use pmix::utility::get_attribute_name;
///
/// let name = get_attribute_name("host name").expect("valid attribute string");
/// assert!(!name.is_empty());
/// ```
pub fn get_attribute_name(attribute: &str) -> Result<String, PmixStatus> {
    let attr_c = std::ffi::CString::new(attribute).map_err(|_| PmixStatus::from_raw(-1))?;
    // SAFETY: PMIx_Get_attribute_name takes a const char* attribute string and
    // returns a pointer to a static, null-terminated string owned by the library.
    // The returned pointer is never null — if the attribute is not found or the
    // library is not initialized, it returns the input string unchanged.
    // No memory is allocated or freed by this call. The returned pointer is
    // valid for the lifetime of the process (it points to read-only data inside
    // the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Get_attribute_name(attr_c.as_ptr()) };
    if c_ptr.is_null() {
        // Should not happen — the C implementation always returns a non-null
        // pointer (either the canonical name or the input unchanged).
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Link_state_string
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a human-readable string description of a PMIx link state value.
///
/// The `pmix_link_state_t` encodes the physical link state of a fabric
/// device port. Defined values are:
///
/// * `PMIX_LINK_STATE_UNKNOWN` (0) — port state is unknown or not applicable.
/// * `PMIX_LINK_DOWN` (1) — port is inactive.
/// * `PMIX_LINK_UP` (2) — port is active.
///
/// The returned string is owned by the PMIx library and must not be freed
/// or modified by the caller. This wrapper copies the string into a Rust
/// `String` so the caller owns the result.
///
/// # C API
/// `const char* PMIx_Link_state_string(pmix_link_state_t state)`
///
/// # Returns
/// * `Ok(String)` — the library's description of the link state.
/// * `Err(PmixStatus)` — if the C function returned a null pointer
///   (should not happen for valid `pmix_link_state_t` values, but guarded
///   against for safety).
///
/// # Examples
/// ```no_run
/// use pmix::{utility::link_state_string, PmixLinkState};
///
/// let state = PmixLinkState::LinkUp;
/// let desc = link_state_string(state).expect("valid state");
/// assert_eq!(desc, "ACTIVE");
/// ```
pub fn link_state_string(state: PmixLinkState) -> Result<String, PmixStatus> {
    let raw = state.to_raw();
    // SAFETY: PMIx_Link_state_string takes a single pmix_link_state_t (u8)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Link_state_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_link_state_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Device_type_string
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_Device_type_string`.
///
/// Returns a human-readable string for a PMIx device type value.
///
/// # C API
/// `const char* PMIx_Device_type_string(pmix_device_type_t type)`
///
/// # Examples
/// ```no_run
/// use pmix::{utility::device_type_string, PmixDeviceType};
///
/// let ty = PmixDeviceType::Gpu;
/// let desc = device_type_string(ty).expect("valid device type");
/// assert_eq!(desc, "GPU");
/// ```
pub fn device_type_string(ty: PmixDeviceType) -> Result<String, PmixStatus> {
    let raw = ty.to_raw();
    // SAFETY: PMIx_Device_type_string takes a single pmix_device_type_t (u64)
    // and returns a pointer to a static, null-terminated string owned by
    // the library. No memory is allocated or freed by this call. The
    // returned pointer is valid for the lifetime of the process (it points
    // to read-only data inside the PMIx shared library).
    let c_ptr = unsafe { ffi::PMIx_Device_type_string(raw) };
    if c_ptr.is_null() {
        // Should not happen for any valid pmix_device_type_t, but guard anyway.
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    // SAFETY: The pointer is non-null and points to a valid null-terminated
    // C string owned by the PMIx library (static lifetime).
    let cstr = unsafe { std::ffi::CStr::from_ptr(c_ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_generate_regex
// ─────────────────────────────────────────────────────────────────────────────

/// Given a comma-separated list of node names, generate a compact regular
/// expression representation suitable for passing to
/// [`PMIx_server_register_nspace`][crate::lifecycle::server_register_nspace].
///
/// The `input` is a comma-separated list of host / node names — e.g.
/// `"node001,node002,node003,node010,node011"`.  The function returns a
/// compressed representation (e.g. `"pmix:node[001-003,010-011]"`) that
/// preserves the order of the input values.
///
/// The returned string is **owned by the caller** — the PMIx library
/// allocates it with `malloc`.  This wrapper takes ownership via
/// [`CString::from_raw`][std::ffi::CString::from_raw] and returns a Rust
/// [`String`], so the caller does not need to worry about freeing the
/// underlying C allocation.
///
/// # C API
/// `pmix_status_t PMIx_generate_regex(const char *input, char **regex)`
///
/// # Returns
/// * `Ok(String)` — the generated regular expression (caller-owned).
/// * `Err(PmixStatus)` — if the input is `None` or empty, or if the C
///   function returned a non-success status.
///
/// # Examples
/// ```no_run
/// use pmix::utility::generate_regex;
///
/// let nodes = "odin001,odin002,odin003,odin010,odin011,odin075";
/// let regex = generate_regex(nodes).expect("valid node list");
/// assert!(!regex.is_empty());
/// assert!(regex.starts_with("pmix:") || regex.starts_with("blob:"));
/// ```
pub fn generate_regex(input: &str) -> Result<String, PmixStatus> {
    // Convert Rust string to C string for the FFI call.
    let input_cstr = std::ffi::CString::new(input).map_err(|_| PmixStatus::from_raw(-27))?; // PMIX_ERR_BAD_PARAM

    let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

    // SAFETY: PMIx_generate_regex takes a const char* input and a double
    // pointer for the output. The input_cstr is valid for the duration of
    // this call (it lives until the end of this scope). The output pointer
    // is written by the PMIx library and points to malloc'd memory that
    // the caller owns. We check the return status before touching the
    // output pointer.
    let status = unsafe { ffi::PMIx_generate_regex(input_cstr.as_ptr(), &mut regex_ptr) };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // SAFETY: On success, regex_ptr is non-null and points to malloc'd
    // memory owned by the caller. We take ownership via CString::from_raw
    // so it will be freed when the CString is dropped (and the String
    // extracted from it is independently owned).
    let owned = unsafe {
        if regex_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        std::ffi::CString::from_raw(regex_ptr)
    };

    // Extract the string content (copies into a Rust-owned String).
    // CString is dropped here, freeing the original C allocation.
    Ok(owned.into_string().unwrap_or_default())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_generate_ppn
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a compressed representation of the process-per-node (PPN) mapping.
///
/// The `input` is a semicolon-separated list of process rank ranges, where each
/// field corresponds to the processes assigned to a node — e.g.
/// `"0-3;4-7;8,9,10"`.  Each semicolon-delimited group maps positionally to
/// the node names provided to [`generate_regex`][crate::utility::generate_regex].
///
/// The returned string is a compressed regex representation (e.g.
/// `"pmix:0-10"` or `"raw:0-3;4-7;8,9,10"` depending on the registered
/// preg module) that identifies which processes run on each node.
///
/// The returned string is **owned by the caller** — the PMIx library
/// allocates it.  This wrapper takes ownership via [`CString::from_raw`][std::ffi::CString::from_raw]
/// and returns a Rust [`String`], so the caller does not need to worry about
/// freeing the underlying C allocation.
///
/// # C API
/// `pmix_status_t PMIx_generate_ppn(const char *input, char **ppn)`
///
/// # Returns
/// * `Ok(String)` — the generated PPN regex (caller-owned).
/// * `Err(PmixStatus)` — if the input is invalid (contains a null byte), or
///   if the C function returned a non-success status (e.g. `PMIX_ERR_INIT`
///   if the library has not been initialized).
///
/// # Examples
/// ```no_run
/// use pmix::utility::generate_ppn;
///
/// // Three nodes: ranks 0-3 on node 1, ranks 4-7 on node 2, ranks 8-10 on node 3.
/// let ppn = generate_ppn("0-3;4-7;8,9,10").expect("valid PPN list");
/// assert!(!ppn.is_empty());
/// ```
pub fn generate_ppn(input: &str) -> Result<String, PmixStatus> {
    // Convert Rust string to C string for the FFI call.
    let input_cstr = std::ffi::CString::new(input).map_err(|_| PmixStatus::from_raw(-27))?; // PMIX_ERR_BAD_PARAM

    let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

    // SAFETY: PMIx_generate_ppn takes a const char* input and a double
    // pointer for the output. The input_cstr is valid for the duration of
    // this call (it lives until the end of this scope). The output pointer
    // is written by the PMIx library and points to malloc'd memory that
    // the caller owns. We check the return status before touching the
    // output pointer.
    let status = unsafe { ffi::PMIx_generate_ppn(input_cstr.as_ptr(), &mut ppn_ptr) };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // SAFETY: On success, ppn_ptr is non-null and points to malloc'd
    // memory owned by the caller. We take ownership via CString::from_raw
    // so it will be freed when the CString is dropped (and the String
    // extracted from it is independently owned).
    let owned = unsafe {
        if ppn_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        std::ffi::CString::from_raw(ppn_ptr)
    };

    // Extract the string content (copies into a Rust-owned String).
    // CString is dropped here, freeing the original C allocation.
    Ok(owned.into_string().unwrap_or_default())
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Register_attributes
// ─────────────────────────────────────────────────────────────────────────────

/// Register host environment attribute support for a PMIx function.
///
/// The `PMIx_Register_attributes` function is used by the host environment to
/// register which attributes it supports for a given PMIx function.  The
/// `function` parameter identifies the PMIx API function (e.g. `"PMIx_Get"`),
/// and `attrs` is a slice of attribute key names that the host supports for
/// that function.
///
/// This function requires that the PMIx library has been initialized via
/// [`PMIx_Init`][crate::lifecycle::init] (or the server equivalent).  Calling
/// it before initialization returns [`PmixStatus::ErrInit`].
///
/// Registered attributes are stored under the *host attributes* level
/// (`PMIX_HOST_ATTRIBUTES`).  Attempting to register the same function twice
/// returns [`PmixStatus::ErrRepeatAttrRegistration`].
///
/// # C API
/// `pmix_status_t PMIx_Register_attributes(char *function, char *attrs[])`
///
/// The C `attrs` parameter is a NULL-terminated argv-style array of `char*`
/// strings.  This wrapper accepts a Rust `&[&str]` and handles the conversion.
///
/// # Returns
/// * `Ok(())` — attributes were registered successfully.
/// * `Err(PmixStatus::ErrInit)` — PMIx has not been initialized.
/// * `Err(PmixStatus::ErrBadParam)` — function name is empty or contains a NUL byte.
/// * `Err(PmixStatus::ErrRepeatAttrRegistration)` — the function was already
///   registered at the host level.
///
/// # Examples
/// ```no_run
/// use pmix::utility::register_attributes;
///
/// // Register attributes that the host supports for PMIx_Get.
/// let attrs = &["pmix.get.timeout", "pmix.get.scope"][..];
/// register_attributes("PMIx_Get", attrs).expect("PMIx must be initialized");
/// ```
pub fn register_attributes(function: &str, attrs: &[&str]) -> Result<(), PmixStatus> {
    // Convert function name to C string.
    let function_cstr = std::ffi::CString::new(function).map_err(|_| PmixStatus::from_raw(-27))?; // PMIX_ERR_BAD_PARAM

    // Build a NULL-terminated array of C strings for the attrs parameter.
    // The C API expects `char *attrs[]` — a NULL-terminated argv-style array.
    // We convert each &str to a CString, collect the raw pointers, and append
    // a null terminator. The CStrs and the Vec must outlive the FFI call.
    let cstrings: Vec<std::ffi::CString> = attrs
        .iter()
        .map(|s| std::ffi::CString::new(*s).unwrap_or_else(|_| std::ffi::CString::new("").unwrap()))
        .collect();

    // Build the NULL-terminated pointer array.
    // The FFI signature expects *mut *mut c_char, but we are only passing
    // immutable data. We use *const *const c_char cast to *mut *mut c_char
    // because the PMIx implementation does not modify the array contents
    // (it copies the strings internally via pmix_argv_copy).
    let attr_ptrs: Vec<*mut std::os::raw::c_char> = cstrings
        .iter()
        .map(|cs| cs.as_ptr() as *mut std::os::raw::c_char)
        .chain(std::iter::once(std::ptr::null_mut()))
        .collect();

    // SAFETY: PMIx_Register_attributes takes a char* for the function name
    // and a NULL-terminated char* array for attributes. Both function_cstr
    // and attr_ptrs (backed by cstrings) live until the end of this scope,
    // so the pointers remain valid for the duration of the FFI call.
    // The PMIx library copies the strings internally (strdup/pmix_argv_copy)
    // and does not retain the pointers after return.
    //
    // The bindgen-generated signature uses *mut c_char for both parameters,
    // but the PMIx implementation does not modify the inputs — it only reads
    // and copies them. The cast from *const to *mut is therefore safe.
    let status = unsafe {
        ffi::PMIx_Register_attributes(
            function_cstr.as_ptr() as *mut std::os::raw::c_char,
            attr_ptrs.as_ptr() as *mut *mut std::os::raw::c_char,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_IOF_pull — IO forwarding registration
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Global registry mapping IOF handles to their Rust callback contexts.
///
/// PMIx_IOF_pull stores only the C function pointer for the IO callback.
/// Our bridge function looks up the handle in this registry to find the
/// corresponding Rust closure. The registry is populated when `iof_pull`
/// or `iof_pull_blocking` is called and cleared when deregistered.
type Registry = HashMap<usize, SendSyncPtr<*mut IoPullContext>>;
static IOF_REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Wrapper that allows raw pointers to cross thread boundaries.
///
/// The raw pointer itself is managed by the PMIx FFI bridge — it is
/// allocated via `Box::into_raw` and freed via `Box::from_raw` or
/// on deregistration. This newtype only exists so the `HashMap` inside
/// `IOF_REGISTRY` satisfies `Send + Sync`.
#[derive(Clone, Copy)]
struct SendSyncPtr<T>(T);
// SAFETY: The pointer is only accessed behind a Mutex guard, so concurrent
// access is serialized. The lifetime of the pointed-to data is managed by
// the caller (Box::into_raw / Box::from_raw), not by Rust's ownership rules.
unsafe impl<T> Send for SendSyncPtr<T> {}
unsafe impl<T> Sync for SendSyncPtr<T> {}

/// Context stored per IO pull registration, carrying both callbacks.
///
/// Allocated on the heap via `Box::into_raw`. Freed when:
/// - Registration fails (immediate)
/// - `iof_deregister` is called (via registry cleanup)
/// - Process exits (OS reclaims memory)
///
/// Type aliases for the callback types used below.
type IoDataCallback = Box<dyn Fn(usize, IOFChannelFlags, &ffi::pmix_proc_t, &[u8]) + Send>;
type IoRegCallback = Box<dyn Fn(PmixStatus, usize) + Send>;

struct IoPullContext {
    io_cb: IoDataCallback,
    reg_cb: IoRegCallback,
}

/// C bridge for the IO callback (`pmix_iof_cbfunc_t`).
///
/// Called by PMIx each time IO data arrives for a registered process.
/// Looks up the `IoPullContext` in the global registry using `iofhdlr`.
extern "C" fn io_callback_bridge(
    iofhdlr: usize,
    channel: ffi::pmix_iof_channel_t,
    source: *mut ffi::pmix_proc_t,
    payload: *mut ffi::pmix_byte_object_t,
    _info: *mut ffi::pmix_info_t,
    _ninfo: usize,
) {
    // Look up the context in the registry.
    let registry = IOF_REGISTRY.lock().unwrap();
    let ctx_ptr = match registry.get(&iofhdlr) {
        Some(wrapped) => wrapped.0,
        None => return, // Context not found — skip.
    };
    drop(registry); // Release lock before calling user code.

    if ctx_ptr.is_null() {
        return;
    }

    // SAFETY: ctx_ptr was allocated via Box::into_raw in iof_pull /
    // iof_pull_blocking and remains valid until deregistration.
    let ctx = unsafe { &*ctx_ptr };

    // SAFETY: source is valid for the duration of this callback.
    // PMIx guarantees the source pointer points to a valid pmix_proc_t.
    let source_proc = unsafe { &*source };

    // SAFETY: payload is valid for the duration of this callback.
    // Extract bytes from the pmix_byte_object_t.
    let bytes = if !payload.is_null() {
        let p = unsafe { &*payload };
        if !p.bytes.is_null() && p.size > 0 {
            unsafe { std::slice::from_raw_parts(p.bytes as *const u8, p.size) }
        } else {
            &[]
        }
    } else {
        &[]
    };

    let channel_flags = IOFChannelFlags(channel);
    (ctx.io_cb)(iofhdlr, channel_flags, source_proc, bytes);
}

/// C bridge for the registration callback (`pmix_hdlr_reg_cbfunc_t`).
///
/// Called by PMIx when the async registration request completes.
/// The `cbdata` parameter points to our `IoPullContext`.
extern "C" fn reg_callback_bridge(
    status: ffi::pmix_status_t,
    refid: usize,
    cbdata: *mut std::os::raw::c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the ctx_ptr we passed to PMIx_IOF_pull.
    // It was allocated via Box::into_raw and is valid.
    let ctx = unsafe { &*(cbdata as *const IoPullContext) };

    // Register the handle in the global registry so the IO callback
    // can look it up later.
    {
        let mut registry = IOF_REGISTRY.lock().unwrap();
        registry.insert(refid, SendSyncPtr(cbdata as *mut IoPullContext));
    }

    let pmix_status = PmixStatus::from_raw(status);
    (ctx.reg_cb)(pmix_status, refid);
}

/// Callback trait for receiving IO data from remote processes.
///
/// Implementations receive:
/// * `handle` — the registration handle returned by `iof_pull`.
/// * `channel` — the IO channel (`stdin`, `stdout`, `stderr`, `stddiag`).
/// * `source` — the process that produced the IO data.
/// * `payload` — the raw byte payload (may be empty).
pub trait IoForwardHandler:
    Fn(usize, IOFChannelFlags, &ffi::pmix_proc_t, &[u8]) + Send + 'static
{
}
impl<F> IoForwardHandler for F where
    F: Fn(usize, IOFChannelFlags, &ffi::pmix_proc_t, &[u8]) + Send + 'static
{
}

/// Callback trait for registration completion notification.
pub trait IoForwardRegHandler: Fn(PmixStatus, usize) + Send + 'static {}
impl<F> IoForwardRegHandler for F where F: Fn(PmixStatus, usize) + Send + 'static {}

/// Register to receive IO forwarded from a set of remote processes (async).
///
/// The `regcb` closure is called when the PMIx server finishes processing
/// the registration request. Use `iof_pull_blocking` if you prefer a
/// synchronous return value.
///
/// # Parameters
/// * `procs` — process identifiers whose IO is being requested.
///   Use `PMIX_RANK_WILDCARD` for all processes in a namespace.
/// * `directives` — optional `pmix_info_t` directives (buffering, tagging, etc.).
/// * `channel` — bitmask of IO channels to receive.
/// * `cb` — Rust closure called for each IO event.
/// * `regcb` — Rust closure called when registration completes.
///
/// # Returns
/// * `Ok(())` — registration was submitted. `regcb` will be invoked.
/// * `Err(PmixStatus)` — registration could not be submitted.
///
/// # C API
/// `pmix_status_t PMIx_IOF_pull(const pmix_proc_t procs[], size_t nprocs,`
///     `const pmix_info_t directives[], size_t ndirs,`
///     `pmix_iof_channel_t channel, pmix_iof_cbfunc_t cbfunc,`
///     `pmix_hdlr_reg_cbfunc_t regcbfunc, void *regcbdata);`
pub fn iof_pull<F, G>(
    procs: &[ffi::pmix_proc_t],
    directives: &[ffi::pmix_info_t],
    channel: IOFChannelFlags,
    cb: F,
    regcb: G,
) -> Result<(), PmixStatus>
where
    F: IoForwardHandler,
    G: IoForwardRegHandler,
{
    // Box both closures into a single context struct.
    let ctx = IoPullContext {
        io_cb: Box::new(cb),
        reg_cb: Box::new(regcb),
    };
    let ctx_ptr: *mut IoPullContext = Box::into_raw(Box::new(ctx));

    // SAFETY: PMIx_IOF_pull is a documented PMIx tool API.
    // - procs: valid slice, passed as const pointer + length.
    // - directives: valid slice, passed as const pointer + length.
    // - channel: valid u16 bitmask.
    // - cbfunc: our io_callback_bridge extern "C" function.
    // - regcbfunc: our reg_callback_bridge extern "C" function.
    // - regcbdata: ctx_ptr, owned by us, valid until deregistration.
    let raw_status = unsafe {
        ffi::PMIx_IOF_pull(
            procs.as_ptr(),
            procs.len(),
            directives.as_ptr(),
            directives.len(),
            channel.raw(),
            Some(io_callback_bridge),
            Some(reg_callback_bridge),
            ctx_ptr as *mut std::os::raw::c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_error() {
        // Registration could not be submitted — free the context immediately.
        // SAFETY: ctx_ptr was allocated by Box::into_raw above and has not
        // been used by PMIx since the call returned an error.
        unsafe {
            drop(Box::from_raw(ctx_ptr));
        }
        Err(pmix_status)
    } else {
        // Registration accepted — the handle will be returned in regcbfunc.
        Ok(())
    }
}

/// Blocking variant of `iof_pull` — waits for registration to complete
/// and returns the registration handle directly.
///
/// # Parameters
/// * `procs` — process identifiers whose IO is being requested.
/// * `directives` — optional `pmix_info_t` directives.
/// * `channel` — bitmask of IO channels to receive.
/// * `cb` — Rust closure called for each IO event.
///
/// # Returns
/// * `Ok(handle)` — the registration handle (use with `iof_deregister`).
/// * `Err(PmixStatus)` — registration failed.
///
/// # C API
/// Same as `PMIx_IOF_pull` with `regcbfunc` set to NULL (blocking mode).
pub fn iof_pull_blocking<F>(
    procs: &[ffi::pmix_proc_t],
    directives: &[ffi::pmix_info_t],
    channel: IOFChannelFlags,
    cb: F,
) -> Result<usize, PmixStatus>
where
    F: IoForwardHandler,
{
    // In blocking mode, regcbfunc is NULL, so we don't need a real reg_cb.
    let ctx = IoPullContext {
        io_cb: Box::new(cb),
        reg_cb: Box::new(|_, _| {
            // Unused in blocking mode.
        }),
    };
    let ctx_ptr: *mut IoPullContext = Box::into_raw(Box::new(ctx));

    // SAFETY: Same as iof_pull, but regcbfunc is None (blocking mode).
    let raw_result: ffi::pmix_status_t = unsafe {
        ffi::PMIx_IOF_pull(
            procs.as_ptr(),
            procs.len(),
            directives.as_ptr(),
            directives.len(),
            channel.raw(),
            Some(io_callback_bridge),
            None, // blocking mode — no async registration callback
            ctx_ptr as *mut std::os::raw::c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_result);
    if pmix_status.is_error() {
        // Registration failed — free the context.
        // SAFETY: ctx_ptr was not handed to PMIx since the call failed.
        unsafe {
            drop(Box::from_raw(ctx_ptr));
        }
        Err(pmix_status)
    } else {
        // In blocking mode, the return value is the registration handle.
        let handle = raw_result as usize;

        // Store the context in the registry so the IO callback can find it.
        {
            let mut registry = IOF_REGISTRY.lock().unwrap();
            registry.insert(handle, SendSyncPtr(ctx_ptr));
        }
        Ok(handle)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_IOF_deregister — IO forwarding deregistration
// ─────────────────────────────────────────────────────────────────────────────

/// Callback for deregistration completion notification.
///
/// Invoked when the PMIx server finishes processing the deregistration
/// request. Receives the final status and the deregistration handle.
pub trait IoForwardDeregHandler: Fn(PmixStatus) + Send + 'static {}
impl<F> IoForwardDeregHandler for F where F: Fn(PmixStatus) + Send + 'static {}

/// Context stored per deregistration request, carrying the Rust callback.
///
/// Allocated on the heap via `Box::into_raw`. Freed when the deregistration
/// callback fires (via `Box::from_raw`), or immediately if the FFI call fails.
struct IoDeregContext {
    cb: Box<dyn Fn(PmixStatus) + Send>,
}

/// C bridge for the deregistration callback (`pmix_op_cbfunc_t`).
///
/// Called by PMIx when the deregistration request completes asynchronously.
/// The `cbdata` parameter points to our `IoDeregContext`.
extern "C" fn dereg_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut std::os::raw::c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata was created from `Box::into_raw(Box::new(ctx))` where
    // `ctx: IoDeregContext`. We reconstruct the original boxed context via
    // `Box::from_raw` and drop it, which calls the closure and frees memory.
    let boxed_ctx: Box<IoDeregContext> = unsafe { Box::from_raw(cbdata as *mut IoDeregContext) };

    let pmix_status = PmixStatus::from_raw(status);

    // Invoke the user's deregistration callback, then the Box drops,
    // freeing both the context and the contained closure.
    (boxed_ctx.cb)(pmix_status);
}

/// Deregister from IO forwarding previously established via `iof_pull`.
///
/// This tells the PMIx server to stop forwarding IO from the processes
/// that were registered under the given handle. Any buffered IO data
/// should be flushed before the deregistration completes.
///
/// # Parameters
/// * `handle` — the registration handle returned by `iof_pull` or
///   `iof_pull_blocking`.
/// * `directives` — optional `pmix_info_t` directives (e.g., timeout).
/// * `cb` — Rust closure called when deregistration completes. Receives
///   the final `PmixStatus` and the handle.
///
/// # Returns
/// * `Ok(())` — deregistration was submitted. The callback will be invoked
///   when the server finishes processing.
/// * `Err(PmixStatus)` — deregistration could not be submitted (e.g.,
///   invalid handle).
///
/// # Note
/// On success, the callback receives `PMIX_SUCCESS` (async processing) or
/// `PMIX_OPERATION_SUCCEEDED` (immediate completion, in which case the
/// callback is still called). On failure, the callback is not invoked.
///
/// # C API
/// `pmix_status_t PMIx_IOF_deregister(size_t iofhdlr,`
///     `const pmix_info_t directives[], size_t ndirs,`
///     `pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn iof_deregister<F>(
    handle: usize,
    directives: &[ffi::pmix_info_t],
    cb: F,
) -> Result<(), PmixStatus>
where
    F: IoForwardDeregHandler,
{
    // Remove the handle from the global registry immediately so no further
    // IO callbacks will be delivered for this registration.
    {
        let mut registry = IOF_REGISTRY.lock().unwrap();
        if let Some(ctx_wrapped) = registry.remove(&handle) {
            let ctx_ptr = ctx_wrapped.0;
            if !ctx_ptr.is_null() {
                // SAFETY: ctx_ptr was allocated via Box::into_raw in
                // iof_pull / iof_pull_blocking and has not been freed yet.
                // We take ownership back and drop it, which frees the
                // IoPullContext and its contained closures.
                unsafe {
                    drop(Box::from_raw(ctx_ptr));
                }
            }
        }
    }

    // Box the deregistration callback context so we can pass it as `*mut c_void`.
    let ctx = IoDeregContext { cb: Box::new(cb) };
    let ctx_ptr: *mut IoDeregContext = Box::into_raw(Box::new(ctx));

    // SAFETY: PMIx_IOF_deregister is a documented PMIx tool API.
    // - iofhdlr: valid registration handle from iof_pull.
    // - directives: valid slice, passed as const pointer + length.
    // - cbfunc: our dereg_callback_bridge extern "C" function.
    // - cbdata: ctx_ptr, owned by us, valid until the callback fires.
    let raw_status = unsafe {
        ffi::PMIx_IOF_deregister(
            handle,
            directives.as_ptr(),
            directives.len(),
            Some(dereg_callback_bridge),
            ctx_ptr as *mut std::os::raw::c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_error() {
        // Deregistration could not be submitted — free the context immediately.
        // SAFETY: ctx_ptr was allocated by Box::into_raw above and has not
        // been used by PMIx since the call returned an error.
        unsafe {
            drop(Box::from_raw(ctx_ptr));
        }
        Err(pmix_status)
    } else {
        // Deregistration accepted — callback will be invoked by PMIx.
        // The registry entry and IoPullContext have already been cleaned up.
        Ok(())
    }
}

/// Blocking variant of `iof_deregister` — waits for deregistration to
/// complete and returns the final status directly.
///
/// # Parameters
/// * `handle` — the registration handle returned by `iof_pull` or
///   `iof_pull_blocking`.
/// * `directives` — optional `pmix_info_t` directives.
///
/// # Returns
/// * `Ok(())` — deregistration completed successfully.
/// * `Err(PmixStatus)` — deregistration failed.
///
/// # C API
/// Same as `PMIx_IOF_deregister` with `cbfunc` set to NULL (blocking mode).
pub fn iof_deregister_blocking(
    handle: usize,
    directives: &[ffi::pmix_info_t],
) -> Result<(), PmixStatus> {
    // Remove the handle from the global registry immediately.
    {
        let mut registry = IOF_REGISTRY.lock().unwrap();
        if let Some(ctx_wrapped) = registry.remove(&handle) {
            let ctx_ptr = ctx_wrapped.0;
            if !ctx_ptr.is_null() {
                // SAFETY: ctx_ptr was allocated via Box::into_raw in
                // iof_pull / iof_pull_blocking and has not been freed yet.
                unsafe {
                    drop(Box::from_raw(ctx_ptr));
                }
            }
        }
    }

    // SAFETY: PMIx_IOF_deregister with NULL callback = blocking mode.
    // The call does not return until the server has processed the
    // deregistration request. No cbdata is needed.
    let raw_status = unsafe {
        ffi::PMIx_IOF_deregister(
            handle,
            directives.as_ptr(),
            directives.len(),
            None, // blocking mode — no callback
            ptr::null_mut(),
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject — wrapper for pmix_byte_object_t
// ─────────────────────────────────────────────────────────────────────────────

/// A byte object wrapper for PMIx I/O push operations.
///
/// Corresponds to the C type `pmix_byte_object_t`, which holds a raw
/// pointer to a buffer and its size. This wrapper takes ownership of
/// the bytes and provides a safe API for constructing and passing
/// data to `PMIx_IOF_push`.
///
/// # Example
/// ```no_run
/// use pmix::utility::PmixByteObject;
///
/// let data = b"hello from stdin";
/// let bo = PmixByteObject::from_slice(data);
/// // Pass `bo` to iof_push(...).
/// ```
#[derive(Debug, Clone)]
pub struct PmixByteObject {
    bytes: Vec<u8>,
}

impl PmixByteObject {
    /// Create a `PmixByteObject` from a byte slice (copies the data).
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            bytes: data.to_vec(),
        }
    }

    /// Create a `PmixByteObject` from a `Vec<u8>` (takes ownership).
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self { bytes: data }
    }

    /// Create an empty `PmixByteObject`.
    pub fn empty() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Returns `true` if the byte object contains no data.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the number of bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns the underlying byte slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert to a C `pmix_byte_object_t` for FFI.
    ///
    /// Returns a heap-allocated `pmix_byte_object_t` whose `bytes` field
    /// points to a `CString`-wrapped copy of our data. The caller must
    /// free the result with `PmixByteObject::free_c_ptr()`.
    ///
    /// Note: `PMIx_IOF_push` copies the byte object's data internally,
    /// so the C struct can be freed immediately after the call returns.
    fn as_c_mut_ptr(&self) -> *mut ffi::pmix_byte_object_t {
        // Allocate a CString from our bytes so the C struct has a valid
        // pointer. The CString is stored inside the C struct's bytes field.
        let c_str = if self.bytes.is_empty() {
            std::ptr::null_mut()
        } else {
            // SAFETY: Our bytes are owned by self (Vec<u8>) and will outlive
            // the FFI call. We create a mutable copy for the C struct.
            let mut data = self.bytes.clone();
            data.as_mut_ptr() as *mut std::os::raw::c_char
        };

        // Build the C struct on the heap.
        let c_bo = Box::new(ffi::pmix_byte_object_t {
            bytes: c_str,
            size: self.bytes.len(),
        });
        Box::into_raw(c_bo)
    }

    /// Free a C `pmix_byte_object_t` that was created by `as_c_mut_ptr`.
    ///
    /// SAFETY: `ptr` must have been returned by `as_c_mut_ptr()` and must
    /// not have been freed already.
    unsafe fn free_c_ptr(ptr: *mut ffi::pmix_byte_object_t) {
        if !ptr.is_null() {
            // The bytes field points into a Vec<u8> that was owned by the
            // Rust PmixByteObject (not allocated by pmix_malloc), so we
            // must NOT call PMIx_Byte_object_destruct (which calls pmix_free).
            // Just free the struct itself.
            unsafe { drop(Box::from_raw(ptr)) };
        }
    }
}

impl AsRef<[u8]> for PmixByteObject {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_IOF_push — push local data to remote process stdin
// ─────────────────────────────────────────────────────────────────────────────

/// Callback for push operation completion notification.
///
/// Invoked when the PMIx server finishes processing the push request.
/// Receives the final status of the operation.
pub trait IoForwardPushHandler: Fn(PmixStatus) + Send + 'static {}
impl<F> IoForwardPushHandler for F where F: Fn(PmixStatus) + Send + 'static {}

/// Context stored per IO push request, carrying the Rust callback.
///
/// Allocated on the heap via `Box::into_raw`. Freed when:
/// - The push callback fires (via `Box::from_raw`)
/// - The FFI call returns an error immediately (no callback invoked)
/// - Process exits (OS reclaims memory)
struct IoPushContext {
    cb: Box<dyn Fn(PmixStatus) + Send>,
}

/// C bridge for the push completion callback (`pmix_op_cbfunc_t`).
///
/// Called by PMIx when the async push request completes.
/// The `cbdata` parameter points to our `IoPushContext`.
extern "C" fn push_callback_bridge(status: ffi::pmix_status_t, cbdata: *mut std::os::raw::c_void) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the ctx_ptr we passed to PMIx_IOF_push.
    // It was allocated via Box::into_raw and is valid until this callback.
    let ctx_ptr = cbdata as *mut IoPushContext;
    // Take ownership back — this callback is the last reference.
    let ctx = unsafe { Box::from_raw(ctx_ptr) };

    let pmix_status = PmixStatus::from_raw(status);
    (ctx.cb)(pmix_status);
}

/// Push data collected locally (typically from stdin) to stdin of target
/// remote processes (async).
///
/// The `cb` closure is called when the PMIx server finishes processing
/// the push request. Use `iof_push_blocking` if you prefer a synchronous
/// return value.
///
/// # Parameters
/// * `targets` — process identifiers to which the data should be delivered.
///   Use `PMIX_RANK_WILDCARD` for all processes in a namespace.
/// * `bo` — byte object containing the payload (e.g., stdin data).
/// * `directives` — optional `pmix_info_t` directives (buffering, etc.).
/// * `cb` — Rust closure called when the push operation completes.
///
/// # Returns
/// * `Ok(())` — push was submitted. `cb` will be invoked with the result.
/// * `Err(PmixStatus)` — push could not be submitted (immediate error).
///
/// # C API
/// `pmix_status_t PMIx_IOF_push(const pmix_proc_t targets[], size_t ntargets,`
///     `pmix_byte_object_t *bo, const pmix_info_t directives[], size_t ndirs,`
///     `pmix_op_cbfunc_t cbfunc, void *cbdata);`
pub fn iof_push<F>(
    targets: &[ffi::pmix_proc_t],
    bo: PmixByteObject,
    directives: &[ffi::pmix_info_t],
    cb: F,
) -> Result<(), PmixStatus>
where
    F: IoForwardPushHandler,
{
    // Allocate the C byte_object_t on the heap.
    let c_bo_ptr = bo.as_c_mut_ptr();

    // Box the callback into a context struct.
    let ctx = IoPushContext { cb: Box::new(cb) };
    let ctx_ptr: *mut IoPushContext = Box::into_raw(Box::new(ctx));

    // SAFETY: PMIx_IOF_push is a documented PMIx tool API.
    // - targets: valid slice, passed as const pointer + length.
    // - bo: heap-allocated pmix_byte_object_t, valid for duration of call.
    //   PMIx may copy or retain the data internally.
    // - directives: valid slice, passed as const pointer + length.
    // - cbfunc: our push_callback_bridge extern "C" function.
    // - cbdata: ctx_ptr, owned by us, reclaimed in the callback.
    let raw_status = unsafe {
        ffi::PMIx_IOF_push(
            targets.as_ptr(),
            targets.len(),
            c_bo_ptr,
            directives.as_ptr(),
            directives.len(),
            Some(push_callback_bridge),
            ctx_ptr as *mut std::os::raw::c_void,
        )
    };

    // Free the C byte_object — PMIx has already copied/retained the data
    // internally by the time this returns.
    // SAFETY: c_bo_ptr was allocated by as_c_mut_ptr() above.
    unsafe { PmixByteObject::free_c_ptr(c_bo_ptr) };

    let pmix_status = PmixStatus::from_raw(raw_status);

    // Per spec: PMIX_SUCCESS means async processing (callback will fire).
    // PMIX_OPERATION_SUCCEEDED means immediate success (callback NOT called).
    // Any error means immediate failure (callback NOT called).
    if pmix_status.is_error() {
        // Immediate error — callback will NOT be called. Free context.
        // SAFETY: ctx_ptr was not handed to PMIx since the call returned error.
        unsafe {
            drop(Box::from_raw(ctx_ptr));
        }
        Err(pmix_status)
    } else {
        // Either async (callback will fire) or immediate success.
        Ok(())
    }
}

/// Blocking variant of `iof_push` — waits for the push to complete
/// and returns the result directly.
///
/// # Parameters
/// * `targets` — process identifiers to which the data should be delivered.
/// * `bo` — byte object containing the payload.
/// * `directives` — optional `pmix_info_t` directives.
///
/// # Returns
/// * `Ok(())` — push completed successfully.
/// * `Err(PmixStatus)` — push failed.
///
/// # C API
/// Same as `PMIx_IOF_push` with `cbfunc` set to NULL (blocking mode).
pub fn iof_push_blocking(
    targets: &[ffi::pmix_proc_t],
    bo: PmixByteObject,
    directives: &[ffi::pmix_info_t],
) -> Result<(), PmixStatus> {
    // Allocate the C byte_object_t on the heap.
    let c_bo_ptr = bo.as_c_mut_ptr();

    // SAFETY: PMIx_IOF_push with NULL callback = blocking mode.
    // The call does not return until the server has processed the
    // push request. No cbdata is needed.
    // - targets: valid slice, passed as const pointer + length.
    // - bo: heap-allocated pmix_byte_object_t.
    // - directives: valid slice, passed as const pointer + length.
    // - cbfunc: None (blocking mode).
    // - cbdata: null (unused in blocking mode).
    let raw_status = unsafe {
        ffi::PMIx_IOF_push(
            targets.as_ptr(),
            targets.len(),
            c_bo_ptr,
            directives.as_ptr(),
            directives.len(),
            None, // blocking mode — no callback
            ptr::null_mut(),
        )
    };

    // Free the C byte_object — PMIx has already processed the data.
    // SAFETY: c_bo_ptr was allocated by as_c_mut_ptr() above.
    unsafe { PmixByteObject::free_c_ptr(c_bo_ptr) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (unit, no PMIx runtime required)
// ─────────────────────────────────────────────────────────────────────────────


#[cfg(test)]
mod tests;
