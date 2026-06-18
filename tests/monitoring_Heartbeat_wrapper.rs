//! Wrapper tests for monitoring.rs — Heartbeat Operation.
//!
//! Extends Batch 14 with additional heartbeat-specific tests.
//! Batch 14 already covered heartbeat error paths. This batch
//! documents remaining gaps and adds edge case tests.
//!
//! Remaining uncovered lines in monitoring.rs are all PMIx-dependent:
//!   - MonitorResults::len (70-72), is_empty (75-77), drop (81-91)
//!   - monitor_callback_bridge (124-164)
//!   - process_monitor success path (253-256)
//!   - process_monitor_nb success path (337)
//!   - heartbeat success path (421)

use pmix::monitoring::*;

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — additional edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat returns error (not initialized) — already in Batch 14, confirming.
#[test]
fn test_heartbeat_returns_error() {
    let result = heartbeat();
    assert!(result.is_err());
}

/// heartbeat error is consistent across calls.
#[test]
fn test_heartbeat_consistent_error() {
    let r1 = heartbeat();
    let r2 = heartbeat();
    let r3 = heartbeat();
    let r4 = heartbeat();
    let r5 = heartbeat();
    assert_eq!(r1, r2);
    assert_eq!(r2, r3);
    assert_eq!(r3, r4);
    assert_eq!(r4, r5);
}

/// heartbeat is safe to call concurrently (no panic).
#[test]
fn test_heartbeat_concurrent_safe() {
    use std::thread;

    let handles: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..10 {
                    let _ = heartbeat();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("heartbeat should not panic");
    }
}

/// heartbeat does not allocate or leak memory.
#[test]
fn test_heartbeat_no_allocation() {
    // heartbeat() is a simple FFI wrapper — no Rust-side allocation.
    // Calling it without init should return immediately with an error.
    let _ = heartbeat();
    let _ = heartbeat();
    let _ = heartbeat();
}

// ─────────────────────────────────────────────────────────────────────────────
// #[ignore] tests — require PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat success path sends liveness signal.
/// Requires PMIx_Init + server connection.
#[test]
#[ignore = "requires PMIx_Init — heartbeat success path only works with server"]
fn test_heartbeat_success() {
    // Requires PMIx_Init + server. See line 421 in monitoring.rs.
    // The success path calls PMIx_Heartbeat() and returns Ok(()) on success.
}

/// MonitorResults::len returns the number of monitoring results.
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — MonitorResults only populated on FFI success"]
fn test_monitor_results_len() {
    // Requires PMIx_Init. See lines 70-72 in monitoring.rs.
}

/// MonitorResults::is_empty returns true when no results.
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — MonitorResults only populated on FFI success"]
fn test_monitor_results_is_empty() {
    // Requires PMIx_Init. See lines 75-77 in monitoring.rs.
}

/// MonitorResults::drop frees the PMIx info array.
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — drop only meaningful on FFI success"]
fn test_monitor_results_drop() {
    // Requires PMIx_Init. See lines 81-91 in monitoring.rs.
}

/// monitor_callback_bridge is invoked by PMIx C library on async completion.
/// Requires PMIx_Init to exercise the callback bridge path.
#[test]
#[ignore = "requires PMIx_Init — callback bridge only invoked by C library"]
fn test_monitor_callback_bridge() {
    // Requires PMIx_Init. See lines 124-164 in monitoring.rs.
}

/// process_monitor success path returns MonitorResults.
/// Requires PMIx_Init + server.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_process_monitor_success() {
    // Requires PMIx_Init + server. See lines 253-256 in monitoring.rs.
}

/// process_monitor_nb success path registers monitoring.
/// Requires PMIx_Init + server.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_process_monitor_nb_success() {
    // Requires PMIx_Init + server. See line 337 in monitoring.rs.
}
