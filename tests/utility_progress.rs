//! Tests for `pmix::progress()` — advances the PMIx internal event loop.
//!
//! progress() is a fire-and-forget wrapper around PMIx_Progress(). It does not
//! return a value and is used to drive the internal event processing of the
//! PMIx client library.
//!
//! These tests are designed to be run in two modes:
//!
//! 1. Standalone (cargo test --test utility_progress):
//!    - Tests that progress() does not panic when called outside DVM
//!
//! 2. Via prterun (prterun -np 1 cargo test --test utility_progress -- --ignored):
//!    - Tests that progress() does not panic/crash when DVM-launched
//!    - Tests that progress() can be called multiple times safely
//!    - Tests that init is valid before/after calling progress

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

/// pmix::progress() does not panic when called without DVM.
#[test]
fn test_progress_no_panic_without_dvm() {
    if is_dvm_launched() {
        return;
    }
    // Should not panic or crash even when not initialized
    pmix::progress();
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests - only run when prterun launches us
// ─────────────────────────────────────────────────────────────────────────────

/// pmix::progress() does not panic/crash when DVM-launched.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_progress_no_crash_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");

    // Should not panic or crash
    pmix::progress();
}

/// pmix::progress() can be called multiple times safely.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_progress_multiple_calls() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");

    for _ in 0..10 {
        pmix::progress();
    }
}

/// pmix::utility::initialized() is true before and after calling progress().
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_initialized_before_and_after_progress() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");

    assert!(
        pmix::utility::initialized(),
        "pmix::utility::initialized() should return true after pmix::init()"
    );

    pmix::progress();

    assert!(
        pmix::utility::initialized(),
        "pmix::utility::initialized() should still return true after pmix::progress()"
    );
}
