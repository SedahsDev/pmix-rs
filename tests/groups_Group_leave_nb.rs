//! Integration tests for `PMIx_Group_leave_nb` via the safe `group_leave_nb()` wrapper.

use pmix::groups::{GroupLeaveCallbackWrapper, group_leave_nb};

#[test]
fn group_leave_nb_compiles() {
    let wrapper = GroupLeaveCallbackWrapper::new(|_status| {});
    let result = group_leave_nb("test_group", &[], wrapper);
    assert!(
        result.is_err(),
        "group_leave_nb should fail without PMIx_Init"
    );
}

#[test]
fn group_leave_callback_wrapper_new() {
    let _w: GroupLeaveCallbackWrapper = GroupLeaveCallbackWrapper::new(|_status| {});
}

#[test]
fn group_leave_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let wrapper = GroupLeaveCallbackWrapper::new(move |_status| {
        INVOKED.store(true, Ordering::SeqCst);
    });
    let result = group_leave_nb("test_group", &[], wrapper);
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
