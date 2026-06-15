//! Integration tests for `PMIx_Lookup_nb` via the safe `lookup_nb()` wrapper.

use pmix::data_ops::{lookup_nb, LookupCallback, PmixPdata};
use pmix::PmixStatus;

#[test]
fn lookup_nb_compiles() {
    struct TestCallback;
    impl LookupCallback for TestCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let result = lookup_nb(&["key1"], None, Box::new(TestCallback));
    assert!(result.is_err(), "lookup_nb should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

#[test]
fn lookup_callback_trait_object() {
    struct TestCallback;
    impl LookupCallback for TestCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let _: Box<dyn LookupCallback> = Box::new(TestCallback);
}

#[test]
fn lookup_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    struct NoInvokeCallback;
    impl LookupCallback for NoInvokeCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {
            INVOKED.store(true, Ordering::SeqCst);
        }
    }
    let result = lookup_nb(&["key1"], None, Box::new(NoInvokeCallback));
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
