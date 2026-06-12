//! Utility functions — `PMIx_Initialized`, `PMIx_Error_string`, `PMIx_Proc_state_string`,
//! `PMIx_Scope_string`, `PMIx_Persistence_string`, `PMIx_Data_range_string`,
//! `PMIx_Info_directives_string`, `PMIx_Data_type_string`, `PMIx_Alloc_directive_string`,
//! `PMIx_IOF_channel_string`, `PMIx_Job_state_string`, `PMIx_Get_attribute_string`,
//! `PMIx_Get_attribute_name`, `PMIx_Link_state_string`, and related helpers.
//!
//! This module provides safe Rust wrappers around PMIx utility APIs
//! that do not fit into the lifecycle, data, or event categories.

use crate::{ffi, IOFChannelFlags, InfoFlags, PmixAllocDirective, PmixDataRange, PmixDataType, PmixDeviceType, PmixJobState, PmixLinkState, PmixPersistence, PmixProcState, PmixScope, PmixStatus};

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
/// use pmix::{utility::error_string, PmixStatus, PmixError};
///
/// let status: PmixStatus = PmixError::Success.into();
/// let desc = error_string(status)?;
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
// Tests (unit, no PMIx runtime required)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `initialized()` is callable and returns a bool.
    ///
    /// Before `PMIx_Init` has been called, the PMIx library's internal
    /// `pmix_globals.initialized` flag is `false`, so we expect `false`.
    ///
    /// Note: this test calls into the real PMIx library. If `libpmix` is
    /// not linked or the library version differs, the FFI call may panic
    /// or return unexpected results. In a CI environment without a running
    /// PMIx daemon, this still works because `PMIx_Initialized` only reads
    /// a local atomic flag — it does not contact the server.
    #[test]
    fn test_initialized_before_init_is_false() {
        let result = initialized();
        // Before PMIx_Init, the client is not initialized.
        assert!(
            !result,
            "PMIx_Initialized should return false before PMIx_Init"
        );
    }

    /// `initialized()` is idempotent — calling it multiple times returns
    /// the same value (no side effects).
    #[test]
    fn test_initialized_idempotent() {
        let first = initialized();
        let second = initialized();
        assert_eq!(first, second, "PMIx_Initialized should be idempotent");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Error_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `error_string` returns `Ok(String)` for known status codes.
    ///
    /// PMIx_Error_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_status_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_error_string_success() {
        let status = PmixStatus::from_raw(0); // PMIX_SUCCESS
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string(PMIX_SUCCESS) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(!desc.is_empty(), "error_string should not return an empty string");
    }

    /// `error_string` returns a readable description for PMIX_ERROR (-1).
    #[test]
    fn test_error_string_generic_error() {
        let status = PmixStatus::from_raw(-1); // PMIX_ERROR
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string(PMIX_ERROR) should return Ok, got {:?}",
            result
        );
    }

    /// `error_string` handles negative error codes in various subsystem
    /// ranges (timeout, bad parameter, not found, etc.).
    #[test]
    fn test_error_string_various_codes() {
        let codes: Vec<i32> = vec![
            0,    // PMIX_SUCCESS
            -1,   // PMIX_ERROR
            -24,  // PMIX_ERR_TIMEOUT
            -27,  // PMIX_ERR_BAD_PARAM
            -33,  // PMIX_ERR_NOT_FOUND
        ];
        for code in codes {
            let status = PmixStatus::from_raw(code);
            let result = error_string(status);
            assert!(
                result.is_ok(),
                "error_string({}) should return Ok, got {:?}",
                code,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "error_string({}) should not return empty string",
                code
            );
        }
    }

    /// `error_string` handles unknown/user-defined status codes (below -9999).
    ///
    /// PMIx reserves values below PMIX_EXTERNAL_ERR_BASE (-9999) for
    /// user/implementation-defined codes. The C function should still
    /// return a string (typically indicating an external error).
    #[test]
    fn test_error_string_unknown_code() {
        let status = PmixStatus::from_raw(-10001); // User-defined range
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string should handle unknown codes gracefully, got {:?}",
            result
        );
    }

    /// `error_string` is deterministic — the same status code always
    /// returns the same string.
    #[test]
    fn test_error_string_deterministic() {
        let status = PmixStatus::from_raw(-24); // PMIX_ERR_TIMEOUT
        let first = error_string(status).unwrap();
        let second = error_string(status).unwrap();
        assert_eq!(
            first, second,
            "error_string must be deterministic for the same input"
        );
    }

    /// `error_string` returns different strings for different status codes.
    #[test]
    fn test_error_string_distinct() {
        let success = error_string(PmixStatus::from_raw(0)).unwrap();
        let error = error_string(PmixStatus::from_raw(-1)).unwrap();
        assert_ne!(
            success, error,
            "error_string(SUCCESS) and error_string(ERROR) must differ"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Proc_state_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `proc_state_string` returns `Ok(String)` for known process states.
    ///
    /// PMIx_Proc_state_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_proc_state_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_proc_state_string_running() {
        let state = PmixProcState::Running;
        let result = proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string(Running) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(!desc.is_empty(), "proc_state_string should not return an empty string");
    }

    /// `proc_state_string` returns the expected string for key lifecycle states.
    #[test]
    fn test_proc_state_string_key_states() {
        use crate::PmixProcState::*;

        let states = [
            Undef,
            Prepped,
            LaunchUnderway,
            Running,
            Connected,
            Terminated,
            Error,
            Aborted,
        ];
        for state in states {
            let result = proc_state_string(state);
            assert!(
                result.is_ok(),
                "proc_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "proc_state_string({:?}) should not return empty string",
                state
            );
        }
    }

    /// `proc_state_string` is deterministic — the same state always returns
    /// the same string.
    #[test]
    fn test_proc_state_string_deterministic() {
        let state = PmixProcState::Terminated;
        let first = proc_state_string(state).unwrap();
        let second = proc_state_string(state).unwrap();
        assert_eq!(
            first, second,
            "proc_state_string must be deterministic for the same input"
        );
    }

    /// `proc_state_string` returns different strings for different states.
    #[test]
    fn test_proc_state_string_distinct() {
        let running = proc_state_string(PmixProcState::Running).unwrap();
        let terminated = proc_state_string(PmixProcState::Terminated).unwrap();
        assert_ne!(
            running, terminated,
            "proc_state_string(Running) and proc_state_string(Terminated) must differ"
        );
    }

    /// `proc_state_string` handles all error-range states (50–63).
    #[test]
    fn test_proc_state_string_error_range() {
        use crate::PmixProcState::*;

        let error_states = [
            Error, KilledByCmd, Aborted, FailedToStart, AbortedBySig,
            TermWoSync, CommFailed, SensorBoundExceeded, CalledAbort,
            HeartbeatFailed, Migrating, CannotRestart, TermNonZero,
            FailedToLaunch,
        ];
        for state in error_states {
            let result = proc_state_string(state);
            assert!(
                result.is_ok(),
                "proc_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
        }
    }

    /// `proc_state_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_proc_state_string_unknown() {
        let state = PmixProcState::Unknown(99);
        let result = proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string(Unknown(99)) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        // The C library returns "UNKNOWN STATE" for unrecognized values.
        assert!(
            !desc.is_empty(),
            "proc_state_string for unknown state should return non-empty string"
        );
    }

    /// `PmixProcState::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_proc_state_from_raw_to_raw_roundtrip() {
        use crate::PmixProcState::*;

        let states = [
            Undef, Prepped, LaunchUnderway, Restart, Terminate,
            Running, Connected, Unterminated, Terminated, Error,
            KilledByCmd, Aborted, FailedToStart, AbortedBySig,
            TermWoSync, CommFailed, SensorBoundExceeded, CalledAbort,
            HeartbeatFailed, Migrating, CannotRestart, TermNonZero,
            FailedToLaunch,
        ];
        for state in states {
            let raw = state.to_raw();
            let recovered = PmixProcState::from_raw(raw);
            assert_eq!(
                state, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                state
            );
        }
    }

    /// `PmixProcState::is_alive` and `is_terminated` classify states correctly.
    #[test]
    fn test_proc_state_classification() {
        use crate::PmixProcState::*;

        assert!(Running.is_alive());
        assert!(Connected.is_alive());
        assert!(Prepped.is_alive());
        assert!(!Running.is_terminated());

        assert!(Terminated.is_terminated());
        assert!(Aborted.is_terminated());
        assert!(KilledByCmd.is_terminated());
        assert!(!Terminated.is_alive());
        assert!(!Aborted.is_alive());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Scope_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `scope_string` returns `Ok(String)` for all known scope values.
    ///
    /// PMIx_Scope_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_scope_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_scope_string_all_known() {
        use crate::PmixScope::*;

        let scopes = [Undef, Local, Remote, Global, Internal];
        for scope in scopes {
            let result = scope_string(scope);
            assert!(
                result.is_ok(),
                "scope_string({:?}) should return Ok, got {:?}",
                scope,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "scope_string({:?}) should not return empty string",
                scope
            );
        }
    }

    /// `scope_string` returns the expected strings for key scopes.
    #[test]
    fn test_scope_string_expected_values() {
        use crate::PmixScope::*;

        let local = scope_string(Local).unwrap();
        let remote = scope_string(Remote).unwrap();
        let global = scope_string(Global).unwrap();

        assert!(local.to_lowercase().contains("local"), "Local scope string should contain 'local', got '{}'", local);
        assert!(remote.to_lowercase().contains("remote"), "Remote scope string should contain 'remote', got '{}'", remote);
        assert!(global.to_lowercase().contains("global"), "Global scope string should contain 'global', got '{}'", global);
    }

    /// `scope_string` is deterministic — the same scope always returns
    /// the same string.
    #[test]
    fn test_scope_string_deterministic() {
        use crate::PmixScope::Global;
        let first = scope_string(Global).unwrap();
        let second = scope_string(Global).unwrap();
        assert_eq!(
            first, second,
            "scope_string must be deterministic for the same input"
        );
    }

    /// `scope_string` returns different strings for different scopes.
    #[test]
    fn test_scope_string_distinct() {
        use crate::PmixScope::*;
        let local = scope_string(Local).unwrap();
        let global = scope_string(Global).unwrap();
        assert_ne!(
            local, global,
            "scope_string(Local) and scope_string(Global) must differ"
        );
    }

    /// `scope_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_scope_string_unknown() {
        use crate::PmixScope::Unknown;
        let scope = Unknown(99);
        let result = scope_string(scope);
        assert!(
            result.is_ok(),
            "scope_string(Unknown(99)) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "scope_string for unknown scope should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixScope enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixScope::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_scope_from_raw_to_raw_roundtrip() {
        use crate::PmixScope::*;

        let scopes = [Undef, Local, Remote, Global, Internal];
        for scope in scopes {
            let raw = scope.to_raw();
            let recovered = PmixScope::from_raw(raw);
            assert_eq!(
                scope, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                scope
            );
        }
    }

    /// `PmixScope::from_raw` maps known raw values correctly.
    #[test]
    fn test_scope_from_raw_known() {
        use crate::PmixScope::*;

        assert_eq!(PmixScope::from_raw(0), Undef);
        assert_eq!(PmixScope::from_raw(1), Local);
        assert_eq!(PmixScope::from_raw(2), Remote);
        assert_eq!(PmixScope::from_raw(3), Global);
        assert_eq!(PmixScope::from_raw(4), Internal);
        assert!(matches!(PmixScope::from_raw(255), Unknown(255)));
    }

    /// `PmixScope::to_raw` returns the expected raw values.
    #[test]
    fn test_scope_to_raw() {
        use crate::PmixScope::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(Local.to_raw(), 1);
        assert_eq!(Remote.to_raw(), 2);
        assert_eq!(Global.to_raw(), 3);
        assert_eq!(Internal.to_raw(), 4);
        assert_eq!(Unknown(42).to_raw(), 42);
    }

    /// `PmixScope` implements Display.
    #[test]
    fn test_scope_display() {
        use crate::PmixScope::*;

        assert_eq!(format!("{}", Undef), "UNDEFINED");
        assert_eq!(format!("{}", Local), "LOCAL");
        assert_eq!(format!("{}", Remote), "REMOTE");
        assert_eq!(format!("{}", Global), "GLOBAL");
        assert_eq!(format!("{}", Internal), "INTERNAL");
        assert_eq!(format!("{}", Unknown(99)), "UNKNOWN SCOPE (99)");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Data_range_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `data_range_string` returns `Ok(String)` for all known range values.
    ///
    /// PMIx_Data_range_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_data_range_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_data_range_string_all_known() {
        use crate::PmixDataRange::*;

        let ranges = [Undef, Rm, Local, Namespace, Session, Global, Custom, ProcLocal, Invalid];
        for range in ranges {
            let result = data_range_string(range);
            assert!(
                result.is_ok(),
                "data_range_string({:?}) should return Ok, got {:?}",
                range,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "data_range_string({:?}) should not return empty string",
                range
            );
        }
    }

    /// `data_range_string` returns the expected strings for key ranges.
    #[test]
    fn test_data_range_string_expected_values() {
        use crate::PmixDataRange::*;

        let local = data_range_string(Local).unwrap();
        let namespace = data_range_string(Namespace).unwrap();
        let session = data_range_string(Session).unwrap();
        let global = data_range_string(Global).unwrap();

        assert!(local.to_lowercase().contains("local"), "Local range string should contain 'local', got '{}'", local);
        assert!(namespace.to_lowercase().contains("namespace"), "Namespace range string should contain 'namespace', got '{}'", namespace);
        assert!(session.to_lowercase().contains("session"), "Session range string should contain 'session', got '{}'", session);
        assert!(global.to_lowercase().contains("global"), "Global range string should contain 'global', got '{}'", global);
    }

    /// `data_range_string` is deterministic — the same range always returns
    /// the same string.
    #[test]
    fn test_data_range_string_deterministic() {
        use crate::PmixDataRange::Session;
        let first = data_range_string(Session).unwrap();
        let second = data_range_string(Session).unwrap();
        assert_eq!(
            first, second,
            "data_range_string must be deterministic for the same input"
        );
    }

    /// `data_range_string` returns different strings for different ranges.
    #[test]
    fn test_data_range_string_distinct() {
        use crate::PmixDataRange::*;
        let local = data_range_string(Local).unwrap();
        let global = data_range_string(Global).unwrap();
        assert_ne!(
            local, global,
            "data_range_string(Local) and data_range_string(Global) must differ"
        );
    }

    /// `data_range_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_data_range_string_unknown() {
        use crate::PmixDataRange::Unknown;
        let range = Unknown;
        let result = data_range_string(range);
        assert!(
            result.is_ok(),
            "data_range_string(Unknown) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_range_string for unknown range should return non-empty string"
        );
    }

    /// `data_range_string` handles the Invalid variant (255).
    #[test]
    fn test_data_range_string_invalid() {
        use crate::PmixDataRange::Invalid;
        let result = data_range_string(Invalid);
        assert!(
            result.is_ok(),
            "data_range_string(Invalid) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_range_string(Invalid) should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixDataRange enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixDataRange::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_data_range_from_raw_to_raw_roundtrip() {
        use crate::PmixDataRange::*;

        let ranges = [Undef, Rm, Local, Namespace, Session, Global, Custom, ProcLocal, Invalid];
        for range in ranges {
            let raw = range.to_raw();
            let recovered = PmixDataRange::from_raw(raw);
            assert_eq!(
                range, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                range
            );
        }
    }

    /// `PmixDataRange::from_raw` maps known raw values correctly.
    #[test]
    fn test_data_range_from_raw_known() {
        use crate::PmixDataRange::*;

        assert_eq!(PmixDataRange::from_raw(0), Undef);
        assert_eq!(PmixDataRange::from_raw(1), Rm);
        assert_eq!(PmixDataRange::from_raw(2), Local);
        assert_eq!(PmixDataRange::from_raw(3), Namespace);
        assert_eq!(PmixDataRange::from_raw(4), Session);
        assert_eq!(PmixDataRange::from_raw(5), Global);
        assert_eq!(PmixDataRange::from_raw(6), Custom);
        assert_eq!(PmixDataRange::from_raw(7), ProcLocal);
        assert_eq!(PmixDataRange::from_raw(255), Invalid);
        assert!(matches!(PmixDataRange::from_raw(200), Unknown));
    }

    /// `PmixDataRange::to_raw` returns the expected raw values.
    #[test]
    fn test_data_range_to_raw() {
        use crate::PmixDataRange::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(Rm.to_raw(), 1);
        assert_eq!(Local.to_raw(), 2);
        assert_eq!(Namespace.to_raw(), 3);
        assert_eq!(Session.to_raw(), 4);
        assert_eq!(Global.to_raw(), 5);
        assert_eq!(Custom.to_raw(), 6);
        assert_eq!(ProcLocal.to_raw(), 7);
        assert_eq!(Invalid.to_raw(), 255);
        assert_eq!(Unknown.to_raw(), 128);
    }

    /// `PmixDataRange` implements Display.
    #[test]
    fn test_data_range_display() {
        use crate::PmixDataRange::*;

        assert_eq!(format!("{}", Undef), "UNDEFINED");
        assert_eq!(format!("{}", Rm), "RM");
        assert_eq!(format!("{}", Local), "LOCAL");
        assert_eq!(format!("{}", Namespace), "NAMESPACE");
        assert_eq!(format!("{}", Session), "SESSION");
        assert_eq!(format!("{}", Global), "GLOBAL");
        assert_eq!(format!("{}", Custom), "CUSTOM");
        assert_eq!(format!("{}", ProcLocal), "PROC LOCAL");
        assert_eq!(format!("{}", Invalid), "INVALID");
        assert_eq!(format!("{}", Unknown), "UNKNOWN RANGE (128)");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Info_directives_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `info_directives_string` returns `Ok(String)` for the REQD flag.
    ///
    /// PMIx_Info_directives_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_info_directives_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_info_directives_string_reqd() {
        let flags = crate::InfoFlags::REQD;
        let result = info_directives_string(flags);
        assert!(
            result.is_ok(),
            "info_directives_string(REQD) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string should not return an empty string"
        );
    }

    /// `info_directives_string` returns `Ok(String)` for all known flag values.
    #[test]
    fn test_info_directives_string_all_known() {
        use crate::InfoFlags;

        let flags = [InfoFlags::REQD, InfoFlags::QUALIFIER, InfoFlags::PERSISTENT, InfoFlags::REQD_PROCESSED];
        for flag in flags {
            let result = info_directives_string(flag);
            assert!(
                result.is_ok(),
                "info_directives_string({:?}) should return Ok, got {:?}",
                flag,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "info_directives_string({:?}) should not return empty string",
                flag
            );
        }
    }

    /// `info_directives_string` handles combined flags (bitwise OR).
    #[test]
    fn test_info_directives_string_combined() {
        use crate::InfoFlags;
        let combined = InfoFlags::REQD | InfoFlags::PERSISTENT;
        let result = info_directives_string(combined);
        assert!(
            result.is_ok(),
            "info_directives_string(combined) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for combined flags should return non-empty string"
        );
    }

    /// `info_directives_string` handles zero flags (no directives set).
    #[test]
    fn test_info_directives_string_empty() {
        use crate::InfoFlags;
        let empty = InfoFlags::default();
        assert!(empty.is_empty(), "default InfoFlags should be empty");
        let result = info_directives_string(empty);
        assert!(
            result.is_ok(),
            "info_directives_string(empty) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for empty flags should return non-empty string"
        );
    }

    /// `info_directives_string` is deterministic — the same flags always return
    /// the same string.
    #[test]
    fn test_info_directives_string_deterministic() {
        use crate::InfoFlags;
        let flags = InfoFlags::REQD | InfoFlags::REQD_PROCESSED;
        let first = info_directives_string(flags).unwrap();
        let second = info_directives_string(flags).unwrap();
        assert_eq!(
            first, second,
            "info_directives_string must be deterministic for the same input"
        );
    }

    /// `info_directives_string` returns different strings for different flags.
    #[test]
    fn test_info_directives_string_distinct() {
        use crate::InfoFlags;
        let reqd = info_directives_string(InfoFlags::REQD).unwrap();
        let persistent = info_directives_string(InfoFlags::PERSISTENT).unwrap();
        assert_ne!(
            reqd, persistent,
            "info_directives_string(REQD) and info_directives_string(PERSISTENT) must differ"
        );
    }

    /// `info_directives_string` handles unknown/reserved flag values.
    #[test]
    fn test_info_directives_string_reserved() {
        use crate::InfoFlags;
        // PMIX_INFO_DIR_RESERVED = 0xFFFF0000
        let reserved = InfoFlags(0xFFFF0000);
        let result = info_directives_string(reserved);
        assert!(
            result.is_ok(),
            "info_directives_string(reserved) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for reserved flags should return non-empty string"
        );
    }

    /// `InfoFlags::raw` and construction round-trip correctly.
    #[test]
    fn test_info_flags_raw_roundtrip() {
        use crate::InfoFlags;
        let flags = InfoFlags::REQD | InfoFlags::PERSISTENT | InfoFlags::REQD_PROCESSED;
        let raw = flags.raw();
        let recovered = InfoFlags(raw);
        assert_eq!(flags, recovered, "InfoFlags(raw(flags)) should round-trip");
        assert_eq!(raw, 1 | 16 | 4, "combined flags should have correct raw value (REQD=1 | PERSISTENT=16 | REQD_PROCESSED=4 = 21)");
    }

    /// `InfoFlags::contains` checks individual bits correctly.
    #[test]
    fn test_info_flags_contains() {
        use crate::InfoFlags;
        let combined = InfoFlags::REQD | InfoFlags::PERSISTENT;
        assert!(combined.contains(InfoFlags::REQD));
        assert!(combined.contains(InfoFlags::PERSISTENT));
        assert!(!combined.contains(InfoFlags::REQD_PROCESSED));
    }

    /// `InfoFlags::is_empty` works for zero and non-zero values.
    #[test]
    fn test_info_flags_is_empty() {
        use crate::InfoFlags;
        assert!(InfoFlags::default().is_empty());
        assert!(!InfoFlags::REQD.is_empty());
        assert!(!InfoFlags::REQD.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_IOF_channel_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `iof_channel_string` returns `Ok(String)` for all known channel values.
    ///
    /// PMIx_IOF_channel_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_iof_channel_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_iof_channel_string_all_known() {
        use crate::IOFChannelFlags;

        let channels = [
            IOFChannelFlags::NO_CHANNELS,
            IOFChannelFlags::STDIN,
            IOFChannelFlags::STDOUT,
            IOFChannelFlags::STDERR,
            IOFChannelFlags::STDDIAG,
            IOFChannelFlags::ALL_CHANNELS,
        ];
        for channel in channels {
            let result = iof_channel_string(channel);
            assert!(
                result.is_ok(),
                "iof_channel_string({:?}) should return Ok, got {:?}",
                channel,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "iof_channel_string({:?}) should not return empty string",
                channel
            );
        }
    }

    /// `iof_channel_string` returns the expected strings for key channels.
    #[test]
    fn test_iof_channel_string_expected_values() {
        use crate::IOFChannelFlags;

        let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
        let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        let stderr = iof_channel_string(IOFChannelFlags::STDERR).unwrap();

        assert!(
            stdin.to_lowercase().contains("stdin"),
            "STDIN channel string should contain 'stdin', got '{}'",
            stdin
        );
        assert!(
            stdout.to_lowercase().contains("stdout"),
            "STDOUT channel string should contain 'stdout', got '{}'",
            stdout
        );
        assert!(
            stderr.to_lowercase().contains("stderr"),
            "STDERR channel string should contain 'stderr', got '{}'",
            stderr
        );
    }

    /// `iof_channel_string` is deterministic — the same channel always returns
    /// the same string.
    #[test]
    fn test_iof_channel_string_deterministic() {
        use crate::IOFChannelFlags;
        let first = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        let second = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        assert_eq!(
            first, second,
            "iof_channel_string must be deterministic for the same input"
        );
    }

    /// `iof_channel_string` returns different strings for different channels.
    #[test]
    fn test_iof_channel_string_distinct() {
        use crate::IOFChannelFlags;
        let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
        let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        assert_ne!(
            stdin, stdout,
            "iof_channel_string(STDIN) and iof_channel_string(STDOUT) must differ"
        );
    }

    /// `iof_channel_string` handles combined channel flags (bitmask OR).
    #[test]
    fn test_iof_channel_string_combined() {
        use crate::IOFChannelFlags;
        let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
        let result = iof_channel_string(combined);
        assert!(
            result.is_ok(),
            "iof_channel_string(STDOUT|STDERR) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "iof_channel_string for combined channels should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOFChannelFlags enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `IOFChannelFlags::raw()` returns the expected raw values.
    #[test]
    fn test_iof_channel_flags_raw() {
        use crate::IOFChannelFlags;

        assert_eq!(IOFChannelFlags::NO_CHANNELS.raw(), 0x0000);
        assert_eq!(IOFChannelFlags::STDIN.raw(), 0x0001);
        assert_eq!(IOFChannelFlags::STDOUT.raw(), 0x0002);
        assert_eq!(IOFChannelFlags::STDERR.raw(), 0x0004);
        assert_eq!(IOFChannelFlags::STDDIAG.raw(), 0x0008);
        assert_eq!(IOFChannelFlags::ALL_CHANNELS.raw(), 0x00FF);
    }

    /// `IOFChannelFlags` bitwise OR works correctly.
    #[test]
    fn test_iof_channel_flags_bitor() {
        use crate::IOFChannelFlags;

        let combined = IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT;
        assert_eq!(combined.raw(), 0x0003);
        assert!(combined.contains(IOFChannelFlags::STDIN));
        assert!(combined.contains(IOFChannelFlags::STDOUT));
        assert!(!combined.contains(IOFChannelFlags::STDERR));
    }

    /// `IOFChannelFlags::contains` checks individual bits correctly.
    #[test]
    fn test_iof_channel_flags_contains() {
        use crate::IOFChannelFlags;

        let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
        assert!(combined.contains(IOFChannelFlags::STDOUT));
        assert!(combined.contains(IOFChannelFlags::STDERR));
        assert!(!combined.contains(IOFChannelFlags::STDIN));
    }

    /// `IOFChannelFlags::is_empty` works for zero and non-zero values.
    #[test]
    fn test_iof_channel_flags_is_empty() {
        use crate::IOFChannelFlags;

        assert!(IOFChannelFlags::NO_CHANNELS.is_empty());
        assert!(IOFChannelFlags::default().is_empty());
        assert!(!IOFChannelFlags::STDIN.is_empty());
        assert!(!IOFChannelFlags::ALL_CHANNELS.is_empty());
    }

    /// `IOFChannelFlags` implements Display.
    #[test]
    fn test_iof_channel_flags_display() {
        use crate::IOFChannelFlags;

        let stdin = format!("{}", IOFChannelFlags::STDIN);
        assert!(stdin.contains("STDIN"), "Display for STDIN should contain 'STDIN', got '{}'", stdin);

        let stdout = format!("{}", IOFChannelFlags::STDOUT);
        assert!(stdout.contains("STDOUT"), "Display for STDOUT should contain 'STDOUT', got '{}'", stdout);

        let no_channels = format!("{}", IOFChannelFlags::NO_CHANNELS);
        assert!(no_channels.contains("NO_CHANNELS"), "Display for NO_CHANNELS should contain 'NO_CHANNELS', got '{}'", no_channels);

        let combined = format!("{}", (IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR));
        assert!(combined.contains("STDOUT"), "Display for combined should contain 'STDOUT', got '{}'", combined);
        assert!(combined.contains("STDERR"), "Display for combined should contain 'STDERR', got '{}'", combined);
    }

    /// `IOFChannelFlags` BitOrAssign works correctly.
    #[test]
    fn test_iof_channel_flags_bitor_assign() {
        use crate::IOFChannelFlags;

        let mut flags = IOFChannelFlags::STDIN;
        flags |= IOFChannelFlags::STDOUT;
        flags |= IOFChannelFlags::STDERR;

        assert!(flags.contains(IOFChannelFlags::STDIN));
        assert!(flags.contains(IOFChannelFlags::STDOUT));
        assert!(flags.contains(IOFChannelFlags::STDERR));
        assert_eq!(flags.raw(), 0x0007);
    }

    // ─────────────────────────────────────────────────────────────────────
    // PMIx_Job_state_string tests
    // ─────────────────────────────────────────────────────────────────────

    /// `job_state_string` returns `Ok(String)` for all known job states.
    #[test]
    fn test_job_state_string_all_known() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
        ];
        for state in states {
            let result = job_state_string(state);
            assert!(
                result.is_ok(),
                "job_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "job_state_string({:?}) should not return an empty string",
                state
            );
        }
    }

    /// `job_state_string` returns expected strings for key lifecycle states.
    #[test]
    fn test_job_state_string_key_states() {
        use crate::PmixJobState::*;

        let undef = job_state_string(Undef).unwrap();
        let running = job_state_string(Running).unwrap();
        let terminated = job_state_string(Terminated).unwrap();
        let terminated_with_error = job_state_string(TerminatedWithError).unwrap();

        assert!(
            !undef.is_empty(),
            "Undef state string should not be empty, got '{}'",
            undef
        );
        assert!(
            running.to_lowercase().contains("run"),
            "Running state string should contain 'run', got '{}'",
            running
        );
        assert!(
            terminated.to_lowercase().contains("terminat"),
            "Terminated state string should contain 'terminat', got '{}'",
            terminated
        );
        assert!(
            terminated_with_error.to_lowercase().contains("error"),
            "TerminatedWithError state string should contain 'error', got '{}'",
            terminated_with_error
        );
    }

    /// `job_state_string` is deterministic — the same state always returns
    /// the same string.
    #[test]
    fn test_job_state_string_deterministic() {
        use crate::PmixJobState::Running;

        let first = job_state_string(Running).unwrap();
        let second = job_state_string(Running).unwrap();
        assert_eq!(
            first, second,
            "job_state_string(Running) should be deterministic: '{}' != '{}'",
            first, second
        );
    }

    /// `PmixJobState::from_raw` round-trips correctly for all known states.
    #[test]
    fn test_job_state_from_raw_to_raw_roundtrip() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
        ];
        for state in states {
            let raw = state.to_raw();
            let recovered = PmixJobState::from_raw(raw);
            assert_eq!(
                state, recovered,
                "Round-trip failed for {:?}: raw={}, recovered={:?}",
                state, raw, recovered
            );
        }
    }

    /// `PmixJobState::from_raw` maps unknown values to `Unknown(n)`.
    #[test]
    fn test_job_state_from_raw_unknown() {
        use crate::PmixJobState;

        let unknown = PmixJobState::from_raw(99);
        assert!(
            matches!(unknown, PmixJobState::Unknown(99)),
            "from_raw(99) should be Unknown(99), got {:?}",
            unknown
        );
    }

    /// `PmixJobState` Display returns a non-empty string for all variants.
    #[test]
    fn test_job_state_display() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
            Unknown(99),
        ];
        for state in states {
            let display = format!("{}", state);
            assert!(
                !display.is_empty(),
                "Display for {:?} should not be empty",
                state
            );
        }
    }

    /// `PmixJobState` raw values match the C header definitions.
    #[test]
    fn test_job_state_raw_values() {
        use crate::PmixJobState::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(AwaitingAlloc.to_raw(), 1);
        assert_eq!(LaunchUnderway.to_raw(), 2);
        assert_eq!(Running.to_raw(), 3);
        assert_eq!(Suspended.to_raw(), 4);
        assert_eq!(Connected.to_raw(), 5);
        assert_eq!(Unterminated.to_raw(), 15);
        assert_eq!(Terminated.to_raw(), 20);
        assert_eq!(TerminatedWithError.to_raw(), 50);
    }

    // ───────────────────────────────────────────────────────────────────────────
    // PMIx_Link_state_string
    // ───────────────────────────────────────────────────────────────────────────

    /// `link_state_string` returns `Ok(String)` for all known link states.
    #[test]
    fn test_link_state_string_all_known() {
        use crate::PmixLinkState::*;

        let states = [UnknownState, LinkDown, LinkUp];
        for state in states {
            let result = link_state_string(state);
            assert!(
                result.is_ok(),
                "link_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "link_state_string({:?}) should not return an empty string",
                state
            );
        }
    }

    /// `link_state_string` returns the expected strings for each state.
    #[test]
    fn test_link_state_string_expected_values() {
        use crate::PmixLinkState::*;

        let unknown = link_state_string(UnknownState).unwrap();
        assert_eq!(unknown, "UNKNOWN");

        let down = link_state_string(LinkDown).unwrap();
        assert_eq!(down, "INACTIVE");

        let up = link_state_string(LinkUp).unwrap();
        assert_eq!(up, "ACTIVE");
    }

    /// `link_state_string` is deterministic — same state always returns same string.
    #[test]
    fn test_link_state_string_deterministic() {
        use crate::PmixLinkState::*;

        let first = link_state_string(LinkUp).unwrap();
        let second = link_state_string(LinkUp).unwrap();
        assert_eq!(first, second, "link_state_string should be deterministic");

        let first = link_state_string(LinkDown).unwrap();
        let second = link_state_string(LinkDown).unwrap();
        assert_eq!(first, second, "link_state_string should be deterministic");
    }

    /// `PmixLinkState` Display matches the C string output.
    #[test]
    fn test_link_state_display() {
        use crate::PmixLinkState::*;

        assert_eq!(format!("{}", UnknownState), "UNKNOWN");
        assert_eq!(format!("{}", LinkDown), "INACTIVE");
        assert_eq!(format!("{}", LinkUp), "ACTIVE");
    }

    /// `PmixLinkState::from_raw` / `to_raw` roundtrip for all known values.
    #[test]
    fn test_link_state_from_raw_to_raw() {
        use crate::PmixLinkState::*;

        assert_eq!(PmixLinkState::from_raw(0), UnknownState);
        assert_eq!(PmixLinkState::from_raw(1), LinkDown);
        assert_eq!(PmixLinkState::from_raw(2), LinkUp);
        assert_eq!(PmixLinkState::from_raw(255), PmixLinkState::Unknown(255));

        assert_eq!(UnknownState.to_raw(), 0);
        assert_eq!(LinkDown.to_raw(), 1);
        assert_eq!(LinkUp.to_raw(), 2);

        // Roundtrip for unknown values
        let unknown = PmixLinkState::from_raw(42);
        assert_eq!(unknown.to_raw(), 42);
    }

    /// `PmixLinkState` raw values match the C header definitions.
    #[test]
    fn test_link_state_raw_values() {
        use crate::PmixLinkState::*;

        assert_eq!(UnknownState.to_raw(), 0); // PMIX_LINK_STATE_UNKNOWN
        assert_eq!(LinkDown.to_raw(), 1);     // PMIX_LINK_DOWN
        assert_eq!(LinkUp.to_raw(), 2);       // PMIX_LINK_UP
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Device_type_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `device_type_string` returns `Ok(String)` for all known device types.
    #[test]
    fn test_device_type_string_all_known() {
        use crate::PmixDeviceType::*;

        let types = [UnknownType, Block, Gpu, Network, OpenFabrics, Dma, Coproc];
        for ty in types {
            let result = device_type_string(ty);
            assert!(
                result.is_ok(),
                "device_type_string({:?}) should return Ok, got {:?}",
                ty,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "device_type_string({:?}) should not return an empty string",
                ty
            );
        }
    }

    /// `device_type_string` returns the expected strings for key device types.
    #[test]
    fn test_device_type_string_expected() {
        use crate::PmixDeviceType::*;

        assert_eq!(device_type_string(UnknownType).unwrap(), "UNKNOWN");
        assert_eq!(device_type_string(Block).unwrap(), "BLOCK");
        assert_eq!(device_type_string(Gpu).unwrap(), "GPU");
        assert_eq!(device_type_string(Network).unwrap(), "NETWORK");
        assert_eq!(device_type_string(OpenFabrics).unwrap(), "OPENFABRICS");
        assert_eq!(device_type_string(Dma).unwrap(), "DMA");
        assert_eq!(device_type_string(Coproc).unwrap(), "COPROCESSOR");
    }

    /// `device_type_string` is deterministic — the same type always returns
    /// the same string.
    #[test]
    fn test_device_type_string_deterministic() {
        use crate::PmixDeviceType::Gpu;

        let first = device_type_string(Gpu).unwrap();
        let second = device_type_string(Gpu).unwrap();
        assert_eq!(first, second, "device_type_string should be deterministic");
    }

    /// `device_type_string` handles unknown device type values gracefully.
    #[test]
    fn test_device_type_string_unknown() {
        use crate::PmixDeviceType;

        let unknown = PmixDeviceType::Unknown(0xFF);
        let result = device_type_string(unknown);
        assert!(result.is_ok(), "device_type_string should handle unknown values");
    }

    /// `PmixDeviceType::from_raw` / `to_raw` roundtrip for all known values.
    #[test]
    fn test_device_type_from_raw_to_raw() {
        use crate::PmixDeviceType::*;

        assert_eq!(PmixDeviceType::from_raw(0x00), UnknownType);
        assert_eq!(PmixDeviceType::from_raw(0x01), Block);
        assert_eq!(PmixDeviceType::from_raw(0x02), Gpu);
        assert_eq!(PmixDeviceType::from_raw(0x04), Network);
        assert_eq!(PmixDeviceType::from_raw(0x08), OpenFabrics);
        assert_eq!(PmixDeviceType::from_raw(0x10), Dma);
        assert_eq!(PmixDeviceType::from_raw(0x20), Coproc);
        assert_eq!(PmixDeviceType::from_raw(0xFF), PmixDeviceType::Unknown(0xFF));

        assert_eq!(UnknownType.to_raw(), 0x00);
        assert_eq!(Block.to_raw(), 0x01);
        assert_eq!(Gpu.to_raw(), 0x02);
        assert_eq!(Network.to_raw(), 0x04);
        assert_eq!(OpenFabrics.to_raw(), 0x08);
        assert_eq!(Dma.to_raw(), 0x10);
        assert_eq!(Coproc.to_raw(), 0x20);

        // Roundtrip for unknown values
        let unknown = PmixDeviceType::from_raw(0xDEAD);
        assert_eq!(unknown.to_raw(), 0xDEAD);
    }

    /// `PmixDeviceType` raw values match the C header definitions.
    #[test]
    fn test_device_type_raw_values() {
        use crate::PmixDeviceType::*;

        assert_eq!(UnknownType.to_raw(), 0x00); // PMIX_DEVTYPE_UNKNOWN
        assert_eq!(Block.to_raw(), 0x01);       // PMIX_DEVTYPE_BLOCK
        assert_eq!(Gpu.to_raw(), 0x02);         // PMIX_DEVTYPE_GPU
        assert_eq!(Network.to_raw(), 0x04);     // PMIX_DEVTYPE_NETWORK
        assert_eq!(OpenFabrics.to_raw(), 0x08); // PMIX_DEVTYPE_OPENFABRICS
        assert_eq!(Dma.to_raw(), 0x10);         // PMIX_DEVTYPE_DMA
        assert_eq!(Coproc.to_raw(), 0x20);      // PMIX_DEVTYPE_COPROC
    }

    /// `PmixDeviceType` Display implementation matches C strings.
    #[test]
    fn test_device_type_display() {
        use crate::PmixDeviceType::*;

        assert_eq!(format!("{}", UnknownType), "UNKNOWN");
        assert_eq!(format!("{}", Block), "BLOCK");
        assert_eq!(format!("{}", Gpu), "GPU");
        assert_eq!(format!("{}", Network), "NETWORK");
        assert_eq!(format!("{}", OpenFabrics), "OPENFABRICS");
        assert_eq!(format!("{}", Dma), "DMA");
        assert_eq!(format!("{}", Coproc), "COPROCESSOR");
    }
}
