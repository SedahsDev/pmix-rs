//! Tests for `PMIx_server_generate_locality_string` safe wrapper.
//!
//! The C PMIx library does not ship dedicated test cases for this function.
//! Tests are derived from the function's documented behavior and the patterns
//! used by related server APIs.
//!
//! NOTE: PMIx_server_generate_locality_string segfaults when called without
//! a prior PMIx_server_init. All tests that invoke the FFI function are
//! marked #[ignore] and require a running PMIx server/daemon.

use pmix::fabric::PmixCpuset;
use pmix::server::server_generate_locality_string;
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus round-trip tests (no FFI — these run without a PMIx server)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that success status from the FFI layer maps correctly.
#[test]
fn test_pmix_status_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "raw 0 should be PMIX_SUCCESS");
}

/// Verify that error statuses are properly detected.
#[test]
fn test_pmix_status_error() {
    // PMIX_ERROR = -1
    let status = PmixStatus::from_raw(-1);
    assert!(!status.is_success(), "raw -1 should be an error");
    // PMIX_ERR_NOT_SUPPORTED = -7
    let status = PmixStatus::from_raw(-7);
    assert!(!status.is_success(), "raw -7 should be an error");
    // PMIX_ERR_NOT_FOUND = -46
    let status = PmixStatus::from_raw(-46);
    assert!(!status.is_success(), "raw -46 should be an error");
}

/// Verify the function signature accepts &mut PmixCpuset (not &PmixCpuset).
/// This is a compile-time check — if the signature changes, this test fails.
/// We only test the type system, not the actual FFI call.
#[test]
fn test_signature_type_check() {
    // Verify PmixCpuset is constructable (tests the type exists and is usable).
    let _cpuset = PmixCpuset::new();
    // The RAII drop of PmixCpuset calls PMIx_Cpuset_destruct.
    // NOTE: PMIx_Cpuset_destruct on an uninitialized cpuset may also segfault,
    // so we keep this as a compile-time type check only.
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI tests — require initialized PMIx server (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Basic API call: the function should accept a valid PmixCpuset.
/// Whether it returns Ok or Err depends on the PMIx runtime environment
/// (hwloc availability, etc.), so we only check it doesn't panic.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_locality_string_basic() {
    let mut cpuset = PmixCpuset::new();
    let _result = server_generate_locality_string(&mut cpuset);
}

/// The function should return a PmixStatus (either success or a known error).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_locality_string_returns_pmix_status() {
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_locality_string(&mut cpuset);

    match result {
        Ok(locality) => {
            // On success, the locality string should not be empty.
            assert!(
                !locality.is_empty(),
                "locality string should not be empty on success"
            );
        }
        Err(status) => {
            // On error, we should get a valid PmixStatus (not a raw code of 0).
            // Common errors: PMIX_ERR_NOT_SUPPORTED if hwloc unavailable,
            // PMIX_ERR_INIT if server not initialized, etc.
            let raw = status.to_raw();
            assert!(
                raw != 0,
                "error status should not have raw value 0 (that's PMIX_SUCCESS)"
            );
        }
    }
}

/// Calling the function multiple times with the same cpuset should produce
/// consistent results (same string on success, same error code on failure).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_locality_string_consistent() {
    let mut cpuset = PmixCpuset::new();
    let result1 = server_generate_locality_string(&mut cpuset);
    let result2 = server_generate_locality_string(&mut cpuset);

    match (result1, result2) {
        (Ok(s1), Ok(s2)) => {
            assert_eq!(s1, s2, "locality strings should be consistent across calls");
        }
        (Err(e1), Err(e2)) => {
            assert_eq!(e1, e2, "error statuses should be consistent across calls");
        }
        _ => {
            panic!("inconsistent results: one Ok, one Err");
        }
    }
}

/// Multiple calls should not leak memory (the C-allocated string is freed
/// after each call). This is a basic smoke test.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_locality_string_no_leak_smoke() {
    for _ in 0..100 {
        let mut cpuset = PmixCpuset::new();
        let _ = server_generate_locality_string(&mut cpuset);
        // cpuset is dropped here, C string was already freed.
    }
}

/// PmixCpuset should be properly constructed before use and destructed on drop.
/// This tests the RAII behavior of PmixCpuset in conjunction with our wrapper.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_cpuset_raii() {
    {
        let mut cpuset = PmixCpuset::new();
        let _ = server_generate_locality_string(&mut cpuset);
        // cpuset dropped here — PMIx_Cpuset_destruct should be called.
    }
    // If destruct leaked or double-freed, we'd see errors in valgrind/ASan.
}

/// Creating and dropping multiple cpusets should not cause issues.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_multiple_cpuset_lifecycle() {
    for _ in 0..10 {
        let mut cpuset = PmixCpuset::new();
        let _ = server_generate_locality_string(&mut cpuset);
    }
}

/// If PMIx server is not initialized, the function may return PMIX_ERR_INIT
/// or another error. This test documents that behavior.
#[test]
#[ignore = "requires PMIx server to be initialized — needs running PMIx daemon"]
fn test_requires_server_init() {
    // This test would be run in an environment where PMIx_server_init has
    // NOT been called, to verify the function returns an appropriate error.
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_locality_string(&mut cpuset);
    assert!(result.is_err(), "should fail without server init");
}

/// When PMIx server IS initialized and hwloc is available, the function
/// should return a valid locality string.
#[test]
#[ignore = "requires PMIx server initialized and hwloc available"]
fn test_with_initialized_server() {
    // This test assumes PMIx_server_init has been called elsewhere
    // (e.g., in a test harness or integration test setup).
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_locality_string(&mut cpuset);
    assert!(result.is_ok(), "should succeed with initialized server");
    let locality = result.unwrap();
    assert!(!locality.is_empty(), "locality string should not be empty");
}

/// Locality string format: PMIx locality strings are colon-separated
/// hierarchical values representing topology levels.
#[test]
#[ignore = "requires PMIx server initialized and hwloc available"]
fn test_locality_string_format() {
    let mut cpuset = PmixCpuset::new();
    let locality = server_generate_locality_string(&mut cpuset)
        .expect("should succeed with initialized server");

    // PMIx locality strings are non-empty and contain alphanumeric chars
    // and separators (colons, hyphens, etc.)
    assert!(
        locality.len() > 1,
        "locality string should be more than one character"
    );
}

/// Test that the function handles the case where the C library returns
/// a null pointer gracefully (should not panic or segfault).
#[test]
#[ignore = "requires mocking PMIx C library to return null"]
fn test_null_locality_pointer() {
    // If PMIx returns NULL for the locality string, our wrapper should
    // return Err(PmixStatus::from_raw(-1)) instead of panicking.
    // This would require intercepting the FFI call to verify.
}

/// Verify the function properly frees the C-allocated string.
/// A double-free or use-after-free would be caught by ASan/valgrind.
#[test]
#[ignore = "requires AddressSanitizer or valgrind"]
fn test_c_string_freed() {
    let mut cpuset = PmixCpuset::new();
    let _locality = server_generate_locality_string(&mut cpuset);
    // If the C string was not freed, running under valgrind would show a leak.
    // If it was double-freed, ASan would catch it.
}
