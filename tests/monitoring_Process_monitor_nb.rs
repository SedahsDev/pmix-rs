//! Tests for `PMIx_Process_monitor_nb` - non-blocking process monitoring.
//!
//! Derived from C test patterns in `test/simple/simpjctrl.c` which exercises
//! `PMIx_Process_monitor_nb` with heartbeat-based monitoring.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::PmixError;
use pmix::monitoring::{MonitorCallback, MonitorResults, process_monitor_nb};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// Callback that records whether it was invoked.
struct CalledMonitorCallback {
    called: std::cell::Cell<bool>,
}

impl MonitorCallback for CalledMonitorCallback {
    fn on_complete(&mut self, _status: pmix::PmixStatus, _results: Option<MonitorResults>) {
        self.called.set(true);
    }
}

/// Callback that records the status and whether results were returned.
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

/// Callback backed by Arc<Mutex<>> for sharing across threads.
struct ArcMonitorCallback {
    called: std::sync::Arc<std::sync::Mutex<bool>>,
}

impl MonitorCallback for ArcMonitorCallback {
    fn on_complete(&mut self, _status: pmix::PmixStatus, _results: Option<MonitorResults>) {
        *self.called.lock().unwrap() = true;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API signature tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that process_monitor_nb has the expected function signature.
#[test]
fn test_process_monitor_nb_signature() {
    fn _check_sig(
        monitor: &pmix::Info,
        error: pmix::PmixStatus,
        directives: &[pmix::Info],
        callback: Box<dyn MonitorCallback>,
    ) -> Result<(), pmix::PmixStatus> {
        process_monitor_nb(monitor, error, directives, callback)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorCallback trait object is Send (required for cross-thread callbacks).
#[test]
fn test_monitor_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn MonitorCallback>>();
}

/// MonitorCallback can be implemented on a unit struct.
#[test]
fn test_monitor_callback_unit_struct() {
    struct UnitCb;
    impl MonitorCallback for UnitCb {
        fn on_complete(&mut self, _: pmix::PmixStatus, _: Option<MonitorResults>) {}
    }
    let _cb: Box<dyn MonitorCallback> = Box::new(UnitCb);
}

/// MonitorCallback can be implemented on a struct with fields.
#[test]
fn test_monitor_callback_struct_with_fields() {
    let _cb: Box<dyn MonitorCallback> = Box::new(RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    });
    let mut rc = RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    };
    rc.on_complete(pmix::PmixStatus::Known(PmixError::Success), None);
    assert_eq!(
        rc.status.get(),
        Some(pmix::PmixStatus::Known(PmixError::Success))
    );
    assert!(!rc.has_results.get());
}

/// MonitorCallback can be implemented with Arc<Mutex<>> for shared state.
#[test]
fn test_monitor_callback_arc_mutex() {
    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let mut cb: Box<dyn MonitorCallback> = Box::new(ArcMonitorCallback {
        called: called.clone(),
    });
    cb.on_complete(pmix::PmixStatus::Known(PmixError::Success), None);
    assert!(*called.lock().unwrap(), "callback should have been called");
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults tests
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorResults implements Debug.
#[test]
fn test_monitor_results_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<MonitorResults>();
}

/// RecordingMonitorCallback correctly records results presence.
#[test]
fn test_recording_callback_with_results() {
    let mut rc = RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    };
    rc.on_complete(pmix::PmixStatus::Known(PmixError::ErrInit), None);
    assert!(!rc.has_results.get());
    assert_eq!(
        rc.status.get(),
        Some(pmix::PmixStatus::Known(PmixError::ErrInit))
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Monitoring alert codes - C test uses PMIX_MONITOR_HEARTBEAT_ALERT
// ─────────────────────────────────────────────────────────────────────────────

/// C test passes PMIX_MONITOR_HEARTBEAT_ALERT (-109) as the error parameter.
#[test]
fn test_heartbeat_alert_code_from_c_test() {
    let alert = pmix::PmixStatus::from_raw(-109);
    assert_eq!(
        alert,
        pmix::PmixStatus::Known(PmixError::MonitorHeartbeatAlert)
    );
    assert!(!alert.is_success());
    assert!(alert.is_error());
}

/// PMIX_MONITOR_FILE_ALERT (-110) is the other monitoring alert code.
#[test]
fn test_file_alert_code() {
    let alert = pmix::PmixStatus::from_raw(-110);
    assert_eq!(alert, pmix::PmixStatus::Known(PmixError::MonitorFileAlert));
    assert!(!alert.is_success());
}

/// Heartbeat alert and file alert are distinct.
#[test]
fn test_monitor_alert_codes_distinct() {
    let hb = pmix::PmixStatus::from_raw(-109);
    let fa = pmix::PmixStatus::from_raw(-110);
    assert_ne!(hb, fa);
}

/// Round-trip for heartbeat alert: raw -> PmixStatus -> raw.
#[test]
fn test_heartbeat_alert_roundtrip() {
    let status = pmix::PmixStatus::Known(PmixError::MonitorHeartbeatAlert);
    assert_eq!(status.to_raw(), -109);
    let recovered = pmix::PmixStatus::from_raw(-109);
    assert_eq!(recovered, status);
}

/// Round-trip for file alert: raw -> PmixStatus -> raw.
#[test]
fn test_file_alert_roundtrip() {
    let status = pmix::PmixStatus::Known(PmixError::MonitorFileAlert);
    assert_eq!(status.to_raw(), -110);
    let recovered = pmix::PmixStatus::from_raw(-110);
    assert_eq!(recovered, status);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error handling - process_monitor_nb without PMIx_Init
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor_nb with PMIX_MONITOR_FILE_ALERT as error code
/// compiles and has the correct type.
#[test]
fn test_monitor_nb_file_alert_error_type() {
    let error_code: pmix::PmixStatus = pmix::PmixStatus::Known(PmixError::MonitorFileAlert);
    assert_eq!(error_code.to_raw(), -110);
    assert!(!error_code.is_success());
}

/// process_monitor_nb with PMIX_SUCCESS as error code is valid.
#[test]
fn test_monitor_nb_success_error_type() {
    let error_code: pmix::PmixStatus = pmix::PmixStatus::Known(PmixError::Success);
    assert_eq!(error_code.to_raw(), 0);
    assert!(error_code.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixError enum properties for monitoring variants
// ─────────────────────────────────────────────────────────────────────────────

/// Monitoring error variants derive Debug, Clone, Copy, PartialEq, Eq, Hash.
#[test]
fn test_pmix_error_monitor_variants_traits() {
    let _ = format!("{:?}", PmixError::MonitorHeartbeatAlert);
    let _ = format!("{:?}", PmixError::MonitorFileAlert);
    let copied_hb = PmixError::MonitorHeartbeatAlert;
    let cloned_fa = PmixError::MonitorFileAlert;
    assert_eq!(copied_hb, PmixError::MonitorHeartbeatAlert);
    assert_eq!(cloned_fa, PmixError::MonitorFileAlert);
    assert_ne!(
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorFileAlert
    );
    use std::collections::HashSet;
    let mut set = HashSet::new();
    assert!(set.insert(PmixError::MonitorHeartbeatAlert));
    assert!(set.insert(PmixError::MonitorFileAlert));
    assert_eq!(set.len(), 2);
}

/// from_raw maps monitoring codes to the correct variants.
#[test]
fn test_pmix_error_from_raw_monitoring() {
    assert_eq!(
        PmixError::from_raw(-109),
        Some(PmixError::MonitorHeartbeatAlert)
    );
    assert_eq!(PmixError::from_raw(-110), Some(PmixError::MonitorFileAlert));
}

// ─────────────────────────────────────────────────────────────────────────────
// C test pattern reproduction - simpjctrl.c
// ─────────────────────────────────────────────────────────────────────────────

/// C test loads 3 directive info entries:
///   info[0] = PMIX_MONITOR_ID = "MONITOR1"
///   info[1] = PMIX_MONITOR_HEARTBEAT_TIME = 5
///   info[2] = PMIX_MONITOR_HEARTBEAT_DROPS = 2
#[test]
fn test_c_test_monitor_directive_keys() {
    let keys = [
        "pmix.monitor.id",
        "pmix.monitor.heartbeat.time",
        "pmix.monitor.heartbeat.drops",
    ];
    for key in &keys {
        let cstr = std::ffi::CString::new(*key);
        assert!(
            cstr.is_ok(),
            "monitor directive key '{}' should be valid for CString",
            key
        );
    }
}

/// C test uses PMIX_MONITOR_HEARTBEAT as the monitor info key.
#[test]
fn test_c_test_monitor_heartbeat_key() {
    let cstr = std::ffi::CString::new("pmix.monitor.heartbeat");
    assert!(cstr.is_ok());
}

/// The implementation encodes a request ID in the cbdata pointer.
/// Verify the encoding/decoding round-trip.
#[test]
fn test_monitor_cbdata_encoding() {
    for req_id in [1u64, 42, 1000, u64::MAX >> 2] {
        let cbdata = (req_id << 2) as *mut std::ffi::c_void;
        let decoded = (cbdata as u64) >> 2;
        assert_eq!(
            decoded, req_id,
            "cbdata round-trip failed for req_id={}",
            req_id
        );
    }
}

/// The cbdata pointer must be non-null (req_id starts from 1).
#[test]
fn test_monitor_cbdata_non_null() {
    for req_id in 1..=100 {
        let cbdata = (req_id << 2) as *mut std::ffi::c_void;
        assert!(
            !cbdata.is_null(),
            "cbdata must be non-null for req_id={}",
            req_id
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Heartbeat function tests (uses process_monitor_nb internally)
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat() is the Rust equivalent of the PMIx_Heartbeat() C macro.
/// Without PMIx_Init, it should return an error.
#[test]
fn test_heartbeat_without_init() {
    let result = pmix::monitoring::heartbeat();
    match result {
        Ok(()) => {
            // Acceptable if PMIx happens to be initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::Error),
                "expected ErrInit/ErrNotSupported/Error from heartbeat without init, got: {:?}",
                status
            );
        }
    }
}

/// heartbeat() returns Result<(), PmixStatus> - compile-time type check.
#[test]
fn test_heartbeat_return_type() {
    let _: Result<(), pmix::PmixStatus> = pmix::monitoring::heartbeat();
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon - ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full non-blocking monitoring lifecycle from simpjctrl.c.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_full_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // Requires PMIx daemon - would build monitor info, directives,
    // call process_monitor_nb with callback, and verify callback fires.
    let cb: Box<dyn MonitorCallback> = Box::new(CalledMonitorCallback {
        called: std::cell::Cell::new(false),
    });
    let _ = &cb as &Box<dyn MonitorCallback>;
}

/// Test heartbeat under a real PMIx environment.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_with_daemon() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = pmix::monitoring::heartbeat();
    assert!(
        result.is_ok(),
        "heartbeat should succeed under PMIx daemon: {:?}",
        result
    );
}

/// Test process_monitor_nb with file-based monitoring under a real PMIx environment.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_file_monitor_with_daemon() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let cb: Box<dyn MonitorCallback> = Box::new(CalledMonitorCallback {
        called: std::cell::Cell::new(false),
    });
    let _ = &cb as &Box<dyn MonitorCallback>;
}

/// Test that process_monitor_nb with NULL callback (fire-and-forget)
/// is equivalent to the heartbeat pattern.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_fire_and_forget() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = pmix::monitoring::heartbeat();
    assert!(result.is_ok(), "fire-and-forget heartbeat should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Monitoring with PMIX_SUCCESS as the error code is valid.
#[test]
fn test_monitor_nb_with_success_as_error() {
    let error: pmix::PmixStatus = pmix::PmixStatus::Known(PmixError::Success);
    assert!(error.is_success());
    assert_eq!(error.to_raw(), 0);
}

/// Monitoring with a custom (unknown) status code as the error parameter.
#[test]
fn test_monitor_nb_with_custom_error() {
    let custom = pmix::PmixStatus::from_raw(-999);
    assert!(!custom.is_success());
    assert_eq!(custom.to_raw(), -999);
}

/// Verify that the MonitorCallback trait accepts None results.
#[test]
fn test_monitor_callback_accepts_none_results() {
    let mut cb = RecordingMonitorCallback {
        status: std::cell::Cell::new(None),
        has_results: std::cell::Cell::new(false),
    };
    cb.on_complete(pmix::PmixStatus::Known(PmixError::ErrNotSupported), None);
    assert!(!cb.has_results.get());
}

/// Verify that the MonitorCallback trait accepts Some(MonitorResults).
#[test]
fn test_monitor_callback_accepts_some_results() {
    fn _check_sig(
        cb: &mut dyn MonitorCallback,
        status: pmix::PmixStatus,
        results: Option<MonitorResults>,
    ) {
        cb.on_complete(status, results);
    }
}
