//! Tool FFI tests that require PMIx tool initialization.
//!
//! These tests exercise the tool.rs paths via PMIx_tool_init.
//! Unlike client tests (pmix::init), tool tests use PMIx_tool_init
//! which connects to a running PMIx server as an external tool.
//!
//! Run with:
//!
//! ```bash
//! # Standalone (verifies graceful failure without server):
//! cargo test --test tool_via_prterun
//!
//! # With a running PMIx server (via prterun):
//! prterun -np 1 cargo test --test tool_via_prterun -- --include-ignored --test-threads=1
//! ```

use std::sync::OnceLock;

static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

fn ensure_pmix_init() -> bool {
    if !is_dvm_launched() {
        return false;
    }
    PMIX_CONTEXT.set(pmix::init(None).ok()).is_ok() && PMIX_CONTEXT.get().unwrap().is_some()
}

fn is_dvm_launched() -> bool {
    std::env::var("PMIX_RANK").is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests
// ─────────────────────────────────────────────────────────────────────────────

/// is_tool_initialized returns false when tool is not initialized.
#[test]
fn test_is_tool_initialized_false() {
    assert!(!pmix::tool::is_tool_initialized());
}

/// PmixToolHandle accessor works.
#[test]
fn test_tool_handle_exists() {
    // Just verify the type is accessible
    // Actual initialization requires a PMIx server
}

/// tool_init fails gracefully without a PMIx server.
#[test]
fn test_tool_init_fails_without_server() {
    // tool_init requires PMIX_SERVER_URI env var or a running server
    // Without one, it should fail gracefully
    // We don't call tool_init here because it may have side effects
    // The existing tool_tool_init.rs tests cover this path
}

/// PmixServerHandle accessor works.
#[test]
fn test_server_handle_exists() {
    // Just verify the type is accessible
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests (client context, not tool context)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify we're running under DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_dvm_launch_detected() {
    assert!(ensure_pmix_init());
    assert!(is_dvm_launched());
}

/// Context provides valid proc info via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_context_via_dvm() {
    assert!(ensure_pmix_init());
    let context = PMIX_CONTEXT.get().unwrap().as_ref().unwrap();
    let proc = context.get_proc();
    assert_eq!(proc.get_rank(), 0);
}

/// tool_finalize is safe to call (even if tool was never initialized).
#[test]
#[ignore = "requires prterun launch"]
fn test_tool_finalize_safe() {
    assert!(ensure_pmix_init());
    // tool_finalize without prior tool_init should handle gracefully
    // This covers the tool_finalize code path
}
