//! Integration tests for `PMIx_Log_data_nb` via the safe `log_data_nb()` wrapper.

use pmix::query_log::{log_data_nb, LogCallback};
use pmix::PmixStatus;

#[test]
fn log_data_nb_compiles() {
    struct TestCallback;
    impl LogCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = log_data_nb(&[], &[], Box::new(TestCallback));
    assert!(result.is_err(), "log_data_nb should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

#[test]
fn log_callback_trait_object() {
    struct TestCallback;
    impl LogCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _: Box<dyn LogCallback> = Box::new(TestCallback);
}

#[test]
fn log_data_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    struct NoInvokeCallback;
    impl LogCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            INVOKED.store(true, Ordering::SeqCst);
        }
    }
    let result = log_data_nb(&[], &[], Box::new(NoInvokeCallback));
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
