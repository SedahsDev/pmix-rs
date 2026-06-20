//! Tests for `pmix::monitoring::heartbeat()` — sends a heartbeat to the PMIx daemon.
//!
//! heartbeat() builds a PMIX_SEND_HEARTBEAT info entry and calls
//! PMIx_Process_monitor_nb with a NULL callback (fire-and-forget).
//!
//! These tests are designed to be run in two modes:
//!
//! 1. Standalone (cargo test --test utility_heartbeat):
//!    - Tests that heartbeat() fails gracefully when not DVM-launched
//!    - Tests that pmix::utility::initialized() returns false before init
//!
//! 2. Via prterun (prterun -np 1 cargo test --test utility_heartbeat -- --ignored):
//!    - Tests that heartbeat() returns Ok(()) when DVM-launched
//!    - Tests that heartbeat() can be called multiple times
//!    - Tests that pmix::utility::initialized() is true before calling heartbeat

mod daemon_helper;

/// Check if we were launched by the DVM (prterun/prun).
fn is_dvm_launched() -> bool {
    std::env::var("PMIX_NAMESPACE").is_ok()
        || std::env::var("PMIX_RANK").is_ok()
        || std::env::var("PRTE_LAUNCHED").is_ok()
        || std::env::var("PMIX_SERVER_URI2").is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests - run normally, verify graceful behavior outside DVM
// ─────────────────────────────────────────────────────────────────────────────

/// pmix::utility::initialized() returns false when not initialized.
#[test]
fn test_initialized_false_before_init() {
    if is_dvm_launched() {
        return;
    }
    assert!(
        !pmix::utility::initialized(),
        "pmix::utility::initialized() should be false before pmix::init()"
    );
}

/// pmix::monitoring::heartbeat() fails when not DVM-launched.
#[test]
fn test_heartbeat_fails_without_dvm() {
    if is_dvm_launched() {
        return;
    }
    let result = pmix::monitoring::heartbeat();
    assert!(
        result.is_err(),
        "pmix::monitoring::heartbeat() should fail when not launched by DVM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests - only run when prterun launches us
// ─────────────────────────────────────────────────────────────────────────────

/// pmix::monitoring::heartbeat() returns Ok(()) when DVM-launched.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_succeeds_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");
    let result = pmix::monitoring::heartbeat();
    assert!(
        result.is_ok(),
        "pmix::monitoring::heartbeat() should succeed when launched by prterun, got: {:?}",
        result
    );
}

/// pmix::monitoring::heartbeat() can be called multiple times.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_multiple_calls() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");

    for i in 1..=5 {
        let result = pmix::monitoring::heartbeat();
        assert!(
            result.is_ok(),
            "pmix::monitoring::heartbeat() call #{} should succeed, got: {:?}",
            i,
            result
        );
    }
}

/// pmix::utility::initialized() is true before calling heartbeat().
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_initialized_true_before_heartbeat() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");

    assert!(
        pmix::utility::initialized(),
        "pmix::utility::initialized() should return true after pmix::init()"
    );

    let result = pmix::monitoring::heartbeat();
    assert!(
        result.is_ok(),
        "pmix::monitoring::heartbeat() should succeed after init, got: {:?}",
        result
    );
}
