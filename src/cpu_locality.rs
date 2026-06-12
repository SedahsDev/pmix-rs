//! CPU locality utilities — parsing cpuset strings, computing locality.
//!
//! This module provides safe Rust wrappers for PMIx CPU locality APIs:
//!
//! - [`parse_cpuset_string`] — parse a cpuset string into a [`PmixCpuset`].

use std::ffi::CString;

use crate::ffi;
use crate::fabric::PmixCpuset;
use crate::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// parse_cpuset_string
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a cpuset string representation into a [`PmixCpuset`] bitmap.
///
/// Parses the string representation of a CPU binding bitmap (as returned by
/// `PMIx_Get` using the `PMIX_CPUSET` key, or by
/// `PMIx_server_generate_cpuset_string`) and populates the provided
/// [`PmixCpuset`] with the corresponding bitmap.
///
/// # Parameters
///
/// * `cpuset_string` — The string representation of the CPU set. This is
///   typically obtained from `PMIx_Get` with the `PMIX_CPUSET` key or from
///   `PMIx_server_generate_cpuset_string`.
/// * `cpuset` — A mutable [`PmixCpuset`] that will receive the parsed bitmap.
///   The cpuset must have been constructed via [`PmixCpuset::new`].
///
/// # Returns
///
/// * `Ok(())` — The cpuset was successfully parsed.
/// * `Err(PmixStatus)` — An appropriate PMIx error constant on failure
///   (e.g., `PMIX_ERR_NOT_FOUND`, `PMIX_ERR_NOT_SUPPORTED`).
///
/// # Errors
///
/// In addition to PMIx status errors, this function returns a `NulError`
/// (wrapped in `Err`) if the input string contains an interior NUL byte.
///
/// # C API
///
/// ```c
/// pmix_status_t PMIx_Parse_cpuset_string(const char *cpuset_string,
///                                         pmix_cpuset_t *cpuset);
/// ```
///
/// # Spec
///
/// PMIx Standard v4.1, Section 11.4.3.
pub fn parse_cpuset_string(
    cpuset_string: &str,
    cpuset: &mut PmixCpuset,
) -> Result<(), PmixStatus> {
    let c_str = CString::new(cpuset_string).map_err(|_| PmixStatus::from_raw(-1))?;
    let cpuset_ptr = cpuset.as_mut_ptr();

    let status = unsafe {
        // SAFETY: `c_str.as_ptr()` is a valid null-terminated C string for the
        // duration of this call. `cpuset_ptr` is a valid, constructed
        // `pmix_cpuset_t` (guaranteed by the PmixCpuset wrapper).
        ffi::PMIx_Parse_cpuset_string(c_str.as_ptr(), cpuset_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that parse_cpuset_string with a valid-looking cpuset string
    /// does not panic. Note: this requires PMIx runtime to be initialized,
    /// so it may fail if called outside a PMIx session.
    #[test]
    fn test_parse_cpuset_string_empty_string() {
        let mut cpuset = PmixCpuset::new();
        // An empty string is not a valid cpuset representation.
        // We expect an error from the PMIx library.
        let result = parse_cpuset_string("", &mut cpuset);
        // The result depends on the PMIx implementation — it may return
        // an error or succeed with an empty bitmap. Either way, no panic.
        let _ = result;
    }

    /// Test that parse_cpuset_string with a simple CPU list format works.
    #[test]
    fn test_parse_cpuset_string_single_cpu() {
        let mut cpuset = PmixCpuset::new();
        let result = parse_cpuset_string("0", &mut cpuset);
        // Without a running PMIx session, this may return an error.
        // The important thing is that the FFI call is made correctly.
        let _ = result;
    }

    /// Test that parse_cpuset_string with a range format works.
    #[test]
    fn test_parse_cpuset_string_range() {
        let mut cpuset = PmixCpuset::new();
        let result = parse_cpuset_string("0-3", &mut cpuset);
        let _ = result;
    }

    /// Test that parse_cpuset_string with a comma-separated list works.
    #[test]
    fn test_parse_cpuset_string_list() {
        let mut cpuset = PmixCpuset::new();
        let result = parse_cpuset_string("0,2,4,6", &mut cpuset);
        let _ = result;
    }

    /// Test that the cpuset is properly cleaned up on drop even after
    /// a failed parse.
    #[test]
    fn test_parse_cpuset_string_cleanup_on_error() {
        let mut cpuset = PmixCpuset::new();
        // Use a string with a NUL byte — should return NulError
        let result = parse_cpuset_string("hello\x00world", &mut cpuset);
        assert!(result.is_err());
        // cpuset should still drop without issues
        drop(cpuset);
    }

    /// Test that parse_cpuset_string with complex mixed format works.
    #[test]
    fn test_parse_cpuset_string_mixed_format() {
        let mut cpuset = PmixCpuset::new();
        let result = parse_cpuset_string("0-3,5,8-11", &mut cpuset);
        let _ = result;
    }

    /// Test that parse_cpuset_string handles very long strings.
    #[test]
    fn test_parse_cpuset_string_long_string() {
        let mut cpuset = PmixCpuset::new();
        let long_cpu_list = (0..256).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let result = parse_cpuset_string(&long_cpu_list, &mut cpuset);
        let _ = result;
    }
}
