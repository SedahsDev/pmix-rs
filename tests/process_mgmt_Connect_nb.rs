//! Integration tests for `PMIx_Connect_nb` via the safe `connect_nb()` wrapper.

use pmix::process_mgmt::{ConnectCallbackWrapper, connect_nb};

#[test]
fn connect_nb_compiles() {
    let wrapper = ConnectCallbackWrapper::new(|_status| {});
    let result = connect_nb(&[], &[], wrapper);
    assert!(result.is_err(), "connect_nb should fail without PMIx_Init");
}

#[test]
fn connect_callback_wrapper_new() {
    let _w: ConnectCallbackWrapper = ConnectCallbackWrapper::new(|_status| {});
}

#[test]
fn connect_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let wrapper = ConnectCallbackWrapper::new(move |_status| {
        INVOKED.store(true, Ordering::SeqCst);
    });
    let result = connect_nb(&[], &[], wrapper);
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
