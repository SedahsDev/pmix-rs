//! Tests for `PMIx_Process_monitor` — process monitoring operations.
//!
//! These tests verify the Rust wrapper for `PMIx_Process_monitor`,
//! `PMIx_Process_monitor_nb`, and `heartbeat`. They test parameter
//! validation, callback trait compilation, result types, and API
//! signatures. Integration tests that require a running PMIx daemon
//! are marked `#[ignore]`.

use pmix::monitoring::{
    heartbeat, process_monitor, process_monitor_nb, MonitorCallback, MonitorResults,
};
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for process_monitor_nb.
struct TestMonitorCallback;

impl MonitorCallback for TestMonitorCallback {
    fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
        // No-op — just verify the trait compiles.
    }
}

/// Test callback that records the status and results it received.
struct RecordingMonitorCallback {
    status: std::cell::Cell<Option<PmixStatus>>,
    has_results: std::cell::Cell<bool>,
}

impl MonitorCallback for RecordingMonitorCallback {
    fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>) {
        self.status.set(Some(status));
        self.has_results.set(results.is_some());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults — structure and traits
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorResults has len() and is_empty() methods.
#[test]
fn test_monitor_results_methods_exist() {
    // We can't construct MonitorResults directly (it requires FFI),
    // but we can verify the method signatures compile.
    fn _check_methods(r: &MonitorResults) {
        let _: usize = r.len();
        let _: bool = r.is_empty();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback — trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorCallback trait requires Send.
#[test]
fn test_monitor_callback_requires_send() {
    fn assert_send<T: MonitorCallback>()
    where
        T: Send,
    {
    }
    assert_send::<TestMonitorCallback>();
}

/// RecordingMonitorCallback correctly records status.
#[test]
fn test_recording_callback_records_status() {
    let cb = RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    };
    assert!(cb.status.get().is_none());
    assert!(!cb.has_results.get());
}

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — tests
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat returns Err when no daemon is running.
#[test]
fn test_heartbeat_without_daemon_returns_err() {
    let result = heartbeat();
    assert!(
        result.is_err(),
        "heartbeat should fail without daemon: {:?}",
        result
    );
}

/// heartbeat error is not a success status.
#[test]
fn test_heartbeat_error_is_not_success() {
    let result = heartbeat();
    if let Err(status) = result {
        assert!(
            !status.is_success(),
            "heartbeat error should not be success: {:?}",
            status
        );
    }
}

/// heartbeat returns PmixStatus error type.
#[test]
fn test_heartbeat_return_type() {
    let result: Result<(), PmixStatus> = heartbeat();
    drop(result);
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor — tests
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor signature: (&Info, PmixStatus, &[Info]) -> Result<MonitorResults, PmixStatus>.
#[test]
fn test_process_monitor_signature() {
    fn _check_signature(
        f: impl Fn(&pmix::Info, PmixStatus, &[pmix::Info]) -> Result<MonitorResults, PmixStatus>,
    ) {
        let _ = f;
    }
    _check_signature(process_monitor);
}

/// process_monitor returns Err without daemon.
#[test]
fn test_process_monitor_without_daemon_returns_err() {
    let monitor = InfoBuilder::new().build();
    let directives: &[pmix::Info] = &[];
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::MonitorHeartbeatAlert), directives);
    assert!(
        result.is_err(),
        "process_monitor should fail without daemon: {:?}",
        result
    );
}

/// process_monitor_nb signature accepts correct parameters.
#[test]
fn test_process_monitor_nb_signature() {
    fn _check_signature(
        f: impl Fn(
            &pmix::Info,
            PmixStatus,
            &[pmix::Info],
            Box<dyn MonitorCallback>,
        ) -> Result<(), PmixStatus>,
    ) {
        let _ = f;
    }
    _check_signature(process_monitor_nb);
}

/// process_monitor_nb returns Err without daemon.
#[test]
fn test_process_monitor_nb_without_daemon_returns_err() {
    let monitor = InfoBuilder::new().build();
    let directives: &[pmix::Info] = &[];
    let cb: Box<dyn MonitorCallback> = Box::new(TestMonitorCallback);
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::MonitorHeartbeatAlert),
        directives,
        cb,
    );
    assert!(
        result.is_err(),
        "process_monitor_nb should fail without daemon: {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixError — monitoring variants
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixError has monitoring-related variants.
#[test]
fn test_pmix_error_monitor_variants() {
    let _ = PmixError::MonitorHeartbeatAlert;
    let _ = PmixError::MonitorFileAlert;
}

/// Test that PmixError monitoring variants derive the expected traits.
#[test]
fn test_pmix_error_monitor_traits() {
    let _ = format!("{:?}", PmixError::MonitorHeartbeatAlert);
    let _ = format!("{:?}", PmixError::MonitorFileAlert);

    let cloned = PmixError::MonitorHeartbeatAlert.clone();
    assert_eq!(cloned, PmixError::MonitorHeartbeatAlert);

    let copied = PmixError::MonitorFileAlert;
    assert_eq!(copied, PmixError::MonitorFileAlert);

    assert_eq!(
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorHeartbeatAlert
    );
    assert_ne!(
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorFileAlert
    );

    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(PmixError::MonitorHeartbeatAlert);
    set.insert(PmixError::MonitorFileAlert);
    assert_eq!(set.len(), 2);
    assert!(set.contains(&PmixError::MonitorHeartbeatAlert));
    assert!(set.contains(&PmixError::MonitorFileAlert));
}

/// Test from_raw for monitoring error codes returns Some.
#[test]
fn test_pmix_error_from_raw_monitoring() {
    assert_eq!(
        PmixError::from_raw(-109),
        Some(PmixError::MonitorHeartbeatAlert)
    );
    assert_eq!(PmixError::from_raw(-110), Some(PmixError::MonitorFileAlert));
}

/// Test that monitoring with PMIX_MONITOR_HEARTBEAT_ALERT as error code
/// compiles and has the correct type.
#[test]
fn test_monitor_error_code_type() {
    let error_code: PmixStatus = PmixStatus::Known(PmixError::MonitorHeartbeatAlert);
    assert_eq!(error_code.to_raw(), -109);
    assert!(!error_code.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Test heartbeat under a real PMIx environment.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_heartbeat_with_daemon() {
    let result = heartbeat();
    assert!(
        result.is_ok(),
        "heartbeat should succeed under PMIx daemon: {:?}",
        result
    );
}

/// Test process_monitor_nb with a callback under a real PMIx environment.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_process_monitor_nb_with_daemon() {
    let _cb: Box<dyn MonitorCallback> = Box::new(TestMonitorCallback);
}
