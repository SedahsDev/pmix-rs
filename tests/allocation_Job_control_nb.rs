//! Integration tests for `PMIx_Job_control_nb` via the safe `job_control_nb()` wrapper.

use pmix::PmixStatus;
use pmix::allocation::{JobControlCallback, JobControlResults, job_control_nb};

#[test]
fn job_control_nb_compiles() {
    struct TestCallback;
    impl JobControlCallback for TestCallback {
        fn on_complete(&self, _status: PmixStatus, _results: JobControlResults) {}
    }
    let result = job_control_nb(&[], &[], Box::new(TestCallback));
    assert!(
        result.is_err(),
        "job_control_nb should fail without PMIx_Init"
    );
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

#[test]
fn job_control_callback_trait_object() {
    struct TestCallback;
    impl JobControlCallback for TestCallback {
        fn on_complete(&self, _status: PmixStatus, _results: JobControlResults) {}
    }
    let _: Box<dyn JobControlCallback> = Box::new(TestCallback);
}
