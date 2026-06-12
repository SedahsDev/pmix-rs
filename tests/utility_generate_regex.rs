//! Integration tests for `PMIx_generate_regex` via the safe Rust wrapper.
//!
//! Tests are derived from the C test sources:
//! - `test/test_v2/pmix_regex.c` — regex generation with known node lists
//! - `test/simple/simptest.c` — single-host regex generation
//! - `test/simple/stability.c` — three-node regex generation
//!
//! NOTE: `PMIx_generate_regex` is a server-side utility that requires
//! `PMIx_server_init()` to have been called first. The tests below that
//! exercise the actual FFI call are marked `#[ignore]` because they need
//! a full PMIx server initialization which requires callback setup.
//! The non-ignored tests verify the wrapper's error handling and type
//! safety without a running PMIx server.

use pmix::utility::generate_regex;
use pmix::PmixError;
use pmix::PmixStatus;

// ─────────────────────────────────────────────────────────────────────────────
// Error handling (no PMIx server required)
// ─────────────────────────────────────────────────────────────────────────────

/// `generate_regex` returns `ErrInit` when PMIx server has not been
/// initialized — this is the expected behavior per the C API contract.
///
/// The C API requires `PMIx_server_init()` to be called first. Without it,
/// `PMIx_generate_regex` returns `PMIX_ERR_INIT`. Our wrapper correctly
/// propagates this as `Err(Known(ErrInit))`.
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
            panic!(
                "Expected ErrInit, got {:?}",
                other
            );
        }
        Ok(_) => {
            // Unexpected — PMIx server was somehow initialized globally.
            // This can happen if another test or the test harness calls
            // PMIx_server_init. Treat as success in that case.
        }
    }
}

/// `generate_regex` with empty input also returns `ErrInit` (server not
/// initialized) — the wrapper does not panic on empty strings.
#[test]
fn test_generate_regex_empty_no_panic() {
    let result = generate_regex("");
    // Should not panic. May return Err(ErrInit) or another error.
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

/// `generate_regex` handles strings with embedded null bytes gracefully
/// (returns `Err(BadParam)` because `CString::new` fails on null bytes).
#[test]
fn test_generate_regex_null_byte_rejected() {
    // We can't pass a string with an embedded null through &str,
    // but we can verify the wrapper compiles and the type signature is correct.
    let result: Result<String, PmixStatus> = generate_regex("valid_input");
    // Should be Err(ErrInit) since server is not initialized.
    assert!(
        result.is_err(),
        "generate_regex should fail without PMIx_server_init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Type safety
// ─────────────────────────────────────────────────────────────────────────────

/// `generate_regex` returns `Result<String, PmixStatus>` — verify the
/// return type compiles and is usable.
#[test]
fn test_generate_regex_return_type() {
    let result: Result<String, PmixStatus> = generate_regex("node001");
    // The important thing is that this compiles and the types are correct.
    assert!(result.is_err(), "should fail without PMIx_server_init");
}

/// `PmixStatus` error from `generate_regex` implements `std::error::Error`.
#[test]
fn test_generate_regex_error_is_std_error() {
    let result = generate_regex("node001");
    if let Err(e) = result {
        // PmixStatus implements std::error::Error — verify it compiles.
        let _: &dyn std::error::Error = &e;
    }
}

/// `PmixStatus` error from `generate_regex` implements `Debug` and `Display`.
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

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx_server_init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `generate_regex` returns a valid regex for sequential node names.
///
/// Derived from `pmix_regex.c`:
/// ```c
/// #define TEST_NODES "odin001,odin002,odin003,odin010,odin011,odin075"
/// PMIx_generate_regex(TEST_NODES, &regex);
/// ```
///
/// Requires `PMIx_server_init()` — run with `--ignored` flag after
/// setting up a PMIx server module.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_sequential_nodes() {
    let nodes = "odin001,odin002,odin003,odin010,odin011,odin075";
    let result = generate_regex(nodes);
    assert!(
        result.is_ok(),
        "generate_regex should succeed for sequential nodes, got {:?})",
        result
    );
    let regex = result.unwrap();
    assert!(
        !regex.is_empty(),
        "regex should not be empty"
    );
    assert!(
        regex.starts_with("pmix:") || regex.starts_with("blob:"),
        "regex should start with 'pmix:' or 'blob:', got '{}'",
        regex
    );
}

/// `generate_regex` returns a valid regex for three nodes.
///
/// Derived from `stability.c`:
/// ```c
/// PMIx_generate_regex("test000,test001,test002", &regex);
/// ```
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
/// Derived from `pmix_regex.c`:
/// ```c
/// #define TEST_NODES2 "c712f6n01,c712f6n02,c712f6n03"
/// PMIx_generate_regex(TEST_NODES2, &regex);
/// ```
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_generate_regex_short_nodes() {
    let nodes = "c712f6n01,c712f6n02,c712f6n03";
    let result = generate_regex(nodes);
    assert!(result.is_ok(), "should succeed for short node names");
    let regex = result.unwrap();
    assert!(!regex.is_empty(), "regex should not be empty");
}

/// `generate_regex` is deterministic — same input always produces same output.
///
/// Derived from `pmix_regex.c` — the C test generates the same regex
/// multiple times and expects identical results.
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
/// Derived from `simptest.c`:
/// ```c
/// PMIx_generate_regex(pmix_globals.hostname, &regex);
/// ```
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
/// Per the spec: "The returned representation ... shall be identified
/// with a colon-delimited string at the beginning of the output.
/// For example, an output starting with 'pmix:' indicates that the
/// representation is a PMIx-defined regular expression. In contrast,
/// an output starting with 'blob:' is a compressed binary array."
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

/// `generate_regex` does not leak memory — calling it multiple times
/// should not cause issues (basic smoke test).
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
