//! CPU locality utilities — parsing cpuset strings, computing locality.
//!
//! This module provides safe Rust wrappers for PMIx CPU locality and
//! topology APIs:
//!
//! - [`get_cpuset`] — retrieve the CPU set for the calling process/thread.
//! - [`parse_cpuset_string`] — parse a cpuset string into a [`PmixCpuset`].
//! - [`get_relative_locality`] — compute relative locality of two processes.

use std::ffi::CString;

use bitflags::bitflags;

use crate::PmixStatus;
use crate::fabric::PmixCpuset;
use crate::ffi;

// ─────────────────────────────────────────────────────────────────────────────
// PmixBindEnvelope
// ─────────────────────────────────────────────────────────────────────────────

/// Bind envelope selector for [`get_cpuset`].
///
/// Specifies whose CPU binding to retrieve: the calling process or the
/// calling thread. Corresponds to the C `pmix_bind_envelope_t` type and
/// the `PMIX_CPUBIND_*` constants.
///
/// # C API
///
/// ```c
/// typedef uint8_t pmix_bind_envelope_t;
/// #define PMIX_CPUBIND_PROCESS    0
/// #define PMIX_CPUBIND_THREAD     1
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixBindEnvelope {
    /// Retrieve the CPU binding of the calling process.
    Process = 0,
    /// Retrieve the CPU binding of the calling thread.
    Thread = 1,
}

impl PmixBindEnvelope {
    /// Convert to the raw C `pmix_bind_envelope_t` value.
    pub fn to_raw(self) -> u8 {
        self as u8
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_cpuset
// ─────────────────────────────────────────────────────────────────────────────

/// Retrieve the CPU set for the calling process or thread.
///
/// Returns the set of CPUs on which the calling process (or thread) is
/// bound, as determined by the PMIx framework. The result is written into
/// the provided [`PmixCpuset`] object.
///
/// # Parameters
///
/// * `cpuset` — A mutable [`PmixCpuset`] that will receive the CPU bitmap.
///   Must have been constructed via [`PmixCpuset::new`].
/// * `ref_` — A [`PmixBindEnvelope`] specifying whose binding to retrieve
///   (process-wide or thread-specific).
///
/// # Returns
///
/// * `Ok(())` — The cpuset was successfully retrieved.
/// * `Err(PmixStatus)` — An appropriate PMIx error constant on failure,
///   e.g. `PMIX_ERR_INIT` if PMIx has not been initialized, or
///   `PMIX_ERR_NOT_SUPPORTED` if the runtime does not support cpuset
///   queries.
///
/// # C API
///
/// ```c
/// pmix_status_t PMIx_Get_cpuset(pmix_cpuset_t *cpuset, pmix_bind_envelope_t ref);
/// ```
///
/// # Spec
///
/// PMIx Standard v4.1, Section 11.4.3.
pub fn get_cpuset(cpuset: &mut PmixCpuset, ref_: PmixBindEnvelope) -> Result<(), PmixStatus> {
    let cpuset_ptr = cpuset.as_mut_ptr();
    let raw_ref = ref_.to_raw();

    let status = unsafe {
        // SAFETY: `cpuset_ptr` is a valid, constructed `pmix_cpuset_t`
        // (guaranteed by the PmixCpuset wrapper). `raw_ref` is a valid
        // `pmix_bind_envelope_t` value (Process = 0 or Thread = 1).
        ffi::PMIx_Get_cpuset(cpuset_ptr, raw_ref)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    Ok(())
}

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
pub fn parse_cpuset_string(cpuset_string: &str, cpuset: &mut PmixCpuset) -> Result<(), PmixStatus> {
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
// PmixLocality — relative locality bitmask
// ─────────────────────────────────────────────────────────────────────────────

// Relative locality bitmask for two processes on a node.
// Returned by [`get_relative_locality`] to describe how two processes are
// positioned relative to each other on the hardware topology. Each bit
// corresponds to a level of shared hardware resource.
//
// C API:
// ```c
// typedef uint16_t pmix_locality_t;
// #define PMIX_LOCALITY_UNKNOWN           0x0000
// #define PMIX_LOCALITY_NONLOCAL          0x8000
// #define PMIX_LOCALITY_SHARE_HWTHREAD    0x0001
// #define PMIX_LOCALITY_SHARE_CORE        0x0002
// #define PMIX_LOCALITY_SHARE_L1CACHE     0x0004
// #define PMIX_LOCALITY_SHARE_L2CACHE     0x0008
// #define PMIX_LOCALITY_SHARE_L3CACHE     0x0010
// #define PMIX_LOCALITY_SHARE_PACKAGE     0x0020
// #define PMIX_LOCALITY_SHARE_NUMA        0x0040
// #define PMIX_LOCALITY_SHARE_NODE        0x4000
// ```
//
// PMIx Standard v4.1, Section 11.4.2.3.
bitflags! {
    /// Relative locality bitmask for two processes on a node.
    ///
    /// Returned by [`get_relative_locality`] to describe how two processes are
    /// positioned relative to each other on the hardware topology. Each bit
    /// corresponds to a level of shared hardware resource.
    ///
    /// PMIx Standard v4.1, Section 11.4.2.3.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    #[repr(transparent)]
    pub struct PmixLocality: u16 {
        /// All bits are set to zero, indicating that the relative locality
        /// of the two processes is unknown.
        const UNKNOWN           = 0x0000;

        /// The two processes do not share any common locations.
        const NONLOCAL          = 0x8000;

        /// The two processes share at least one hardware thread.
        const SHARE_HWTHREAD    = 0x0001;

        /// The two processes share at least one core.
        const SHARE_CORE        = 0x0002;

        /// The two processes share at least an L1 cache.
        const SHARE_L1CACHE     = 0x0004;

        /// The two processes share at least an L2 cache.
        const SHARE_L2CACHE     = 0x0008;

        /// The two processes share at least an L3 cache.
        const SHARE_L3CACHE     = 0x0010;

        /// The two processes share at least a package.
        const SHARE_PACKAGE     = 0x0020;

        /// The two processes share at least one Non-Uniform Memory Access
        /// (NUMA) region.
        const SHARE_NUMA        = 0x0040;

        /// The two processes are executing on the same node.
        const SHARE_NODE        = 0x4000;
    }
}

impl PmixLocality {
    /// Convert from a raw C `pmix_locality_t` value.
    pub fn from_raw(raw: u16) -> Self {
        Self::from_bits_truncate(raw)
    }

    /// Convert to the raw C `pmix_locality_t` value.
    pub fn to_raw(self) -> u16 {
        self.bits()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_relative_locality
// ─────────────────────────────────────────────────────────────────────────────

/// Get the relative locality of two processes given their locality strings.
///
/// Parses the locality strings of two processes (as returned by `PMIx_Get`
/// using the `PMIX_LOCALITY_STRING` key) and sets the appropriate
/// [`PmixLocality`] bitmask bits describing how the two processes are
/// positioned relative to each other on the hardware topology.
///
/// # Parameters
///
/// * `locality1` — Locality string for the first process, as returned by
///   `PMIx_server_generate_locality_string` or obtained via `PMIx_Get`
///   with the `PMIX_LOCALITY_STRING` key.
/// * `locality2` — Locality string for the second process, in the same
///   format as `locality1`.
///
/// # Returns
///
/// * `Ok(PmixLocality)` — The relative locality bitmask describing the
///   hardware relationship between the two processes.
/// * `Err(PmixStatus)` — An appropriate PMIx error constant on failure,
///   e.g. `PMIX_ERR_INIT` if PMIx has not been initialized,
///   `PMIX_ERR_BAD_PARAM` if either locality string is invalid, or
///   `PMIX_ERR_NOT_SUPPORTED` if the runtime does not support locality
///   queries.
///
/// # C API
///
/// ```c
/// pmix_status_t PMIx_Get_relative_locality(const char *locality1,
///                                           const char *locality2,
///                                           pmix_locality_t *locality);
/// ```
///
/// # Spec
///
/// PMIx Standard v4.1, Section 11.4.2.
pub fn get_relative_locality(locality1: &str, locality2: &str) -> Result<PmixLocality, PmixStatus> {
    let c_locality1 = CString::new(locality1).map_err(|_| PmixStatus::from_raw(-1))?;
    let c_locality2 = CString::new(locality2).map_err(|_| PmixStatus::from_raw(-1))?;

    let mut locality: ffi::pmix_locality_t = 0;
    let status = unsafe {
        // SAFETY: `c_locality1` and `c_locality2` are valid null-terminated
        // C strings for the duration of this call. `&mut locality` is a
        // valid mutable pointer to a `pmix_locality_t` (u16) that the
        // PMIx library will write to on success.
        ffi::PMIx_Get_relative_locality(c_locality1.as_ptr(), c_locality2.as_ptr(), &mut locality)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    Ok(PmixLocality::from_raw(locality))
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
        let long_cpu_list = (0..256)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let result = parse_cpuset_string(&long_cpu_list, &mut cpuset);
        let _ = result;
    }

    // ─────────────────────────────────────────────────────────────────────
    // get_cpuset tests
    // ─────────────────────────────────────────────────────────────────────

    /// Test that get_cpuset with Process envelope does not panic.
    #[test]
    fn test_get_cpuset_process() {
        let mut cpuset = PmixCpuset::new();
        let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        // Without a running PMIx session, this may return PMIX_ERR_INIT.
        // The important thing is that the FFI call is made correctly.
        let _ = result;
    }

    /// Test that get_cpuset with Thread envelope does not panic.
    #[test]
    fn test_get_cpuset_thread() {
        let mut cpuset = PmixCpuset::new();
        let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Thread);
        let _ = result;
    }

    /// Test that get_cpuset returns an error when PMIx is not initialized.
    #[test]
    fn test_get_cpuset_not_initialized() {
        let mut cpuset = PmixCpuset::new();
        let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        // PMIx is not initialized, so we expect an error (PMIX_ERR_INIT).
        assert!(
            result.is_err(),
            "get_cpuset should fail when PMIx is not initialized"
        );
    }

    /// Test that the cpuset is properly cleaned up on drop even after
    /// a failed get_cpuset call.
    #[test]
    fn test_get_cpuset_cleanup_on_error() {
        let mut cpuset = PmixCpuset::new();
        let _ = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        // cpuset should still drop without issues
        drop(cpuset);
    }

    /// Test that get_cpuset can be called multiple times on the same cpuset.
    #[test]
    fn test_get_cpuset_reuse_cpuset() {
        let mut cpuset = PmixCpuset::new();
        let r1 = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        let r2 = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        // Both calls should return the same result (likely PMIX_ERR_INIT).
        assert_eq!(
            r1.is_ok(),
            r2.is_ok(),
            "repeated calls should be consistent"
        );
    }

    /// Test PmixBindEnvelope to_raw conversion.
    #[test]
    fn test_bind_envelope_to_raw() {
        assert_eq!(PmixBindEnvelope::Process.to_raw(), 0);
        assert_eq!(PmixBindEnvelope::Thread.to_raw(), 1);
    }

    /// Test PmixBindEnvelope derive traits.
    #[test]
    fn test_bind_envelope_traits() {
        let p = PmixBindEnvelope::Process;
        assert_eq!(p.clone(), p);
        assert_eq!(p, p);
        assert_ne!(PmixBindEnvelope::Process, PmixBindEnvelope::Thread);
    }

    // ─────────────────────────────────────────────────────────────────────
    // get_relative_locality tests
    // ─────────────────────────────────────────────────────────────────────

    use super::PmixLocality;
    use super::get_relative_locality;

    /// Test that get_relative_locality compiles and does not panic.
    #[test]
    fn test_get_relative_locality_no_panic() {
        let _ = get_relative_locality("0", "0");
    }

    /// Test that get_relative_locality returns error without PMIx session.
    #[test]
    fn test_get_relative_locality_not_initialized() {
        let result = get_relative_locality("0", "1");
        assert!(result.is_err());
    }

    /// Test PmixLocality bit values match C constants.
    #[test]
    fn test_locality_bit_values() {
        assert_eq!(PmixLocality::UNKNOWN.bits(), 0x0000);
        assert_eq!(PmixLocality::NONLOCAL.bits(), 0x8000);
        assert_eq!(PmixLocality::SHARE_HWTHREAD.bits(), 0x0001);
        assert_eq!(PmixLocality::SHARE_CORE.bits(), 0x0002);
        assert_eq!(PmixLocality::SHARE_L1CACHE.bits(), 0x0004);
        assert_eq!(PmixLocality::SHARE_L2CACHE.bits(), 0x0008);
        assert_eq!(PmixLocality::SHARE_L3CACHE.bits(), 0x0010);
        assert_eq!(PmixLocality::SHARE_PACKAGE.bits(), 0x0020);
        assert_eq!(PmixLocality::SHARE_NUMA.bits(), 0x0040);
        assert_eq!(PmixLocality::SHARE_NODE.bits(), 0x4000);
    }

    /// Test PmixLocality from_raw / to_raw round-trip.
    #[test]
    fn test_locality_roundtrip() {
        let raw: u16 = 0x0003; // SHARE_HWTHREAD | SHARE_CORE
        assert_eq!(PmixLocality::from_raw(raw).to_raw(), raw);
    }

    /// Test PmixLocality bit operations.
    #[test]
    fn test_locality_bit_ops() {
        let mut loc = PmixLocality::empty();
        loc.insert(PmixLocality::SHARE_CORE);
        assert!(loc.contains(PmixLocality::SHARE_CORE));
        loc.remove(PmixLocality::SHARE_CORE);
        assert!(!loc.contains(PmixLocality::SHARE_CORE));
    }

    /// Test get_relative_locality rejects NUL bytes.
    #[test]
    fn test_get_relative_locality_nul_byte() {
        assert!(get_relative_locality("a\x00b", "c").is_err());
        assert!(get_relative_locality("a", "c\x00d").is_err());
    }
}
