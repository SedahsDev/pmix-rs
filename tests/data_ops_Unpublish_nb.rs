//! Integration tests for `PMIx_Unpublish_nb` via the safe `unpublish_nb()` wrapper.

use pmix::data_ops::{unpublish_nb, UnpublishCallback};
use pmix::PmixStatus;

#[test]
fn unpublish_nb_compiles() {
    struct TestCallback;
    impl UnpublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = unpublish_nb(None, None, Box::new(TestCallback));
    assert!(result.is_err(), "unpublish_nb should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

#[test]
fn unpublish_callback_trait_object() {
    struct TestCallback;
    impl UnpublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _: Box<dyn UnpublishCallback> = Box::new(TestCallback);
}

#[test]
fn unpublish_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    struct NoInvokeCallback;
    impl UnpublishCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            INVOKED.store(true, Ordering::SeqCst);
        }
    }
    let result = unpublish_nb(None, None, Box::new(NoInvokeCallback));
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
