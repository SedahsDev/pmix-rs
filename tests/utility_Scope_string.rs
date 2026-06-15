//! Integration tests for `PMIx_Scope_string` via the safe `scope_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Scope_string` only looks up a static string table.

use pmix::{PmixScope, utility::scope_string};

/// `scope_string` returns Ok for all defined scope values.
#[test]
fn scope_string_all_defined_values() {
    let scopes = [
        PmixScope::Undef,
        PmixScope::Local,
        PmixScope::Remote,
        PmixScope::Global,
        PmixScope::Internal,
    ];
    for scope in scopes {
        let result = scope_string(scope);
        assert!(
            result.is_ok(),
            "scope_string({:?}) should return Ok, got {:?}",
            scope,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "scope_string({:?}) should not return empty string",
            scope
        );
    }
}

/// `scope_string` returns different strings for different scopes.
#[test]
fn scope_string_distinct_values() {
    let local = scope_string(PmixScope::Local).unwrap();
    let global = scope_string(PmixScope::Global).unwrap();
    assert_ne!(
        local, global,
        "scope_string(Local) and scope_string(Global) must differ"
    );
}

/// `scope_string` handles unknown scope values gracefully.
#[test]
fn scope_string_unknown_value() {
    let unknown = PmixScope::Unknown(99);
    let result = scope_string(unknown);
    assert!(
        result.is_ok(),
        "scope_string(Unknown(99)) should handle gracefully, got {:?}",
        result
    );
}

/// `scope_string` is deterministic.
#[test]
fn scope_string_deterministic() {
    let first = scope_string(PmixScope::Remote).unwrap();
    let second = scope_string(PmixScope::Remote).unwrap();
    assert_eq!(first, second, "scope_string must be deterministic");
}

/// Compile-time type check: returns `Result<String, PmixStatus>`.
#[test]
fn scope_string_return_type() {
    let _result: Result<String, pmix::PmixStatus> = scope_string(PmixScope::Local);
}
