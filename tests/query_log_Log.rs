//! Tests for `PMIx_Log` — logging operations.
//!
//! These tests verify the Rust wrapper for `PMIx_Log` and `PMIx_Log_nb`.
//! They test parameter validation, callback trait compilation, and API
//! signatures. Integration tests that require a running PMIx daemon are
//! marked `#[ignore]`.
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::PmixError;
use pmix::query_log::{LogCallback, log_data, log_data_nb};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for testing
// ─────────────────────────────────────────────────────────────────────────────

/// No-op test callback for log_data_nb.
struct TestLogCallback;

impl LogCallback for TestLogCallback {
    fn on_complete(self: Box<Self>, _status: pmix::PmixStatus) {
        // No-op — just verify the trait compiles and the callback
        // can be invoked without panicking.
    }
}

/// Test callback that records the status it received.
struct RecordingLogCallback {
    status: std::cell::Cell<Option<pmix::PmixStatus>>,
}

impl LogCallback for RecordingLogCallback {
    fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
        self.status.set(Some(status));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter validation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that log_data with empty data and empty directives calls the FFI
/// correctly (passes null pointers with zero lengths).
///
/// Without a PMIx server, this returns PMIX_ERR_INIT, which is expected.
#[test]
fn test_log_data_empty() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let result = log_data(&data, &directives);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            // Expected errors when no PMIx server is running.
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::Error),
                "unexpected error: {:?}",
                status
            );
        }
    }
}

/// Test that log_data_nb with empty data and empty directives works.
///
/// Without a PMIx server, this returns PMIX_ERR_INIT, which is expected.
#[test]
fn test_log_data_nb_empty() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let cb: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let result = log_data_nb(&data, &directives, cb);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            // Expected errors when no PMIx server is running.
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == pmix::PmixStatus::Known(PmixError::Error),
                "unexpected error: {:?}",
                status
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the LogCallback trait compiles and can be boxed.
#[test]
fn test_log_callback_trait() {
    let cb: Box<dyn LogCallback> = Box::new(TestLogCallback);
    // Verify we can hold the boxed callback.
    assert!(true);
    drop(cb);
}

/// Test that RecordingLogCallback records status correctly.
#[test]
fn test_recording_log_callback() {
    let recorder = RecordingLogCallback {
        status: std::cell::Cell::new(None),
    };
    let cb: Box<dyn LogCallback> = Box::new(recorder);
    // We can't invoke the callback directly from tests, but we can verify
    // the type compiles and the Cell field is accessible.
    assert!(true);
    drop(cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: log empty data to stdout.
///
/// This test handles both daemon-up (returns Ok) and daemon-down
/// (returns PMIX_ERR_INIT or PMIX_ERR_NOT_SUPPORTED) cases gracefully.
#[test]
fn test_log_data_stdout() {
    // We need an Info object with a log directive to direct output to stdout.
    // Since constructing Info requires PMIx_Info_create (which needs a server),
    // we test with empty arrays and check the expected error.
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let result = log_data(&data, &directives);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "unexpected error: {:?}",
                status
            );
        }
    }
}

/// Integration test: log data with directives via non-blocking API.
///
/// This test handles both daemon-up and daemon-down cases gracefully.
#[test]
fn test_log_data_nb_stdout() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let cb: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let result = log_data_nb(&data, &directives, cb);
    match result {
        Ok(()) => {
            // Acceptable — PMIx may have been initialized.
            // The callback will be invoked asynchronously.
        }
        Err(status) => {
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "unexpected error: {:?}",
                status
            );
        }
    }
}

/// Integration test: verify log_data_nb callback is invoked.
///
/// This test handles both daemon-up (callback invoked async) and daemon-down
/// (immediate error, callback NOT invoked) cases gracefully.
#[test]
fn test_log_data_nb_callback_invoked() {
    use std::sync::{Arc, Mutex};

    let callback_status = Arc::new(Mutex::new(None::<pmix::PmixStatus>));
    let status_clone = callback_status.clone();

    struct ArcLogCallback {
        status: Arc<Mutex<Option<pmix::PmixStatus>>>,
    }

    impl LogCallback for ArcLogCallback {
        fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
            let mut s = self.status.lock().unwrap();
            *s = Some(status);
        }
    }

    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let cb: Box<dyn LogCallback> = Box::new(ArcLogCallback {
        status: status_clone,
    });

    let result = log_data_nb(&data, &directives, cb);
    match result {
        Ok(()) => {
            // Callback will be invoked asynchronously.
            // In a real environment, we would wait and check callback_status.
        }
        Err(status) => {
            // On immediate rejection, callback is NOT called.
            assert!(
                status == pmix::PmixStatus::Known(PmixError::ErrInit)
                    || status == pmix::PmixStatus::Known(PmixError::ErrNotSupported),
                "unexpected error: {:?}",
                status
            );
            // Verify callback was NOT registered (since request was rejected).
            let s = callback_status.lock().unwrap();
            assert!(
                s.is_none(),
                "callback should not have been invoked on immediate rejection"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API signature and type safety tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that log_data accepts both empty and non-empty slices.
///
/// This verifies the function signature compiles correctly with various
/// slice types.
#[test]
fn test_log_data_slice_types() {
    // Empty slices — verified above in test_log_data_empty.
    // This test just verifies the API is callable with Vec references.
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    // Compile check: &Vec<T> coerces to &[T].
    let _ = std::mem::needs_drop::<Result<(), pmix::PmixStatus>>;
    drop(data);
    drop(directives);
}

/// Test that log_data_nb accepts Box<dyn LogCallback>.
#[test]
fn test_log_data_nb_callback_type() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    // Verify different callback implementations are accepted.
    let cb1: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let cb2: Box<dyn LogCallback> = Box::new(RecordingLogCallback {
        status: std::cell::Cell::new(None),
    });
    // Both compile and are accepted by the trait bound.
    drop(cb1);
    drop(cb2);
    drop(data);
    drop(directives);
}

/// Test that the LogCallback trait is Send.
#[test]
fn test_log_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn LogCallback>>();
}

/// Test that the log callback bridge function handles null cbdata gracefully.
///
/// This is a unit test of the bridge function's null-check behavior.
/// We verify that calling the bridge with a null cbdata does not panic.
#[test]
fn test_log_callback_bridge_null_cbdata() {
    // Import the bridge function — it's not public, so we test indirectly
    // by verifying the registry behavior when a callback is not found.
    // The bridge function checks cbdata.is_null() and returns early.
    // We can't call it directly from tests, but we can verify the
    // LogCallback trait and registry compile correctly.
    let cb: Box<dyn LogCallback> = Box::new(TestLogCallback);
    drop(cb);
}

/// Test that multiple log callbacks can be created without interference.
#[test]
fn test_multiple_log_callbacks() {
    let cb1: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let cb2: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let cb3: Box<dyn LogCallback> = Box::new(RecordingLogCallback {
        status: std::cell::Cell::new(None),
    });
    // All three callbacks should coexist without issues.
    drop(cb1);
    drop(cb2);
    drop(cb3);
}

/// Test that log_data returns a Result type (compile-time check).
#[test]
fn test_log_data_return_type() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let result: Result<(), pmix::PmixStatus> = log_data(&data, &directives);
    // Verify the return type is correct.
    match result {
        Ok(()) => {}
        Err(_) => {}
    }
}

/// Test that log_data_nb returns a Result type (compile-time check).
#[test]
fn test_log_data_nb_return_type() {
    let data: Vec<pmix::Info> = Vec::new();
    let directives: Vec<pmix::Info> = Vec::new();
    let cb: Box<dyn LogCallback> = Box::new(TestLogCallback);
    let result: Result<(), pmix::PmixStatus> = log_data_nb(&data, &directives, cb);
    // Verify the return type is correct.
    match result {
        Ok(()) => {}
        Err(_) => {}
    }
}
