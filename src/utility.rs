//! Utility functions — `PMIx_Initialized`, `PMIx_Error_string`, `PMIx_Proc_state_string`, and related helpers.
//!
//! This module provides safe Rust wrappers around PMIx utility APIs
//! that do not fit into the lifecycle, data, or event categories.

use crate::{ffi, PmixProcState, PmixStatus};

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
}
