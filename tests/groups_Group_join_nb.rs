//! Integration tests for `PMIx_Group_join_nb` via the safe `group_join_nb()` wrapper.

use pmix::groups::{group_join_nb, GroupJoinCallbackWrapper, pmix_group_opt_t};
use pmix::{Info, Proc};

#[test]
fn group_join_nb_compiles() {
    let wrapper = GroupJoinCallbackWrapper::new(|_status, _info| {});
    let proc = Proc::new("test_ns", 0).unwrap();
    let info: &[Info] = &[];
    let opt = pmix_group_opt_t::PMIX_GROUP_DECLINE;
    let result = group_join_nb("test_group", &proc, opt, info, wrapper);
    assert!(
        result.is_err(),
        "group_join_nb should fail without PMIx_Init"
    );
}

#[test]
fn group_join_callback_wrapper_new() {
    let _w: GroupJoinCallbackWrapper = GroupJoinCallbackWrapper::new(|_status, _info| {});
}

#[test]
fn group_join_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let wrapper = GroupJoinCallbackWrapper::new(move |_status, _info| {
        INVOKED.store(true, Ordering::SeqCst);
    });
    let proc = Proc::new("test_ns", 0).unwrap();
    let info: &[Info] = &[];
    let opt = pmix_group_opt_t::PMIX_GROUP_DECLINE;
    let result = group_join_nb("test_group", &proc, opt, info, wrapper);
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
