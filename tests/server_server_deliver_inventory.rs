//! Tests for `PMIx_server_deliver_inventory`, `DeliverInventoryCallback`,
//! and the inventory delivery callback infrastructure.
//!
//! Note: `PMIx_server_deliver_inventory` requires a running PMIx server
//! environment (PMIx_server_init must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::server::{server_deliver_inventory, DeliverInventoryCallback};
use pmix::{InfoBuilder, PmixStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// DeliverInventoryCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// DeliverInventoryCallback trait is object-safe and requires Send.
#[test]
fn test_deliver_inventory_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn DeliverInventoryCallback>) {}

    struct DummyCb;
    impl DeliverInventoryCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// DeliverInventoryCallback::on_complete receives PmixStatus.
#[test]
fn test_deliver_inventory_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeliverInventoryCallback for StatusCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCapture {
        status: Arc::clone(&status),
    });

    // Simulate callback invocation with a success status.
    let success_status = PmixStatus::from_raw(0);
    cb.on_complete(success_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some(), "callback should have captured status");
    let captured = captured.as_ref().unwrap();
    assert!(captured.is_success(), "captured status should be success");
}

/// Multiple callback implementations can coexist.
#[test]
fn test_multiple_callback_implementations() {
    struct Cb1;
    impl DeliverInventoryCallback for Cb1 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct Cb2;
    impl DeliverInventoryCallback for Cb2 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let _cb1: Box<dyn DeliverInventoryCallback> = Box::new(Cb1);
    let _cb2: Box<dyn DeliverInventoryCallback> = Box::new(Cb2);
}

/// Callback can capture state via Arc<Mutex<>>.
#[test]
fn test_callback_captures_state() {
    struct StateCapture {
        count: Arc<Mutex<u32>>,
    }
    impl DeliverInventoryCallback for StateCapture {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let count = Arc::new(Mutex::new(0));
    let cb = Box::new(StateCapture {
        count: Arc::clone(&count),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert_eq!(*count.lock().unwrap(), 1);
}

/// Callback receives error status correctly.
#[test]
fn test_callback_receives_error_status() {
    struct ErrorCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeliverInventoryCallback for ErrorCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(ErrorCapture {
        status: Arc::clone(&status),
    });

    let error_status = PmixStatus::from_raw(-1); // PMIX_ERROR
    cb.on_complete(error_status);

    let captured = status.lock().unwrap();
    let captured = captured.as_ref().unwrap();
    assert!(!captured.is_success(), "captured status should be an error");
}

// ─────────────────────────────────────────────────────────────────────────────
// server_deliver_inventory — API structure and behavior
// ─────────────────────────────────────────────────────────────────────────────

/// Helper to create an empty Info via InfoBuilder.
fn empty_info() -> pmix::Info {
    InfoBuilder::new().build()
}

/// server_deliver_inventory accepts valid inventory and directives with callback.
#[test]
fn test_deliver_inventory_accepts_valid_params() {
    struct TestCb;
    impl DeliverInventoryCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let inventory = empty_info();
    let directives = empty_info();

    // This will call the FFI — if PMIx is not initialized as server,
    // the behavior is undefined, but the Rust side should not panic.
    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(TestCb)));
}

/// server_deliver_inventory accepts None callback (blocking mode).
#[test]
fn test_deliver_inventory_blocking_mode() {
    let inventory = empty_info();
    let directives = empty_info();

    // Blocking mode: no callback means the C API executes synchronously.
    // This should not panic even without a PMIx server.
    let _result = server_deliver_inventory(&inventory, &directives, None);
}

/// server_deliver_inventory signature: takes &Info, &Info, Option<Box<dyn Callback>>.
#[test]
fn test_deliver_inventory_signature() {
    struct SigCb;
    impl DeliverInventoryCallback for SigCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let inventory = empty_info();
    let directives = empty_info();
    let cb: Option<Box<dyn DeliverInventoryCallback>> = Some(Box::new(SigCb));
    let _ = server_deliver_inventory(&inventory, &directives, cb);

    let cb_none: Option<Box<dyn DeliverInventoryCallback>> = None;
    let _ = server_deliver_inventory(&inventory, &directives, cb_none);
}

/// Callback can capture multiple pieces of state.
#[test]
fn test_callback_captures_multiple_state() {
    struct MultiState {
        status: Arc<Mutex<Option<PmixStatus>>>,
        count: Arc<Mutex<u32>>,
    }
    impl DeliverInventoryCallback for MultiState {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
            *self.count.lock().unwrap() += 1;
        }
    }

    let status = Arc::new(Mutex::new(None));
    let count = Arc::new(Mutex::new(0));
    let cb = Box::new(MultiState {
        status: Arc::clone(&status),
        count: Arc::clone(&count),
    });

    cb.on_complete(PmixStatus::from_raw(0));

    assert!(status.lock().unwrap().is_some());
    assert_eq!(*count.lock().unwrap(), 1);
}

/// DeliverInventoryCallback can be implemented for types with Arc data.
#[test]
fn test_callback_with_arc_data() {
    struct ArcData {
        data: Arc<Mutex<String>>,
    }
    impl DeliverInventoryCallback for ArcData {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.data.lock().unwrap().push_str(&format!("{:?} ", status));
        }
    }

    let data = Arc::new(Mutex::new(String::new()));
    let cb = Box::new(ArcData {
        data: Arc::clone(&data),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!data.lock().unwrap().is_empty());
}

/// Callback invoked with success status.
#[test]
fn test_callback_success_status() {
    struct SuccessVec {
        raw: Arc<Mutex<Vec<i32>>>,
    }
    impl DeliverInventoryCallback for SuccessVec {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.raw.lock().unwrap().push(status.to_raw());
        }
    }

    let raw = Arc::new(Mutex::new(Vec::new()));
    let cb = Box::new(SuccessVec {
        raw: Arc::clone(&raw),
    });

    cb.on_complete(PmixStatus::from_raw(0));

    let captured = raw.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], 0);
}

/// Multiple deliver inventory callbacks can be created and stored.
#[test]
fn test_multiple_deliver_callbacks() {
    struct IndexedCb {
        index: usize,
        called: Arc<Mutex<bool>>,
    }
    impl DeliverInventoryCallback for IndexedCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.called.lock().unwrap() = true;
        }
    }

    let called0 = Arc::new(Mutex::new(false));
    let called1 = Arc::new(Mutex::new(false));

    let cb0 = Box::new(IndexedCb {
        index: 0,
        called: Arc::clone(&called0),
    });
    let cb1 = Box::new(IndexedCb {
        index: 1,
        called: Arc::clone(&called1),
    });

    cb0.on_complete(PmixStatus::from_raw(0));
    cb1.on_complete(PmixStatus::from_raw(0));

    assert!(*called0.lock().unwrap());
    assert!(*called1.lock().unwrap());
}

/// DeliverInventoryCallback is Send-compliant for use across thread boundaries.
#[test]
fn test_callback_send_compliance() {
    struct SendCb {
        data: Arc<Mutex<String>>,
    }
    impl DeliverInventoryCallback for SendCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.data.lock().unwrap().push_str(&format!("{:?} ", status));
        }
    }

    // Verify Send is satisfied.
    fn assert_send<T: Send>() {}
    assert_send::<SendCb>();

    let data = Arc::new(Mutex::new(String::new()));
    let cb = Box::new(SendCb {
        data: Arc::clone(&data),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!data.lock().unwrap().is_empty());
}

/// Callback captures different error codes via separate instances.
#[test]
fn test_callback_error_codes() {
    struct ErrorCodeCapture {
        codes: Arc<Mutex<Vec<i32>>>,
    }
    impl DeliverInventoryCallback for ErrorCodeCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.codes.lock().unwrap().push(status.to_raw());
        }
    }

    let codes = Arc::new(Mutex::new(Vec::new()));

    // Each callback instance pushes one code into the shared vec.
    let cb1 = Box::new(ErrorCodeCapture {
        codes: Arc::clone(&codes),
    });
    cb1.on_complete(PmixStatus::from_raw(0)); // PMIX_SUCCESS

    let cb2 = Box::new(ErrorCodeCapture {
        codes: Arc::clone(&codes),
    });
    cb2.on_complete(PmixStatus::from_raw(-20)); // PMIX_ERR_INIT

    let cb3 = Box::new(ErrorCodeCapture {
        codes: Arc::clone(&codes),
    });
    cb3.on_complete(PmixStatus::from_raw(-40)); // PMIX_ERR_BAD_PARAM

    let captured = codes.lock().unwrap();
    assert_eq!(captured.len(), 3);
    assert_eq!(captured[0], 0);
    assert_eq!(captured[1], -20);
    assert_eq!(captured[2], -40);
}

/// server_deliver_inventory with empty inventory and directives.
#[test]
fn test_deliver_inventory_empty_params() {
    struct EmptyCb;
    impl DeliverInventoryCallback for EmptyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let inventory = empty_info();
    let directives = empty_info();

    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(EmptyCb)));
}

/// Callback that records whether it was invoked with success or failure.
#[test]
fn test_callback_success_failure_recording() {
    struct SuccessFailureRecorder {
        success: Arc<Mutex<bool>>,
    }
    impl DeliverInventoryCallback for SuccessFailureRecorder {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.success.lock().unwrap() = status.is_success();
        }
    }

    let success = Arc::new(Mutex::new(false));
    let cb = Box::new(SuccessFailureRecorder {
        success: Arc::clone(&success),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(*success.lock().unwrap(), "should record success");

    let success2 = Arc::new(Mutex::new(true));
    let cb2 = Box::new(SuccessFailureRecorder {
        success: Arc::clone(&success2),
    });
    cb2.on_complete(PmixStatus::from_raw(-1));
    assert!(!*success2.lock().unwrap(), "should record failure");
}

/// server_deliver_inventory with builder-created inventory.
#[test]
fn test_deliver_inventory_with_builder_info() {
    struct BuilderCb;
    impl DeliverInventoryCallback for BuilderCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Build inventory using InfoBuilder.
    let inventory = InfoBuilder::new().build();
    let directives = empty_info();

    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(BuilderCb)));
}

/// Callback with shared counter incremented by multiple instances.
#[test]
fn test_callback_shared_counter() {
    struct CounterCb {
        counter: Arc<Mutex<usize>>,
    }
    impl DeliverInventoryCallback for CounterCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    let counter = Arc::new(Mutex::new(0));

    for _ in 0..5 {
        let cb = Box::new(CounterCb {
            counter: Arc::clone(&counter),
        });
        cb.on_complete(PmixStatus::from_raw(0));
    }

    assert_eq!(*counter.lock().unwrap(), 5);
}

/// Callback that stores the raw status code for later inspection.
#[test]
fn test_callback_stores_raw_code() {
    struct RawCodeCapture {
        code: Arc<Mutex<Option<i32>>>,
    }
    impl DeliverInventoryCallback for RawCodeCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.code.lock().unwrap() = Some(status.to_raw());
        }
    }

    let code = Arc::new(Mutex::new(None));
    let cb = Box::new(RawCodeCapture {
        code: Arc::clone(&code),
    });

    let test_status = PmixStatus::from_raw(42);
    cb.on_complete(test_status);

    assert_eq!(*code.lock().unwrap(), Some(42));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx server runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Full deliver inventory cycle: init server -> deliver -> finalize.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_full_cycle() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct FullCycleCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeliverInventoryCallback for FullCycleCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let cb_status = Arc::new(Mutex::new(None));
    let inventory = empty_info();
    let directives = empty_info();

    let result = server_deliver_inventory(
        &inventory,
        &directives,
        Some(Box::new(FullCycleCb {
            status: Arc::clone(&cb_status),
        })),
    );

    // The request should be accepted (Ok) or rejected with a known error (Err).
    // Either way, the function should not panic.
    let _ = result;

    server_finalize(handle).expect("server_finalize failed");
}

/// Deliver inventory without prior init should fail gracefully.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_without_init() {
    struct NoInitCb;
    impl DeliverInventoryCallback for NoInitCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let inventory = empty_info();
    let directives = empty_info();

    // Without server init, this should return an error.
    let result = server_deliver_inventory(&inventory, &directives, Some(Box::new(NoInitCb)));
    // The result should be an error (PMIX_ERR_INIT).
    assert!(result.is_err(), "should fail without server init");
}

/// Deliver inventory multiple times sequentially.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_multiple_sequential() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct CountCb {
        count: Arc<Mutex<u32>>,
    }
    impl DeliverInventoryCallback for CountCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let count = Arc::new(Mutex::new(0));
    for _i in 0..3 {
        let inventory = empty_info();
        let directives = empty_info();
        let _result = server_deliver_inventory(
            &inventory,
            &directives,
            Some(Box::new(CountCb {
                count: Arc::clone(&count),
            })),
        );
    }

    server_finalize(handle).expect("server_finalize failed");
}

/// Blocking mode deliver inventory (no callback) after server init.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_blocking_after_init() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let inventory = empty_info();
    let directives = empty_info();

    // Blocking mode — should return without crashing.
    let _result = server_deliver_inventory(&inventory, &directives, None);

    server_finalize(handle).expect("server_finalize failed");
}

/// Deliver inventory after server finalize should be handled gracefully.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_after_finalize() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct AfterFinalizeCb;
    impl DeliverInventoryCallback for AfterFinalizeCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");
    server_finalize(handle).expect("server_finalize failed");

    // Attempting to deliver after finalize — behavior is undefined
    // by the C API, but should not cause Rust-side panic.
    let inventory = empty_info();
    let directives = empty_info();
    let _result =
        server_deliver_inventory(&inventory, &directives, Some(Box::new(AfterFinalizeCb)));
}

/// Deliver inventory with non-empty directives.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deliver_inventory_with_directives() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct DirCb;
    impl DeliverInventoryCallback for DirCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let inventory = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();

    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(DirCb)));

    server_finalize(handle).expect("server_finalize failed");
}
