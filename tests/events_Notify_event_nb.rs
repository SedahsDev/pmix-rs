//! Integration tests for `PMIx_Notify_event_nb`.

use pmix::events::{notify_event_nb, OpCbFn};
use pmix::{InfoBuilder, PmixDataRange, PmixError, PmixStatus, Proc};

#[test]
fn notify_event_nb_compiles() {
    let info = InfoBuilder::new().build();
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = notify_event_nb(
        PmixStatus::Known(PmixError::Success),
        &proc,
        PmixDataRange::Global,
        &info,
        None as OpCbFn,
        std::ptr::null_mut(),
    );
    assert!(
        result.is_err(),
        "notify_event_nb should fail without PMIx_Init"
    );
}
