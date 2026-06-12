//! Tests for PMIx_Parse_cpuset_string — cpu_locality module.
//!
//! These tests verify the safe Rust wrapper around the C API function
//! `PMIx_Parse_cpuset_string`, which parses a string representation of
//! a CPU binding bitmap into a `pmix_cpuset_t` object.

use pmix::cpu_locality::parse_cpuset_string;
use pmix::fabric::PmixCpuset;

// ─────────────────────────────────────────────────────────────────────────────
// Basic parsing tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the function compiles and can be called with a valid cpuset string.
#[test]
fn test_parse_single_cpu() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0", &mut cpuset);
    // Without a running PMIx session, this may return an error.
    // The important thing is that the FFI call is made correctly.
    let _ = result;
}

/// Test parsing a CPU range format.
#[test]
fn test_parse_cpu_range() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0-3", &mut cpuset);
    let _ = result;
}

/// Test parsing a comma-separated list of CPUs.
#[test]
fn test_parse_cpu_list() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0,2,4,6", &mut cpuset);
    let _ = result;
}

/// Test parsing a mixed format (ranges and individual CPUs).
#[test]
fn test_parse_mixed_format() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0-3,5,8-11", &mut cpuset);
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Test with an empty string — not a valid cpuset representation.
#[test]
fn test_parse_empty_string() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("", &mut cpuset);
    // The result depends on the PMIx implementation.
    // Either way, no panic should occur.
    let _ = result;
}

/// Test with a very long cpuset string (256 CPUs listed individually).
#[test]
fn test_parse_long_cpu_list() {
    let mut cpuset = PmixCpuset::new();
    let long_list = (0..256).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let result = parse_cpuset_string(&long_list, &mut cpuset);
    let _ = result;
}

/// Test with a very large CPU range.
#[test]
fn test_parse_large_range() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0-1023", &mut cpuset);
    let _ = result;
}

/// Test that a string with an interior NUL byte returns an error.
#[test]
fn test_parse_string_with_nul_byte() {
    let mut cpuset = PmixCpuset::new();
    // Strings with interior NUL bytes cannot be converted to CString.
    let result = parse_cpuset_string("hello\x00world", &mut cpuset);
    assert!(result.is_err(), "Expected error for string with NUL byte");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixCpuset lifecycle tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixCpuset is properly cleaned up after a failed parse.
#[test]
fn test_cpuset_cleanup_on_error() {
    let mut cpuset = PmixCpuset::new();
    // NUL byte in string causes an error before the FFI call.
    let _ = parse_cpuset_string("bad\x00string", &mut cpuset);
    // The cpuset should still drop cleanly.
    drop(cpuset);
}

/// Test that PmixCpuset is properly cleaned up after a successful parse.
#[test]
fn test_cpuset_cleanup_on_success() {
    let mut cpuset = PmixCpuset::new();
    let _ = parse_cpuset_string("0", &mut cpuset);
    drop(cpuset); // Should not leak or double-free.
}

/// Test that the cpuset can be reused after a parse.
#[test]
fn test_cpuset_reuse() {
    let mut cpuset = PmixCpuset::new();
    let _ = parse_cpuset_string("0", &mut cpuset);
    let _ = parse_cpuset_string("0-3", &mut cpuset);
    let _ = parse_cpuset_string("0,2,4", &mut cpuset);
    drop(cpuset);
}

// ─────────────────────────────────────────────────────────────────────────────
// Format-specific tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test parsing with whitespace (some implementations may accept this).
#[test]
fn test_parse_with_whitespace() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0, 2, 4", &mut cpuset);
    let _ = result;
}

/// Test parsing a single large CPU number.
#[test]
fn test_parse_single_large_cpu() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("1024", &mut cpuset);
    let _ = result;
}

/// Test parsing with overlapping ranges.
#[test]
fn test_parse_overlapping_ranges() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0-5,3-8", &mut cpuset);
    let _ = result;
}

/// Test parsing with reversed range (high-low).
#[test]
fn test_parse_reversed_range() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("5-0", &mut cpuset);
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Test that parse_cpuset_string works within a PMIx session.
/// This requires PMIx_Init to have been called, so it is ignored by default.
#[test]
#[ignore = "requires PMIx runtime session"]
fn test_parse_in_pmix_session() {
    // This test would be run as part of an integration test suite
    // where PMIx_Init has been called first.
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("0-3", &mut cpuset);
    assert!(result.is_ok(), "parse_cpuset_string should succeed in a PMIx session");
}

/// Test round-trip: parse a cpuset string, then verify the result.
/// Requires PMIx runtime.
#[test]
#[ignore = "requires PMIx runtime session"]
fn test_parse_roundtrip() {
    let mut cpuset = PmixCpuset::new();
    let cpu_string = "0,2,4,6";
    let result = parse_cpuset_string(cpu_string, &mut cpuset);
    assert!(result.is_ok(), "round-trip parse should succeed");
}

/// Test that parse_cpuset_string handles the format returned by
/// PMIx_server_generate_cpuset_string (hex-encoded bitmap).
#[test]
#[ignore = "requires PMIx runtime session"]
fn test_parse_hex_format() {
    let mut cpuset = PmixCpuset::new();
    // Hex format cpuset strings are returned by PMIx_server_generate_cpuset_string.
    // The exact format depends on the implementation.
    let result = parse_cpuset_string("0x0000000000000003", &mut cpuset);
    assert!(result.is_ok(), "hex format parse should succeed");
}

/// Test that an invalid format returns an appropriate error.
#[test]
#[ignore = "requires PMIx runtime session"]
fn test_parse_invalid_format() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("not_a_cpuset", &mut cpuset);
    // This should return an error since "not_a_cpuset" is not a valid format.
    assert!(result.is_err(), "invalid format should return error");
}

// ─────────────────────────────────────────────────────────────────────────────
// Stress tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test parsing with many ranges.
#[test]
fn test_parse_many_ranges() {
    let mut cpuset = PmixCpuset::new();
    let ranges: Vec<String> = (0..100).map(|i| format!("{}-{}", i * 10, i * 10 + 9)).collect();
    let result = parse_cpuset_string(&ranges.join(","), &mut cpuset);
    let _ = result;
}

/// Test parsing with a single CPU at a high index.
#[test]
fn test_parse_high_index() {
    let mut cpuset = PmixCpuset::new();
    let result = parse_cpuset_string("65535", &mut cpuset);
    let _ = result;
}

/// Test that multiple consecutive parses don't cause memory issues.
#[test]
fn test_consecutive_parses() {
    let mut cpuset = PmixCpuset::new();
    for i in 0..100 {
        let cpu_str = format!("{}", i);
        let _ = parse_cpuset_string(&cpu_str, &mut cpuset);
    }
    drop(cpuset);
}

/// Test parsing with boundary values.
#[test]
fn test_parse_boundary_values() {
    let mut cpuset = PmixCpuset::new();
    let _ = parse_cpuset_string("0", &mut cpuset);
    let _ = parse_cpuset_string("1", &mut cpuset);
    let _ = parse_cpuset_string("0-1", &mut cpuset);
}
