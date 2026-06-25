//! Integration tests for `PMIx_Disconnect_nb` via the safe `disconnect_nb()` wrapper.

use pmix::PmixStatus;
use pmix::process_mgmt::{DisconnectCallbackWrapper, disconnect_nb};

#[test]
fn disconnect_nb_compiles() {
    let wrapper = DisconnectCallbackWrapper::new(|_status| {});
    let result = disconnect_nb(&[], &[], wrapper);
    assert!(
        result.is_err(),
        "disconnect_nb should fail without PMIx_Init"
    );
}

#[test]
fn disconnect_callback_wrapper_new() {
    let _w: DisconnectCallbackWrapper = DisconnectCallbackWrapper::new(|_status| {});
}

#[test]
fn disconnect_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let wrapper = DisconnectCallbackWrapper::new(move |_status| {
        INVOKED.store(true, Ordering::SeqCst);
    });
    let result = disconnect_nb(&[], &[], wrapper);
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}
