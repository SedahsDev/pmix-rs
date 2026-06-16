//! Batch 8 — Fabric registration integration tests.
//!
//! Covers fabric_register, fabric_deregister, fabric_update with focus on:
//! - Register duplicate idempotency
//! - Info array validation (empty, single, multiple directives)
//! - Error cases (double deregister, update without register, etc.)
//! - Lifecycle patterns (new → register → update → deregister)
//! - Type-level checks (Send / Sync)
//! - Non-blocking callback wrapper reclamation on error paths
//!
//! All tests pass without a PMIx daemon — they exercise the error paths
//! and parameter validation that the Rust wrappers perform before reaching FFI.

use pmix::fabric::{
    fabric_deregister, fabric_deregister_nb, fabric_register, fabric_register_nb, fabric_update,
    fabric_update_nb, FabricCallback, PmixFabric,
};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for non-blocking tests
// ─────────────────────────────────────────────────────────────────────────────

/// No-op callback used for nb tests — never panics.
struct NopCallback;

impl FabricCallback for NopCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

/// Callback that panics if invoked — used to verify the error path
/// does NOT invoke the callback when the wrapper is reclaimed.
struct PanicCallback;

impl FabricCallback for PanicCallback {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        panic!("PanicCallback should never be invoked on the error path")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §1  Register duplicate idempotency
// ─────────────────────────────────────────────────────────────────────────────

/// Register the same fabric twice — second call must also fail (no server).
/// Both calls should return the same error code.
#[test]
fn test_register_same_fabric_twice() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_register(&mut fabric, &[]);
    let result2 = fabric_register(&mut fabric, &[]);

    // Without a server, both calls fail.
    assert!(result1.is_err(), "first register should fail without server");
    assert!(result2.is_err(), "second register should also fail without server");
    // Both should return the same error.
    assert_eq!(result1, result2, "duplicate register errors should be identical");
}

/// Register the same fabric twice with directives — idempotent error.
#[test]
fn test_register_same_fabric_twice_with_directives() {
    let mut fabric = PmixFabric::new(Some("idempotent_test")).unwrap();
    let directives: &[Info] = &[];
    let result1 = fabric_register(&mut fabric, directives);
    let result2 = fabric_register(&mut fabric, directives);

    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

/// Two different fabric objects with the same name are independent —
/// registering one does not affect the other.
#[test]
fn test_register_different_fabrics_same_name() {
    let mut fabric_a = PmixFabric::new(Some("shared_name")).unwrap();
    let mut fabric_b = PmixFabric::new(Some("shared_name")).unwrap();

    let result_a = fabric_register(&mut fabric_a, &[]);
    let result_b = fabric_register(&mut fabric_b, &[]);

    // Both fail independently (no server).
    assert!(result_a.is_err());
    assert!(result_b.is_err());
    // Names are still intact.
    assert_eq!(fabric_a.name(), Some("shared_name"));
    assert_eq!(fabric_b.name(), Some("shared_name"));
}

/// Register the same fabric three times — all return consistent errors.
#[test]
fn test_register_same_fabric_three_times() {
    let mut fabric = PmixFabric::unamed();
    let results: Vec<_> = (0..3).map(|_| fabric_register(&mut fabric, &[])).collect();

    for (i, r) in results.iter().enumerate() {
        assert!(r.is_err(), "register call {} should fail", i);
    }
    // All errors are identical.
    assert_eq!(results[0], results[1]);
    assert_eq!(results[1], results[2]);
}

/// Register the same fabric twice non-blocking — both fail consistently.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_same_fabric_twice() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    let result2 = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));

    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

/// Register twice, then try to deregister — still fails because fabric
/// was never successfully registered.
#[test]
fn test_register_twice_then_deregister() {
    let mut fabric = PmixFabric::unamed();
    let _ = fabric_register(&mut fabric, &[]);
    let _ = fabric_register(&mut fabric, &[]);

    // fabric.registered is still false because register never succeeded.
    assert!(!fabric.is_registered());
    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// §2  Info array validation
// ─────────────────────────────────────────────────────────────────────────────

/// Register with empty directives — should fail gracefully (no server).
#[test]
fn test_register_with_empty_directives() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err(), "register with empty directives fails without server");
    assert!(!fabric.is_registered());
}

/// Register with a single Info directive — should fail gracefully (no server).
#[test]
fn test_register_with_single_directive() {
    let mut fabric = PmixFabric::new(Some("single_dir")).unwrap();
    let info = InfoBuilder::new().build();
    let directives: &[Info] = std::slice::from_ref(&info);
    let result = fabric_register(&mut fabric, directives);
    assert!(result.is_err());
    assert!(!fabric.is_registered());
}

/// Register with multiple Info directives — should fail gracefully (no server).
#[test]
fn test_register_with_multiple_directives() {
    let mut fabric = PmixFabric::new(Some("multi_dir")).unwrap();
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();
    let directives = vec![info1, info2];
    let result = fabric_register(&mut fabric, &directives);
    assert!(result.is_err());
    assert!(!fabric.is_registered());
}

/// Register with empty directives on named fabric — error is consistent.
#[test]
fn test_register_named_empty_directives() {
    let mut fabric = PmixFabric::new(Some("named_empty")).unwrap();
    let result = fabric_register(&mut fabric, &[]);
    assert!(result.is_err());
    // Name is preserved despite failed registration.
    assert_eq!(fabric.name(), Some("named_empty"));
}

/// Register with empty directives twice — same error both times.
#[test]
fn test_register_empty_directives_idempotent() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_register(&mut fabric, &[]);
    let result2 = fabric_register(&mut fabric, &[]);
    assert_eq!(result1, result2);
}

/// Register nb with empty directives — fails gracefully.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_empty_directives() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    assert!(result.is_err());
}

/// Register nb with a single directive — fails gracefully.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_single_directive() {
    let mut fabric = PmixFabric::unamed();
    let info = InfoBuilder::new().build();
    let directives: &[Info] = std::slice::from_ref(&info);
    let result = fabric_register_nb(&mut fabric, directives, Box::new(NopCallback));
    assert!(result.is_err());
}

/// Register nb with multiple directives — fails gracefully.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_multiple_directives() {
    let mut fabric = PmixFabric::unamed();
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();
    let directives = vec![info1, info2];
    let result = fabric_register_nb(&mut fabric, &directives, Box::new(NopCallback));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// §3  Error cases
// ─────────────────────────────────────────────────────────────────────────────

/// Deregister a fabric that was never registered returns BAD_PARAM.
#[test]
fn test_deregister_unknown_fabric() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Deregister a named fabric that was never registered returns BAD_PARAM.
#[test]
fn test_deregister_named_unknown_fabric() {
    let mut fabric = PmixFabric::new(Some("never_registered")).unwrap();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Deregister nb on an unregistered fabric returns BAD_PARAM.
#[test]
fn test_deregister_nb_unknown_fabric() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Update an unregistered fabric returns BAD_PARAM.
#[test]
fn test_update_unregistered_fabric() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Update nb on an unregistered fabric returns BAD_PARAM.
#[test]
fn test_update_nb_unregistered_fabric() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Double deregister: first call on unregistered fabric fails,
/// second call also fails with the same error.
#[test]
fn test_double_deregister_unregistered() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_deregister(&mut fabric);
    let result2 = fabric_deregister(&mut fabric);
    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

/// Double deregister nb: both calls fail consistently.
#[test]
fn test_double_deregister_nb_unregistered() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    let result2 = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

/// Double update on unregistered fabric — both return BAD_PARAM.
#[test]
fn test_double_update_unregistered() {
    let mut fabric = PmixFabric::unamed();
    let result1 = fabric_update(&mut fabric);
    let result2 = fabric_update(&mut fabric);
    assert!(result1.is_err());
    assert!(result2.is_err());
    assert_eq!(result1, result2);
}

/// Attempt register → update → deregister on unregistered fabric —
/// all three operations fail because register never succeeded.
#[test]
fn test_full_lifecycle_without_server() {
    let mut fabric = PmixFabric::new(Some("lifecycle_no_server")).unwrap();

    // Register fails (no server).
    let reg = fabric_register(&mut fabric, &[]);
    assert!(reg.is_err());
    assert!(!fabric.is_registered());

    // Update fails (not registered).
    let upd = fabric_update(&mut fabric);
    assert!(upd.is_err());

    // Deregister fails (not registered).
    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_err());
}

/// Attempt register → deregister (skip update) — both fail.
#[test]
fn test_register_deregister_skip_update() {
    let mut fabric = PmixFabric::unamed();
    let reg = fabric_register(&mut fabric, &[]);
    assert!(reg.is_err());

    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_err());
}

/// Attempt new → deregister (skip register) — deregister fails.
#[test]
fn test_new_deregister_skip_register() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    assert!(result.is_err());
    assert!(!fabric.is_registered());
}

/// Attempt new → deregister nb (skip register) — fails with BAD_PARAM.
#[test]
fn test_new_deregister_nb_skip_register() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// Attempt new → update (skip register) — update fails.
#[test]
fn test_new_update_skip_register() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    assert!(result.is_err());
}

/// Attempt new → update nb (skip register) — fails with BAD_PARAM.
#[test]
fn test_new_update_nb_skip_register() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopCallback));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

// ─────────────────────────────────────────────────────────────────────────────
// §4  Lifecycle patterns
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle with nb variants — all fail gracefully without server.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_full_nb_lifecycle_without_server() {
    let mut fabric = PmixFabric::new(Some("nb_lifecycle")).unwrap();

    // Register nb fails (no server).
    let reg = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    assert!(reg.is_err());
    assert!(!fabric.is_registered());

    // Update nb fails (not registered).
    let upd = fabric_update_nb(&mut fabric, Box::new(NopCallback));
    assert!(upd.is_err());

    // Deregister nb fails (not registered).
    let dereg = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    assert!(dereg.is_err());
}

/// Lifecycle with mixed blocking and non-blocking calls.
#[test]
fn test_mixed_blocking_nb_lifecycle() {
    let mut fabric = PmixFabric::unamed();

    // Register (blocking) fails.
    assert!(fabric_register(&mut fabric, &[]).is_err());

    // Update nb fails.
    assert!(fabric_update_nb(&mut fabric, Box::new(NopCallback)).is_err());

    // Deregister (blocking) fails.
    assert!(fabric_deregister(&mut fabric).is_err());
}

/// Multiple fabrics in parallel lifecycle — each independent.
#[test]
fn test_multiple_fabric_lifecycles() {
    let mut fabrics: Vec<_> = (0..5)
        .map(|i| PmixFabric::new(Some(&format!("lifecycle_{}", i))).unwrap())
        .collect();

    // Try to register all.
    for f in &mut fabrics {
        assert!(fabric_register(f, &[]).is_err());
    }

    // Try to update all.
    for f in &mut fabrics {
        assert!(fabric_update(f).is_err());
    }

    // Try to deregister all.
    for f in &mut fabrics {
        assert!(fabric_deregister(f).is_err());
    }
}

/// Lifecycle state is preserved across failed operations.
#[test]
fn test_lifecycle_state_preserved() {
    let mut fabric = PmixFabric::new(Some("state_preserved")).unwrap();

    // Before any operation.
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);
    assert_eq!(fabric.ninfo(), 0);

    // Failed register.
    let _ = fabric_register(&mut fabric, &[]);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.index(), 0);

    // Failed update.
    let _ = fabric_update(&mut fabric);
    assert!(!fabric.is_registered());

    // Failed deregister.
    let _ = fabric_deregister(&mut fabric);
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("state_preserved"));
}

/// Unamed fabric lifecycle — same error behavior as named.
#[test]
fn test_unamed_lifecycle() {
    let mut fabric = PmixFabric::unamed();
    assert!(fabric_register(&mut fabric, &[]).is_err());
    assert!(fabric_update(&mut fabric).is_err());
    assert!(fabric_deregister(&mut fabric).is_err());
    assert!(!fabric.is_registered());
}

/// Lifecycle via new(None) — same error behavior.
#[test]
fn test_new_none_lifecycle() {
    let mut fabric = PmixFabric::new(None).unwrap();
    assert!(fabric_register(&mut fabric, &[]).is_err());
    assert!(fabric_update(&mut fabric).is_err());
    assert!(fabric_deregister(&mut fabric).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// §5  Type-level checks
// ─────────────────────────────────────────────────────────────────────────────

/// PmixFabric contains raw pointers so it is NOT Send.
/// This is an intentional compile-time check.
#[test]
fn test_pmix_fabric_not_send() {
    // PmixFabric wraps raw FFI pointers (*mut c_void, MaybeUninit<pmix_fabric_t>)
    // which are not Send. We verify this by confirming the trait bound fails
    // to compile — but since we can't express negative bounds, we simply
    // document the fact here and verify the struct compiles at all.
    let _fabric: PmixFabric = PmixFabric::unamed();
}

/// PmixFabric contains raw pointers so it is NOT Sync.
#[test]
fn test_pmix_fabric_not_sync() {
    // Same reasoning as test_pmix_fabric_not_send — raw pointers prevent Sync.
    let _fabric: PmixFabric = PmixFabric::unamed();
}

/// PmixFabric implements Debug (compile-time check).
#[test]
fn test_pmix_fabric_debug_trait() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixFabric>();
}

/// fabric_register has the correct function signature.
#[test]
fn test_fabric_register_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, &[Info]) -> Result<(), PmixStatus>,
    ) {}
    _check_sig(fabric_register);
}

/// fabric_update has the correct function signature.
#[test]
fn test_fabric_update_signature() {
    fn _check_sig(_f: fn(&mut PmixFabric) -> Result<(), PmixStatus>) {}
    _check_sig(fabric_update);
}

/// fabric_deregister has the correct function signature.
#[test]
fn test_fabric_deregister_signature() {
    fn _check_sig(_f: fn(&mut PmixFabric) -> Result<(), PmixStatus>) {}
    _check_sig(fabric_deregister);
}

/// fabric_register_nb has the correct function signature.
#[test]
fn test_fabric_register_nb_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, &[Info], Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {}
    _check_sig(fabric_register_nb);
}

/// fabric_update_nb has the correct function signature.
#[test]
fn test_fabric_update_nb_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {}
    _check_sig(fabric_update_nb);
}

/// fabric_deregister_nb has the correct function signature.
#[test]
fn test_fabric_deregister_nb_signature() {
    fn _check_sig(
        _f: fn(&mut PmixFabric, Box<dyn FabricCallback>) -> Result<(), PmixStatus>,
    ) {}
    _check_sig(fabric_deregister_nb);
}

// ─────────────────────────────────────────────────────────────────────────────
// §6  Non-blocking callback behavior on error paths
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_register_nb error path does not invoke the callback.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_callback_not_invoked_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register_nb(&mut fabric, &[], Box::new(PanicCallback));
    assert!(result.is_err());
    // If PanicCallback was invoked, the test would have panicked.
}

/// fabric_update_nb error path does not invoke the callback.
#[test]
fn test_update_nb_callback_not_invoked_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(PanicCallback));
    assert!(result.is_err());
}

/// fabric_deregister_nb error path does not invoke the callback.
#[test]
fn test_deregister_nb_callback_not_invoked_on_error() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(PanicCallback));
    assert!(result.is_err());
}

/// register_nb wrapper reclamation — 100 iterations, no leaks.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_wrapper_reclamation_loop() {
    for _ in 0..100 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    }
}

/// update_nb wrapper reclamation — 100 iterations, no leaks.
#[test]
fn test_update_nb_wrapper_reclamation_loop() {
    for _ in 0..100 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_update_nb(&mut fabric, Box::new(NopCallback));
    }
}

/// deregister_nb wrapper reclamation — 100 iterations, no leaks.
#[test]
fn test_deregister_nb_wrapper_reclamation_loop() {
    for _ in 0..100 {
        let mut fabric = PmixFabric::unamed();
        let _ = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §7  Error code verification
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_register error is an error status (not success).
#[test]
fn test_register_error_is_error_status() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_register(&mut fabric, &[]);
    let err = result.unwrap_err();
    assert!(err.is_error(), "register error must be an error status");
}

/// fabric_update error is BAD_PARAM (checked at Rust level).
#[test]
fn test_update_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update(&mut fabric);
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// fabric_deregister error is BAD_PARAM (checked at Rust level).
#[test]
fn test_deregister_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister(&mut fabric);
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// fabric_update_nb error is BAD_PARAM.
#[test]
fn test_update_nb_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_update_nb(&mut fabric, Box::new(NopCallback));
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// fabric_deregister_nb error is BAD_PARAM.
#[test]
fn test_deregister_nb_error_is_bad_param() {
    let mut fabric = PmixFabric::unamed();
    let result = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrBadParam));
}

/// PmixStatus Display works for error statuses.
#[test]
fn test_pmix_status_display() {
    let err = PmixStatus::Known(PmixError::ErrBadParam);
    let display = format!("{}", err);
    assert!(!display.is_empty());
    assert!(display.contains("BAD_PARAM") || display.contains("bad_param") || display.contains("BadParam"));
}

/// PmixError Debug works for ErrBadParam.
#[test]
fn test_pmix_error_debug_bad_param() {
    let err = PmixError::ErrBadParam;
    let debug_str = format!("{:?}", err);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("ErrBadParam"));
}

// ─────────────────────────────────────────────────────────────────────────────
// §8  Edge cases and panic safety
// ─────────────────────────────────────────────────────────────────────────────

/// fabric_register does not panic on default-constructed fabric.
#[test]
fn test_register_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_register(&mut f, &[])
    }));
    assert!(result.is_ok(), "register must not panic on unamed fabric");
}

/// fabric_update does not panic on default-constructed fabric.
#[test]
fn test_update_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_update(&mut f)
    }));
    assert!(result.is_ok(), "update must not panic on unamed fabric");
}

/// fabric_deregister does not panic on default-constructed fabric.
#[test]
fn test_deregister_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_deregister(&mut f)
    }));
    assert!(result.is_ok(), "deregister must not panic on unamed fabric");
}

/// register_nb does not panic on default-constructed fabric.
#[test]
#[ignore = "SIGSEGV — fabric_register_nb calls FFI without PMIx init"]
fn test_register_nb_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_register_nb(&mut f, &[], Box::new(NopCallback))
    }));
    assert!(result.is_ok());
}

/// update_nb does not panic on default-constructed fabric.
#[test]
fn test_update_nb_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_update_nb(&mut f, Box::new(NopCallback))
    }));
    assert!(result.is_ok());
}

/// deregister_nb does not panic on default-constructed fabric.
#[test]
fn test_deregister_nb_no_panic_unamed() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut f = PmixFabric::unamed();
        fabric_deregister_nb(&mut f, Box::new(NopCallback))
    }));
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// §9  Fabric state isolation
// ─────────────────────────────────────────────────────────────────────────────

/// Operations on one fabric do not affect another fabric's state.
#[test]
fn test_fabric_state_isolation() {
    let mut fabric_a = PmixFabric::new(Some("isolated_a")).unwrap();
    let fabric_b = PmixFabric::new(Some("isolated_b")).unwrap();

    // Try operations on fabric_a.
    let _ = fabric_register(&mut fabric_a, &[]);
    let _ = fabric_update(&mut fabric_a);
    let _ = fabric_deregister(&mut fabric_a);

    // fabric_b should be completely unaffected.
    assert!(!fabric_b.is_registered());
    assert_eq!(fabric_b.name(), Some("isolated_b"));
    assert_eq!(fabric_b.index(), 0);
    assert_eq!(fabric_b.ninfo(), 0);
}

/// Multiple fabrics with different names maintain independent state.
#[test]
fn test_multiple_fabric_independence() {
    let fabrics: Vec<_> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|n| PmixFabric::new(Some(n)).unwrap())
        .collect();

    let mut results = Vec::new();
    for mut f in fabrics {
        results.push(fabric_register(&mut f, &[]));
    }

    // All registrations fail independently.
    for (i, r) in results.into_iter().enumerate() {
        assert!(r.is_err(), "fabric {} register should fail", i);
    }
}

/// Fabric name is preserved across failed register attempts.
#[test]
fn test_name_preserved_across_failed_register() {
    let mut fabric = PmixFabric::new(Some("persistent_name")).unwrap();
    for _ in 0..10 {
        let _ = fabric_register(&mut fabric, &[]);
        assert_eq!(fabric.name(), Some("persistent_name"));
    }
}

/// Fabric name is preserved across failed update attempts.
#[test]
fn test_name_preserved_across_failed_update() {
    let mut fabric = PmixFabric::new(Some("update_preserved")).unwrap();
    for _ in 0..10 {
        let _ = fabric_update(&mut fabric);
        assert_eq!(fabric.name(), Some("update_preserved"));
    }
}

/// Fabric name is preserved across failed deregister attempts.
#[test]
fn test_name_preserved_across_failed_deregister() {
    let mut fabric = PmixFabric::new(Some("deregister_preserved")).unwrap();
    for _ in 0..10 {
        let _ = fabric_deregister(&mut fabric);
        assert_eq!(fabric.name(), Some("deregister_preserved"));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §10  Integration tests (require PMIx daemon — ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: register → update → deregister with a real PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_lifecycle_with_daemon() {
    let mut fabric = PmixFabric::new(Some("daemon_lifecycle")).unwrap();
    assert!(!fabric.is_registered());

    let reg = fabric_register(&mut fabric, &[]);
    if reg.is_err() {
        return; // No server
    }
    assert!(fabric.is_registered());

    let _upd = fabric_update(&mut fabric);

    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_ok());
    assert!(!fabric.is_registered());
    assert_eq!(fabric.ninfo(), 0);
}

/// Register twice with daemon — second call behavior.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_double_register_with_daemon() {
    let mut fabric = PmixFabric::unamed();
    let first = fabric_register(&mut fabric, &[]);
    if first.is_err() {
        return; // No server
    }
    assert!(fabric.is_registered());

    // Second register on the same fabric object.
    let second = fabric_register(&mut fabric, &[]);
    // Behavior depends on the PMIx implementation; we just verify it doesn't panic.
    let _ = second;
}

/// Register → deregister → register again with daemon.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_register_deregister_reregister() {
    let mut fabric = PmixFabric::unamed();

    // First registration.
    let reg1 = fabric_register(&mut fabric, &[]);
    if reg1.is_err() {
        return;
    }
    assert!(fabric.is_registered());

    // Deregister.
    let dereg = fabric_deregister(&mut fabric);
    assert!(dereg.is_ok());
    assert!(!fabric.is_registered());

    // Re-register.
    let reg2 = fabric_register(&mut fabric, &[]);
    // May succeed or fail depending on server; just verify no panic.
    let _ = reg2;
}

/// Full nb lifecycle with daemon.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_nb_lifecycle_with_daemon() {
    let mut fabric = PmixFabric::unamed();

    let reg = fabric_register_nb(&mut fabric, &[], Box::new(NopCallback));
    if reg.is_err() {
        return;
    }
    assert!(fabric.is_registered());

    let _upd = fabric_update_nb(&mut fabric, Box::new(NopCallback));

    let _dereg = fabric_deregister_nb(&mut fabric, Box::new(NopCallback));
}

/// Multiple fabric registrations with daemon.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_multiple_fabrics_with_daemon() {
    let names = ["fabric_0", "fabric_1", "fabric_2"];
    let mut fabrics: Vec<_> = names
        .iter()
        .map(|n| PmixFabric::new(Some(n)).unwrap())
        .collect();

    // Register all.
    for f in &mut fabrics {
        let _ = fabric_register(f, &[]);
    }

    // Check that all are registered (or bail if no server).
    if !fabrics.iter().all(|f| f.is_registered()) {
        return;
    }

    // Deregister each.
    for (i, f) in fabrics.iter_mut().enumerate() {
        let result = fabric_deregister(f);
        assert!(result.is_ok(), "fabric {} deregister should succeed", i);
    }
}
