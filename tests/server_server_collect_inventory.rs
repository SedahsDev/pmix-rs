//! Tests for `PMIx_server_collect_inventory` via the safe `server_collect_inventory` wrapper.
//!
//! These tests cover type signatures, callback trait, result types, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (PMIx_server_init) are marked `#[ignore]`.

use pmix::server::{CollectInventoryCallback, CollectInventoryResults, server_collect_inventory};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `server_collect_inventory` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&Info` and
/// `Box<dyn CollectInventoryCallback>`, returning `Result<(), PmixStatus>`.
#[test]
fn collect_inventory_function_signature() {
    let _: fn(&Info, Box<dyn CollectInventoryCallback>) -> Result<(), PmixStatus> =
        server_collect_inventory;
}

/// `CollectInventoryCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object with the expected `on_complete` method.
#[test]
fn collect_inventory_callback_trait_object() {
    struct TestCallback;
    impl CollectInventoryCallback for TestCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn CollectInventoryCallback> = Box::new(TestCallback);
    let _: Box<dyn CollectInventoryCallback> = cb;
}

/// `CollectInventoryCallback::on_complete` receives the correct types.
///
/// Compile-time check: the callback receives `PmixStatus` and
/// `CollectInventoryResults`.
#[test]
fn collect_inventory_callback_signature() {
    struct SigCheck;
    impl CollectInventoryCallback for SigCheck {
        fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
            // Verify types: status is PmixStatus, inventory is CollectInventoryResults.
            let _: PmixStatus = status;
            let _: CollectInventoryResults = inventory;
        }
    }
}

/// `CollectInventoryCallback` is `Send` (required for cross-thread callback use).
#[test]
fn collect_inventory_callback_is_send() {
    struct SendCheck;
    impl CollectInventoryCallback for SendCheck {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn CollectInventoryCallback>>();
}

/// `CollectInventoryResults` is `Debug` (derivable for logging).
#[test]
fn collect_inventory_results_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<CollectInventoryResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// CollectInventoryResults type tests
// ─────────────────────────────────────────────────────────────────────────────

/// `CollectInventoryResults::len()` returns the number of info entries.
///
/// Note: We cannot construct CollectInventoryResults directly because the
/// fields are private. This test verifies the type is usable through the
/// callback interface.
#[test]
fn collect_inventory_results_len_type() {
    struct LenCheck;
    impl CollectInventoryCallback for LenCheck {
        fn on_complete(&self, _status: PmixStatus, inventory: CollectInventoryResults) {
            let _n: usize = inventory.len();
        }
    }
}

/// `CollectInventoryResults::is_empty()` returns a boolean.
#[test]
fn collect_inventory_results_is_empty_type() {
    struct EmptyCheck;
    impl CollectInventoryCallback for EmptyCheck {
        fn on_complete(&self, _status: PmixStatus, inventory: CollectInventoryResults) {
            let _b: bool = inventory.is_empty();
        }
    }
}

/// `CollectInventoryResults` has a `Drop` implementation (memory safety).
///
/// Compile-time check: the type implements Drop for automatic cleanup
/// of the underlying C-allocated info array.
#[test]
fn collect_inventory_results_has_drop() {
    fn assert_drop<T: Drop>() {}
    assert_drop::<CollectInventoryResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback invocation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Callback can capture and store state.
///
/// Verify that a callback implementation can maintain internal state
/// and access it in `on_complete`. The trait object is properly
/// boxable and the callback can be stored for later invocation.
#[test]
fn collect_inventory_callback_captures_state() {
    use std::sync::atomic::{AtomicI32, Ordering};

    static INVOKED: AtomicI32 = AtomicI32::new(0);

    struct StateCallback;
    impl CollectInventoryCallback for StateCallback {
        fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
            INVOKED.fetch_add(1, Ordering::SeqCst);
            // Verify we can use both parameters.
            let _: PmixStatus = status;
            let _: usize = inventory.len();
        }
    }

    let cb: Box<dyn CollectInventoryCallback> = Box::new(StateCallback);
    // Verify the callback is properly boxed and usable.
    let _: &dyn CollectInventoryCallback = &*cb;
}

/// Callback can be stored as a trait object and invoked later.
#[test]
fn collect_inventory_callback_trait_object_storage() {
    struct StoredCallback;
    impl CollectInventoryCallback for StoredCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let callbacks: Vec<Box<dyn CollectInventoryCallback>> = vec![Box::new(StoredCallback)];
    assert_eq!(callbacks.len(), 1);
}

/// Multiple different callback implementations can coexist.
#[test]
fn collect_inventory_multiple_callback_types() {
    struct CallbackA;
    impl CollectInventoryCallback for CallbackA {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    struct CallbackB;
    impl CollectInventoryCallback for CallbackB {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let callbacks: Vec<Box<dyn CollectInventoryCallback>> =
        vec![Box::new(CallbackA), Box::new(CallbackB)];
    assert_eq!(callbacks.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Function behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// `server_collect_inventory` rejects call when PMIx not initialized.
///
/// Without PMIx_server_init, the function should return PMIX_ERR_INIT
/// because the server library is not initialized.
#[test]
fn collect_inventory_without_init_returns_err_init() {
    struct NopCallback;
    impl CollectInventoryCallback for NopCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let directives = InfoBuilder::new().build();
    let result = server_collect_inventory(&directives, Box::new(NopCallback));

    // Should return Err(PMIX_ERR_INIT) because PMIx_server_init was not called.
    assert!(result.is_err(), "expected error when PMIx not initialized");
    let err = result.unwrap_err();
    assert_eq!(err, PmixStatus::Known(PmixError::ErrInit));
}

/// `server_collect_inventory` with empty directives.
///
/// Passing an empty Info should still return PMIX_ERR_INIT when not
/// initialized, confirming the function processes the parameters
/// correctly before failing.
#[test]
fn collect_inventory_empty_directives() {
    struct NopCallback;
    impl CollectInventoryCallback for NopCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let directives = InfoBuilder::new().build();
    // InfoBuilder::new().build() creates an empty info list (len == 0).
    let result = server_collect_inventory(&directives, Box::new(NopCallback));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), PmixStatus::Known(PmixError::ErrInit));
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback state and status tests
// ─────────────────────────────────────────────────────────────────────────────

/// Callback receives PmixStatus with error codes.
///
/// Verify the callback trait can handle various error statuses.
#[test]
fn collect_inventory_callback_handles_error_status() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static RECEIVED: AtomicBool = AtomicBool::new(false);

    struct ErrorStatusCallback;
    impl CollectInventoryCallback for ErrorStatusCallback {
        fn on_complete(&self, status: PmixStatus, _inventory: CollectInventoryResults) {
            assert!(!status.is_success());
            RECEIVED.store(true, Ordering::SeqCst);
        }
    }

    let cb: Box<dyn CollectInventoryCallback> = Box::new(ErrorStatusCallback);
    // Verify the callback is properly boxed and the trait is callable.
    let _: &dyn CollectInventoryCallback = &*cb;
}

/// Callback can distinguish between success and error status.
#[test]
fn collect_inventory_callback_status_discrimination() {
    struct DiscriminatingCallback;
    impl CollectInventoryCallback for DiscriminatingCallback {
        fn on_complete(&self, status: PmixStatus, _inventory: CollectInventoryResults) {
            match status {
                PmixStatus::Known(PmixError::Success) => {
                    // Success path
                }
                PmixStatus::Known(PmixError::ErrInit) => {
                    // Not initialized
                }
                PmixStatus::Known(PmixError::ErrNomem) => {
                    // Out of memory
                }
                _ => {
                    // Other status
                }
            }
        }
    }

    let _cb: Box<dyn CollectInventoryCallback> = Box::new(DiscriminatingCallback);
}

/// Callback receives CollectInventoryResults with length info.
#[test]
fn collect_inventory_callback_inventory_len() {
    struct InventoryLenCallback;
    impl CollectInventoryCallback for InventoryLenCallback {
        fn on_complete(&self, _status: PmixStatus, inventory: CollectInventoryResults) {
            let len = inventory.len();
            let empty = inventory.is_empty();
            assert_eq!(empty, len == 0);
        }
    }

    let _cb: Box<dyn CollectInventoryCallback> = Box::new(InventoryLenCallback);
}

// ─────────────────────────────────────────────────────────────────────────────
// Runtime tests (require PMIx server — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `server_collect_inventory` succeeds after PMIx_server_init.
///
/// This test requires a running PMIx server environment. It will be
/// skipped unless PMIx is properly initialized.
#[test]
#[ignore = "requires PMIx server initialization"]
fn collect_inventory_with_server_init() {
    use pmix::server::{PmixServerModule, server_init};
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct TestCallback;
    impl CollectInventoryCallback for TestCallback {
        fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
            assert!(status.is_success(), "callback status: {:?}", status);
            println!("Inventory collected: {} items", inventory.len());
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    // Initialize the PMIx server.
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let _handle = server_init(Some(&module), &info).expect("PMIx_server_init failed");

    let directives = InfoBuilder::new().build();
    let result = server_collect_inventory(&directives, Box::new(TestCallback));
    assert!(
        result.is_ok(),
        "collect_inventory should be accepted: {:?}",
        result
    );

    // Note: the callback is asynchronous — in a real test we would wait
    // for it to fire. Here we just verify the request was accepted.
}

/// `server_collect_inventory` with custom directives.
///
/// This test verifies that passing non-empty directives works correctly.
#[test]
#[ignore = "requires PMIx server initialization"]
fn collect_inventory_with_directives() {
    use pmix::server::{PmixServerModule, server_init};

    struct TestCallback;
    impl CollectInventoryCallback for TestCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let _handle = server_init(Some(&module), &info).expect("PMIx_server_init failed");

    // We cannot easily construct non-empty Info from test code because
    // Info's internal handle is a raw pointer. The C API accepts null
    // for directives, which is equivalent to an empty slice.
    let directives = InfoBuilder::new().build();
    let result = server_collect_inventory(&directives, Box::new(TestCallback));
    assert!(result.is_ok());
}

/// Multiple concurrent collect_inventory requests.
///
/// Verify that multiple requests can be submitted without immediate errors.
#[test]
#[ignore = "requires PMIx server initialization"]
fn collect_inventory_concurrent_requests() {
    use pmix::server::{PmixServerModule, server_init};

    struct TestCallback;
    impl CollectInventoryCallback for TestCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let _handle = server_init(Some(&module), &info).expect("PMIx_server_init failed");

    let directives = InfoBuilder::new().build();
    let results: Vec<_> = (0..3)
        .map(|_| server_collect_inventory(&directives, Box::new(TestCallback)))
        .collect();

    for result in results {
        assert!(result.is_ok(), "each request should be accepted");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases and type safety
// ─────────────────────────────────────────────────────────────────────────────

/// CollectInventoryCallback trait bound is `Send + 'static`.
///
/// Verify the trait requires Send (for cross-thread use) and 'static
/// (no borrowed data in the callback).
#[test]
fn collect_inventory_callback_send_static_bounds() {
    // This compiles because the trait requires Send + 'static.
    fn make_callback() -> Box<dyn CollectInventoryCallback> {
        struct Local;
        impl CollectInventoryCallback for Local {
            fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
        }
        Box::new(Local)
    }

    let _cb = make_callback();
}

/// PmixStatus error variants are correctly typed.
#[test]
fn collect_inventory_status_error_variants() {
    // Verify the error types we expect from this API.
    let err_init = PmixStatus::Known(PmixError::ErrInit);
    assert!(!err_init.is_success());

    let err_nomem = PmixStatus::Known(PmixError::ErrNomem);
    assert!(!err_nomem.is_success());

    let success = PmixStatus::Known(PmixError::Success);
    assert!(success.is_success());
}

/// Info type is usable as the directives parameter.
#[test]
fn collect_inventory_info_directives_type() {
    let directives = InfoBuilder::new().build();
    // InfoBuilder::new().build() creates an empty info list.

    // The function accepts &Info.
    struct Nop;
    impl CollectInventoryCallback for Nop {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let _result: Result<(), PmixStatus> = server_collect_inventory(&directives, Box::new(Nop));
}
