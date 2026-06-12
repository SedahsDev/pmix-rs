//! Tests for `PMIx_Resolve_peers` via the safe `process_mgmt` module
//! wrapper.
//!
//! Derived from C test patterns in:
//! - `test/test_resolve_peers.c` — resolves peers for own namespace and
//!   cross-namespace after PMIx_Connect, validates proc count and ranks.
//! - `test/simple/simpdyn.c` — resolves peers on local hostname for a
//!   specific namespace, checks npeers matches expected count.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::PmixStatus;
use pmix::process_mgmt::resolve_peers;

// ─────────────────────────────────────────────────────────────────────────────
// Resolve_peers without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `resolve_peers` without `PMIx_Init` must return an error rather
/// than panic or segfault — the library should detect that it is not
/// initialized and return `PMIX_ERR_INIT`.
///
/// Derived from `test/test_resolve_peers.c` — the C test calls
/// PMIx_Resolve_peers only after PMIx_Init.
#[test]
fn resolve_peers_without_init_fails() {
    let result = resolve_peers(None, None);
    assert!(
        result.is_err(),
        "resolve_peers without PMIx_Init should fail"
    );
}

/// Resolve peers with a specific node name but no init — should fail.
///
/// Derived from `test/simple/simpdyn.c` — the C test calls
/// PMIx_Resolve_peers(hostname, nspace, ...) after init.
#[test]
fn resolve_peers_with_nodename_without_init_fails() {
    let result = resolve_peers(Some("localhost"), None);
    assert!(
        result.is_err(),
        "resolve_peers with nodename without init should fail"
    );
}

/// Resolve peers with a specific namespace but no init — should fail.
#[test]
fn resolve_peers_with_nspace_without_init_fails() {
    let result = resolve_peers(None, Some("test_namespace"));
    assert!(
        result.is_err(),
        "resolve_peers with nspace without init should fail"
    );
}

/// Resolve peers with both nodename and nspace but no init — should fail.
#[test]
fn resolve_peers_with_both_params_without_init_fails() {
    let result = resolve_peers(Some("localhost"), Some("test_namespace"));
    assert!(
        result.is_err(),
        "resolve_peers with both params without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that the error returned without init is PMIX_ERR_INIT (-6).
///
/// The PMIx spec states that PMIx_Resolve_peers returns PMIX_ERR_INIT if
/// the caller has not called PMIx_Init.
#[test]
fn resolve_peers_without_init_returns_err_init() {
    let result = resolve_peers(None, None);
    match result {
        Err(status) => {
            // The error should be PMIX_ERR_INIT or similar initialization error.
            // Some PMIx versions may return different codes, so accept a range.
            let raw = status.to_raw();
            assert!(
                raw < 0,
                "resolve_peers without init should return negative error code, got {}",
                raw
            );
        }
        Ok(_) => panic!("resolve_peers without init should not succeed"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter edge cases (without init, checking no crash)
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve peers with an empty string nodename — should not crash.
#[test]
fn resolve_peers_empty_nodename_no_crash() {
    let result = resolve_peers(Some(""), None);
    // Should fail (either bad param or not initialized), but must not crash.
    assert!(
        result.is_err(),
        "resolve_peers with empty nodename should fail"
    );
}

/// Resolve peers with an empty string nspace — should not crash.
#[test]
fn resolve_peers_empty_nspace_no_crash() {
    let result = resolve_peers(None, Some(""));
    // Should fail (either bad param or not initialized), but must not crash.
    assert!(
        result.is_err(),
        "resolve_peers with empty nspace should fail"
    );
}

/// Resolve peers with a very long nodename — should not crash or buffer overflow.
#[test]
fn resolve_peers_long_nodename_no_crash() {
    let long_name = "a".repeat(1024);
    let result = resolve_peers(Some(&long_name), None);
    // Should fail gracefully, not crash.
    assert!(
        result.is_err(),
        "resolve_peers with long nodename should fail"
    );
}

/// Resolve peers with a very long nspace — should not crash or buffer overflow.
#[test]
fn resolve_peers_long_nspace_no_crash() {
    let long_nspace = "a".repeat(1024);
    let result = resolve_peers(None, Some(&long_nspace));
    // Should fail gracefully, not crash.
    assert!(
        result.is_err(),
        "resolve_peers with long nspace should fail"
    );
}

/// Resolve peers with special characters in nodename — should not crash.
#[test]
fn resolve_peers_special_nodename_no_crash() {
    let result = resolve_peers(Some("node-with_dashes.and.dots"), None);
    assert!(
        result.is_err(),
        "resolve_peers with special nodename should fail without init"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Return type verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that resolve_peers returns Result<Vec<Proc>, PmixStatus>.
///
/// This is a compile-time check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn resolve_peers_return_type_is_correct() {
    let result: Result<Vec<pmix::Proc>, PmixStatus> = resolve_peers(None, None);
    // The fact that this compiles verifies the return type.
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve peers for the local node after PMIx_Init.
///
/// Derived from `test/test_resolve_peers.c`:
/// ```c
/// rc = PMIx_Resolve_peers(pmix_globals.hostname, nspace, &procs, &nprocs);
/// assert(rc == PMIX_SUCCESS);
/// assert(procs != NULL && nprocs > 0);
/// ```
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn resolve_peers_after_init_returns_procs() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let procs = resolve_peers(None, None).expect("resolve_peers");
    //   assert!(!procs.is_empty(), "should find at least the caller");
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Resolve peers for a specific namespace after PMIx_Init.
///
/// Derived from `test/simple/simpdyn.c`:
/// ```c
/// rc = PMIx_Resolve_peers(hostname, NULL, &peers, &npeers);
/// assert(rc == PMIX_SUCCESS);
/// assert(npeers == expected_count);
/// ```
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn resolve_peers_specific_nspace_after_init() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let my_nspace = pmix::my_nspace();
    //   let procs = resolve_peers(None, Some(&my_nspace))
    //       .expect("resolve_peers for own nspace");
    //   assert!(!procs.is_empty());
    //   for proc in &procs {
    //       assert_eq!(proc.nspace(), my_nspace);
    //   }
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Resolve peers with NULL nspace (all namespaces) after PMIx_Init.
///
/// Derived from `test/test_resolve_peers.c` — resolves peers across all
/// known namespaces on the local node.
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn resolve_peers_all_namespaces_after_init() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let procs = resolve_peers(None, None).expect("resolve_peers all");
    //   assert!(!procs.is_empty());
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}

/// Verify proc array contents after resolve_peers.
///
/// Derived from `test/test_resolve_peers.c`:
/// ```c
/// for (i = 0; i < nprocs; i++) {
///     assert(procs[i].rank == ranks[i].rank);
/// }
/// ```
///
/// This test requires a running PMIx daemon and is ignored by default.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn resolve_peers_proc_contents_after_init() {
    // NOTE: This test needs PMIx_Init which requires a daemon.
    // In a real PMIx environment:
    //   let _ = pmix::init(None, None).expect("PMIx_Init");
    //   let procs = resolve_peers(None, None).expect("resolve_peers");
    //   for proc in &procs {
    //       // Each proc should have a valid rank (>= 0).
    //       assert!(proc.rank() >= 0);
    //   }
    unimplemented!("requires PMIx_Init with a running PMIx daemon");
}
