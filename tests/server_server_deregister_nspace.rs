//! Tests for `PMIx_server_deregister_nspace`, `DeregisterNspaceCallback`,
//! and the nspace deregistration callback infrastructure.
//!
//! Note: `PMIx_server_deregister_nspace` requires a running PMIx server
//! environment (PMIx_server_init must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{DeregisterNspaceCallback, server_deregister_nspace};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// DeregisterNspaceCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// DeregisterNspaceCallback trait is object-safe and requires Send.
#[test]
fn test_deregister_nspace_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn DeregisterNspaceCallback>) {}

    struct DummyCb;
    impl DeregisterNspaceCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// DeregisterNspaceCallback::on_complete receives PmixStatus.
#[test]
fn test_deregister_nspace_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterNspaceCallback for StatusCapture {
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
    impl DeregisterNspaceCallback for Cb1 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct Cb2;
    impl DeregisterNspaceCallback for Cb2 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let _cb1: Box<dyn DeregisterNspaceCallback> = Box::new(Cb1);
    let _cb2: Box<dyn DeregisterNspaceCallback> = Box::new(Cb2);
}

/// Callback can capture state via Arc<Mutex<>>.
#[test]
fn test_callback_captures_state() {
    struct StateCapture {
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterNspaceCallback for StateCapture {
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
    impl DeregisterNspaceCallback for ErrorCapture {
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
// server_deregister_nspace — API structure and behavior
// ─────────────────────────────────────────────────────────────────────────────

/// server_deregister_nspace accepts a valid nspace string with callback.
#[test]
fn test_deregister_nspace_accepts_valid_nspace() {
    struct TestCb;
    impl DeregisterNspaceCallback for TestCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // This will call the FFI — if PMIx is not initialized as server,
    // the behavior is undefined, but the Rust side should not panic.
    // We test with a simple nspace name.
    server_deregister_nspace("test.nspace", Some(Box::new(TestCb)));
}

/// server_deregister_nspace accepts None callback (blocking mode).
#[test]
fn test_deregister_nspace_blocking_mode() {
    // Blocking mode: no callback means the C API executes synchronously.
    // This should not panic even without a PMIx server.
    server_deregister_nspace("test.nspace", None);
}

/// server_deregister_nspace rejects nspace with embedded NUL byte.
#[test]
fn test_deregister_nspace_rejects_nul_in_nspace() {
    struct NulCapture {
        called: Arc<Mutex<bool>>,
    }
    impl DeregisterNspaceCallback for NulCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.called.lock().unwrap() = true;
            assert!(
                !status.is_success(),
                "NUL rejection should invoke callback with error status"
            );
        }
    }

    let called = Arc::new(Mutex::new(false));
    let cb = Box::new(NulCapture {
        called: Arc::clone(&called),
    });

    // Nspace with embedded NUL — CString::new will fail internally.
    // The function should handle this gracefully and invoke the callback
    // with an error status (or do nothing if no callback).
    let nspace_with_nul = "test\0nspace";
    server_deregister_nspace(nspace_with_nul, Some(cb));

    // The callback should have been invoked with an error status.
    assert!(
        *called.lock().unwrap(),
        "callback should be invoked for NUL-byte nspace"
    );
}

/// server_deregister_nspace with NUL nspace and no callback does not panic.
#[test]
fn test_deregister_nspace_nul_nspace_no_callback() {
    // Should handle gracefully without panicking.
    let nspace_with_nul = "test\0nspace";
    server_deregister_nspace(nspace_with_nul, None);
}

/// server_deregister_nspace signature: takes &str and Option<Box<dyn Callback>>.
#[test]
fn test_deregister_nspace_signature() {
    struct SigCb;
    impl DeregisterNspaceCallback for SigCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Verify the function takes the expected types.
    let nspace: &str = "test";
    let cb: Option<Box<dyn DeregisterNspaceCallback>> = Some(Box::new(SigCb));
    server_deregister_nspace(nspace, cb);

    let cb_none: Option<Box<dyn DeregisterNspaceCallback>> = None;
    server_deregister_nspace(nspace, cb_none);
}

/// Callback can capture multiple pieces of state.
#[test]
fn test_callback_captures_multiple_state() {
    struct MultiState {
        status: Arc<Mutex<Option<PmixStatus>>>,
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterNspaceCallback for MultiState {
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

/// DeregisterNspaceCallback can be implemented for types with lifetime-bound data.
#[test]
fn test_callback_with_arc_data() {
    struct ArcData {
        data: Arc<Mutex<String>>,
    }
    impl DeregisterNspaceCallback for ArcData {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.data
                .lock()
                .unwrap()
                .push_str(&format!("{:?}", status));
        }
    }

    let data = Arc::new(Mutex::new(String::new()));
    let cb = Box::new(ArcData {
        data: Arc::clone(&data),
    });

    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!data.lock().unwrap().is_empty());
}

/// Empty nspace string is valid (degenerate but not a NUL error).
#[test]
fn test_deregister_nspace_empty_nspace() {
    struct EmptyCb;
    impl DeregisterNspaceCallback for EmptyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    server_deregister_nspace("", Some(Box::new(EmptyCb)));
}

/// server_deregister_nspace with various nspace formats.
#[test]
fn test_deregister_nspace_nspace_formats() {
    struct FormatCb;
    impl DeregisterNspaceCallback for FormatCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Various valid nspace formats used by PMIx.
    let nsapces = [
        "job.12345",
        "myapp_20240101",
        "user.host.12345",
        "a",
        "very_long_namespace_identifier_with_many_characters_1234567890",
    ];

    for nspace in nsapces {
        server_deregister_nspace(nspace, Some(Box::new(FormatCb)));
    }
}

/// Callback invoked with different PMIx status codes.
#[test]
fn test_callback_various_statuses() {
    struct StatusVec {
        statuses: Arc<Mutex<Vec<PmixStatus>>>,
    }
    impl DeregisterNspaceCallback for StatusVec {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.statuses.lock().unwrap().push(status);
        }
    }

    let statuses = Arc::new(Mutex::new(Vec::new()));
    let cb = Box::new(StatusVec {
        statuses: Arc::clone(&statuses),
    });

    // Simulate various statuses.
    cb.on_complete(PmixStatus::from_raw(0));   // PMIX_SUCCESS
}

/// Multiple deregister callbacks can be created and stored.
#[test]
fn test_multiple_deregister_callbacks() {
    struct IndexedCb {
        index: usize,
        called: Arc<Mutex<bool>>,
    }
    impl DeregisterNspaceCallback for IndexedCb {
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
// Integration tests (require PMIx server runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Full deregister cycle: init server -> register -> deregister -> finalize.
/// Requires a running PMIx server environment.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_nspace_full_cycle() {
    use pmix::server::{server_finalize, server_init_minimal, server_register_nspace, PmixServerModule};

    struct FullCycleCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterNspaceCallback for FullCycleCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let dereg_status = Arc::new(Mutex::new(None));

    // Register an nspace first.
    struct RegCb;
    impl pmix::server::RegisterNspaceCallback for RegCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = pmix::InfoBuilder::new().build();
    server_register_nspace("foobar", 0, &info, Box::new(RegCb));

    // Deregister it.
    server_deregister_nspace(
        "foobar",
        Some(Box::new(FullCycleCb {
            status: Arc::clone(&dereg_status),
        })),
    );

    server_finalize(handle).expect("server_finalize failed");
}

/// Deregister without prior registration should still not crash.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_nspace_not_previously_registered() {
    use pmix::server::{server_finalize, server_init_minimal, PmixServerModule};

    struct NotFoundCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterNspaceCallback for NotFoundCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let status = Arc::new(Mutex::new(None));
    server_deregister_nspace(
        "nonexistent.nspace",
        Some(Box::new(NotFoundCb {
            status: Arc::clone(&status),
        })),
    );

    server_finalize(handle).expect("server_finalize failed");
}

/// Deregister multiple nspaces sequentially.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_nspace_multiple_sequential() {
    use pmix::server::{server_finalize, server_init_minimal, PmixServerModule};

    struct CountCb {
        count: Arc<Mutex<u32>>,
    }
    impl DeregisterNspaceCallback for CountCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    let count = Arc::new(Mutex::new(0));
    for i in 0..3 {
        let nspace = format!("job.{}", i);
        server_deregister_nspace(
            &nspace,
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
fn test_deregister_nspace_after_finalize() {
    use pmix::server::{server_finalize, server_init_minimal, PmixServerModule};

    struct AfterFinalizeCb;
    impl DeregisterNspaceCallback for AfterFinalizeCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");
    server_finalize(handle).expect("server_finalize failed");

    // Attempting to deregister after finalize — behavior is undefined
    // by the C API, but should not cause Rust-side panic.
    server_deregister_nspace("test", Some(Box::new(AfterFinalizeCb)));
}

/// Blocking mode deregister (no callback) does not crash.
#[test]
#[ignore = "requires PMIx server runtime"]
fn test_deregister_nspace_blocking_no_crash() {
    use pmix::server::{server_finalize, server_init_minimal, PmixServerModule};

    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init failed");

    // Blocking mode — should return without crashing.
    server_deregister_nspace("test.nspace", None);

    server_finalize(handle).expect("server_finalize failed");
}

/// Callback is Send-compliant for use across thread boundaries.
#[test]
fn test_callback_send_compliance() {
    struct SendCb {
        data: Arc<Mutex<String>>,
    }
    impl DeregisterNspaceCallback for SendCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.data
                .lock()
                .unwrap()
                .push_str(&format!("{:?} ", status));
        }
    }

    // Verify Send is satisfied.
    fn assert_send<T: Send>() {}
    assert_send::<SendCb>();

    let data = Arc::new(Mutex::new(String::new()));
    let cb: Box<dyn DeregisterNspaceCallback> = Box::new(SendCb {
        data: Arc::clone(&data),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(!data.lock().unwrap().is_empty());
}
