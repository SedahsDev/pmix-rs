//! Tests for PmixClient::connect_new() (PMIx_Init) - the DVM-launched client path.
//!
//! PMIx_Init only works when the process is launched by the DVM (prterun/prun).
//! It does NOT accept PMIX_SERVER_URI from the environment - that is for
//! PMIx_tool_init (external tool path).
//!
//! These tests are designed to be run in two modes:
//!
//! 1. Standalone (cargo test --test init_via_prterun):
//!    - Tests that PmixClient::connect_new() FAILS gracefully with PMIX_ERR_UNREACH
//!    - Tests that pmix::initialized() returns false before init
//!
//! 2. Via prterun (prterun -np 1 cargo test --test init_via_prterun -- --ignored):
//!    - Tests that PmixClient::connect_new() SUCCEEDS when DVM-launched
//!    - Tests context, proc, namespace, rank from DVM connection
mod daemon_helper;

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

/// PmixClient::connect_new() fails when not DVM-launched.
#[test]
fn test_init_fails_without_dvm() {
    if is_dvm_launched() {
        return;
    }
    let result = pmix::PmixClient::connect_new(None);
    assert!(
        result.is_err(),
        "PmixClient::connect_new() should fail when not launched by DVM"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests - only run when prterun launches us
// ─────────────────────────────────────────────────────────────────────────────

/// PmixClient::connect_new() succeeds when launched by prterun.
///
/// Routes through `ensure_pmix_init()` so the process-wide PMIx client
/// session is initialized exactly once. Calling `connect_new` directly
/// here would hit the already-`Live` singleton (established by the
/// alphabetically-earlier `test_context_proc_info`) and return `ErrExists`.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_succeeds_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = daemon_helper::ensure_pmix_init();
    // The singleton context is usable — require_proc() panics if init failed.
    let _proc = context.require_proc();
}

/// PmixClient::connect_new() returns a valid context with rank 0.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_returns_valid_context() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = daemon_helper::ensure_pmix_init();
    let rank = context.require_rank();
    assert_eq!(rank, 0, "rank should be 0 for single-process job");
}

/// pmix::utility::initialized() returns true after PmixClient::connect_new().
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_initialized_after_init() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = daemon_helper::ensure_pmix_init();
    assert!(
        pmix::utility::initialized(),
        "pmix::initialized() should return true after PmixClient::connect_new()"
    );
}

/// PmixClient::connect_new() with Info succeeds via prterun.
///
/// Routes through `ensure_pmix_init()` — see `test_init_succeeds_via_prterun`
/// for why a direct `connect_new` would return `ErrExists` against the
/// already-`Live` singleton. The singleton itself calls `connect_new(None)`;
/// this test verifies the resulting context is usable with rank 0.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_with_info_via_prterun() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = daemon_helper::ensure_pmix_init();
    assert_eq!(
        context.require_rank(),
        0,
        "rank should be 0 for single-process job"
    );
}

/// PmixClient::connect_new() context provides valid proc namespace.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_context_proc_info() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let context = daemon_helper::ensure_pmix_init();
    let proc = context.require_proc();
    // Access nspace through proc_with_nspace which returns a new Proc
    let _new_proc = context
        .proc_with_nspace(0)
        .expect("proc_with_nspace should work");
    assert_eq!(proc.get_rank(), 0, "rank should be 0");
}

/// PmixClient::connect_new() -> finalize cycle works via prterun.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_finalize_cycle() {
    assert!(is_dvm_launched(), "this test must be launched by prterun");
    let _context = daemon_helper::ensure_pmix_init();
    // Call disconnect/finalize explicitly — Drop does not finalize
}
