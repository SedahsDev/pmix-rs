//! Wrapper tests for monitoring.rs — Process Monitor Core.
//!
//! Tests exercise `process_monitor`, `process_monitor_nb`, `heartbeat`
//! and `MonitorResults` wrapper logic without PMIx_Init.
//! FFI calls return errors gracefully (no segfault).
//!
//! Coverage targets:
//!   - MonitorResults (lines 60-91) — len, is_empty, drop (drop needs FFI success)
//!   - process_monitor (lines 205-256) — FFI call + error path
//!   - process_monitor_nb (lines 288-337) — callback reg + FFI + cleanup
//!   - heartbeat (lines 379-421) — FFI call + error path

use pmix::monitoring::*;
use pmix::{InfoBuilder, PmixStatus, PmixError};

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor — synchronous process monitoring
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor with empty directives returns error.
#[test]
fn test_process_monitor_empty_directives() {
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    assert!(result.is_err());
}

/// process_monitor with directives returns error (not initialized).
#[test]
fn test_process_monitor_with_directives() {
    let monitor = InfoBuilder::new().build();
    let directive = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[directive]);
    assert!(result.is_err());
}

/// process_monitor with multiple directives returns error.
#[test]
fn test_process_monitor_multiple_directives() {
    let monitor = InfoBuilder::new().build();
    let d1 = InfoBuilder::new().build();
    let d2 = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[d1, d2]);
    assert!(result.is_err());
}

/// process_monitor is deterministic.
#[test]
fn test_process_monitor_deterministic() {
    let monitor = InfoBuilder::new().build();
    let r1 = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    let r2 = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    assert_eq!(r1.is_err(), r2.is_err());
}

/// process_monitor with error status returns error.
#[test]
fn test_process_monitor_error_status() {
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[]);
    assert!(result.is_err());
}

/// process_monitor repeated calls are idempotent.
#[test]
fn test_process_monitor_idempotent() {
    let monitor = InfoBuilder::new().build();
    let r1 = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    let r2 = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    let r3 = process_monitor(&monitor, PmixStatus::Known(PmixError::Success), &[]);
    assert_eq!(r1.is_err(), r2.is_err());
    assert_eq!(r2.is_err(), r3.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor_nb — non-blocking process monitoring
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor_nb with empty directives returns error, callback not invoked.
#[test]
fn test_process_monitor_nb_empty_directives() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl MonitorCallback for Cb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let monitor = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::Success),
        &[],
        Box::new(Cb { c: Arc::clone(&called) }),
    );
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// process_monitor_nb with directives returns error, callback not invoked.
#[test]
fn test_process_monitor_nb_with_directives() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl MonitorCallback for Cb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let monitor = InfoBuilder::new().build();
    let directive = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::Success),
        &[directive],
        Box::new(Cb { c: Arc::clone(&called) }),
    );
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// process_monitor_nb with multiple directives returns error, callback not invoked.
#[test]
fn test_process_monitor_nb_multiple_directives() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl MonitorCallback for Cb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let monitor = InfoBuilder::new().build();
    let d1 = InfoBuilder::new().build();
    let d2 = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::Success),
        &[d1, d2],
        Box::new(Cb { c: Arc::clone(&called) }),
    );
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// process_monitor_nb is deterministic.
#[test]
fn test_process_monitor_nb_deterministic() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl MonitorCallback for Cb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let monitor = InfoBuilder::new().build();
    let r1 = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::Success),
        &[],
        Box::new(Cb { c: Arc::clone(&called) }),
    );
    let called2 = Arc::new(AtomicBool::new(false));
    let r2 = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::Success),
        &[],
        Box::new(Cb { c: Arc::clone(&called2) }),
    );
    assert_eq!(r1.is_err(), r2.is_err());
    assert!(!called.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));
}

/// process_monitor_nb with error status returns error.
#[test]
fn test_process_monitor_nb_error_status() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl MonitorCallback for Cb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let monitor = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::ErrNotFound),
        &[],
        Box::new(Cb { c: Arc::clone(&called) }),
    );
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst));
}

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — liveness signal
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat returns error (not initialized).
#[test]
fn test_heartbeat_no_init() {
    let result = heartbeat();
    assert!(result.is_err());
}

/// heartbeat is deterministic.
#[test]
fn test_heartbeat_deterministic() {
    let r1 = heartbeat();
    let r2 = heartbeat();
    assert_eq!(r1.is_err(), r2.is_err());
}

/// heartbeat repeated calls are idempotent.
#[test]
fn test_heartbeat_idempotent() {
    let r1 = heartbeat();
    let r2 = heartbeat();
    let r3 = heartbeat();
    assert_eq!(r1.is_err(), r2.is_err());
    assert_eq!(r2.is_err(), r3.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults — len, is_empty, drop (#[ignore] — require FFI success)
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorResults::len returns the number of results.
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
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_process_monitor_success() {
    // Requires PMIx_Init + server. See lines 253-256 in monitoring.rs.
}

/// process_monitor_nb success path registers monitoring.
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_process_monitor_nb_success() {
    // Requires PMIx_Init + server. See lines 337 in monitoring.rs.
}

/// heartbeat success path sends liveness signal.
/// Requires PMIx_Init to exercise the success path.
#[test]
#[ignore = "requires PMIx_Init — success path only works with server"]
fn test_heartbeat_success() {
    // Requires PMIx_Init + server. See lines 421 in monitoring.rs.
}
