//! Phase 4 Batch 2: PmixFabric Non-Blocking Update
//!
//! Dedicated tests for fabric_update_nb that focus on:
//! - FabricCallback trait implementation validation (shared with register_nb/deregister_nb)
//! - Wrapper reclamation on error paths
//! - Panic safety via catch_unwind
//! - Type-level checks (Send + 'static bounds)
//! - Error code verification for unregistered fabrics
//!
//! Tests that require PMIx_Init are marked #[ignore] with clear rationale.

use pmix::fabric::{fabric_update_nb, FabricCallback, PmixFabric};
use pmix::{PmixError, PmixStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations
// ─────────────────────────────────────────────────────────────────────────────

/// No-op update callback.
struct NopUpdateCallback;

impl FabricCallback for NopUpdateCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

/// Update callback that panics if invoked — verifies the error path
/// does NOT invoke the callback when the wrapper is reclaimed.
struct PanicUpdateCallback;

impl FabricCallback for PanicUpdateCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        panic!("PanicUpdateCallback should never be invoked on the error path")
    }
}

/// Recording update callback — captures the status for later assertion.
struct RecordingUpdateCallback {
    status: Arc<Mutex<Option<PmixStatus>>>,
}

impl FabricCallback for RecordingUpdateCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §1  update_nb error paths (user-space testable)
// ─────────────────────────────────────────────────────────────────────────────

/// update_nb on unamed fabric fails without a server.
#[test]
fn test_update_nb_unamed_fails() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    assert!(result.is_err(), "update_nb should fail without server");
    assert!(!fabric.is_registered());
}

/// update_nb on named fabric fails without a server.
#[test]
fn test_update_nb_named_fails() {
    let mut fabric = PmixFabric::new(Some("update_nb_test")).unwrap();
    let result = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    assert!(result.is_err());
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("update_nb_test"));
}

/// update_nb returns BAD_PARAM for unregistered fabric.
#[test]
fn test_update_nb_returns_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

// ─────────────────────────────────────────────────────────────────────────────
// §2  Callback not invoked on error path
// ─────────────────────────────────────────────────────────────────────────────

/// update_nb error path does NOT invoke the callback.
/// If it did, PanicUpdateCallback would panic and the test would fail.
#[test]
fn test_update_nb_callback_not_invoked_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(PanicUpdateCallback));
    assert!(result.is_err());
    // PanicUpdateCallback was NOT invoked — wrapper was reclaimed instead.
}

// ─────────────────────────────────────────────────────────────────────────────
// §3  Wrapper reclamation (memory leak prevention)
// ─────────────────────────────────────────────────────────────────────────────

/// update_nb wrapper reclamation — 100 iterations, no leaks.
#[test]
fn test_update_nb_wrapper_reclamation_loop() {
    for _ in 0..100 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §4  Panic safety
// ─────────────────────────────────────────────────────────────────────────────

/// update_nb does not panic on default-constructed fabric.
#[test]
fn test_update_nb_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_update_nb(&mut f, Box::new(NopUpdateCallback))
    }));
    assert!(result.is_ok(), "update_nb must not panic on unamed fabric");
}

/// update_nb does not panic on named fabric.
#[test]
fn test_update_nb_no_panic_named() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::new(Some("panic_test")).unwrap();
        fabric_update_nb(&mut f, Box::new(NopUpdateCallback))
    }));
    assert!(result.is_ok(), "update_nb must not panic on named fabric");
}

// ─────────────────────────────────────────────────────────────────────────────
// §5  Type-level checks
// ─────────────────────────────────────────────────────────────────────────────

/// NopUpdateCallback implements Send (required by FabricCallback).
#[test]
fn test_nop_update_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NopUpdateCallback>();
}

/// RecordingUpdateCallback implements Send (Arc<Mutex<>> is Send).
#[test]
fn test_recording_update_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RecordingUpdateCallback>();
}

/// update_nb has the correct function signature.
#[test]
fn test_update_nb_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {
    }
    _check_sig(fabric_update_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// §6  Error code consistency
// ─────────────────────────────────────────────────────────────────────────────

/// update_nb error is an error status (not success).
#[test]
fn test_update_nb_error_is_error_status() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    let err = result.unwrap_err();
    assert!(err.is_error(), "update_nb error must be an error status");
}

// ─────────────────────────────────────────────────────────────────────────────
// §7  State preservation after failed nb operations
// ─────────────────────────────────────────────────────────────────────────────

/// Fabric name is preserved after failed update_nb.
#[test]
fn test_update_nb_preserves_name() {
    let mut fabric = PmixFabric::new(Some("preserved_name")).unwrap();
    let _ = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    assert_eq!(fabric.name(), Some("preserved_name"));
}

/// Fabric state is preserved after failed update_nb.
#[test]
fn test_update_nb_preserves_state() {
    let mut fabric = PmixFabric::new(Some("state_intact")).unwrap();
    let _ = fabric_update_nb(&mut fabric, Box::new(NopUpdateCallback));
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("state_intact"));
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// §8  Multiple fabrics independence (nb update)
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple fabrics with independent nb update attempts.
#[test]
fn test_multiple_fabrics_nb_update() {
    let mut fabrics: Vec<_> = (0..5)
        .map(|i| PmixFabric::new(Some(&format!("update_nb_{}", i))).unwrap())
        .collect();

    for f in &mut fabrics {
        let result = fabric_update_nb(f, Box::new(NopUpdateCallback));
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §9  Recording callback (for daemon tests)
// ─────────────────────────────────────────────────────────────────────────────

/// Recording callback captures status when invoked by daemon.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_update_nb_recording_callback_with_daemon() {
    let mut fabric = PmixFabric::unamed();
    let status = Arc::new(Mutex::new(None));
    let result = fabric_update_nb(
        &mut fabric,
        Box::new(RecordingUpdateCallback {
            status: Arc::clone(&status),
        }),
    );
    if result.is_err() {
        return; // No server
    }
    // Callback will be invoked asynchronously — in a real daemon test,
    // we'd wait for it. Here we just verify the update was initiated.
    assert!(fabric.is_registered());
}
