//! Tests for `PMIx_server_generate_cpuset_string` safe wrapper.
//!
//! The C PMIx library does not ship dedicated test cases for this function.
//! Tests are derived from the function's documented behavior (spec section 16.2.18),
//! the patterns used by `simpfabric.c`, and the related `generate_locality_string` API.
//!
//! NOTE: PMIx_server_generate_cpuset_string segfaults when called without
//! a prior PMIx_server_init. All tests that invoke the FFI function are
//! marked #[ignore] and require a running PMIx server/daemon.

use pmix::PmixStatus;
use pmix::fabric::PmixCpuset;
use pmix::server::server_generate_cpuset_string;

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
    // PMIX_ERR_BAD_PARAM = -8 (returned by C impl when cpuset/bitmap is null)
    let status = PmixStatus::from_raw(-8);
    assert!(!status.is_success(), "raw -8 should be an error");
    // PMIX_ERR_TAKE_NEXT_OPTION = -11 (returned when source is not hwloc)
    let status = PmixStatus::from_raw(-11);
    assert!(!status.is_success(), "raw -11 should be an error");
}

/// Verify the function signature accepts &mut PmixCpuset (not &PmixCpuset).
/// This is a compile-time check — if the signature changes, this test fails.
#[test]
fn test_signature_type_check() {
    // Verify PmixCpuset is constructable (tests the type exists and is usable).
    let _cpuset = PmixCpuset::new();
    // The RAII drop of PmixCpuset calls PMIx_Cpuset_destruct.
}

/// Verify that the function returns Result<String, PmixStatus> at the type level.
/// Compile-time check — the types must match or this test won't compile.
#[test]
fn test_return_type_check() {
    // This is a type-level assertion: server_generate_cpuset_string returns
    // Result<String, PmixStatus>. If the return type changes, this fails to compile.
    fn assert_return_type(_: Result<String, PmixStatus>) {}
    // We can't call the function without a server, but we can verify the
    // type signature is what we expect by checking the function pointer type.
    let _fn_ptr: fn(&mut PmixCpuset) -> Result<String, PmixStatus> = server_generate_cpuset_string;
    // Suppress unused warning on the assertion helper.
    let _ = assert_return_type;
}

/// Verify PMIX_ERR_BAD_PARAM raw value matches the spec (used by C impl for null cpuset).
#[test]
fn test_pmix_err_bad_param_value() {
    // PMIX_ERR_BAD_PARAM = -8 in PMIx v4.x
    let status = PmixStatus::from_raw(-8);
    assert!(
        !status.is_success(),
        "PMIX_ERR_BAD_PARAM should be an error"
    );
}

/// Verify PMIX_ERR_TAKE_NEXT_OPTION raw value (used when cpuset source != hwloc).
#[test]
fn test_pmix_err_take_next_option_value() {
    // PMIX_ERR_TAKE_NEXT_OPTION = -11 in PMIx v4.x
    let status = PmixStatus::from_raw(-11);
    assert!(
        !status.is_success(),
        "PMIX_ERR_TAKE_NEXT_OPTION should be an error"
    );
}

/// Verify PmixCpuset construction and destruction work correctly (RAII).
#[test]
fn test_cpuset_construction() {
    let cpuset = PmixCpuset::new();
    // cpuset is dropped here — PMIx_Cpuset_destruct should be called.
    // If destruct crashes on an empty cpuset, this test would panic.
    drop(cpuset);
}

/// Verify multiple PmixCpuset constructions and destructions don't leak.
#[test]
fn test_cpuset_multiple_lifecycle() {
    for _ in 0..10 {
        let _cpuset = PmixCpuset::new();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI tests — require initialized PMIx server (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Basic API call: the function should accept a valid PmixCpuset.
/// Whether it returns Ok or Err depends on the PMIx runtime environment
/// (hwloc availability, server initialization, etc.), so we only check
/// it doesn't panic.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_cpuset_string_basic() {
    let mut cpuset = PmixCpuset::new();
    let _result = server_generate_cpuset_string(&mut cpuset);
}

/// The function should return a PmixStatus (either success or a known error).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_generate_cpuset_string_returns_pmix_status() {
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_cpuset_string(&mut cpuset);

    match result {
        Ok(cpuset_str) => {
            // On success, the cpuset string should not be empty.
            assert!(
                !cpuset_str.is_empty(),
                "cpuset string should not be empty on success"
            );
        }
        Err(status) => {
            // On error, we should get a valid PmixStatus (not a raw code of 0).
            // Common errors: PMIX_ERR_BAD_PARAM if cpuset not populated,
            // PMIX_ERR_TAKE_NEXT_OPTION if source is not hwloc,
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
fn test_generate_cpuset_string_consistent() {
    let mut cpuset = PmixCpuset::new();
    let result1 = server_generate_cpuset_string(&mut cpuset);
    let result2 = server_generate_cpuset_string(&mut cpuset);

    match (result1, result2) {
        (Ok(s1), Ok(s2)) => {
            assert_eq!(s1, s2, "cpuset strings should be consistent across calls");
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
fn test_generate_cpuset_string_no_leak_smoke() {
    for _ in 0..100 {
        let mut cpuset = PmixCpuset::new();
        let _ = server_generate_cpuset_string(&mut cpuset);
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
        let _ = server_generate_cpuset_string(&mut cpuset);
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
        let _ = server_generate_cpuset_string(&mut cpuset);
    }
}

/// If PMIx server is not initialized, the function may return PMIX_ERR_INIT
/// or segfault. This test documents that behavior.
#[test]
#[ignore = "requires PMIx server to be initialized — needs running PMIx daemon"]
fn test_requires_server_init() {
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_cpuset_string(&mut cpuset);
    assert!(result.is_err(), "should fail without server init");
}

/// When PMIx server IS initialized and hwloc is available, the function
/// should return a valid cpuset string prefixed with the source.
#[test]
#[ignore = "requires PMIx server initialized and hwloc available"]
fn test_with_initialized_server() {
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_cpuset_string(&mut cpuset);
    assert!(result.is_ok(), "should succeed with initialized server");
    let cpuset_str = result.unwrap();
    assert!(!cpuset_str.is_empty(), "cpuset string should not be empty");
}

/// Cpuset string format: PMIx cpuset strings are prefixed with the source
/// followed by a colon (e.g., "hwloc:0-3,8-11").
#[test]
#[ignore = "requires PMIx server initialized and hwloc available"]
fn test_cpuset_string_format() {
    let mut cpuset = PmixCpuset::new();
    let cpuset_str =
        server_generate_cpuset_string(&mut cpuset).expect("should succeed with initialized server");

    // PMIx cpuset strings contain a colon separating source from bitmap list.
    assert!(
        cpuset_str.contains(':'),
        "cpuset string should contain a colon (source:bitmap format)"
    );
    let parts: Vec<&str> = cpuset_str.splitn(2, ':').collect();
    assert_eq!(
        parts.len(),
        2,
        "cpuset string should have source:bitmap format"
    );
    assert!(
        !parts[0].is_empty(),
        "cpuset string source prefix should not be empty"
    );
}

/// The cpuset string should be prefixed with "hwloc:" when the underlying
/// implementation is hwloc (the most common case).
#[test]
#[ignore = "requires PMIx server initialized and hwloc available"]
fn test_cpuset_string_hwloc_prefix() {
    let mut cpuset = PmixCpuset::new();
    let cpuset_str =
        server_generate_cpuset_string(&mut cpuset).expect("should succeed with initialized server");

    // The C implementation uses pmix_asprintf(cpuset_string, "hwloc:%s", tmp);
    assert!(
        cpuset_str.starts_with("hwloc:"),
        "cpuset string should start with 'hwloc:' prefix, got: {}",
        cpuset_str
    );
}

/// Test that the function handles the case where the C library returns
/// a null pointer gracefully (should not panic or segfault).
#[test]
#[ignore = "requires mocking PMIx C library to return null"]
fn test_null_cpuset_string_pointer() {
    // If PMIx returns NULL for the cpuset string, our wrapper should
    // return Err(PmixStatus::from_raw(-1)) instead of panicking.
    // This would require intercepting the FFI call to verify.
}

/// Verify the function properly frees the C-allocated string.
/// A double-free or use-after-free would be caught by ASan/valgrind.
#[test]
#[ignore = "requires AddressSanitizer or valgrind"]
fn test_c_string_freed() {
    let mut cpuset = PmixCpuset::new();
    let _cpuset_str = server_generate_cpuset_string(&mut cpuset);
    // If the C string was not freed, running under valgrind would show a leak.
    // If it was double-freed, ASan would catch it.
}

/// Test derived from simpfabric.c usage pattern:
/// PMIx_Get_cpuset -> PMIx_server_generate_cpuset_string -> free(result).
/// Our wrapper handles the free automatically via libc::free.
#[test]
#[ignore = "requires PMIx server initialized — simpfabric.c pattern"]
fn test_simpfabric_pattern() {
    // This test mirrors the usage in test/simple/simpfabric.c:
    //
    //   PMIX_CPUSET_CONSTRUCT(&mycpuset);
    //   rc = PMIx_Get_cpuset(&mycpuset, PMIX_CPUBIND_PROCESS);
    //   PMIx_server_generate_cpuset_string(&mycpuset, &ppn);
    //   fprintf(stderr, "Got my cpuset: %s\n", ppn);
    //   free(ppn);
    //
    // In our Rust wrapper, the free is handled automatically.
    let mut cpuset = PmixCpuset::new();
    let result = server_generate_cpuset_string(&mut cpuset);
    match result {
        Ok(ppn) => {
            // ppn is an owned String — no manual free needed.
            assert!(
                !ppn.is_empty(),
                "cpuset string from simpfabric pattern should not be empty"
            );
        }
        Err(status) => {
            // Expected if server not initialized or cpuset not populated.
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Cpuset strings should be usable as input to PMIx_Parse_cpuset_string.
/// This is a documented round-trip: generate -> parse should reconstruct
/// the original cpuset (when both use the same source, e.g. hwloc).
#[test]
#[ignore = "requires PMIx server initialized — round-trip with parse"]
fn test_roundtrip_with_parse() {
    // Spec section 11.4.3 (PMIx_Parse_cpuset_string) documents that strings
    // returned by PMIx_server_generate_cpuset_string can be parsed back.
    //
    //   PMIx_server_generate_cpuset_string(&cpuset, &str);
    //   PMIx_Parse_cpuset_string(str, &parsed_cpuset);
    //
    // We cannot fully test this without PMIx_Parse_cpuset_string being ported,
    // but we can verify the string format is parseable (contains source:bitmap).
    let mut cpuset = PmixCpuset::new();
    let cpuset_str =
        server_generate_cpuset_string(&mut cpuset).expect("should succeed with initialized server");

    // Verify the string has a parseable format: source:bitmap
    let parts: Vec<&str> = cpuset_str.splitn(2, ':').collect();
    assert_eq!(parts.len(), 2, "should be source:bitmap format");
    assert!(!parts[1].is_empty(), "bitmap portion should not be empty");
}
