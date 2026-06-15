//! Integration tests for `PMIx_Register_event_handler_nb`.
//!
//! These tests verify the function compiles and returns errors without init.

use pmix::events::{register_event_handler_nb, HandlerRegCbFn, NotificationFn};
use pmix::InfoBuilder;

/// `register_event_handler_nb` compiles with correct signature.
#[test]
fn register_event_handler_nb_compiles() {
    let info = InfoBuilder::new().build();
    let result = register_event_handler_nb(
        &[],
        &info,
        None as NotificationFn,
        None as HandlerRegCbFn,
        std::ptr::null_mut(),
    );
    assert!(
        result.is_err(),
        "register_event_handler_nb should fail without PMIx_Init"
    );
}
