//! Integration tests for `PMIx_Get_version` via the safe `get_version()` wrapper.
//!
//! `PMIx_Get_version` does NOT require PMIx initialization — it returns
//! the library version string from a static table.

use pmix::get_version;

/// `get_version` returns a non-empty version string.
#[test]
fn get_version_returns_non_empty() {
    let version = get_version();
    assert!(!version.is_empty(), "get_version should return non-empty string");
}

/// `get_version` returns a string matching typical version format (e.g. "4.1.1").
#[test]
fn get_version_has_version_format() {
    let version = get_version();
    // Version strings typically contain digits and dots
    assert!(
        version.chars().any(|c| c.is_ascii_digit()),
        "get_version('{}') should contain digits",
        version
    );
}

/// `get_version` returns `&'static str`, not `String`.
#[test]
fn get_version_return_type() {
    let _v: &str = get_version();
}

/// `get_version` is deterministic — same call returns same value.
#[test]
fn get_version_deterministic() {
    let v1 = get_version();
    let v2 = get_version();
    assert_eq!(v1, v2, "get_version must be deterministic");
}
