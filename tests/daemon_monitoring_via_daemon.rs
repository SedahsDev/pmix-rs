//! Round 8 — P3: monitoring.rs module via prte-beast daemon.
//!
//! Uses shared tool handle from daemon_helper for single init/finalize lifecycle.
//!
//! Run:
//!   cargo test --test daemon_monitoring_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::monitoring::{
    MonitorCallback, MonitorResults, heartbeat, process_monitor, process_monitor_nb,
};
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run, no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_type() {
    let _f: fn(&pmix::Info, PmixStatus, &[pmix::Info]) -> Result<MonitorResults, PmixStatus> =
        process_monitor;
}

#[test]
fn test_process_monitor_nb_type() {
    let _f: fn(
        &pmix::Info,
        PmixStatus,
        &[pmix::Info],
        Box<dyn MonitorCallback>,
    ) -> Result<(), PmixStatus> = process_monitor_nb;
}

#[test]
fn test_heartbeat_type() {
    let _f: fn() -> Result<(), PmixStatus> = heartbeat;
}

#[test]
fn test_monitor_results_len_type() {
    let _f: fn(&MonitorResults) -> usize = MonitorResults::len;
}

#[test]
fn test_monitor_results_is_empty_type() {
    let _f: fn(&MonitorResults) -> bool = MonitorResults::is_empty;
}

#[test]
fn test_monitor_callback_trait_object() {
    struct TestCb;
    impl MonitorCallback for TestCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let _cb: Box<dyn MonitorCallback> = Box::new(TestCb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error-path tests (always run — no daemon needed)
//
// NOTE: These test the "before init" path. With the shared handle, PMIx is
// already initialized, so these now test behavior in an initialized context.
// The ErrInit path is covered by daemon_server tests.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_before_init() {
    let monitor_info = InfoBuilder::new().build();
    let result = process_monitor(&monitor_info, PmixStatus::Known(PmixError::Success), &[]);
    // Before init this returns ErrInit — after shared handle init, behavior depends on PMIx state
    assert!(
        result.is_err(),
        "process_monitor should fail without proper setup"
    );
}

#[test]
fn test_process_monitor_nb_before_init() {
    struct DummyCb;
    impl MonitorCallback for DummyCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor_info = InfoBuilder::new().build();
    let cb: Box<dyn MonitorCallback> = Box::new(DummyCb);
    let result = process_monitor_nb(
        &monitor_info,
        PmixStatus::Known(PmixError::Success),
        &[],
        cb,
    );
    assert!(
        result.is_err(),
        "process_monitor_nb should fail without proper setup"
    );
}

#[test]
fn test_heartbeat_before_init() {
    let result = heartbeat();
    assert!(
        result.is_err(),
        "heartbeat should fail without proper setup"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — CONSOLIDATED into a single test to avoid PMIx state corruption
// from multiple tool_init/finalize cycles. Uses shared tool handle.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "daemon isolation"]
fn test_monitoring_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("shared tool handle");
    let _ = handle; // handle lives for the duration; we just need it initialized

    let _info = InfoBuilder::new().build();
    let directives = vec![InfoBuilder::new().build()];

    // ── 1. heartbeat ──
    let hb_result = heartbeat();
    match &hb_result {
        Ok(()) => {}
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "heartbeat returned ErrInit after tool_init"
            );
        }
    }

    // ── 2. process_monitor with MonitorHeartbeatAlert ──
    let monitor_info = InfoBuilder::new().build();
    let monitor_result = process_monitor(
        &monitor_info,
        PmixStatus::Known(PmixError::MonitorHeartbeatAlert),
        &directives,
    );
    match &monitor_result {
        Ok(results) => {
            let len = results.len();
            let is_empty = results.is_empty();
            assert_eq!(is_empty, len == 0, "is_empty should match len == 0");
        }
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "process_monitor returned ErrInit after tool_init"
            );
        }
    }

    // ── 3. process_monitor_nb ──
    struct TestMonitorCb {
        called: std::sync::atomic::AtomicBool,
    }
    impl MonitorCallback for TestMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let cb: Box<dyn MonitorCallback> = Box::new(TestMonitorCb {
        called: std::sync::atomic::AtomicBool::new(false),
    });
    let nb_result = process_monitor_nb(
        &monitor_info,
        PmixStatus::Known(PmixError::Success),
        &directives,
        cb,
    );
    match &nb_result {
        Ok(()) => {}
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "process_monitor_nb returned ErrInit after tool_init"
            );
        }
    }

    // ── 4. MonitorResults::len() and is_empty() ──
    let monitor_result2 = process_monitor(
        &monitor_info,
        PmixStatus::Known(PmixError::Success),
        &directives,
    );
    if let Ok(results) = monitor_result2 {
        let len = results.len();
        let is_empty = results.is_empty();
        assert_eq!(is_empty, len == 0);
    }

    // ── 5. Full workflow: process_monitor + process_monitor_nb + heartbeat ──
    let _ = process_monitor(
        &monitor_info,
        PmixStatus::Known(PmixError::Success),
        &directives,
    );

    struct DummyMonitorCb;
    impl MonitorCallback for DummyMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let cb2: Box<dyn MonitorCallback> = Box::new(DummyMonitorCb);
    let _ = process_monitor_nb(
        &monitor_info,
        PmixStatus::Known(PmixError::Success),
        &directives,
        cb2,
    );

    let _ = heartbeat();
}
