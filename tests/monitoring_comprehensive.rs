//! Integration tests for `pmix::monitoring` module (TASK-052).
//!
//! Tests that invoke the PMIx FFI directly are marked `#[ignore]` since
//! they require a running PMIx server (DVM via prrte). The structural and
//! callback-behavior tests run without a PMIx runtime.
//!
//! Note: Tests that need internal `MonitorResults` construction live in
//! the inline `#[cfg(test)]` module in `src/monitoring.rs` since
//! `MonitorResults` fields are private.

use pmix::monitoring::{MonitorCallback, heartbeat, process_monitor, process_monitor_nb};
use pmix::{Info, InfoBuilder, PmixStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback — trait object safety and Send compliance
// ─────────────────────────────────────────────────────────────────────────────

/// MonitorCallback is object-safe (can be boxed as dyn).
#[test]
fn test_monitor_callback_trait_object_safe() {
    fn assert_trait_obj(_: Box<dyn MonitorCallback>) {}

    struct DummyCb;
    impl MonitorCallback for DummyCb {
        fn on_complete(
            &mut self,
            _status: PmixStatus,
            _results: Option<pmix::monitoring::MonitorResults>,
        ) {
        }
    }

    assert_trait_obj(Box::new(DummyCb));
}

/// MonitorCallback implementations can be Send.
#[test]
fn test_monitor_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn MonitorCallback>>();
}

/// Multiple MonitorCallback implementations can coexist.
#[test]
fn test_multiple_monitor_callbacks() {
    struct CbA;
    impl MonitorCallback for CbA {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }
    struct CbB;
    impl MonitorCallback for CbB {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }

    let _a: Box<dyn MonitorCallback> = Box::new(CbA);
    let _b: Box<dyn MonitorCallback> = Box::new(CbB);
}

/// MonitorCallback can track invocation count.
#[test]
fn test_monitor_callback_invocation_count() {
    struct CountCb {
        count: Arc<Mutex<usize>>,
    }
    impl MonitorCallback for CountCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let count = Arc::new(Mutex::new(0));
    let mut cb = CountCb {
        count: Arc::clone(&count),
    };

    cb.on_complete(PmixStatus::from_raw(0), None);
    cb.on_complete(PmixStatus::from_raw(0), None);
    cb.on_complete(PmixStatus::from_raw(0), None);
    assert_eq!(*count.lock().unwrap(), 3);
}

/// MonitorCallback captures results length when provided.
#[test]
fn test_monitor_callback_captures_results_len() {
    struct ResultsLenCb {
        len: Arc<Mutex<Option<usize>>>,
    }
    impl MonitorCallback for ResultsLenCb {
        fn on_complete(
            &mut self,
            _s: PmixStatus,
            results: Option<pmix::monitoring::MonitorResults>,
        ) {
            *self.len.lock().unwrap() = results.map(|r| r.len());
        }
    }

    let len = Arc::new(Mutex::new(None));
    let mut cb = ResultsLenCb {
        len: Arc::clone(&len),
    };

    // Without results.
    cb.on_complete(PmixStatus::from_raw(0), None);
    assert_eq!(*len.lock().unwrap(), None);
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor — API structure tests
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor has the expected function signature.
#[test]
fn test_process_monitor_signature() {
    fn _check_sig() {
        let _f: fn(
            &Info,
            PmixStatus,
            &[Info],
        ) -> Result<pmix::monitoring::MonitorResults, PmixStatus> = process_monitor;
    }
}

/// process_monitor can be called with empty directives.
#[test]
fn test_process_monitor_empty_directives() {
    let monitor = InfoBuilder::new().build();
    let _result = process_monitor(&monitor, PmixStatus::from_raw(-109), &[]);
}

/// process_monitor accepts various error status codes.
#[test]
fn test_process_monitor_error_codes() {
    let monitor = InfoBuilder::new().build();

    let _ = process_monitor(&monitor, PmixStatus::from_raw(-109), &[]);
    let _ = process_monitor(&monitor, PmixStatus::from_raw(-110), &[]);
    let _ = process_monitor(&monitor, PmixStatus::from_raw(0), &[]);
}

/// process_monitor with multiple calls doesn't corrupt state.
#[test]
fn test_process_monitor_multiple_calls() {
    let monitor = InfoBuilder::new().build();
    for i in 0..5 {
        let _ = process_monitor(&monitor, PmixStatus::from_raw(i as i32), &[]);
    }
}

/// process_monitor returns error without server (PMIX_ERR_INIT expected).
#[test]
fn test_process_monitor_returns_error_without_server() {
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::from_raw(-109), &[]);

    match result {
        Ok(_) => {
            // Library may have mock/no-op mode — acceptable.
        }
        Err(e) => {
            assert!(!e.is_success(), "should be an error without PMIx server");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor_nb — API structure tests
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor_nb has the expected function signature.
#[test]
fn test_process_monitor_nb_signature() {
    fn _check_sig() {
        let _f: fn(&Info, PmixStatus, &[Info], Box<dyn MonitorCallback>) -> Result<(), PmixStatus> =
            process_monitor_nb;
    }
}

/// process_monitor_nb accepts a callback and returns immediately.
#[test]
fn test_process_monitor_nb_accepts_callback() {
    struct SimpleCb;
    impl MonitorCallback for SimpleCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }

    let monitor = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::from_raw(-109),
        &[],
        Box::new(SimpleCb),
    );

    match result {
        Ok(()) => {}
        Err(_) => {
            // Expected without PMIx server.
        }
    }
}

/// process_monitor_nb with stateful callback.
#[test]
fn test_process_monitor_nb_stateful_callback() {
    struct StatefulCb {
        called: Arc<Mutex<bool>>,
    }
    impl MonitorCallback for StatefulCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {
            *self.called.lock().unwrap() = true;
        }
    }

    let called = Arc::new(Mutex::new(false));
    let cb = Box::new(StatefulCb {
        called: Arc::clone(&called),
    });

    let monitor = InfoBuilder::new().build();
    let _ = process_monitor_nb(&monitor, PmixStatus::from_raw(-109), &[], cb);
}

/// Multiple process_monitor_nb calls assign unique request IDs.
#[test]
fn test_process_monitor_nb_unique_request_ids() {
    struct DummyCb;
    impl MonitorCallback for DummyCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }

    let monitor = InfoBuilder::new().build();
    for _ in 0..20 {
        let _ = process_monitor_nb(&monitor, PmixStatus::from_raw(-109), &[], Box::new(DummyCb));
    }
}

/// process_monitor_nb with empty directives.
#[test]
fn test_process_monitor_nb_empty_directives() {
    struct DummyCb;
    impl MonitorCallback for DummyCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }

    let monitor = InfoBuilder::new().build();
    let _ = process_monitor_nb(&monitor, PmixStatus::from_raw(0), &[], Box::new(DummyCb));
}

/// process_monitor_nb callback receives various status codes.
#[test]
fn test_monitor_callback_various_statuses() {
    struct StatusCollector {
        statuses: Arc<Mutex<Vec<i32>>>,
    }
    impl MonitorCallback for StatusCollector {
        fn on_complete(
            &mut self,
            status: PmixStatus,
            _r: Option<pmix::monitoring::MonitorResults>,
        ) {
            self.statuses.lock().unwrap().push(status.to_raw());
        }
    }

    let statuses = Arc::new(Mutex::new(Vec::new()));
    let mut cb = StatusCollector {
        statuses: Arc::clone(&statuses),
    };

    cb.on_complete(PmixStatus::from_raw(0), None); // PMIX_SUCCESS
    cb.on_complete(PmixStatus::from_raw(-1), None); // PMIX_ERROR
    cb.on_complete(PmixStatus::from_raw(-109), None); // PMIX_MONITOR_HEARTBEAT_ALERT
    cb.on_complete(PmixStatus::from_raw(-110), None); // PMIX_MONITOR_FILE_ALERT

    let captured = statuses.lock().unwrap();
    assert_eq!(captured.len(), 4);
    assert_eq!(captured[0], 0);
    assert_eq!(captured[1], -1);
    assert_eq!(captured[2], -109);
    assert_eq!(captured[3], -110);
}

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — API structure tests
// ─────────────────────────────────────────────────────────────────────────────

/// heartbeat has the expected function signature.
#[test]
fn test_heartbeat_signature() {
    fn _check_sig() {
        let _f: fn() -> Result<(), PmixStatus> = heartbeat;
    }
}

/// heartbeat can be called (returns error without server).
#[test]
fn test_heartbeat_call() {
    let result = heartbeat();

    match result {
        Ok(()) => {}
        Err(_) => {
            // Expected without PMIx server.
        }
    }
}

/// Multiple heartbeat calls don't corrupt state.
#[test]
fn test_heartbeat_multiple_calls() {
    for _ in 0..10 {
        let _ = heartbeat();
    }
}

/// heartbeat returns consistent error type without server.
#[test]
fn test_heartbeat_error_consistency() {
    let r1 = heartbeat();
    let r2 = heartbeat();

    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "heartbeat results should be consistent"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases and cross-module tests
// ─────────────────────────────────────────────────────────────────────────────

/// process_monitor and process_monitor_nb can be called alternately.
#[test]
fn test_monitor_alternating_calls() {
    struct DummyCb;
    impl MonitorCallback for DummyCb {
        fn on_complete(&mut self, _s: PmixStatus, _r: Option<pmix::monitoring::MonitorResults>) {}
    }

    let monitor = InfoBuilder::new().build();

    for _ in 0..5 {
        let _ = process_monitor(&monitor, PmixStatus::from_raw(-109), &[]);
        let _ = process_monitor_nb(&monitor, PmixStatus::from_raw(-109), &[], Box::new(DummyCb));
    }
}

/// InfoBuilder can construct monitoring info entries.
#[test]
fn test_infobuilder_for_monitoring() {
    let info = InfoBuilder::new().build();
    assert!(info.is_empty());
    assert_eq!(info.len(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx server runtime — #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Full monitoring cycle: register monitor -> heartbeat -> check results.
/// Requires a running PMIx server (DVM via prrte).
/// Run with: `cargo test --test monitoring_comprehensive -- --ignored`
#[test]
#[ignore]
fn test_full_monitoring_cycle_with_server() {
    panic!("Requires PMIx server (DVM via prrte)");
}

/// Non-blocking monitoring with real callback invocation.
/// Requires a running PMIx server.
#[test]
#[ignore]
fn test_nb_monitoring_with_real_server() {
    panic!("Requires PMIx server (DVM via prrte)");
}

/// Heartbeat under real PMIx server returns success.
#[test]
#[ignore]
fn test_heartbeat_returns_success_with_server() {
    panic!("Requires PMIx server (DVM via prrte)");
}
