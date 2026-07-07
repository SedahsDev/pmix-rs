//! Integration tests for `PMIx_Spawn_nb` via the safe `spawn_nb()` wrapper.

use pmix::process_mgmt::{PmixApp, SpawnCallbackWrapper, spawn_nb};

#[test]
fn spawn_nb_compiles() {
    let wrapper = SpawnCallbackWrapper::new(|_status, _nspace| {});
    let result = spawn_nb(&[], &[], wrapper);
    assert!(result.is_err(), "spawn_nb should fail without PMIx_Init");
}

#[test]
fn spawn_callback_wrapper_new() {
    let _w: SpawnCallbackWrapper = SpawnCallbackWrapper::new(|_status, _nspace| {});
}

#[test]
fn spawn_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let wrapper = SpawnCallbackWrapper::new(move |_status, _nspace| {
        INVOKED.store(true, Ordering::SeqCst);
    });
    let result = spawn_nb(&[], &[], wrapper);
    assert!(result.is_err());
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "callback should not be invoked on failure"
    );
}

#[test]
fn spawn_nb_with_app() {
    let wrapper = SpawnCallbackWrapper::new(|_status, _nspace| {});
    let app = PmixApp::builder().cmd("/bin/true").build().unwrap();
    let result = spawn_nb(&[], &[app], wrapper);
    assert!(result.is_err(), "spawn_nb should fail without PMIx_Init");
}
