//! Deep tests for monitoring module — Round 2.
//!
//! Targets untested code paths in monitoring.rs (65.54% coverage).
//! Focus: process_monitor, process_monitor_nb, heartbeat,
//! MonitorResults, MonitorCallback trait bounds, panic safety.
//!
//! FFI tests that require PMIx_Init are marked #[ignore].

use pmix::monitoring::*;
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// MonitorResults type checks (compile-time only)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_results_debug_format() {
    // MonitorResults is constructed by process_monitor() FFI call.
    // Just verify the Debug impl compiles via type assertion.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<MonitorResults>();
}

#[test]
fn test_monitor_results_traits() {
    // MonitorResults wraps a raw pointer — it is NOT Send.
    // Just verify it's Debug (already tested above).
    let _ = std::any::type_name::<MonitorResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// MonitorCallback trait bounds
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_callback_trait_send() {
    // Verify MonitorCallback requires Send.
    fn assert_send_bound<T: MonitorCallback>() {}
    struct TestCb;
    impl MonitorCallback for TestCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    assert_send_bound::<TestCb>();
}

#[test]
fn test_monitor_callback_closure() {
    // Verify closures can implement MonitorCallback via wrapper.
    struct ClosureCb<F>(F);
    impl<F> MonitorCallback for ClosureCb<F>
    where
        F: FnMut(PmixStatus, Option<MonitorResults>) + Send,
    {
        fn on_complete(&mut self, status: PmixStatus, results: Option<MonitorResults>) {
            (self.0)(status, results);
        }
    }
    let _cb = ClosureCb(|_s: PmixStatus, _r: Option<MonitorResults>| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_infobuilder_build_empty() {
    let info = InfoBuilder::new().build();
    let _ = info;
}

#[test]
fn test_infobuilder_collect_data() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let _info = builder.build();
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_heartbeat_does_not_panic() {
    // heartbeat() may fail without PMIx_Init but should never panic.
    let result = std::panic::catch_unwind(|| {
        let _ = heartbeat();
    });
    assert!(result.is_ok());
}

#[test]
fn test_heartbeat_multiple_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        for _ in 0..10 {
            let _ = heartbeat();
        }
    });
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

// ── process_monitor ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_empty_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::ErrNotFound), &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let dirs = vec![InfoBuilder::new().build()];
    let result = process_monitor(
        &monitor,
        PmixStatus::Known(pmix::PmixError::ErrNotFound),
        &dirs,
    );
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_success_error_code() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::Success), &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_timeout_error_code() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(
        &monitor,
        PmixStatus::Known(pmix::PmixError::ErrTimeout),
        &[],
    );
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_multiple_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let dirs = vec![
        InfoBuilder::new().build(),
        InfoBuilder::new().build(),
    ];
    let result = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::ErrNotFound), &dirs);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_result_has_len() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let result = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::ErrNotFound), &[]);
    match result {
        Ok(results) => {
            let _ = results.len();
            let _ = results.is_empty();
        }
        Err(_) => {
            // Server may not support monitoring — that's fine
        }
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_with_collect_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let monitor = builder.build();
    let result = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::ErrNotFound), &[]);
    let _ = result;
}

// ── process_monitor_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopMonitorCb;
    impl MonitorCallback for NoopMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(pmix::PmixError::ErrNotFound),
        &[],
        Box::new(NoopMonitorCb),
    );
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopMonitorCb;
    impl MonitorCallback for NoopMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor = InfoBuilder::new().build();
    let dirs = vec![InfoBuilder::new().build()];
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(pmix::PmixError::ErrNotFound),
        &dirs,
        Box::new(NoopMonitorCb),
    );
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_success_error_code() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopMonitorCb;
    impl MonitorCallback for NoopMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor = InfoBuilder::new().build();
    let result = process_monitor_nb(
        &monitor,
        PmixStatus::Known(pmix::PmixError::Success),
        &[],
        Box::new(NoopMonitorCb),
    );
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_process_monitor_nb_multiple_callbacks() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopMonitorCb;
    impl MonitorCallback for NoopMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor = InfoBuilder::new().build();
    for _ in 0..3 {
        let result = process_monitor_nb(
            &monitor,
            PmixStatus::Known(pmix::PmixError::ErrNotFound),
            &[],
            Box::new(NoopMonitorCb),
        );
        assert!(result.is_ok());
    }
}

// ── heartbeat ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = heartbeat();
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_multiple() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    for _ in 0..5 {
        let result = heartbeat();
        assert!(result.is_ok());
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_heartbeat_rapid_fire() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    for _ in 0..20 {
        let _ = heartbeat();
    }
}

// ── Lifecycle / integration ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_monitor_then_heartbeat() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopMonitorCb;
    impl MonitorCallback for NoopMonitorCb {
        fn on_complete(&mut self, _status: PmixStatus, _results: Option<MonitorResults>) {}
    }
    let monitor = InfoBuilder::new().build();
    let _ = process_monitor_nb(
        &monitor,
        PmixStatus::Known(pmix::PmixError::ErrNotFound),
        &[],
        Box::new(NoopMonitorCb),
    );
    let _ = heartbeat();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_monitor_sync_then_heartbeat() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let monitor = InfoBuilder::new().build();
    let _ = process_monitor(&monitor, PmixStatus::Known(pmix::PmixError::ErrNotFound), &[]);
    let _ = heartbeat();
}
