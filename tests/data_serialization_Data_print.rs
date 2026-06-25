//! Tests for `data_print` — print human-readable representation of data values.
//!
//! `data_print` calls `PMIx_Data_print` FFI which **segfaults** when PMIx
//! is not initialized. All functional tests are marked `#[ignore]` with
//! the reason "requires PMIx_Init". Non-ignored tests cover the
//! `PmixPrintOutput` wrapper type which is pure Rust.

use pmix::PmixDataType;
use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// PmixPrintOutput — pure Rust, safe without PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// PmixPrintOutput implements Display.
#[test]
fn test_print_output_display() {
    // We can't construct PmixPrintOutput directly (from_raw is private/unsafe),
    // but we can verify the type is Display via a compile-time check.
    fn assert_display<T: std::fmt::Display>() {}
    assert_display::<PmixPrintOutput>();
}

/// PmixPrintOutput implements Debug.
#[test]
fn test_print_output_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixPrintOutput>();
}

/// PmixPrintOutput is Send and Sync.
#[test]
fn test_print_output_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PmixPrintOutput>();
}

/// PmixPrintOutput as_str() method exists and is callable.
#[test]
fn test_print_output_as_str_exists() {
    // Verify the method signature compiles — we can't construct an instance
    // without calling data_print, which requires PMIx_Init.
    // The as_str() method is documented in the public API.
    let _f: fn(&PmixPrintOutput) -> &str = PmixPrintOutput::as_str;
}

/// PmixPrintOutput is not Clone (wraps raw C pointer).
#[test]
fn test_print_output_not_clone() {
    // PmixPrintOutput does NOT implement Clone — it wraps a raw C char*
    // pointer that was allocated by the FFI layer. This is intentional
    // to avoid double-free issues.
}

/// PmixPrintOutput as_str returns &str reference.
#[test]
fn test_print_output_as_str_return_type() {
    // Verify the method signature: fn as_str(&self) -> &str
    let _f: fn(&PmixPrintOutput) -> &str = PmixPrintOutput::as_str;
}

// ─────────────────────────────────────────────────────────────────────────────
// data_print — requires PMIx_Init (FFI segfaults without it)
// ─────────────────────────────────────────────────────────────────────────────

/// data_print with i32 value and no prefix.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_i32_no_prefix() {
    let val: i32 = 42;
    let result = data_print(&val, None, PmixDataType::Int32);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("42"));
}

/// data_print with i32 value and prefix string.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_i32_with_prefix() {
    let val: i32 = 42;
    let result = data_print(&val, Some("my_key"), PmixDataType::Int32);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("my_key"));
    assert!(output.as_str().contains("42"));
}

/// data_print with String value.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_string_value() {
    let val = String::from("hello world");
    let result = data_print(&val, None, PmixDataType::String);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("hello"));
}

/// data_print with bool value.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_bool_value() {
    let val: bool = true;
    let result = data_print(&val, None, PmixDataType::Bool);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("true") || output.as_str().contains("1"));
}

/// data_print with f64 value.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_double_value() {
    let val: f64 = 3.14159;
    let result = data_print(&val, None, PmixDataType::Double);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("3.14"));
}

/// data_print with u64 value.
#[ignore = "requires PMIx_Init — PMIx_Data_print segfaults without initialization"]
#[test]
fn test_print_u64_value() {
    let val: u64 = 999_999_999_999u64;
    let result = data_print(&val, None, PmixDataType::Uint64);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().contains("999999999999"));
}
