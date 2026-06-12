//! Tests for `PMIx_Data_print` and `PmixPrintOutput`.
//!
//! `PMIx_Data_print` converts a data value of any PMIx-defined type to a
//! human-readable string. It requires `PMIx_Init` because it accesses
//! `pmix_globals.mypeer` to determine the active bfrops peer for type
//! resolution.
//!
//! Tests that call the FFI function are marked `#[ignore]` and need a PMIx
//! runtime environment to execute. Compile-only tests verify the API surface.

use pmix::data_serialization::*;
use pmix::{PmixDataType, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// API surface — compile-only type checks (no FFI call, no PMIx_Init needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify data_print<T> is callable with i32 and returns the right type.
#[test]
fn test_data_print_signature_i32() {
    fn check<F>(_: F) {}
    check::<fn(&i32, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with i64.
#[test]
fn test_data_print_signature_i64() {
    fn check<F>(_: F) {}
    check::<fn(&i64, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with f64.
#[test]
fn test_data_print_signature_f64() {
    fn check<F>(_: F) {}
    check::<fn(&f64, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with u8.
#[test]
fn test_data_print_signature_u8() {
    fn check<F>(_: F) {}
    check::<fn(&u8, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with u32 (size_t on many platforms).
#[test]
fn test_data_print_signature_u32() {
    fn check<F>(_: F) {}
    check::<fn(&u32, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with u64.
#[test]
fn test_data_print_signature_u64() {
    fn check<F>(_: F) {}
    check::<fn(&u64, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with bool.
#[test]
fn test_data_print_signature_bool() {
    fn check<F>(_: F) {}
    check::<fn(&bool, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with isize (pid_t).
#[test]
fn test_data_print_signature_isize() {
    fn check<F>(_: F) {}
    check::<fn(&isize, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

/// Verify data_print<T> is callable with f32.
#[test]
fn test_data_print_signature_f32() {
    fn check<F>(_: F) {}
    check::<fn(&f32, Option<&str>, PmixDataType) -> Result<PmixPrintOutput, PmixStatus>>(
        data_print,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPrintOutput type checks (compile-only, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify PmixPrintOutput implements std::fmt::Display.
#[test]
fn test_pmix_print_output_display() {
    fn is_display<T: std::fmt::Display>() {}
    is_display::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput implements std::fmt::Debug.
#[test]
fn test_pmix_print_output_debug() {
    fn is_debug<T: std::fmt::Debug>() {}
    is_debug::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput implements Deref<Target = str>.
#[test]
fn test_pmix_print_output_deref_str() {
    fn is_deref_str<T>() where T: std::ops::Deref<Target = str> {}
    is_deref_str::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput implements From<PmixPrintOutput> for String.
#[test]
fn test_pmix_print_output_into_string() {
    fn can_into_string<T>() where String: From<T> {}
    can_into_string::<PmixPrintOutput>();
}

/// Verify PmixPrintOutput has Drop (implicit — String field implements Drop).
/// Rust automatically provides Drop for types containing Drop fields.
/// PmixPrintOutput wraps a String, so it drops correctly by default.
#[test]
fn test_pmix_print_output_has_drop() {
    // PmixPrintOutput contains a String field, which implements Drop.
    // The C string is freed during from_raw() construction (via CString::from_raw),
    // so the Drop impl is handled by the String field itself.
    // This test documents the memory management approach.
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx_Init (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Print an i32 value and verify the output contains the value.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_i32() {
    let val: i32 = 42;
    let result = data_print(&val, None, PmixDataType::Int32);
    assert!(result.is_ok(), "data_print should succeed for Int32: {:?}", result);
    let output = result.unwrap();
    assert!(
        output.contains("42"),
        "Printed output should contain '42', got: {}",
        output
    );
}

/// Print an i32 value with a prefix.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_i32_with_prefix() {
    let val: i32 = 100;
    let result = data_print(&val, Some("val="), PmixDataType::Int32);
    assert!(result.is_ok(), "data_print with prefix should succeed: {:?}", result);
    let output = result.unwrap();
    // Output should contain the prefix
    assert!(
        output.contains("val="),
        "Output should contain prefix 'val=', got: {}",
        output
    );
}

/// Print a u64 value.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_u64() {
    let val: u64 = 0xDEADBEEF;
    let result = data_print(&val, None, PmixDataType::Uint64);
    assert!(result.is_ok(), "data_print should succeed for Uint64: {:?}", result);
    let output = result.unwrap();
    assert!(
        !output.is_empty(),
        "Printed output should not be empty"
    );
}

/// Print a bool value.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_bool() {
    let val: bool = true;
    let result = data_print(&val, None, PmixDataType::Bool);
    assert!(result.is_ok(), "data_print should succeed for Bool: {:?}", result);
    let output = result.unwrap();
    assert!(
        !output.is_empty(),
        "Printed output should not be empty"
    );
}

/// Print a float value.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_f64() {
    let val: f64 = 3.14159;
    let result = data_print(&val, None, PmixDataType::Double);
    assert!(result.is_ok(), "data_print should succeed for Float64: {:?}", result);
    let output = result.unwrap();
    assert!(
        !output.is_empty(),
        "Printed output should not be empty"
    );
}

/// Print with empty prefix should behave like None.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_empty_prefix() {
    let val: i32 = 7;
    let result = data_print(&val, Some(""), PmixDataType::Int32);
    // Empty prefix should be treated as no prefix (passes null to C).
    assert!(result.is_ok(), "empty prefix should succeed: {:?}", result);
}

/// Convert PmixPrintOutput to String via Into.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_into_string() {
    let val: i32 = 99;
    let output: String = data_print(&val, None, PmixDataType::Int32)
        .expect("data_print should succeed")
        .into();
    assert!(!output.is_empty(), "Converted string should not be empty");
    assert!(output.contains("99"), "String should contain '99', got: {}", output);
}

/// Print multiple values of the same type in sequence.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_multiple() {
    let values: [i32; 4] = [1, 2, 3, 4];
    for &val in &values {
        let result = data_print(&val, None, PmixDataType::Int32);
        assert!(result.is_ok(), "data_print should succeed for each value");
        let output = result.unwrap();
        assert!(!output.is_empty(), "Output should not be empty for value {}", val);
    }
}

/// Verify PmixPrintOutput Deref works for str operations.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_deref_operations() {
    let val: i32 = 42;
    let output = data_print(&val, None, PmixDataType::Int32)
        .expect("data_print should succeed");
    // Deref<Target = str> should allow str methods
    let _len = output.len();
    let _contains = output.contains("INT32");
    // Both should compile and not panic
}

/// Print with a data type that may not be supported.
/// Requires PMIx_Init — needs a running PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init and PMIx daemon"]
fn test_data_print_type_name() {
    let val: i32 = 0;
    let result = data_print(&val, None, PmixDataType::Int32);
    assert!(result.is_ok(), "Int32 should be supported");
    let output = result.unwrap();
    // PMIx print output includes the type name
    assert!(
        output.to_lowercase().contains("int32") || output.to_lowercase().contains("int"),
        "Output should mention the type, got: {}",
        output
    );
}
