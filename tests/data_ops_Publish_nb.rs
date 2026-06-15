//! Integration tests for `PMIx_Publish_nb` via the safe `publish_nb()` wrapper.

use pmix::data_ops::{publish_nb, PublishCallback};
use pmix::{InfoBuilder, PmixStatus};

/// `publish_nb` function compiles with correct signature.
#[test]
fn publish_nb_compiles() {
    struct TestCallback;
    impl PublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(TestCallback));
    assert!(result.is_err(), "publish_nb should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `PublishCallback` trait is importable and can be used as a trait object.
#[test]
fn publish_callback_trait_object() {
    struct TestCallback;
    impl PublishCallback for TestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _: Box<dyn PublishCallback> = Box::new(TestCallback);
}

/// `publish_nb` callback is not invoked on immediate failure.
#[test]
fn publish_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    struct NoInvokeCallback;
    impl PublishCallback for NoInvokeCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            INVOKED.store(true, Ordering::SeqCst);
        }
    }
    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(NoInvokeCallback));
    assert!(result.is_err());
    assert!(!INVOKED.load(Ordering::SeqCst), "callback should not be invoked on failure");
}
