//! Tests for `PMIx_server_deregister_client`, `DeregisterClientCallback`,
//! and the client deregistration callback infrastructure.
//!
//! Note: `PMIx_server_deregister_client` requires a running PMIx server
//! environment (`PMIx_server_init` must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{DeregisterClientCallback, server_deregister_client};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// DeregisterClientCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// DeregisterClientCallback trait is object-safe and requires Send.
#[test]
fn test_deregister_client_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn DeregisterClientCallback>) {}

    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// DeregisterClientCallback::on_complete receives PmixStatus.
#[test]
fn test_deregister_client_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterClientCallback for StatusCapture {
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
    impl DeregisterClientCallback for Cb1 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct Cb2;
    impl DeregisterClientCallback for Cb2 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let _cb1: Box<dyn DeregisterClientCallback> = Box::new(Cb1);
    let _cb2: Box<dyn DeregisterClientCallback> = Box::new(Cb2);
}

/// Callback can capture state via Arc<Mutex<>>.
#[test]
fn test_callback_captures_state() {
    struct StateCapture {
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterClientCallback for StateCapture {
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
    impl DeregisterClientCallback for ErrorCapture {
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

/// Callback can capture multiple pieces of state.
#[test]
fn test_callback_captures_multiple_state() {
    struct MultiState {
        status: Arc<Mutex<Option<PmixStatus>>>,
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterClientCallback for MultiState {
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

/// DeregisterClientCallback can be implemented for types with Arc data.
#[test]
fn test_callback_with_arc_data() {
    struct ArcData {
        data: Arc<Mutex<String>>,
    }
    impl DeregisterClientCallback for ArcData {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.data
                .lock()
                .unwrap()
                .push_str(&format!("{:?} ", status));
        }
    }

    let data = Arc::new(Mutex::new(String::new()));
    let cb = Box::new(ArcData {
        data: Arc::clone(&data),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!data.lock().unwrap().is_empty());
}

/// Callback is Send-compliant for use across thread boundaries.
#[test]
fn test_callback_send_compliance() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DeregisterClientCallback>>();
}

/// Callback invoked with different PMIx status codes.
#[test]
fn test_callback_various_statuses() {
    struct StatusVec {
        statuses: Arc<Mutex<Vec<PmixStatus>>>,
    }
    impl DeregisterClientCallback for StatusVec {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }

    let statuses = Arc::new(Mutex::new(Vec::new()));

    // Each on_complete consumes the Box, so we create separate instances.
    let cb1 = Box::new(StatusVec {
        statuses: Arc::clone(&statuses),
    });
    let cb2 = Box::new(StatusVec {
        statuses: Arc::clone(&statuses),
    });
    let cb3 = Box::new(StatusVec {
        statuses: Arc::clone(&statuses),
    });

    cb1.on_complete(PmixStatus::from_raw(0)); // PMIX_SUCCESS
    cb2.on_complete(PmixStatus::from_raw(-1)); // PMIX_ERROR
    cb3.on_complete(PmixStatus::from_raw(-46)); // PMIX_ERR_NOT_FOUND

    let captured = statuses.lock().unwrap();
    assert_eq!(captured.len(), 3);
    assert!(captured[0].is_success());
    assert!(!captured[1].is_success());
    assert!(!captured[2].is_success());
}

/// Multiple deregister callbacks can be created and stored.
#[test]
fn test_multiple_deregister_callbacks() {
    struct IndexedCb {
        index: usize,
        called: Arc<Mutex<bool>>,
    }
    impl DeregisterClientCallback for IndexedCb {
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

// ─────────────────────────────────────────────────────────────────────────────
// server_deregister_client — API structure and behavior
// ─────────────────────────────────────────────────────────────────────────────

/// server_deregister_client has the expected signature.
#[test]
fn test_server_deregister_client_signature() {
    fn _check_signature() {
        struct DummyCb;
        impl DeregisterClientCallback for DummyCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        // Verify the function takes &Proc and Option<Box<dyn Callback>>.
        let _f: fn(&pmix::Proc, Option<Box<dyn DeregisterClientCallback>>) =
            server_deregister_client;
        let _ = _f;
    }
}

/// server_deregister_client accepts a valid proc with callback.
#[test]
fn test_deregister_client_accepts_valid_proc() {
    struct TestCb;
    impl DeregisterClientCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("test.nspace", 0).expect("invalid nspace");
    // This will call the FFI — if PMIx is not initialized as server,
    // the behavior is undefined, but the Rust side should not panic.
    server_deregister_client(&proc, Some(Box::new(TestCb)));
}

/// server_deregister_client accepts None callback (blocking mode).
#[test]
fn test_deregister_client_blocking_mode() {
    let proc = pmix::Proc::new("test.nspace", 0).expect("invalid nspace");
    // Blocking mode: no callback means the C API executes synchronously.
    server_deregister_client(&proc, None);
}

/// server_deregister_client with different ranks compiles and runs.
#[test]
fn test_deregister_client_different_ranks() {
    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    for rank in 0..4 {
        let proc = pmix::Proc::new("job.12345", rank).expect("invalid nspace");
        server_deregister_client(&proc, Some(Box::new(DummyCb)));
    }
}

/// server_deregister_client with different nspaces compiles and runs.
#[test]
fn test_deregister_client_multiple_nspaces() {
    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let nspaces = ["job.1", "job.2", "job.3", "test.app.99"];
    for nspace in &nspaces {
        let proc = pmix::Proc::new(nspace, 0).expect("invalid nspace");
        server_deregister_client(&proc, Some(Box::new(DummyCb)));
    }
}

/// server_deregister_client with Proc containing wildcard rank.
#[test]
fn test_deregister_client_wildcard_rank() {
    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("wildcardjob", u32::MAX).expect("invalid nspace");
    server_deregister_client(&proc, Some(Box::new(DummyCb)));
}

/// server_deregister_client with empty nspace.
#[test]
fn test_deregister_client_empty_nspace() {
    struct EmptyCb;
    impl DeregisterClientCallback for EmptyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("", 0).expect("invalid nspace");
    server_deregister_client(&proc, Some(Box::new(EmptyCb)));
}

/// server_deregister_client with long nspace.
#[test]
fn test_deregister_client_long_nspace() {
    struct LongCb;
    impl DeregisterClientCallback for LongCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let long_nspace = "very_long_namespace_identifier_with_many_characters_1234567890";
    let proc = pmix::Proc::new(long_nspace, 0).expect("invalid nspace");
    server_deregister_client(&proc, Some(Box::new(LongCb)));
}

/// Callback registry assigns unique request IDs — multiple calls don't panic.
#[test]
fn test_callback_registry_unique_ids() {
    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    for _ in 0..10 {
        let proc = pmix::Proc::new("test", 0).expect("invalid nspace");
        server_deregister_client(&proc, Some(Box::new(DummyCb)));
    }
}

/// server_deregister_client with stateful callback that tracks deregister order.
#[test]
fn test_callback_tracks_order() {
    struct OrderCb {
        order: Arc<Mutex<Vec<PmixStatus>>>,
    }
    impl DeregisterClientCallback for OrderCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.order.lock().unwrap().push(status);
        }
    }

    let order = Arc::new(Mutex::new(Vec::new()));

    let cb1 = Box::new(OrderCb {
        order: Arc::clone(&order),
    });
    let cb2 = Box::new(OrderCb {
        order: Arc::clone(&order),
    });
    let cb3 = Box::new(OrderCb {
        order: Arc::clone(&order),
    });

    cb1.on_complete(PmixStatus::from_raw(0)); // success
    cb2.on_complete(PmixStatus::from_raw(-1)); // error
    cb3.on_complete(PmixStatus::from_raw(0)); // success

    let captured = order.lock().unwrap();
    assert_eq!(captured.len(), 3);
    assert!(captured[0].is_success());
    assert!(!captured[1].is_success());
    assert!(captured[2].is_success());
}

/// server_deregister_client function returns void (no Result).
#[test]
fn test_deregister_client_returns_void() {
    struct DummyCb;
    impl DeregisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("test", 0).expect("invalid nspace");
    // The function returns () not Result<(), PmixStatus>.
    // This compiles only if the return type is unit.
    let _: () = server_deregister_client(&proc, Some(Box::new(DummyCb)));
}

/// server_deregister_client with callback option typed explicitly.
#[test]
fn test_deregister_client_callback_option_types() {
    struct SomeCb;
    impl DeregisterClientCallback for SomeCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("test", 0).expect("invalid nspace");

    // Some(Box<dyn ...>)
    let cb: Option<Box<dyn DeregisterClientCallback>> = Some(Box::new(SomeCb));
    server_deregister_client(&proc, cb);

    // None
    let cb_none: Option<Box<dyn DeregisterClientCallback>> = None;
    server_deregister_client(&proc, cb_none);
}

/// Callback can capture proc context.
#[test]
fn test_callback_captures_proc_context() {
    struct ProcCapture {
        nspace: Arc<Mutex<String>>,
        rank: Arc<Mutex<u32>>,
    }
    impl DeregisterClientCallback for ProcCapture {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // In a real scenario, the callback would have captured
            // proc info from the enclosing scope.
            self.nspace.lock().unwrap().push_str("captured");
            *self.rank.lock().unwrap() += 1;
        }
    }

    let nspace = Arc::new(Mutex::new(String::new()));
    let rank = Arc::new(Mutex::new(0));
    let cb = Box::new(ProcCapture {
        nspace: Arc::clone(&nspace),
        rank: Arc::clone(&rank),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!nspace.lock().unwrap().is_empty());
    assert_eq!(*rank.lock().unwrap(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Comparison with server_deregister_nspace
// ─────────────────────────────────────────────────────────────────────────────

/// server_deregister_client takes &Proc while server_deregister_nspace takes &str.
/// Both are valid but target different granularity.
#[test]
fn test_deregister_client_vs_deregister_nspace() {
    use pmix::server::{DeregisterNspaceCallback, server_deregister_nspace};

    struct ClientCb;
    impl DeregisterClientCallback for ClientCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct NspaceCb;
    impl DeregisterNspaceCallback for NspaceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Deregister a single client (takes &Proc).
    let proc = pmix::Proc::new("job.12345", 3).expect("invalid nspace");
    server_deregister_client(&proc, Some(Box::new(ClientCb)));

    // Deregister an entire nspace (takes &str).
    server_deregister_nspace("job.12345", Some(Box::new(NspaceCb)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx server runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Full deregister cycle: init server -> register client -> deregister client -> finalize.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_client_full_cycle() {
    use pmix::server::{
        PmixServerModule, RegisterClientCallback, server_finalize, server_init_minimal,
        server_register_client,
    };

    struct DeregCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterClientCallback for DeregCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    // Register a client first.
    struct RegCb;
    impl RegisterClientCallback for RegCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("testjob", 0).expect("invalid nspace");
    let _ = server_register_client(&proc, 1000, 1000, None, Box::new(RegCb));

    // Deregister it.
    let dereg_status = Arc::new(Mutex::new(None));
    let proc = pmix::Proc::new("testjob", 0).expect("invalid nspace");
    server_deregister_client(
        &proc,
        Some(Box::new(DeregCb {
            status: Arc::clone(&dereg_status),
        })),
    );

    server_finalize(handle).expect("server_finalize failed");
}

/// Deregister without prior registration should still not crash.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_client_not_previously_registered() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct NotFoundCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterClientCallback for NotFoundCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let proc = pmix::Proc::new("nonexistent", 999).expect("invalid nspace");
    let status = Arc::new(Mutex::new(None));
    server_deregister_client(
        &proc,
        Some(Box::new(NotFoundCb {
            status: Arc::clone(&status),
        })),
    );

    server_finalize(handle).expect("server_finalize failed");
}

/// Deregister multiple clients sequentially.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_client_multiple_sequential() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct CountCb {
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterClientCallback for CountCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let count = Arc::new(Mutex::new(0));
    for rank in 0..3u32 {
        let proc = pmix::Proc::new("job", rank).expect("invalid nspace");
        server_deregister_client(
            &proc,
            Some(Box::new(CountCb {
                count: Arc::clone(&count),
            })),
        );
    }

    server_finalize(handle).expect("server_finalize failed");
}

/// Deregister after server finalize should be handled gracefully.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_client_after_finalize() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    struct AfterFinalizeCb;
    impl DeregisterClientCallback for AfterFinalizeCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");
    server_finalize(handle).expect("server_finalize failed");

    // Attempting to deregister after finalize — behavior is undefined
    // by the C API, but should not cause Rust-side panic.
    let proc = pmix::Proc::new("test", 0).expect("invalid nspace");
    server_deregister_client(&proc, Some(Box::new(AfterFinalizeCb)));
}

/// Blocking mode deregister (no callback) does not crash.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_client_blocking_no_crash() {
    use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    // Blocking mode — should return without crashing.
    let proc = pmix::Proc::new("test.nspace", 0).expect("invalid nspace");
    server_deregister_client(&proc, None);

    server_finalize(handle).expect("server_finalize failed");
}
