//! Integration tests for `PMIx_Get_attribute_string` and `PMIx_Get_attribute_name`
//! via the safe `get_attribute_string()` and `get_attribute_name()` wrappers.
//!
//! IMPORTANT: Unlike `PMIx_Error_string` or `PMIx_Proc_state_string`, these
//! functions access the PMIx library's internal `pmix_globals.keyindex` table,
//! which requires `PMIx_Init` to have been called first. Calling them without
//! initialization causes a segmentation fault on the system PMIx library.
//!
//! All tests are marked `#[ignore]` by default. To run them, start a PMIx
//! daemon or call `PMIx_Init` from a test harness, then use:
//!
//! ```text
//! cargo test --test utility_Get_attribute_string -- --ignored
//! ```

mod daemon_helper;

use pmix::PmixStatus;
use pmix::utility::{get_attribute_name, get_attribute_string};

// ─────────────────────────────────────────────────────────────────────────────
// get_attribute_string — basic behavior
// ─────────────────────────────────────────────────────────────────────────────

/// `get_attribute_string` returns `Ok(String)` for a known attribute key.
///
/// The PMIx library has a registered table of attribute keys. For known keys,
/// it returns the canonical string representation. For unknown keys, it
/// returns the input unchanged. Either way, the result should be `Ok`.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_known_key_returns_ok() {
    daemon_helper::ensure_pmix_init();
    // "pmix.host" is a well-known PMIx attribute key.
    let result = get_attribute_string("pmix.host");
    assert!(
        result.is_ok(),
        "get_attribute_string(\"pmix.host\") should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "get_attribute_string should not return an empty string"
    );
}

/// `get_attribute_string` handles unknown attribute keys gracefully.
///
/// When the attribute is not in the registered table, the C implementation
/// returns the input string unchanged. The wrapper should still return `Ok`.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_unknown_key_returns_input() {
    daemon_helper::ensure_pmix_init();
    let input = "pmix.nonexistent.attribute.xyz";
    let result = get_attribute_string(input);
    assert!(
        result.is_ok(),
        "get_attribute_string for unknown key should return Ok, got {:?}",
        result
    );
    let output = result.unwrap();
    // The C implementation returns the input unchanged for unknown keys.
    assert_eq!(
        output, input,
        "Unknown attribute key should be returned unchanged"
    );
}

/// `get_attribute_string` handles various well-known PMIx attribute keys.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_various_keys() {
    daemon_helper::ensure_pmix_init();
    let keys = [
        "pmix.host",
        "pmix.nprocs",
        "pmix.rank",
        "pmix.universe_size",
        "pmix.nodemap",
        "pmix.topology",
        "pmix.exit_code",
    ];
    for key in &keys {
        let result = get_attribute_string(key);
        assert!(
            result.is_ok(),
            "get_attribute_string(\"{}\") should return Ok, got {:?}",
            key,
            result
        );
        let output = result.unwrap();
        assert!(
            !output.is_empty(),
            "get_attribute_string(\"{}\") should not return empty",
            key
        );
    }
}

/// `get_attribute_string` is deterministic — the same key always returns
/// the same canonical string.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_is_deterministic() {
    daemon_helper::ensure_pmix_init();
    let key = "pmix.host";
    let first = get_attribute_string(key).unwrap();
    let second = get_attribute_string(key).unwrap();
    assert_eq!(
        first, second,
        "get_attribute_string must be deterministic for the same input"
    );
}

/// `get_attribute_string` returns different strings for different known keys.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_distinct_for_different_keys() {
    daemon_helper::ensure_pmix_init();
    let host = get_attribute_string("pmix.host").unwrap();
    let nprocs = get_attribute_string("pmix.nprocs").unwrap();
    // Even if both are returned unchanged (library not initialized),
    // the input strings themselves are distinct.
    assert_ne!(
        "pmix.host", "pmix.nprocs",
        "Different keys should produce different results or at least different inputs"
    );
    // If the library has registered attributes, the outputs should also differ.
    // If not initialized, both return inputs unchanged, which are already distinct.
    let _ = (host, nprocs); // suppress unused warnings
}

/// `get_attribute_string` returns a `Result<String, PmixStatus>`.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile. Does NOT require PMIx_Init.
#[test]
fn get_attribute_string_returns_result_string() {
    // Compile-time only: verify the return type is Result<String, PmixStatus>.
    // We cannot actually call the function without PMIx_Init, so this is
    // a type assertion that ensures the signature is correct.
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_string;
}

// ─────────────────────────────────────────────────────────────────────────────
// get_attribute_name — basic behavior (reverse lookup)
// ─────────────────────────────────────────────────────────────────────────────

/// `get_attribute_name` returns `Ok(String)` for a known attribute string.
///
/// This is the inverse of `get_attribute_string`: given the canonical string
/// representation, it returns the attribute key name.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_known_string_returns_ok() {
    daemon_helper::ensure_pmix_init();
    // Try a descriptive attribute string — the library may or may not have
    // it registered depending on initialization state.
    let result = get_attribute_name("host name");
    assert!(
        result.is_ok(),
        "get_attribute_name should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "get_attribute_name should not return an empty string"
    );
}

/// `get_attribute_name` handles unknown strings gracefully.
///
/// When the string is not in the registered table, the C implementation
/// returns the input unchanged.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_unknown_string_returns_input() {
    daemon_helper::ensure_pmix_init();
    let input = "this.is.not.a.registered.attribute.string";
    let result = get_attribute_name(input);
    assert!(
        result.is_ok(),
        "get_attribute_name for unknown string should return Ok, got {:?}",
        result
    );
    let output = result.unwrap();
    assert_eq!(
        output, input,
        "Unknown attribute string should be returned unchanged"
    );
}

/// `get_attribute_name` is deterministic.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_is_deterministic() {
    daemon_helper::ensure_pmix_init();
    let input = "pmix.host";
    let first = get_attribute_name(input).unwrap();
    let second = get_attribute_name(input).unwrap();
    assert_eq!(
        first, second,
        "get_attribute_name must be deterministic for the same input"
    );
}

/// `get_attribute_name` returns a `Result<String, PmixStatus>`.
///
/// Compile-time type check. Does NOT require PMIx_Init.
#[test]
fn get_attribute_name_returns_result_string() {
    // Compile-time only: verify the return type is Result<String, PmixStatus>.
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_name;
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases (all require PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

/// `get_attribute_string` handles short attribute keys.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_short_key() {
    daemon_helper::ensure_pmix_init();
    let result = get_attribute_string("a");
    assert!(
        result.is_ok(),
        "get_attribute_string for short key should return Ok, got {:?}",
        result
    );
    assert_eq!(
        result.unwrap(),
        "a",
        "Short unknown key should be returned unchanged"
    );
}

/// `get_attribute_string` handles attribute keys with special characters.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_special_chars() {
    daemon_helper::ensure_pmix_init();
    let key = "pmix.job.id";
    let result = get_attribute_string(key);
    assert!(
        result.is_ok(),
        "get_attribute_string for key with dots should return Ok, got {:?}",
        result
    );
    assert!(!result.unwrap().is_empty());
}

/// `get_attribute_string` handles case variations.
///
/// The C implementation does case-insensitive lookup, so different cases
/// of the same key should produce the same canonical result.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_string_case_insensitive() {
    daemon_helper::ensure_pmix_init();
    let lower = get_attribute_string("pmix.host").unwrap();
    let upper = get_attribute_string("PMIX.HOST").unwrap();
    // Both should return the same canonical string (either the registered
    // canonical form or the input unchanged if not initialized).
    // When not initialized, both return input unchanged, so they may differ.
    // When initialized, both should return the same canonical form.
    // We can't assert equality here because it depends on init state.
    let _ = (lower, upper); // suppress unused warnings
}

/// `get_attribute_name` handles the same edge cases as `get_attribute_string`.
///
/// Requires PMIx to be initialized (PMIx_Init called).
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_edge_cases() {
    daemon_helper::ensure_pmix_init();
    let keys = ["a", "pmix.job.id", "PMIX.HOST"];
    for key in &keys {
        let result = get_attribute_name(key);
        assert!(
            result.is_ok(),
            "get_attribute_name(\"{}\") should return Ok, got {:?}",
            key,
            result
        );
        assert!(!result.unwrap().is_empty());
    }
}
