//! Utility functions — `PMIx_Initialized`, `PMIx_Error_string`, `PMIx_Proc_state_string`,
//! `PMIx_Scope_string`, `PMIx_Persistence_string`, `PMIx_Data_range_string`,
//! `PMIx_Info_directives_string`, and related helpers.
//!
//! This module provides safe Rust wrappers around PMIx utility APIs
//! that do not fit into the lifecycle, data, or event categories.

use crate::{ffi, InfoFlags, PmixDataRange, PmixPersistence, PmixProcState, PmixScope, PmixStatus};

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
        let range = Unknown(99);
        let result = data_range_string(range);
        assert!(
            result.is_ok(),
            "data_range_string(Unknown(99)) should return Ok, got {:?}",
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
        assert!(matches!(PmixDataRange::from_raw(200), Unknown(200)));
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
        assert_eq!(Unknown(42).to_raw(), 42);
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
        use crate::InfoFlags::*;

        let flags = [REQD, QUALIFIER, PERSISTENT, REQD_PROCESSED];
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
        assert_eq!(raw, 1 | 8 | 4, "combined flags should have correct raw value");
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
}
