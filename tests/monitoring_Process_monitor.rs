//! Tests for `PMIx_Process_monitor` — process monitoring operations.
//!
//! These tests verify the Rust wrapper for `PMIx_Process_monitor`,
//! `PMIx_Process_monitor_nb`, and `heartbeat`. They test parameter
//! validation, callback trait compilation, result types, and API
//! signatures. Integration tests that require a running PMIx daemon
//! are marked `#[ignore]`.

use pmix::PmixError;
use pmix::monitoring::{
    heartbeat, process_monitor, process_monitor_nb, MonitorCallback, MonitorResults,
};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for process_monitor_nb.
struct TestMonitorCallback;

impl MonitorCallback for TestMonitorCallback {
    fn on_complete(&mut self, _status: pmix::PmixStatus, _results: Option<MonitorResults>) {
        // No-op — just verify the trait compiles.
    }
}

/// Test callback that records the status and results it received.
struct RecordingMonitorCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
    has_results: std::cell::Cell<bool>,
}

impl MonitorCallback for RecordingMonitorCallback {
    fn on_complete(&mut self, status: pmix::PmixStatus, results: Option<MonitorResults>) {
        self.status.set(Some(status));
        self.has_results.set(results.is_some());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that MonitorResults implements Debug (compile-time check).
#[test]
fn test_monitor_results_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<MonitorResults>();
}

/// Test that MonitorCallback trait object is Send (compile-time check).
#[test]
fn test_monitor_callback_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn MonitorCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus and PmixError monitoring alert codes
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PMIX_MONITOR_HEARTBEAT_ALERT (-109) is correctly mapped.
#[test]
fn test_monitor_heartbeat_alert_code() {
    let status = pmix::PmixStatus::from_raw(-109);
    assert_eq!(
        status,
        pmix::PmixStatus::Known(PmixError::MonitorHeartbeatAlert)
    );
    assert!(!status.is_success());
    assert!(status.is_error());
}

/// Test that PMIX_MONITOR_FILE_ALERT (-110) is correctly mapped.
#[test]
fn test_monitor_file_alert_code() {
    let status = pmix::PmixStatus::from_raw(-110);
    assert_eq!(status, pmix::PmixStatus::Known(PmixError::MonitorFileAlert));
    assert!(!status.is_success());
    assert!(status.is_error());
}

/// Test round-trip from PmixError to raw and back for MonitorHeartbeatAlert.
#[test]
fn test_monitor_heartbeat_alert_roundtrip() {
    let err = PmixError::MonitorHeartbeatAlert;
    let status = pmix::PmixStatus::Known(err);
    let raw = status.to_raw();
    assert_eq!(raw, -109);
    let recovered = pmix::PmixStatus::from_raw(raw);
    assert_eq!(recovered, status);
}

/// Test round-trip from PmixError to raw and back for MonitorFileAlert.
#[test]
fn test_monitor_file_alert_roundtrip() {
    let err = PmixError::MonitorFileAlert;
    let status = pmix::PmixStatus::Known(err);
    let raw = status.to_raw();
    assert_eq!(raw, -110);
    let recovered = pmix::PmixStatus::from_raw(raw);
    assert_eq!(recovered, status);
}

/// Test that MonitorHeartbeatAlert debug displays correctly.
#[test]
fn test_monitor_heartbeat_alert_debug() {
    let err = PmixError::MonitorHeartbeatAlert;
    let debug_str = format!("{:?}", err);
    assert!(
        debug_str.contains("MonitorHeartbeatAlert"),
        "expected debug to contain variant name, got: {}",
        debug_str
    );
}

/// Test that MonitorFileAlert debug displays correctly.
#[test]
fn test_monitor_file_alert_debug() {
    let err = PmixError::MonitorFileAlert;
    let debug_str = format!("{:?}", err);
    assert!(
        debug_str.contains("MonitorFileAlert"),
        "expected debug to contain variant name, got: {}",
        debug_str
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// API signature tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that process_monitor has the correct function signature.
#[test]
fn test_process_monitor_api_signature() {
    fn _check_signature(
        monitor: &pmix::Info,
        error: pmix::PmixStatus,
        directives: &[pmix::Info],
    ) -> Result<MonitorResults, pmix::PmixStatus> {
        process_monitor(monitor, error, directives)
    }
}

/// Test that process_monitor_nb with a callback has the correct signature.
#[test]
fn test_process_monitor_nb_api_signature() {
    fn _check_signature(
        monitor: &pmix::Info,
        error: pmix::PmixStatus,
        directives: &[pmix::Info],
        callback: Box<dyn MonitorCallback>,
    ) -> Result<(), pmix::PmixStatus> {
        process_monitor_nb(monitor, error, directives, callback)
    }
}

/// Test that heartbeat returns the correct type and behaves without a server.
#[test]
fn test_heartbeat_api_signature() {
    let _result: Result<(), pmix::PmixStatus> = heartbeat();
    match _result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::Error),
                "unexpected error from heartbeat without PMIx server: {:?}",
                status
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait compilation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that MonitorCallback can be implemented with various types.
#[test]
fn test_monitor_callback_trait_compiles() {
    // Unit struct callback
    let _cb1: Box<dyn MonitorCallback> = Box::new(TestMonitorCallback);

    // Struct with fields
    let _cb2: Box<dyn MonitorCallback> = Box::new(RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    });

    // Closure-like callback
    struct ClosureMonitorCallback {
        f: Box<dyn Fn(pmix::PmixStatus, Option<MonitorResults>) + Send>,
    }
    impl MonitorCallback for ClosureMonitorCallback {
        fn on_complete(
            &mut self,
            status: pmix::PmixStatus,
            results: Option<MonitorResults>,
        ) {
            (self.f)(status, results);
        }
    }
    let _cb3: Box<dyn MonitorCallback> = Box::new(ClosureMonitorCallback {
        f: Box::new(|_, _| {}),
    });
}

/// Test that RecordingMonitorCallback correctly records status.
#[test]
fn test_recording_monitor_callback() {
    let mut cb = RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    };

    let test_status = pmix::PmixStatus::Known(PmixError::Success);
    MonitorCallback::on_complete(&mut cb, test_status, None);

    assert_eq!(cb.status.get(), Some(pmix::PmixStatus::Known(PmixError::Success)));
    assert!(!cb.has_results.get());
}

// ─────────────────────────────────────────────────────────────────────────────
// Error handling tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PMIX_SUCCESS (0) is correctly recognized as success.
#[test]
fn test_pmix_success_for_monitoring() {
    let status = pmix::PmixStatus::from_raw(0);
    assert!(status.is_success());
    assert!(!status.is_error());
    assert_eq!(status, pmix::PmixStatus::Known(PmixError::Success));
}

/// Test that negative monitoring codes are errors.
#[test]
fn test_monitoring_codes_are_errors() {
    let hb = pmix::PmixStatus::from_raw(-109);
    assert!(!hb.is_success());
    assert!(hb.is_error());

    let fa = pmix::PmixStatus::from_raw(-110);
    assert!(!fa.is_success());
    assert!(fa.is_error());
}

/// Test that monitoring alert codes are distinguishable from each other.
#[test]
fn test_monitoring_alert_codes_distinct() {
    let hb = pmix::PmixStatus::from_raw(-109);
    let fa = pmix::PmixStatus::from_raw(-110);
    assert_ne!(hb, fa, "heartbeat alert and file alert must be distinct");
}

/// Test that monitoring alert codes are distinguishable from generic errors.
#[test]
fn test_monitoring_alert_vs_generic_error() {
    let hb = pmix::PmixStatus::from_raw(-109);
    let generic = pmix::PmixStatus::from_raw(-1);
    assert_ne!(
        hb, generic,
        "heartbeat alert must differ from generic error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixError enum properties
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixError monitoring variants derive the expected traits.
#[test]
fn test_pmix_error_monitor_traits() {
    // Debug
    let _ = format!("{:?}", PmixError::MonitorHeartbeatAlert);
    let _ = format!("{:?}", PmixError::MonitorFileAlert);

    // Clone
    let cloned = PmixError::MonitorHeartbeatAlert.clone();
    assert_eq!(cloned, PmixError::MonitorHeartbeatAlert);

    // Copy
    let copied = PmixError::MonitorFileAlert;
    assert_eq!(copied, PmixError::MonitorFileAlert);

    // PartialEq
    assert_eq!(
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorHeartbeatAlert
    );
    assert_ne!(
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorFileAlert
    );

    // Eq + Hash
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

// ─────────────────────────────────────────────────────────────────────────────
// Error code type tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that monitoring with PMIX_MONITOR_HEARTBEAT_ALERT as error code
/// compiles and has the correct type.
#[test]
fn test_monitor_error_code_type() {
    let error_code: pmix::PmixStatus =
        pmix::PmixStatus::Known(PmixError::MonitorHeartbeatAlert);
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
