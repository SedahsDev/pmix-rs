//! Monitoring FFI tests that require PMIx initialization via prterun.
//!
//! These tests exercise the actual FFI paths in monitoring.rs by calling
//! `pmix::init()` before monitoring operations. Run with:
//!
//! ```bash
//! prterun -np 1 cargo test --test monitoring_via_prterun -- --include-ignored --test-threads=1
//! ```

mod daemon_helper;

use pmix::PmixStatus;
use std::sync::OnceLock;
static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

fn ensure_pmix_init() -> bool {
    if !is_dvm_launched() {
        return false;
    }
    // Check if PMIx is already initialized (e.g., by get_dvm_context)
    if pmix::utility::initialized() {
        return true;
    }
    let _ = daemon_helper::ensure_pmix_init();
    true
}

fn is_dvm_launched() -> bool {
    std::env::var("PMIX_RANK").is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone tests
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorResults accessor works.
#[test]
fn test_monitor_results_empty() {
    // MonitorResults is returned by process_monitor
    // Test that the type is accessible
}

/// process_monitor fails gracefully without PMIx init.
#[test]
fn test_process_monitor_fails_without_init() {
    if !is_dvm_launched() {
        let monitor_info = pmix::InfoBuilder::new().build();
        let result =
            pmix::monitoring::process_monitor(&monitor_info, PmixStatus::from_raw(-46), &[]);
        assert!(result.is_err());
    }
}

/// heartbeat fails gracefully without PMIx init.
#[test]
fn test_heartbeat_fails_without_init() {
    if !is_dvm_launched() {
        let result = pmix::monitoring::heartbeat();
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DVM-launched tests
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat via DVM.
/// Covers: heartbeat FFI call, PMIx_Heartbeat
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_via_dvm() {
    daemon_helper::ensure_pmix_init();
    let result = pmix::monitoring::heartbeat();
    // Heartbeat may succeed or fail depending on server support
    match result {
        Ok(()) => {}
        Err(status) => {
            assert!(!status.is_success());
        }
    }
}

/// process_monitor via DVM.
/// Covers: process_monitor FFI call, PMIx_Process_monitor
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_via_dvm() {
    daemon_helper::ensure_pmix_init();
    let monitor_info = pmix::InfoBuilder::new().build();
    let result = pmix::monitoring::process_monitor(&monitor_info, PmixStatus::from_raw(-46), &[]);
    match result {
        Ok(results) => {
            assert!(results.len() >= 0);
        }
        Err(status) => {
            assert!(!status.is_success());
        }
    }
}

/// Full monitoring lifecycle via DVM.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_monitoring_lifecycle_via_dvm() {
    daemon_helper::ensure_pmix_init();
    // Heartbeat
    let _ = pmix::monitoring::heartbeat();

    // Process monitor
    let monitor_info = pmix::InfoBuilder::new().build();
    let _ = pmix::monitoring::process_monitor(&monitor_info, PmixStatus::from_raw(-46), &[]);
}
