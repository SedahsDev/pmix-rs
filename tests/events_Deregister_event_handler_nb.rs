//! Integration tests for `PMIx_Deregister_event_handler_nb`.

use pmix::events::{OpCbFn, deregister_event_handler_nb};

/// `deregister_event_handler_nb` compiles with correct signature.
#[test]
fn deregister_event_handler_nb_compiles() {
    let result = deregister_event_handler_nb(0, None as OpCbFn, std::ptr::null_mut());
    assert!(
        result.is_err(),
        "deregister_event_handler_nb should fail without PMIx_Init"
    );
}
