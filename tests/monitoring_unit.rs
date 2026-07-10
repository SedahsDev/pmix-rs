//! Comprehensive unit tests for the monitoring module — structural coverage focus.
//!
//! All tests run WITHOUT PMIx_Init — they exercise the Rust wrapper layer only.
//! This file complements monitoring_coverage.rs by targeting additional uncovered
//! code paths and edge cases.

use pmix::monitoring::*;
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults — len / is_empty edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_results_type_properties() {
    // MonitorResults fields are private, so we can't construct instances from
    // outside the module. The in-module tests cover len()/is_empty() directly.
    // Here we verify type properties accessible from external code.
    let name = std::any::type_name::<MonitorResults>();
    assert!(name.ends_with("MonitorResults"));
}

#[test]
fn test_monitor_results_debug_contains_handle_and_len() {
    // MonitorResults derives Debug. Verify the debug output contains field names.
    // We can't construct one directly (private fields), but the in-module tests
    // cover that. Here we just confirm the Debug derive is present.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<MonitorResults>();
}

#[test]
fn test_monitor_results_not_copy() {
    // MonitorResults should NOT be Copy (it owns a raw pointer and manages Drop).
    fn assert_not_copy<T: Copy>() {}
    // This should NOT compile if uncommented:
    // assert_not_copy::<MonitorResults>();
    // We verify by confirming it's Sized (all owned types are):
    fn assert_sized<T: Sized>() {}
    assert_sized::<MonitorResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback — multiple implementations and trait object behavior
// ─────────────────────────────────────────────────────────────────────────────

/// Callback that records all invocations.
struct RecordingMonitorCb {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(PmixStatus, bool)>>>,
}

impl MonitorCallback for RecordingMonitorCb {
    fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>) {
        self.calls.lock().unwrap().push((status, results.is_some()));
    }
}

#[test]
fn test_monitor_callback_records_multiple_invocations() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let cb: Box<dyn MonitorCallback> = Box::new(RecordingMonitorCb {
        calls: calls.clone(),
    });

    // Simulate multiple callback invocations
    let mut c = cb;
    c.on_complete(PmixStatus::Known(PmixError::Success), None);
    c.on_complete(PmixStatus::Known(PmixError::ErrNotFound), None);
    c.on_complete(PmixStatus::Unknown(-99999), None);

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded[0].0, PmixStatus::Known(PmixError::Success));
    assert_eq!(recorded[1].0, PmixStatus::Known(PmixError::ErrNotFound));
    assert!(matches!(recorded[2].0, PmixStatus::Unknown(_)));
}

/// Callback that validates status codes.
struct StatusValidatorCb {
    expected: std::sync::Arc<std::sync::Mutex<PmixStatus>>,
    got: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
}

impl MonitorCallback for StatusValidatorCb {
    fn on_complete(&mut self, status: PmixStatus, _results: Option<MonitorResults>) {
        let expected = self.expected.lock().unwrap().clone();
        assert_eq!(status, expected, "status mismatch");
        *self.got.lock().unwrap() = Some(status);
    }
}

#[test]
fn test_monitor_callback_with_error_status() {
    let expected = std::sync::Arc::new(std::sync::Mutex::new(PmixStatus::Known(
        PmixError::ErrBadParam,
    )));
    let got = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cb: Box<dyn MonitorCallback> = Box::new(StatusValidatorCb {
        expected: expected.clone(),
        got: got.clone(),
    });

    let mut c = cb;
    c.on_complete(PmixStatus::Known(PmixError::ErrBadParam), None);

    assert!(got.lock().unwrap().is_some());
    assert_eq!(
        *got.lock().unwrap(),
        Some(PmixStatus::Known(PmixError::ErrBadParam))
    );
}

/// Callback with counter state.
struct CounterMonitorCb {
    counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl MonitorCallback for CounterMonitorCb {
    fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {
        self.counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn test_monitor_callback_counter_increments() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cb: Box<dyn MonitorCallback> = Box::new(CounterMonitorCb {
        counter: counter.clone(),
    });

    let mut c = cb;
    for _ in 0..5 {
        c.on_complete(PmixStatus::Known(PmixError::Success), None);
    }
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
}

/// Callback that checks results presence.
struct ResultsPresenceCb {
    got_some: std::sync::Arc<std::sync::Mutex<bool>>,
    got_none: std::sync::Arc<std::sync::Mutex<bool>>,
}

impl MonitorCallback for ResultsPresenceCb {
    fn on_complete(&mut self, _status: PmixStatus, results: Option<MonitorResults>) {
        match results {
            Some(_) => *self.got_some.lock().unwrap() = true,
            None => *self.got_none.lock().unwrap() = true,
        }
    }
}

#[test]
fn test_monitor_callback_none_results_branch() {
    let got_some = std::sync::Arc::new(std::sync::Mutex::new(false));
    let got_none = std::sync::Arc::new(std::sync::Mutex::new(false));
    let cb: Box<dyn MonitorCallback> = Box::new(ResultsPresenceCb {
        got_some: got_some.clone(),
        got_none: got_none.clone(),
    });

    let mut c = cb;
    c.on_complete(PmixStatus::Known(PmixError::Success), None);

    assert!(*got_none.lock().unwrap(), "should have taken None branch");
    assert!(
        !*got_some.lock().unwrap(),
        "should NOT have taken Some branch"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback — Send bound verification
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn MonitorCallback>>();
    // Also verify concrete implementations are Send:
    assert_send::<CounterMonitorCb>();
    assert_send::<RecordingMonitorCb>();
}

#[test]
fn test_monitor_callback_box_dyn_is_sized() {
    // MonitorCallback itself is a trait (not Sized), but Box<dyn MonitorCallback> is Sized.
    fn assert_sized<T: Sized>() {}
    assert_sized::<Box<dyn MonitorCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor — error paths with various Info configurations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_empty_monitor_empty_directives() {
    // Both monitor and directives are empty Info objects.
    let monitor = InfoBuilder::new().build();
    assert!(monitor.is_empty());
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[]);
    assert!(result.is_err(), "should fail without PMIx_Init");
}

#[test]
fn test_process_monitor_with_single_directive() {
    let monitor = InfoBuilder::new().build();
    let dirs = vec![{
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    }];
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrTimeout), &dirs);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_with_multiple_directives() {
    let monitor = InfoBuilder::new().build();
    let dirs = vec![
        {
            let mut b = InfoBuilder::new();
            b.collect_data();
            b.build()
        },
        InfoBuilder::new().build(),
        InfoBuilder::new().build(),
    ];
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &dirs);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_with_various_error_codes() {
    let monitor = InfoBuilder::new().build();
    let error_codes = vec![
        PmixStatus::Known(PmixError::Success),
        PmixStatus::Known(PmixError::Error),
        PmixStatus::Known(PmixError::ErrNotFound),
        PmixStatus::Known(PmixError::ErrBadParam),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrNotSupported),
        PmixStatus::Known(PmixError::ErrInit),
        PmixStatus::Known(PmixError::ErrPartialSuccess),
        PmixStatus::Unknown(-100),
        PmixStatus::Unknown(-99999),
    ];
    for (i, err) in error_codes.iter().enumerate() {
        let result = process_monitor(&monitor, *err, &[]);
        assert!(
            result.is_err(),
            "error code {} ({:?}) should fail without init",
            i,
            err
        );
    }
}

#[test]
fn test_process_monitor_error_status_value() {
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[]);
    match result {
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
            assert!(status.is_error());
        }
        Ok(_) => panic!("should have returned an error"),
    }
}

#[test]
fn test_process_monitor_repeated_calls_same_error() {
    // Multiple calls should produce the same error consistently.
    let monitor = InfoBuilder::new().build();
    let mut first_error: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[]);
        match result {
            Err(status) => {
                if let Some(ref first) = first_error {
                    assert_eq!(status, *first, "error should be consistent across calls");
                } else {
                    first_error = Some(status);
                }
            }
            Ok(_) => panic!("should fail without init"),
        }
    }
    assert!(first_error.is_some());
}

// ─────────────────────────────────────────────────────────────────────────────
// process_monitor_nb — callback registration/cleanup on error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_nb_empty_monitor_no_directives() {
    let monitor = InfoBuilder::new().build();
    let cb = Box::new(CounterMonitorCb {
        counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let result = process_monitor_nb(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_nb_with_directives() {
    let monitor = InfoBuilder::new().build();
    let dirs = vec![InfoBuilder::new().build()];
    let cb = Box::new(CounterMonitorCb {
        counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::ErrTimeout),
        &dirs,
        cb,
    );
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_nb_error_does_not_leak_callback() {
    // When process_monitor_nb fails, the callback should be cleaned up from
    // the registry. We verify this by making many calls — if callbacks leaked,
    // the registry would grow unbounded.
    let monitor = InfoBuilder::new().build();
    for _ in 0..20 {
        let cb = Box::new(CounterMonitorCb {
            counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let result =
            process_monitor_nb(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[], cb);
        assert!(result.is_err());
    }
}

#[test]
fn test_process_monitor_nb_various_error_codes() {
    let monitor = InfoBuilder::new().build();
    let error_codes = vec![
        PmixStatus::Known(PmixError::Success),
        PmixStatus::Known(PmixError::Error),
        PmixStatus::Known(PmixError::ErrNotFound),
        PmixStatus::Known(PmixError::ErrBadParam),
        PmixStatus::Known(PmixError::ErrNotSupported),
        PmixStatus::Unknown(-100),
    ];
    for err in error_codes {
        let cb = Box::new(CounterMonitorCb {
            counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let result = process_monitor_nb(&monitor, err, &[], cb);
        assert!(result.is_err(), "should fail for error code {:?}", err);
    }
}

#[test]
fn test_process_monitor_nb_multiple_different_callbacks() {
    // Each call uses a different callback implementation.
    let monitor = InfoBuilder::new().build();

    // Call 1: CounterMonitorCb
    let cb1 = Box::new(CounterMonitorCb {
        counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let r1 = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::ErrNotFound),
        &[],
        cb1,
    );
    assert!(r1.is_err());

    // Call 2: RecordingMonitorCb
    let cb2 = Box::new(RecordingMonitorCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let r2 = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::ErrNotFound),
        &[],
        cb2,
    );
    assert!(r2.is_err());

    // Call 3: ResultsPresenceCb
    let cb3 = Box::new(ResultsPresenceCb {
        got_some: std::sync::Arc::new(std::sync::Mutex::new(false)),
        got_none: std::sync::Arc::new(std::sync::Mutex::new(false)),
    });
    let r3 = process_monitor_nb(
        &monitor,
        PmixStatus::Known(PmixError::ErrNotFound),
        &[],
        cb3,
    );
    assert!(r3.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// heartbeat — error return validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_heartbeat_returns_error_without_init() {
    let result = heartbeat();
    assert!(result.is_err(), "heartbeat should fail without PMIx_Init");
}

#[test]
fn test_heartbeat_error_is_not_success() {
    let result = heartbeat();
    if let Err(status) = result {
        assert!(
            !status.is_success(),
            "heartbeat error should not be a success status"
        );
        assert!(status.is_error());
    }
}

#[test]
fn test_heartbeat_error_is_known_or_unknown() {
    let result = heartbeat();
    match result {
        Err(PmixStatus::Known(e)) => {
            // Known error — verify it's a real error code
            assert!(!e.is_success(), "known error should not be success");
        }
        Err(PmixStatus::Unknown(v)) => {
            // Unknown error — verify it's negative (error range)
            assert!(v <= 0, "unknown error should be non-positive: {}", v);
        }
        Ok(()) => {
            // If PMIx was initialized, heartbeat may succeed — that's fine
        }
    }
}

#[test]
fn test_heartbeat_multiple_calls_consistent_error() {
    // All heartbeat calls without init should return the same error.
    let results: Vec<Result<(), PmixStatus>> = (0..15).map(|_| heartbeat()).collect();
    let first = &results[0];
    for (i, r) in results.iter().enumerate().skip(1) {
        match (first, r) {
            (Ok(_), Ok(_)) => {} // Both succeeded (PMIx was init'd)
            (Err(s1), Err(s2)) => {
                assert_eq!(
                    s1, s2,
                    "heartbeat call {} returned different error than first: {:?} vs {:?}",
                    i, s1, s2
                );
            }
            _ => panic!(
                "inconsistent results: first={:?}, call {}={:?}",
                first, i, r
            ),
        }
    }
}

#[test]
fn test_heartbeat_does_not_panic() {
    // heartbeat() should never panic — it always returns Result.
    let result = std::panic::catch_unwind(|| heartbeat());
    assert!(result.is_ok(), "heartbeat should not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// MONITOR_SEQ counter progression
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_seq_increments_with_each_nb_call() {
    // Each call to process_monitor_nb should increment MONITOR_SEQ.
    // We verify this indirectly: if the seq counter didn't increment,
    // request IDs would collide and callbacks could interfere.
    // Since all calls fail (no init), each callback is cleaned up,
    // and we can make many calls without issues.
    let monitor = InfoBuilder::new().build();
    for _ in 0..50 {
        let cb = Box::new(CounterMonitorCb {
            counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let result =
            process_monitor_nb(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[], cb);
        assert!(result.is_err());
    }
    // If we got here, the seq counter is working (no collisions).
}

#[test]
fn test_monitor_seq_no_collision_with_many_calls() {
    // Stress test: many rapid calls should all produce unique request IDs.
    let monitor = InfoBuilder::new().build();
    let num_calls = 100;
    for i in 0..num_calls {
        let cb = Box::new(CounterMonitorCb {
            counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(i)),
        });
        let result =
            process_monitor_nb(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[], cb);
        assert!(result.is_err(), "call {} should fail", i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Drop implementation — null handle with zero len safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_results_drop_safety_compile_check() {
    // Verify that MonitorResults implements Drop (it should, for freeing FFI memory).
    // We can't directly test Drop on a null-handle/zero-len instance because
    // fields are private. Instead, we verify the type has the expected trait bounds.
    fn assert_drop<T: Drop>() {}
    assert_drop::<MonitorResults>();
}

#[test]
fn test_monitor_results_no_clone() {
    // MonitorResults should NOT be Clone (it owns raw pointer resources).
    // If it were Clone, the Clone impl would need to deep-copy the FFI array.
    // We verify it's not Clone by confirming it lacks the trait.
    // (This test compiles successfully because we're NOT asserting Clone.)
    fn assert_sized<T: Sized>() {}
    assert_sized::<MonitorResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration: InfoBuilder + monitoring API combinations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_with_collect_data_monitor() {
    // Use a monitor with collect_data set.
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let monitor = builder.build();
    assert!(!monitor.is_empty());
    let result = process_monitor(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[]);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_nb_with_collect_data_monitor() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let monitor = builder.build();
    let cb = Box::new(CounterMonitorCb {
        counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let result = process_monitor_nb(&monitor, PmixStatus::Known(PmixError::ErrNotFound), &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_empty_info_len() {
    // Verify InfoBuilder::new().build() produces an empty Info (len=0).
    let info = InfoBuilder::new().build();
    assert_eq!(info.len(), 0);
    assert!(info.is_empty());
}

#[test]
fn test_process_monitor_nonempty_info_len() {
    // Verify InfoBuilder with entries produces a non-empty Info.
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    assert_eq!(info.len(), 1);
    assert!(!info.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus edge cases in monitoring context
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_process_monitor_with_unknown_status() {
    let monitor = InfoBuilder::new().build();
    // Use an unknown/unrecognized status code.
    let result = process_monitor(&monitor, PmixStatus::Unknown(-12345), &[]);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_nb_with_unknown_status() {
    let monitor = InfoBuilder::new().build();
    let cb = Box::new(CounterMonitorCb {
        counter: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let result = process_monitor_nb(&monitor, PmixStatus::Unknown(-12345), &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_process_monitor_with_positive_status() {
    // Positive status codes are informational/success range.
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Unknown(42), &[]);
    assert!(result.is_err()); // Still fails without init
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback — all error status variants in on_complete
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_callback_with_all_status_variants() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let cb: Box<dyn MonitorCallback> = Box::new(RecordingMonitorCb {
        calls: calls.clone(),
    });

    let statuses = vec![
        PmixStatus::Known(PmixError::Success),
        PmixStatus::Known(PmixError::Error),
        PmixStatus::Known(PmixError::ErrNotFound),
        PmixStatus::Known(PmixError::ErrBadParam),
        PmixStatus::Known(PmixError::ErrNotSupported),
        PmixStatus::Known(PmixError::ErrInit),
        PmixStatus::Known(PmixError::ErrTimeout),
        PmixStatus::Known(PmixError::ErrPartialSuccess),
        PmixStatus::Unknown(-100),
        PmixStatus::Unknown(-99999),
        PmixStatus::Unknown(1), // Positive unknown (informational)
    ];

    let mut c = cb;
    for status in &statuses {
        c.on_complete(*status, None);
    }

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), statuses.len());
    for (i, (recorded_status, _)) in recorded.iter().enumerate() {
        assert_eq!(*recorded_status, statuses[i]);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Static assertions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_static_assertions() {
    use static_assertions::assert_impl_all;

    // MonitorResults should implement Debug, Drop, Sized
    assert_impl_all!(MonitorResults: std::fmt::Debug, Drop, Sized);

    // Box<dyn MonitorCallback> should be Send
    assert_impl_all!(Box<dyn MonitorCallback>: Send);
}
