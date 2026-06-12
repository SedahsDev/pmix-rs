//! Integration tests for `PMIx_Alloc_directive_string` via the safe `alloc_directive_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Alloc_directive_string` only looks up a static string
//! table inside the library.

use pmix::{utility::alloc_directive_string, PmixAllocDirective, PmixStatus};

/// `alloc_directive_string` returns `Ok(String)` for PMIX_ALLOC_DIRECTIVE (43).
///
/// The PMIx spec defines `PMIx_Alloc_directive_string` as returning a
/// non-null, null-terminated string for any valid `pmix_alloc_directive_t`.
#[test]
fn alloc_directive_string_known_returns_ok() {
    let directive = PmixAllocDirective::AllocDirective;
    let result = alloc_directive_string(directive);
    assert!(
        result.is_ok(),
        "alloc_directive_string(AllocDirective) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "alloc_directive_string should not return an empty string"
    );
}

/// `alloc_directive_string` returns a readable description for the known
/// allocation directive value.
#[test]
fn alloc_directive_string_returns_readable() {
    let directive = PmixAllocDirective::AllocDirective;
    let desc = alloc_directive_string(directive).unwrap();
    // The C library returns a human-readable string for PMIX_ALLOC_DIRECTIVE.
    assert!(
        !desc.is_empty(),
        "alloc_directive_string(AllocDirective) should return non-empty string, got '{}'",
        desc
    );
}

/// `alloc_directive_string` handles unknown directive values gracefully.
///
/// Future PMIx versions may define new directive values. The C library should
/// still return a valid string (typically "UNKNOWN" or similar).
#[test]
fn alloc_directive_string_unknown_returns_ok() {
    let directive = PmixAllocDirective::Unknown(99);
    let result = alloc_directive_string(directive);
    assert!(
        result.is_ok(),
        "alloc_directive_string(Unknown(99)) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "alloc_directive_string for unknown directive should return non-empty string"
    );
}

/// `alloc_directive_string` is deterministic — the same directive always
/// produces the same string description.
#[test]
fn alloc_directive_string_is_deterministic() {
    let directive = PmixAllocDirective::AllocDirective;
    let first = alloc_directive_string(directive).unwrap();
    let second = alloc_directive_string(directive).unwrap();
    assert_eq!(
        first, second,
        "alloc_directive_string must be deterministic for the same input"
    );
}

/// `alloc_directive_string` returns a `Result<String, PmixStatus>`, not a raw pointer.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn alloc_directive_string_returns_result_string() {
    let directive = PmixAllocDirective::AllocDirective;
    let _result: Result<String, PmixStatus> = alloc_directive_string(directive);
}

/// `PmixAllocDirective::from_raw` maps 43 to `AllocDirective`.
#[test]
fn alloc_directive_from_raw_known() {
    assert_eq!(
        PmixAllocDirective::from_raw(43),
        PmixAllocDirective::AllocDirective,
        "from_raw(43) should map to AllocDirective"
    );
}

/// `PmixAllocDirective::from_raw` maps unknown values to `Unknown`.
#[test]
fn alloc_directive_from_raw_unknown() {
    assert!(
        matches!(PmixAllocDirective::from_raw(0), PmixAllocDirective::Unknown(0)),
        "from_raw(0) should map to Unknown(0)"
    );
    assert!(
        matches!(PmixAllocDirective::from_raw(255), PmixAllocDirective::Unknown(255)),
        "from_raw(255) should map to Unknown(255)"
    );
}

/// `PmixAllocDirective::to_raw` returns the expected raw values.
#[test]
fn alloc_directive_to_raw() {
    assert_eq!(
        PmixAllocDirective::AllocDirective.to_raw(),
        43,
        "AllocDirective.to_raw() should return 43"
    );
    assert_eq!(
        PmixAllocDirective::Unknown(99).to_raw(),
        99,
        "Unknown(99).to_raw() should return 99"
    );
}

/// `from_raw` and `to_raw` round-trip for known values.
#[test]
fn alloc_directive_roundtrip() {
    let directive = PmixAllocDirective::AllocDirective;
    let raw = directive.to_raw();
    let recovered = PmixAllocDirective::from_raw(raw);
    assert_eq!(
        directive, recovered,
        "from_raw(to_raw(AllocDirective)) should round-trip"
    );
}

/// `PmixAllocDirective` implements Display.
#[test]
fn alloc_directive_display() {
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocDirective),
        "ALLOC_DIRECTIVE",
        "Display for AllocDirective should be 'ALLOC_DIRECTIVE'"
    );
    assert_eq!(
        format!("{}", PmixAllocDirective::Unknown(42)),
        "UNKNOWN DIRECTIVE (42)",
        "Display for Unknown(42) should be 'UNKNOWN DIRECTIVE (42)'"
    );
}

/// `PmixAllocDirective` derives PartialEq, Eq, Hash.
#[test]
fn alloc_directive_equality() {
    assert_eq!(
        PmixAllocDirective::AllocDirective,
        PmixAllocDirective::AllocDirective,
        "AllocDirective should equal itself"
    );
    assert_ne!(
        PmixAllocDirective::AllocDirective,
        PmixAllocDirective::Unknown(43),
        "AllocDirective should not equal Unknown(43) even with same raw value"
    );
}
