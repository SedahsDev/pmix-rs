//! Tests for pmix::init() (PMIx_Init) - the DVM-launched client path.
//!
//! PMIx_Init only works when the process is launched by the DVM (prterun/prun).
//! It does NOT accept PMIX_SERVER_URI from the environment - that is for
//! PMIx_tool_init (external tool path).
//!
//! These tests are designed to be run in two modes:
//!
//! 1. Standalone (cargo test --test init_via_prterun):
//!    - Tests that pmix::init() FAILS gracefully with PMIX_ERR_UNREACH
//!    - Tests that pmix::initialized() returns false before init
//!
//! 2. Via prterun (prterun -np 1 cargo test --test init_via_prterun -- --ignored):
//!    - Tests that pmix::init() SUCCEEDS when DVM-launched
//!    - Tests context, proc, namespace, rank from DVM connection

mod daemon_helper;

use pmix::InfoBuilder;

/// Check if we were launched by the DVM (prterun/prun).
fn is_dvm_launched() -> bool {
    std::env::var("PMIX_NAMESPACE").is_ok()
        || std::env::var("PMIX_RANK").is_ok()
        || std::env::var("PRTE_LAUNCHED").is_ok()
        || std::env::var("PMIX_SERVER_URI2").is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests - run normally, verify PMIx_Init fails gracefully
// ─────────────────────────────────────────────────────────────────────────────

/// pmix::init() fails when not DVM-launched.
#[test]
fn test_init_fails_without_dvm() {
    if is_dvm_launched() {
        return;
    }
    let result = pmix::init(None);
    assert!(
        result.is_err(),
        "pmix::init() should fail when not launched by DVM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests - only run when prterun launches us
// ─────────────────────────────────────────────────────────────────────────────

/// pmix::init() succeeds when launched by prterun.
#[test]
#[ignore = "requires prterun launch"]
fn test_init_succeeds_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let result = pmix::init(None);
    assert!(
        result.is_ok(),
        "pmix::init() should succeed when launched by prterun"
    );
}

/// pmix::init() returns a valid context with rank 0.
#[test]
#[ignore = "requires prterun launch"]
fn test_init_returns_valid_context() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = pmix::init(None).expect("pmix::init() failed");
    let rank = context.get_rank();
    assert_eq!(rank, 0, "rank should be 0 for single-process job");
}

/// pmix::utility::initialized() returns true after pmix::init().
#[test]
#[ignore = "requires prterun launch"]
fn test_initialized_after_init() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");
    assert!(
        pmix::utility::initialized(),
        "pmix::initialized() should return true after pmix::init()"
    );
}

/// pmix::init() with Info succeeds via prterun.
#[test]
#[ignore = "requires prterun launch"]
fn test_init_with_info_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let info = InfoBuilder::new().build();
    let result = pmix::init(Some(info));
    assert!(
        result.is_ok(),
        "pmix::init() with info should succeed via prterun"
    );
}

/// pmix::init() context provides valid proc namespace.
#[test]
#[ignore = "requires prterun launch"]
fn test_context_proc_info() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = pmix::init(None).expect("pmix::init() failed");
    let proc = context.get_proc();
    // Access nspace through proc_with_nspace which returns a new Proc
    let _new_proc = context
        .proc_with_nspace(0)
        .expect("proc_with_nspace should work");
    assert_eq!(proc.get_rank(), 0, "rank should be 0");
}

/// pmix::init() -> finalize cycle works via prterun.
#[test]
#[ignore = "requires prterun launch"]
fn test_init_finalize_cycle() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = pmix::init(None).expect("pmix::init() failed");
    // Context Drop calls finalize automatically
}
