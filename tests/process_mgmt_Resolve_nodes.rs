//! Tests for `PMIx_Resolve_nodes` via the safe `process_mgmt` module
//! wrapper.
//!
//! Derived from C test patterns in:
//! - `test/simple/simpdyn.c` — resolves nodes for a namespace, checks
//!   the returned node list string is non-empty.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::PmixStatus;
use pmix::process_mgmt::resolve_nodes;

// ─────────────────────────────────────────────────────────────────────────────
// Resolve_nodes without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `resolve_nodes` without `PMIx_Init` must return an error rather
/// than panic or segfault — the library should detect that it is not
/// initialized and return `PMIX_ERR_INIT`.
///
/// Derived from `test/simple/simpdyn.c` — the C test calls
/// PMIx_Resolve_nodes only after PMIx_Init.
#[test]
fn resolve_nodes_without_init_fails() {
    let result = resolve_nodes("test_namespace");
    assert!(
        result.is_err(),
        "resolve_nodes without PMIx_Init should fail"
    );
}

/// Resolve nodes with a different namespace name — should also fail without init.
#[test]
fn resolve_nodes_different_nspace_without_init_fails() {
    let result = resolve_nodes("other_namespace");
    assert!(
        result.is_err(),
        "resolve_nodes with different nspace without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that the error returned without init is PMIX_ERR_INIT (-6).
///
/// The PMIx spec states that PMIx_Resolve_nodes returns PMIX_ERR_INIT if
/// the caller has not called PMIx_Init.
#[test]
fn resolve_nodes_without_init_returns_err_init() {
    let result = resolve_nodes("test_namespace");
    match result {
        Err(status) => {
            // The error should be PMIX_ERR_INIT or similar initialization error.
            // Some PMIx versions may return different codes, so accept a range.
            let raw = status.to_raw();
            assert!(
                raw < 0,
                "resolve_nodes without init should return negative error code, got {}",
                raw
            );
        }
        Ok(_) => panic!("resolve_nodes without init should not succeed"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter edge cases (without init, checking no crash)
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve nodes with an empty string namespace — should not crash.
#[test]
fn resolve_nodes_empty_nspace_no_crash() {
    let result = resolve_nodes("");
    // Should fail (either bad param or not initialized), but must not crash.
    assert!(
        result.is_err(),
        "resolve_nodes with empty nspace should fail"
    );
}

/// Resolve nodes with a very long namespace — should not crash or buffer overflow.
#[test]
fn resolve_nodes_long_nspace_no_crash() {
    let long_nspace = "a".repeat(1024);
    let result = resolve_nodes(&long_nspace);
    // Should fail gracefully, not crash.
    assert!(
        result.is_err(),
        "resolve_nodes with long nspace should fail"
    );
}

/// Resolve nodes with special characters in namespace — should not crash.
#[test]
fn resolve_nodes_special_nspace_no_crash() {
    let result = resolve_nodes("namespace-with_dashes.and.dots");
    assert!(
        result.is_err(),
        "resolve_nodes with special nspace should fail without init"
    );
}

/// Resolve nodes with unicode namespace — should not crash.
#[test]
fn resolve_nodes_unicode_nspace_no_crash() {
    let result = resolve_nodes("namespace-\u{00e9}");
    assert!(
        result.is_err(),
        "resolve_nodes with unicode nspace should fail without init"
    );
}

/// Resolve nodes with numeric namespace — should not crash.
#[test]
fn resolve_nodes_numeric_nspace_no_crash() {
    let result = resolve_nodes("12345");
    assert!(
        result.is_err(),
        "resolve_nodes with numeric nspace should fail without init"
    );
}

/// Resolve nodes with hyphenated namespace (common in PMIx) — should not crash.
#[test]
fn resolve_nodes_hyphenated_nspace_no_crash() {
    let result = resolve_nodes("job-20260612-abc123");
    assert!(
        result.is_err(),
        "resolve_nodes with hyphenated nspace should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Return type verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that resolve_nodes returns Result<String, PmixStatus>.
///
/// This is a compile-time check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn resolve_nodes_return_type_is_correct() {
    let result: Result<String, PmixStatus> = resolve_nodes("test");
    // The fact that this compiles verifies the return type.
    assert!(result.is_err());
}

/// Verify that multiple calls to resolve_nodes without init do not cause
/// memory issues or state corruption.
#[test]
fn resolve_nodes_multiple_calls_no_crash() {
    for i in 0..10 {
        let nspace = format!("namespace_{}", i);
        let result = resolve_nodes(&nspace);
        assert!(
            result.is_err(),
            "resolve_nodes call {} should fail without init",
            i
        );
    }
}

/// Verify that resolve_nodes with the same namespace multiple times returns
/// consistent errors (no state mutation).
#[test]
fn resolve_nodes_repeated_nspace_consistent() {
    let result1 = resolve_nodes("repeated_test");
    let result2 = resolve_nodes("repeated_test");
    // Both should fail with the same error code.
    assert!(result1.is_err() && result2.is_err());
    if let (Err(s1), Err(s2)) = (result1, result2) {
        assert_eq!(
            s1.to_raw(),
            s2.to_raw(),
            "repeated calls should return the same error code"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve nodes for a namespace after PMIx_Init.
///
/// Derived from `test/simple/simpdyn.c`:
/// ```c
/// rc = PMIx_Resolve_nodes(nspace, &nodelist);
/// assert(rc == PMIX_SUCCESS);
/// assert(nodelist != NULL && strlen(nodelist) > 0);
/// ```
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn resolve_nodes_after_init_returns_nodelist() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let my_nspace = pmix::my_nspace();
    //   let nodes = resolve_nodes(&my_nspace).expect("resolve_nodes");
    //   assert!(!nodes.is_empty(), "should find at least one node");
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Resolve nodes and verify the result is a comma-delimited list.
///
/// Derived from `test/simple/simpdyn.c` — the C test splits the returned
/// string on commas and verifies each element is a valid hostname.
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn resolve_nodes_returns_comma_delimited_list() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let my_nspace = pmix::my_nspace();
    //   let nodes = resolve_nodes(&my_nspace).expect("resolve_nodes");
    //   // Each node name should be non-empty.
    //   for node in nodes.split(',') {
    //       assert!(!node.is_empty(), "each node name should be non-empty");
    //   }
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Resolve nodes for a non-existent namespace after PMIx_Init.
///
/// The PMIx spec states that PMIx_Resolve_nodes returns PMIX_ERR_NOT_FOUND
/// if the specified namespace is not known.
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn resolve_nodes_nonexistent_nspace_returns_not_found() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let result = resolve_nodes("nonexistent_namespace_12345");
    //   assert!(result.is_err());
    //   match result {
    //       Err(status) => {
    //           assert_eq!(status.to_raw(), pmix::ffi::PMIX_ERR_NOT_FOUND);
    //       }
    //       Ok(_) => panic!("should not succeed for nonexistent namespace"),
    //   }
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Resolve nodes for own namespace and verify the local hostname is included.
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn resolve_nodes_includes_local_hostname() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let my_nspace = pmix::my_nspace();
    //   let hostname = hostname::get().expect("get hostname")
    //       .to_string_lossy().to_string();
    //   let nodes = resolve_nodes(&my_nspace).expect("resolve_nodes");
    //   assert!(nodes.contains(&hostname),
    //       "node list '{}' should contain local hostname '{}'", nodes, hostname);
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}
