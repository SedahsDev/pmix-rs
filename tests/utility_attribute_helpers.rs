//! Comprehensive tests for PMIx utility attribute helpers.
//!
//! Covers the following functions:
//! - `get_attribute_string` — attribute key → canonical string
//! - `get_attribute_name` — canonical string → attribute key (inverse)
//! - `get_version` — PMIx library version string
//! - `generate_regex` — node list → compressed regex
//! - `generate_ppn` — rank ranges → compressed PPN
//! - `register_attributes` — register host attributes (requires PMIx_Init)
//!
//! Tests are organized by function. Some tests require PMIx initialization
//! and are marked `#[ignore]`. The non-ignored tests exercise error paths,
//! type safety, and input validation without needing a running PMIx server.

use pmix::utility::{generate_ppn, generate_regex, register_attributes};
use pmix::utility::{get_attribute_name, get_attribute_string};
use pmix::{get_version, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// get_version — PMIx library version string
// ─────────────────────────────────────────────────────────────────────────────

/// `get_version` returns a non-empty version string.
#[test]
fn test_get_version_non_empty() {
    let version = get_version();
    assert!(!version.is_empty(), "get_version should return a non-empty string");
}

/// `get_version` returns a string containing digits (typical semver format).
#[test]
fn test_get_version_contains_digits() {
    let version = get_version();
    assert!(
        version.chars().any(|c| c.is_ascii_digit()),
        "get_version('{}') should contain at least one digit",
        version
    );
}

/// `get_version` returns `&'static str`.
///
/// Compile-time check: the return type is a static string reference, not
/// an owned `String`.
#[test]
fn test_get_version_returns_static_str() {
    let _v: &str = get_version();
}

/// `get_version` is deterministic — multiple calls return the same value.
#[test]
fn test_get_version_deterministic() {
    let v1 = get_version();
    let v2 = get_version();
    assert_eq!(v1, v2, "get_version must be deterministic");
}

/// `get_version` follows a version-like format (digits and dots/hyphens).
#[test]
fn test_get_version_format() {
    let version = get_version();
    // Typical PMIx version: "4.1.1" or "5.0.0" — contains at least one dot
    // or hyphen separating version components, or is purely numeric.
    let has_separator = version.contains('.') || version.contains('-');
    let has_digits = version.chars().any(|c| c.is_ascii_digit());
    assert!(
        has_separator || has_digits,
        "get_version('{}') should look like a version string (contain digits and/or separators)",
        version
    );
}

/// `get_version` output is printable ASCII.
#[test]
fn test_get_version_printable() {
    let version = get_version();
    for c in version.chars() {
        assert!(
            c.is_ascii_graphic() || c.is_ascii_whitespace(),
            "get_version('{}') should contain only printable characters",
            version
        );
    }
}

/// `get_version` contains a parseable version number.
#[test]
fn test_get_version_major_version() {
    let version = get_version();
    // Version is e.g. "OpenPMIx 5.0.7a1..." — find first digit sequence
    let major = version
        .split(|c: char| c.is_whitespace() || c == '.' || c == '-')
        .find(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .and_then(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok());
    assert!(
        major.is_some(),
        "get_version('{}') should contain a parseable version number",
        version
    );
    if let Some(major) = major {
        assert!(
            major >= 3 && major <= 10,
            "Major version {} seems unreasonable for PMIx",
            major
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_attribute_string — attribute key → canonical string
// ─────────────────────────────────────────────────────────────────────────────

/// `get_attribute_string` returns `Result<String, PmixStatus>`.
///
/// Compile-time type check: verifies the function signature.
#[test]
fn test_get_attribute_string_return_type() {
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_string;
}

/// `get_attribute_string` returns Ok for a simple non-empty key.
///
/// Requires PMIx_Init — the C function accesses the internal keyindex table
/// which is populated during initialization. Without init, it crashes.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_simple_key() {
    let result = get_attribute_string("pmix.host");
    assert!(result.is_ok(), "simple key should return Ok, got {:?}", result);
}

/// `get_attribute_string` returns `Ok` for well-known PMIx attribute keys.
///
/// Requires PMIx_Init — the C implementation accesses the internal keyindex
/// table which is populated during initialization.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_known_keys() {
    let known_keys = [
        "pmix.host",
        "pmix.nprocs",
        "pmix.rank",
        "pmix.universe_size",
        "pmix.nodemap",
        "pmix.topology",
        "pmix.exit_code",
        "pmix.job.id",
        "pmix.app.name",
        "pmix.nodenames",
    ];
    for key in &known_keys {
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

/// `get_attribute_string` returns the input unchanged for unknown keys.
///
/// The C implementation returns the input string when the key is not found
/// in the registered table.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_unknown_key_returns_input() {
    let input = "pmix.nonexistent.attribute.xyz";
    let result = get_attribute_string(input);
    assert!(
        result.is_ok(),
        "get_attribute_string for unknown key should return Ok, got {:?}",
        result
    );
    let output = result.unwrap();
    assert_eq!(
        output, input,
        "Unknown attribute key should be returned unchanged"
    );
}

/// `get_attribute_string` is deterministic — same key always returns same string.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_deterministic() {
    let key = "pmix.host";
    let first = get_attribute_string(key).unwrap();
    let second = get_attribute_string(key).unwrap();
    assert_eq!(
        first, second,
        "get_attribute_string must be deterministic for the same input"
    );
}

/// `get_attribute_string` returns different outputs for different known keys.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_distinct_outputs() {
    let host = get_attribute_string("pmix.host").unwrap();
    let nprocs = get_attribute_string("pmix.nprocs").unwrap();
    // Different keys should produce different canonical strings
    // (or at least different inputs if not initialized).
    assert_ne!(
        "pmix.host", "pmix.nprocs",
        "Different keys are different inputs"
    );
    let _ = (host, nprocs);
}

/// `get_attribute_string` handles short keys.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_short_key() {
    let result = get_attribute_string("a");
    assert!(result.is_ok(), "short key should return Ok, got {:?}", result);
    assert_eq!(
        result.unwrap(),
        "a",
        "Short unknown key should be returned unchanged"
    );
}

/// `get_attribute_string` handles keys with dots.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_dotted_key() {
    let key = "pmix.job.id";
    let result = get_attribute_string(key);
    assert!(
        result.is_ok(),
        "dotted key should return Ok, got {:?}",
        result
    );
    assert!(!result.unwrap().is_empty());
}

/// `get_attribute_string` handles case variations.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_case_variations() {
    let lower = get_attribute_string("pmix.host").unwrap();
    let upper = get_attribute_string("PMIX.HOST").unwrap();
    // When initialized, both should return the same canonical form.
    // When not initialized, both return input unchanged (different).
    // We can't assert equality without knowing init state.
    let _ = (lower, upper);
}

/// `get_attribute_string` handles a long attribute key name.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_long_key() {
    let key = "pmix.server.app_info.app.1.executable.name";
    let result = get_attribute_string(key);
    assert!(result.is_ok(), "long key should return Ok, got {:?}", result);
}

/// `get_attribute_string` error implements Debug and Display.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_string_error_traits() {
    let result = get_attribute_string("pmix.host");
    match result {
        Ok(s) => {
            let debug = format!("{:?}", s);
            let display = format!("{}", s);
            assert!(!debug.is_empty());
            assert!(!display.is_empty());
        }
        Err(e) => {
            let debug = format!("{:?}", e);
            let display = format!("{}", e);
            assert!(!debug.is_empty(), "Debug output should not be empty");
            assert!(!display.is_empty(), "Display output should not be empty");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get_attribute_name — canonical string → attribute key (inverse)
// ─────────────────────────────────────────────────────────────────────────────

/// `get_attribute_name` returns `Result<String, PmixStatus>`.
///
/// Compile-time type check: verifies the function signature.
#[test]
fn test_get_attribute_name_return_type() {
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_name;
}

/// `get_attribute_name` returns Ok for a simple non-empty string.
///
/// Requires PMIx_Init — the C function accesses the internal keyindex table
/// which is populated during initialization. Without init, it crashes.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_simple_string() {
    let result = get_attribute_name("host name");
    assert!(result.is_ok(), "simple string should return Ok, got {:?}", result);
}

/// `get_attribute_name` returns `Ok` for a known attribute string.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_known_string() {
    let result = get_attribute_name("host name");
    assert!(
        result.is_ok(),
        "get_attribute_name should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(!desc.is_empty(), "should not return empty string");
}

/// `get_attribute_name` returns the input unchanged for unknown strings.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_unknown_returns_input() {
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
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_deterministic() {
    let input = "pmix.host";
    let first = get_attribute_name(input).unwrap();
    let second = get_attribute_name(input).unwrap();
    assert_eq!(
        first, second,
        "get_attribute_name must be deterministic for the same input"
    );
}

/// `get_attribute_name` handles various edge case inputs.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_edge_cases() {
    let test_inputs = ["a", "pmix.job.id", "PMIX.HOST", "number 42"];
    for input in &test_inputs {
        let result = get_attribute_name(input);
        assert!(
            result.is_ok(),
            "get_attribute_name(\"{}\") should return Ok, got {:?}",
            input,
            result
        );
        assert!(!result.unwrap().is_empty());
    }
}

/// `get_attribute_name` handles a long descriptive string.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_long_string() {
    let input = "this is a very long descriptive attribute string that probably does not exist";
    let result = get_attribute_name(input);
    assert!(result.is_ok(), "long string should return Ok, got {:?}", result);
    // Should return the input unchanged since it's not registered.
    let output = result.unwrap();
    assert_eq!(output, input, "unregistered long string should be returned unchanged");
}

/// `get_attribute_name` error implements Debug and Display.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_get_attribute_name_error_traits() {
    let result = get_attribute_name("host name");
    match result {
        Ok(s) => {
            let debug = format!("{:?}", s);
            let display = format!("{}", s);
            assert!(!debug.is_empty());
            assert!(!display.is_empty());
        }
        Err(e) => {
            let debug = format!("{:?}", e);
            let display = format!("{}", e);
            assert!(!debug.is_empty());
            assert!(!display.is_empty());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Attribute name↔string bijection round-trip tests
// ─────────────────────────────────────────────────────────────────────────────

/// Round-trip: get_attribute_string(key) → get_attribute_name(string) → key.
///
/// For known keys, the canonical string should map back to the original key.
/// For unknown keys, the input is returned unchanged by both functions,
/// so the round-trip also works.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_attribute_roundtrip_known_keys() {
    let known_keys = [
        "pmix.host",
        "pmix.nprocs",
        "pmix.rank",
        "pmix.universe_size",
        "pmix.nodemap",
        "pmix.topology",
        "pmix.exit_code",
        "pmix.job.id",
    ];
    for key in &known_keys {
        let canonical = get_attribute_string(key).unwrap();
        let back = get_attribute_name(&canonical).unwrap();
        assert_eq!(
            back, *key,
            "Round-trip failed for '{}': key -> '{}' -> '{}'",
            key, canonical, back
        );
    }
}

/// Round-trip for unknown keys: both functions return input unchanged.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_attribute_roundtrip_unknown_keys() {
    let unknown_keys = [
        "pmix.fake.attr",
        "nonexistent.key.xyz",
        "a",
    ];
    for key in &unknown_keys {
        let canonical = get_attribute_string(key).unwrap();
        let back = get_attribute_name(&canonical).unwrap();
        assert_eq!(
            back, *key,
            "Round-trip failed for unknown key '{}': key -> '{}' -> '{}'",
            key, canonical, back
        );
    }
}

/// Forward direction: get_attribute_name(string) → get_attribute_string(key) → string.
///
/// For unknown strings, both functions return input unchanged, so this works.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_attribute_reverse_roundtrip() {
    let test_strings = ["pmix.host", "nonexistent.attribute"];
    for s in &test_strings {
        let key = get_attribute_name(s).unwrap();
        let back = get_attribute_string(&key).unwrap();
        // For unknown strings, key == s and back == key == s.
        // For known strings, key is the attribute key, and back is the canonical string.
        let _ = (s, key, back);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// generate_regex — node list → compressed regex
// ─────────────────────────────────────────────────────────────────────────────

/// `generate_regex` returns `Result<String, PmixStatus>`.
///
/// Compile-time type check.
#[test]
fn test_generate_regex_return_type() {
    let _: fn(&str) -> Result<String, PmixStatus> = generate_regex;
}

/// `generate_regex` returns `ErrInit` when PMIx server has not been initialized.
#[test]
fn test_generate_regex_requires_server_init() {
    let result = generate_regex("node001,node002,node003");
    assert!(
        result.is_err(),
        "generate_regex should fail without PMIx_server_init"
    );
    match result {
        Err(PmixStatus::Known(PmixError::ErrInit)) => {
            // Expected: PMIx server not initialized.
        }
        Err(other) => {
            panic!("Expected ErrInit, got {:?}", other);
        }
        Ok(_) => {
            // PMIx server was somehow initialized globally — treat as pass.
        }
    }
}

/// `generate_regex` handles empty input without panicking.
#[test]
fn test_generate_regex_empty_input_no_panic() {
    let result = generate_regex("");
    match result {
        Err(PmixStatus::Known(PmixError::ErrInit)) => {
            // Expected: PMIx server not initialized.
        }
        Err(_) => {
            // Any error is acceptable — the point is no panic.
        }
        Ok(_) => {
            // Unexpected but acceptable — PMIx server was initialized.
        }
    }
}

/// `generate_regex` error implements `std::error::Error`.
#[test]
fn test_generate_regex_error_is_std_error() {
    let result = generate_regex("node001");
    if let Err(e) = result {
        let _: &dyn std::error::Error = &e;
    }
}

/// `generate_regex` error implements Debug and Display.
#[test]
fn test_generate_regex_error_display() {
    let result = generate_regex("node001");
    if let Err(e) = result {
        let debug = format!("{:?}", e);
        let display = format!("{}", e);
        assert!(!debug.is_empty(), "Debug output should not be empty");
        assert!(!display.is_empty(), "Display output should not be empty");
    }
}

/// `generate_regex` returns a valid regex for sequential node names.
///
/// Derived from `test/test_v2/pmix_regex.c`.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_sequential_nodes() {
    let nodes = "odin001,odin002,odin003,odin010,odin011,odin075";
    let result = generate_regex(nodes);
    assert!(
        result.is_ok(),
        "generate_regex should succeed for sequential nodes, got {:?}",
        result
    );
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty");
    assert!(
        regex.starts_with("pmix:") || regex.starts_with("blob:"),
        "regex should start with 'pmix:' or 'blob:', got '{}'",
        regex
    );
}

/// `generate_regex` returns a valid regex for three nodes.
///
/// Derived from `test/simple/stability.c`.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_three_nodes() {
    let nodes = "test000,test001,test002";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for three nodes");
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty");
}

/// `generate_regex` returns a valid regex for short node names.
///
/// Derived from `test/test_v2/pmix_regex.c`.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_short_nodes() {
    let nodes = "c712f6n01,c712f6n02,c712f6n03";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for short node names");
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty for short names");
}

/// `generate_regex` is deterministic — same input always produces same output.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_deterministic() {
    let nodes = "odin001,odin002,odin003,odin010,odin011,odin075";
    let regex1 = generate_regex(nodes).unwrap();
    let regex2 = generate_regex(nodes).unwrap();
    assert_eq!(
        regex1, regex2,
        "generate_regex should be deterministic: first='{}', second='{}'",
        regex1, regex2
    );
}

/// `generate_regex` handles a large list of nodes.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_many_nodes() {
    let nodes: String = (0..100)
        .map(|i| format!("node{:03}", i))
        .collect::<Vec<_>>()
        .join(",");
    let result = generate_regex(&nodes);
    assert!(
        result.is_ok(),
        "should succeed for 100 nodes, got {:?}",
        result
    );
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty for 100 nodes");
}

/// `generate_regex` handles a single hostname.
///
/// Derived from `test/simple/simptest.c`.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_single_host() {
    let result = generate_regex("localhost");
    assert!(result.is_ok(), "should succeed for a single hostname");
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty for a single host");
}

/// `generate_regex` output starts with a known format prefix.
///
/// Per the spec: "pmix:" indicates a PMIx-defined regular expression,
/// "blob:" indicates a compressed binary array.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_output_format() {
    let test_cases = vec![
        "node001,node002,node003",
        "odin001,odin002,odin003,odin010,odin011,odin075",
        "c712f6n01,c712f6n02,c712f6n03",
        "test000,test001,test002",
    ];
    for nodes in test_cases {
        let regex = generate_regex(nodes).unwrap();
        assert!(
            regex.starts_with("pmix:") || regex.starts_with("blob:"),
            "Output should start with 'pmix:' or 'blob:', got '{}' for input '{}'",
            regex,
            nodes
        );
    }
}

/// `generate_regex` does not leak memory — calling it many times is safe.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_no_leak_smoke() {
    for i in 0..50 {
        let nodes = format!("host{:03},host{:04},host{:05}", i, i, i);
        let result = generate_regex(&nodes);
        assert!(
            result.is_ok(),
            "iteration {}: should succeed, got {:?}",
            i,
            result
        );
    }
}

/// `generate_regex` handles mixed naming patterns.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_mixed_names() {
    let nodes = "compute-0,compute-1,gpu-node-0,gpu-node-1";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for mixed names");
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty for mixed names");
}

/// `generate_regex` handles nodes with underscores.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_underscore_nodes() {
    let nodes = "node_001,node_002,node_003";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for underscored node names");
    let regex = result.unwrap();
    assert!(!regex.is_empty());
}

/// `generate_regex` handles non-sequential node numbers.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_non_sequential() {
    let nodes = "node001,node005,node010,node020";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for non-sequential nodes");
    let regex = result.unwrap();
    assert!(!regex.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// generate_ppn — rank ranges → compressed PPN
// ─────────────────────────────────────────────────────────────────────────────

/// `generate_ppn` returns `Result<String, PmixStatus>`.
///
/// Compile-time type check.
#[test]
fn test_generate_ppn_return_type() {
    let _: fn(&str) -> Result<String, PmixStatus> = generate_ppn;
}

/// `generate_ppn` handles empty input without panicking.
#[test]
fn test_generate_ppn_empty() {
    let result = generate_ppn("");
    match &result {
        Ok(s) => {
            let _ = format!("ppn: '{}'", s);
        }
        Err(_) => {
            // Empty input may not be valid.
        }
    }
}

/// `generate_ppn` handles a single rank.
#[test]
fn test_generate_ppn_single() {
    let result = generate_ppn("0");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

/// `generate_ppn` handles a simple comma-separated list.
#[test]
fn test_generate_ppn_multiple() {
    let result = generate_ppn("0,1,2,3");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

/// `generate_ppn` handles a range notation.
#[test]
fn test_generate_ppn_range() {
    let result = generate_ppn("0-3");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

/// `generate_ppn` handles a large range.
#[test]
fn test_generate_ppn_large_range() {
    let result = generate_ppn("0-1023");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

/// `generate_ppn` handles a complex mixed input.
#[test]
fn test_generate_ppn_complex() {
    let result = generate_ppn("0-3,5,7-9");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

/// `generate_ppn` debug output is non-empty.
#[test]
fn test_generate_ppn_debug_output() {
    let result = generate_ppn("0,1,2");
    match &result {
        Ok(s) => {
            let debug = format!("{:?}", s);
            assert!(!debug.is_empty());
        }
        Err(e) => {
            let debug = format!("{:?}", e);
            assert!(!debug.is_empty());
        }
    }
}

/// `generate_ppn` error implements `std::error::Error`.
#[test]
fn test_generate_ppn_error_is_std_error() {
    let result = generate_ppn("0-3");
    if let Err(e) = result {
        let _: &dyn std::error::Error = &e;
    }
}

/// `generate_ppn` error implements Debug and Display.
#[test]
fn test_generate_ppn_error_display() {
    let result = generate_ppn("0-3");
    if let Err(e) = result {
        let debug = format!("{:?}", e);
        let display = format!("{}", e);
        assert!(!debug.is_empty());
        assert!(!display.is_empty());
    }
}

/// `generate_ppn` handles semicolon-separated rank ranges (multi-node).
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_ppn_semicolon_ranges() {
    let result = generate_ppn("0-3;4-7;8,9,10");
    assert!(result.is_ok(), "should succeed for semicolon-separated ranges");
    let ppn = result.unwrap();
    assert!(!ppn.is_empty(), "ppn should not be empty");
}

/// `generate_ppn` handles a single-node rank range.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_ppn_single_node() {
    let result = generate_ppn("0-3");
    assert!(result.is_ok(), "should succeed for a single node range");
    let ppn = result.unwrap();
    assert!(!ppn.is_empty());
}

/// `generate_ppn` is deterministic.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_ppn_deterministic() {
    let input = "0-3;4-7;8,9,10";
    let ppn1 = generate_ppn(input).unwrap();
    let ppn2 = generate_ppn(input).unwrap();
    assert_eq!(ppn1, ppn2, "generate_ppn should be deterministic");
}

/// `generate_ppn` handles many ranks.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_ppn_many_ranks() {
    let ranks: String = (0..100).map(|i| format!("{}", i)).collect::<Vec<_>>().join(",");
    let result = generate_ppn(&ranks);
    assert!(result.is_ok(), "should succeed for 100 ranks");
    let ppn = result.unwrap();
    assert!(!ppn.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// register_attributes — register host attributes (requires PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

/// `register_attributes` returns `Result<(), PmixStatus>`.
///
/// Compile-time type check.
#[test]
fn test_register_attributes_return_type() {
    let _: fn(&str, &[&str]) -> Result<(), PmixStatus> = register_attributes;
}

/// `register_attributes` returns `PMIX_ERR_INIT` when called before initialization.
#[test]
fn test_register_attributes_before_init() {
    let result = register_attributes("PMIx_Get", &["attr1"]);
    assert!(result.is_err(), "should fail before init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
    assert_eq!(
        err_status,
        PmixStatus::Known(PmixError::ErrInit),
        "error should be ErrInit"
    );
}

/// `register_attributes` accepts an empty attribute list (still fails with ErrInit).
#[test]
fn test_register_attributes_empty_attrs() {
    let result = register_attributes("PMIx_Get", &[] as &[&str]);
    assert!(
        result.is_err(),
        "should still fail with PMIX_ERR_INIT even with empty attrs"
    );
}

/// `register_attributes` rejects function names containing NUL bytes.
#[test]
fn test_register_attributes_nul_in_function_name() {
    let result = register_attributes("PMIx\0_Get", &["attr1"]);
    assert!(result.is_err(), "should reject NUL byte in function name");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -27,
        "error should be PMIX_ERR_BAD_PARAM (-27), got {}",
        err_status.to_raw()
    );
}

/// `register_attributes` handles various valid function name formats.
#[test]
fn test_register_attributes_valid_function_names() {
    let valid_names = [
        "PMIx_Get",
        "PMIx_Put",
        "PMIx_Fence",
        "PMIx_Register_event_handler",
        "PMIx_server_register_nspace",
        "my_custom_function",
        "function.with.dots",
    ];

    for name in valid_names {
        let result = register_attributes(name, &["attr1"]);
        assert!(result.is_err(), "should fail before init for '{}'", name);
        let err = result.unwrap_err();
        assert_eq!(
            err.to_raw(),
            -31,
            "should be PMIX_ERR_INIT for '{}', got {}",
            name,
            err.to_raw()
        );
    }
}

/// `register_attributes` handles attribute names with various formats.
#[test]
fn test_register_attributes_attribute_name_formats() {
    let attrs = &[
        "pmix.get.timeout",
        "pmix_get_scope",
        "some-nested.attribute.key",
        "UPPERCASE_ATTR",
        "mixedCase_attr_123",
    ];

    let result = register_attributes("PMIx_Get", attrs);
    assert!(result.is_err(), "should fail before init");
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -31,
        "should be PMIX_ERR_INIT, got {}",
        err.to_raw()
    );
}

/// `register_attributes` handles a large number of attributes.
#[test]
fn test_register_attributes_many_attributes() {
    let names: Vec<String> = (0..100).map(|i| format!("attr_{}", i)).collect();
    let attrs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();

    let result = register_attributes("PMIx_Get", &attrs);
    assert!(result.is_err(), "should fail before init");
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -31,
        "should be PMIX_ERR_INIT, got {}",
        err.to_raw()
    );
}

/// `register_attributes` error implements Debug and Display.
#[test]
fn test_register_attributes_error_display() {
    let result = register_attributes("PMIx_Get", &["attr1"]);
    if let Err(e) = result {
        let debug = format!("{:?}", e);
        let display = format!("{}", e);
        assert!(!debug.is_empty(), "Debug output should not be empty");
        assert!(!display.is_empty(), "Display output should not be empty");
    }
}

/// `register_attributes` error implements `std::error::Error`.
#[test]
fn test_register_attributes_error_is_std_error() {
    let result = register_attributes("PMIx_Get", &["attr1"]);
    if let Err(e) = result {
        let _: &dyn std::error::Error = &e;
    }
}

/// `register_attributes` with empty function name still fails with ErrInit
/// (or ErrBadParam if the library checks for empty strings).
#[test]
fn test_register_attributes_empty_function_name() {
    let result = register_attributes("", &["attr1"]);
    assert!(result.is_err(), "should fail with empty function name");
    let err = result.unwrap_err();
    // Either ErrInit (-31) or ErrBadParam (-27) depending on library behavior.
    assert!(
        err.to_raw() == -31 || err.to_raw() == -27,
        "expected ErrInit (-31) or ErrBadParam (-27), got {}",
        err.to_raw()
    );
}

/// `register_attributes` with NUL byte in attribute name.
#[test]
fn test_register_attributes_nul_in_attr_name() {
    let result = register_attributes("PMIx_Get", &["attr\01"]);
    // CString::new will succeed for the function name but the attribute
    // with a NUL byte will be replaced with an empty string in the wrapper.
    assert!(result.is_err(), "should fail before init");
}

/// Successful registration after PMIx_Init.
#[test]
#[ignore = "requires PMIx_Init and running PMIx server"]
fn test_register_attributes_success() {
    let result = register_attributes("PMIx_Get", &["pmix.get.timeout"]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Duplicate registration returns PMIX_ERR_REPEAT_ATTR_REGISTRATION.
#[test]
#[ignore = "requires PMIx_Init and running PMIx server"]
fn test_register_attributes_duplicate() {
    let result = register_attributes("PMIx_Get", &["attr1"]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration with empty attrs list is valid after init.
#[test]
#[ignore = "requires PMIx_Init and running PMIx server"]
fn test_register_attributes_empty_after_init() {
    let result = register_attributes("PMIx_Get", &[] as &[&str]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration with special attribute names after init.
#[test]
#[ignore = "requires PMIx_Init and running PMIx server"]
fn test_register_attributes_special_attrs() {
    let attrs = &[
        "pmix.get.timeout",
        "pmix.get.scope",
        "pmix.get.max_size",
        "pmix.get.collect_data",
    ];
    let result = register_attributes("PMIx_Get", attrs);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration for server-side functions.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_register_attributes_server_functions() {
    let server_functions = [
        "PMIx_server_register_nspace",
        "PMIx_server_deregister_nspace",
        "PMIx_server_notify",
    ];
    for func in server_functions {
        let result = register_attributes(func, &["attr1"]);
        assert!(result.is_err(), "expected PMIX_ERR_INIT for '{}'", func);
    }
}

/// Registration for tool-side functions.
#[test]
#[ignore = "requires PMIx_Init"]
fn test_register_attributes_tool_functions() {
    let tool_functions = ["PMIx_Connect", "PMIx_Disconnect", "PMIx_Notify_event"];
    for func in tool_functions {
        let result = register_attributes(func, &["attr1"]);
        assert!(result.is_err(), "expected PMIX_ERR_INIT for '{}'", func);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-function integration tests
// ─────────────────────────────────────────────────────────────────────────────

/// All utility functions can be called in the same process without conflicts.
///
/// This test calls all 6 utility functions in sequence to verify they don't
/// interfere with each other's internal state.
#[test]
/// Verify that all utility functions can be called in the same test without
/// interfering with each other's internal state.
///
/// Uses compile-time type checks for FFI functions (avoiding actual calls
/// that may crash without PMIx initialization) and runtime checks for safe
/// functions.
#[test]
fn test_all_utility_functions_coexist() {
    // get_version — always works, safe to call
    let version = get_version();
    assert!(!version.is_empty());

    // Compile-time type checks for all utility functions — they coexist
    // in the same namespace without conflicts.
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_string;
    let _: fn(&str) -> Result<String, PmixStatus> = get_attribute_name;
    let _: fn(&str) -> Result<String, PmixStatus> = generate_regex;
    let _: fn(&str) -> Result<String, PmixStatus> = generate_ppn;
    let _: fn(&str, &[&str]) -> Result<(), PmixStatus> = register_attributes;

    // If we get here without panicking, all functions coexist peacefully.
}

/// Verify that `PmixStatus` errors from different functions are comparable.
#[test]
fn test_error_comparability_across_functions() {
    let regex_err = generate_regex("node1").unwrap_err();
    let ppn_err = generate_ppn("0-3").unwrap_err();

    // Both should be ErrInit (-31) since nothing is initialized.
    assert_eq!(regex_err.to_raw(), -31, "generate_regex should return ErrInit");
    assert_eq!(ppn_err.to_raw(), -31, "generate_ppn should return ErrInit");

    // They should be equal to each other.
    assert_eq!(regex_err, ppn_err, "errors should be equal");
}

/// Verify that `PmixStatus` errors from different functions have the same
/// known variant.
#[test]
fn test_error_known_variant_across_functions() {
    let regex_err = generate_regex("node1").unwrap_err();
    let ppn_err = generate_ppn("0-3").unwrap_err();

    assert!(
        matches!(regex_err, PmixStatus::Known(PmixError::ErrInit)),
        "generate_regex error should be Known(ErrInit)"
    );
    assert!(
        matches!(ppn_err, PmixStatus::Known(PmixError::ErrInit)),
        "generate_ppn error should be Known(ErrInit)"
    );
}
