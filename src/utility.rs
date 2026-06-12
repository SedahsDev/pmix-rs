//! Utility functions — `PMIx_Initialized` and related helpers.
//!
//! This module provides safe Rust wrappers around PMIx utility APIs
//! that do not fit into the lifecycle, data, or event categories.

use crate::ffi;

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
        assert!(!result, "PMIx_Initialized should return false before PMIx_Init");
    }

    /// `initialized()` is idempotent — calling it multiple times returns
    /// the same value (no side effects).
    #[test]
    fn test_initialized_idempotent() {
        let first = initialized();
        let second = initialized();
        assert_eq!(first, second, "PMIx_Initialized should be idempotent");
    }
}
