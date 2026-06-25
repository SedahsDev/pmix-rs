//! Integration tests for `PMIx_Query_info_nb` via the safe `query_info_nb()` wrapper.

use pmix::PmixStatus;
use pmix::query_log::{QueryCallback, QueryResults, query_info_nb};

#[test]
fn query_info_nb_compiles() {
    struct TestCallback;
    impl QueryCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let result = query_info_nb(&[], Box::new(TestCallback));
    assert!(
        result.is_err(),
        "query_info_nb should fail without PMIx_Init"
    );
}

#[test]
fn query_callback_trait_object() {
    struct TestCallback;
    impl QueryCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let _: Box<dyn QueryCallback> = Box::new(TestCallback);
}

#[test]
fn query_info_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    struct NoInvokeCallback;
    impl QueryCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
            INVOKED.store(true, Ordering::SeqCst);
        }
    }
    let result = query_info_nb(&[], Box::new(NoInvokeCallback));
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
