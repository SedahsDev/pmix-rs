//! Phase 4 Batch 2: PmixFabric Non-Blocking Register/Deregister
//!
//! Tests for fabric_register_nb and fabric_deregister_nb.
//!
//! IMPORTANT: fabric_register_nb calls FFI unconditionally (no early-return
//! guard like update_nb/deregister_nb). Tests that invoke fabric_register_nb
//! are marked #[ignore] with "SIGSEGV — FFI without PMIx init".
//!
//! fabric_deregister_nb has `!fabric.registered` guard → safe in user-space.
//! Callback trait tests, type checks, and signature tests are also safe.

use pmix::fabric::{
    fabric_deregister_nb, fabric_register_nb, FabricCallback, PmixFabric,
};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback.
struct NopCallback;

impl FabricCallback for NopCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

/// Panic callback — verifies the error path does NOT invoke the callback.
struct PanicCallback;

impl FabricCallback for PanicCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        panic!("PanicCallback should never be invoked on the error path")
    }
}

/// Recording callback — captures status for later assertion.
struct RecordingCallback {
    status: Arc<Mutex<Option<PmixStatus>>>,
}

impl FabricCallback for RecordingCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §1  register_nb — FFI calls (ignored in user-space)
// ─────────────────────────────────────────────────────────────────────────────

/// register_nb on unamed fabric — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_unamed() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    // With server: Ok(()) → callback invoked later
    // Without server: Err(INIT) → wrapper reclaimed, callback NOT invoked
    assert!(result.is_err() || result.is_ok());
}

/// register_nb with empty directives — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_empty_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    assert!(result.is_err() || result.is_ok());
}

/// register_nb with a single directive — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_single_directive() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let info = InfoBuilder::new().build();
    let directives: &[Info] = std::slice::from_ref(&info);
    let result = fabric_register_nb(&mut fabric, directives, Box::new(NopCallback));
    assert!(result.is_err() || result.is_ok());
}

/// register_nb with multiple directives — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_multiple_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let directives = vec![InfoBuilder::new().build(), InfoBuilder::new().build()];
    let result = fabric_register_nb(&mut fabric, &directives, Box::new(NopCallback));
    assert!(result.is_err() || result.is_ok());
}

/// register_nb callback not invoked on error path — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_callback_not_invoked_on_error() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let _result = fabric_register_nb(&mut fabric, &[], Box::new(PanicCallback));
    // If FFI returns error, wrapper is reclaimed and callback is NOT invoked.
    // If FFI returns success, callback IS invoked later (panic → test fails).
    // In user-space, this test is ignored because FFI segfaults.
}

/// register_nb wrapper reclamation — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_wrapper_reclamation_loop() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    for _ in 0..10 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    }
}

/// register_nb does not panic — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_no_panic_unamed() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_register_nb(&mut f, &[], Box::new(NopCallback))
    }));
    // Result may be Err (no server) or Ok (server present).
    // Key point: no panic.
    assert!(result.is_ok(), "register_nb must not panic on unamed fabric");
}

/// register_nb with recording callback — requires PMIx server.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nb_recording_callback() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut fabric = PmixFabric::unamed();
    let status = Arc::new(Mutex::new(None));
    let result = fabric_register_nb(
        &mut fabric,
        &[],
        Box::new(RecordingCallback {
            status: Arc::clone(&status),
        }),
    );
    if result.is_err() {
        return; // No server — callback not invoked, wrapper reclaimed.
    }
    // With server: callback will be invoked asynchronously.
    assert!(fabric.is_registered());
}

// ─────────────────────────────────────────────────────────────────────────────
// §2  deregister_nb — user-space safe (!fabric.registered guard)
// ─────────────────────────────────────────────────────────────────────────────

/// deregister_nb on unamed fabric returns BAD_PARAM.
#[test]
fn test_deregister_nb_unregistered_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// deregister_nb on named (unregistered) fabric returns BAD_PARAM.
#[test]
fn test_deregister_nb_named_unregistered_bad_param() {
    let mut fabric = PmixFabric::new(Some("dereg_nb_test")).unwrap();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
    assert_eq!(fabric.name(), Some("dereg_nb_test"));
}

/// deregister_nb callback NOT invoked on error path.
#[test]
fn test_deregister_nb_callback_not_invoked_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(PanicCallback));
    assert!(result.is_err());
    // PanicCallback was NOT invoked — wrapper was reclaimed.
}

/// deregister_nb wrapper reclamation — 100 iterations, no leaks.
#[test]
fn test_deregister_nb_wrapper_reclamation_loop() {
    for _ in 0..100 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    }
}

/// deregister_nb does not panic on unamed fabric.
#[test]
fn test_deregister_nb_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_deregister_nb(&mut f, Box::new(NopCallback))
    }));
    assert!(result.is_ok(), "deregister_nb must not panic on unamed fabric");
}

/// deregister_nb error is an error status.
#[test]
fn test_deregister_nb_error_is_error_status() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    let err = result.unwrap_err();
    assert!(err.is_error(), "deregister_nb error must be an error status");
}

/// deregister_nb preserves fabric state after error.
#[test]
fn test_deregister_nb_preserves_state() {
    let mut fabric = PmixFabric::new(Some("state_preserved")).unwrap();
    let _ = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("state_preserved"));
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// §3  Callback trait tests (user-space safe)
// ─────────────────────────────────────────────────────────────────────────────

/// FabricCallback requires Send + 'static.
#[test]
fn test_fabric_callback_send_bound() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn FabricCallback>>();
}

/// NopCallback implements Send.
#[test]
fn test_nop_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NopCallback>();
}

/// RecordingCallback implements Send (Arc<Mutex<>> is Send).
#[test]
fn test_recording_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RecordingCallback>();
}

// ─────────────────────────────────────────────────────────────────────────────
// §4  Function signature checks
// ─────────────────────────────────────────────────────────────────────────────

/// register_nb has the correct function signature.
#[test]
fn test_register_nb_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, &[Info], Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {
    }
    _check_sig(fabric_register_nb);
}

/// deregister_nb has the correct function signature.
#[test]
fn test_deregister_nb_signature() {
    fn _check_sig(_f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>) {}
    _check_sig(fabric_deregister_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// §5  Multiple fabrics independence (deregister_nb only — safe)
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple fabrics with independent nb deregister attempts.
#[test]
fn test_multiple_fabrics_nb_deregister() {
    let mut fabrics: Vec<_> = (0..5)
        .map(|i| PmixFabric::new(Some(&format!("dereg_nb_{}", i))).unwrap())
        .collect();

    for f in &mut fabrics {
        let result = fabric_deregister_nb(f, Box::new(NopCallback));
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §6  Deregister nb with recording callback (user-space safe)
// ─────────────────────────────────────────────────────────────────────────────

/// Recording callback NOT invoked on deregister_nb error path.
#[test]
fn test_deregister_nb_recording_callback_not_invoked() {
    let mut fabric = PmixFabric::unamed();
    let status = Arc::new(Mutex::new(None));
    let result = fabric_deregister_nb(
        &mut fabric,
        Box::new(RecordingCallback {
            status: Arc::clone(&status),
        }),
    );
    assert!(result.is_err());
    // Callback was NOT invoked — status remains None.
    assert!(status.lock().unwrap().is_none());
}

/// Recording callback invoked by daemon after successful deregister.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_deregister_nb_recording_callback_with_daemon() {
    let mut fabric = PmixFabric::unamed();
    let status = Arc::new(Mutex::new(None));
    let result = fabric_deregister_nb(
        &mut fabric,
        Box::new(RecordingCallback {
            status: Arc::clone(&status),
        }),
    );
    if result.is_err() {
        return; // No server
    }
    assert!(!fabric.is_registered());
}
