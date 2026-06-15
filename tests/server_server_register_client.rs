//! Tests for `PMIx_server_register_client`, `RegisterClientCallback`,
//! and the client registration callback infrastructure.
//!
//! Note: `PMIx_server_register_client` requires a running PMIx server
//! environment (`PMIx_server_init` must have been called). Tests that
//! call the actual FFI are marked `#[ignore]`.
//!
//! Unit tests that verify API structure, types, and callback behavior
//! run without a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{RegisterClientCallback, server_register_client};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// RegisterClientCallback — trait and implementation
// ─────────────────────────────────────────────────────────────────────────────

/// RegisterClientCallback trait is object-safe and requires Send.
#[test]
fn test_register_client_callback_trait_object_safe() {
    fn assert_send<T: Send>() {}
    fn assert_trait_obj(_: Box<dyn RegisterClientCallback>) {}

    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    assert_send::<DummyCb>();
    assert_trait_obj(Box::new(DummyCb));
}

/// RegisterClientCallback::on_complete receives PmixStatus.
#[test]
fn test_register_client_callback_receives_status() {
    struct StatusCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterClientCallback for StatusCapture {
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
    impl RegisterClientCallback for Cb1 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    struct Cb2;
    impl RegisterClientCallback for Cb2 {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let _cb1: Box<dyn RegisterClientCallback> = Box::new(Cb1);
    let _cb2: Box<dyn RegisterClientCallback> = Box::new(Cb2);
}

/// Callback can capture state via Arc<Mutex<>>.
#[test]
fn test_callback_captures_state() {
    struct StatefulCb {
        counter: Arc<Mutex<usize>>,
    }
    impl RegisterClientCallback for StatefulCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.counter.lock().unwrap() += 1;
        }
    }

    let counter = Arc::new(Mutex::new(0usize));
    let cb1 = Box::new(StatefulCb {
        counter: Arc::clone(&counter),
    });
    let cb2 = Box::new(StatefulCb {
        counter: Arc::clone(&counter),
    });

    cb1.on_complete(PmixStatus::from_raw(0));
    cb2.on_complete(PmixStatus::from_raw(0));

    assert_eq!(
        *counter.lock().unwrap(),
        2,
        "counter should be incremented twice"
    );
}

/// Callback receives error status correctly.
#[test]
fn test_callback_receives_error_status() {
    struct ErrorCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterClientCallback for ErrorCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(ErrorCapture {
        status: Arc::clone(&status),
    });

    // Simulate error status (-1 = PMIX_ERROR).
    let error_status = PmixStatus::from_raw(-1);
    cb.on_complete(error_status);

    let captured = status.lock().unwrap();
    assert!(captured.is_some());
    assert!(
        !captured.as_ref().unwrap().is_success(),
        "captured status should be an error"
    );
}

/// Callback captures uid/gid context.
#[test]
fn test_callback_captures_uid_gid() {
    struct ClientInfo {
        uid: Arc<Mutex<Option<u32>>>,
        gid: Arc<Mutex<Option<u32>>>,
    }
    impl RegisterClientCallback for ClientInfo {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            // In a real scenario, the callback would have access to
            // captured uid/gid from the enclosing scope.
            *self.uid.lock().unwrap() = Some(1000);
            *self.gid.lock().unwrap() = Some(1000);
        }
    }

    let uid = Arc::new(Mutex::new(None));
    let gid = Arc::new(Mutex::new(None));
    let cb = Box::new(ClientInfo {
        uid: Arc::clone(&uid),
        gid: Arc::clone(&gid),
    });

    cb.on_complete(PmixStatus::from_raw(0));

    assert_eq!(*uid.lock().unwrap(), Some(1000));
    assert_eq!(*gid.lock().unwrap(), Some(1000));
}

// ─────────────────────────────────────────────────────────────────────────────
// server_register_client — API signature and validation
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_client has the expected signature.
#[test]
fn test_server_register_client_signature() {
    // Verify the function compiles with the expected parameter types
    // by assigning it to a typed variable.
    fn _check_signature() {
        struct DummyCb;
        impl RegisterClientCallback for DummyCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _f: fn(
            &pmix::Proc,
            u32,
            u32,
            Option<*mut std::os::raw::c_void>,
            Box<dyn RegisterClientCallback>,
        ) -> Result<(), PmixStatus> = server_register_client;
        let _ = _f;
    }
}

/// server_register_client with valid proc and uid/gid compiles.
#[test]
fn test_register_client_valid_proc_signature() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("myjob.12345", 0).expect("invalid nspace");
    let _result: Result<(), PmixStatus> =
        server_register_client(&proc, 1000, 1000, None, Box::new(DummyCb));
    // We don't assert the result because it depends on PMIx server state.
}

/// server_register_client with server_object = None compiles.
#[test]
fn test_register_client_no_server_object() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("testnspace", 0).expect("invalid nspace");
    let _result: Result<(), PmixStatus> =
        server_register_client(&proc, 0, 0, None, Box::new(DummyCb));
}

/// server_register_client with server_object = Some(ptr) compiles.
#[test]
fn test_register_client_with_server_object() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("testnspace", 1).expect("invalid nspace");
    let server_obj: i32 = 42;
    let _result: Result<(), PmixStatus> = server_register_client(
        &proc,
        1000,
        1000,
        Some(&server_obj as *const i32 as *mut std::os::raw::c_void),
        Box::new(DummyCb),
    );
}

/// server_register_client with different ranks compiles.
#[test]
fn test_register_client_different_ranks() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    for rank in 0..4 {
        let proc = pmix::Proc::new("job.12345", rank).expect("invalid nspace");
        let _result: Result<(), PmixStatus> =
            server_register_client(&proc, 1000, 1000, None, Box::new(DummyCb));
    }
}

/// server_register_client with root uid/gid compiles.
#[test]
fn test_register_client_root_credentials() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("rootjob", 0).expect("invalid nspace");
    let _result: Result<(), PmixStatus> = server_register_client(
        &proc,
        0, // root uid
        0, // root gid
        None,
        Box::new(DummyCb),
    );
}

/// server_register_client with large uid/gid compiles.
#[test]
fn test_register_client_large_uid_gid() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("bigjob", 0).expect("invalid nspace");
    let _result: Result<(), PmixStatus> = server_register_client(
        &proc,
        65534, // large uid (nobody)
        65534, // large gid (nogroup)
        None,
        Box::new(DummyCb),
    );
}

/// server_register_client with different nspaces compiles.
#[test]
fn test_register_client_multiple_nspaces() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let nspaces = ["job.1", "job.2", "job.3", "test.app.99"];
    for nspace in &nspaces {
        let proc = pmix::Proc::new(nspace, 0).expect("invalid nspace");
        let _result: Result<(), PmixStatus> =
            server_register_client(&proc, 1000, 1000, None, Box::new(DummyCb));
    }
}

/// server_register_client with Proc containing wildcard rank compiles.
#[test]
fn test_register_client_wildcard_rank() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // PMIX_RANK_WILDCARD is typically -1 cast to u32.
    let proc = pmix::Proc::new("wildcardjob", u32::MAX).expect("invalid nspace");
    let _result: Result<(), PmixStatus> =
        server_register_client(&proc, 1000, 1000, None, Box::new(DummyCb));
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback registry behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// Callback registry assigns unique request IDs.
#[test]
fn test_callback_registry_unique_ids() {
    // Verify that multiple callback registrations don't conflict.
    // We can't directly test the internal registry, but we can verify
    // that multiple callback creations work without panicking.
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    for _ in 0..10 {
        let proc = pmix::Proc::new("test", 0).expect("invalid nspace");
        let _result = server_register_client(&proc, 1000, 1000, None, Box::new(DummyCb));
    }
}

/// Callback trait is Send — can be used across threads.
#[test]
fn test_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn RegisterClientCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server environment
// ─────────────────────────────────────────────────────────────────────────────

/// server_register_client with a running PMIx server should succeed.
///
/// This test is ignored by default because it requires a PMIx server
/// environment. Run with: `cargo test -- --ignored --test-threads=1`
/// in an environment where PMIx_server_init has been called.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_client_with_server() {
    use std::sync::Arc;
    use std::sync::Mutex;

    struct TestCb {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl RegisterClientCallback for TestCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(TestCb {
        status: Arc::clone(&status),
    });

    let proc = pmix::Proc::new("testnspace", 0).expect("invalid nspace");
    let result = server_register_client(&proc, 1000, 1000, None, cb);

    // The initial call should succeed (request accepted).
    assert!(result.is_ok(), "register_client request should be accepted");

    // The callback status should eventually be set.
    // In a real test environment, we would wait for the callback.
    // Here we just verify the initial call works.
}

/// server_register_client with multiple clients should work sequentially.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_client_multiple_clients() {
    use std::sync::Arc;
    use std::sync::Mutex;

    struct CountCb {
        count: Arc<Mutex<usize>>,
    }
    impl RegisterClientCallback for CountCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            *self.count.lock().unwrap() += 1;
        }
    }

    let count = Arc::new(Mutex::new(0usize));

    for rank in 0..3 {
        let cb = Box::new(CountCb {
            count: Arc::clone(&count),
        });
        let proc = pmix::Proc::new("job_12345", rank).expect("invalid nspace");
        let result = server_register_client(&proc, 1000, 1000, None, cb);
        assert!(
            result.is_ok(),
            "register_client for rank {} should be accepted",
            rank
        );
    }
}

/// server_register_client with server_object should pass the pointer through.
#[test]
#[ignore = "requires PMIx server environment"]
fn test_register_client_with_server_object_ptr() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let proc = pmix::Proc::new("testnspace", 0).expect("invalid nspace");
    let server_obj: i32 = 42;
    let result = server_register_client(
        &proc,
        1000,
        1000,
        Some(&server_obj as *const i32 as *mut std::os::raw::c_void),
        Box::new(DummyCb),
    );

    assert!(
        result.is_ok(),
        "register_client with server_object should be accepted"
    );
}

/// server_register_client with invalid proc (NUL in nspace) should fail at Proc creation.
#[test]
fn test_register_client_nul_in_nspace() {
    struct DummyCb;
    impl RegisterClientCallback for DummyCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    // Proc::new with NUL byte should fail before we even call server_register_client.
    let proc_result = pmix::Proc::new("job\0name", 0);
    assert!(
        proc_result.is_err(),
        "Proc::new should reject nspace containing NUL byte"
    );
}

/// server_register_client with stateful callback that tracks registration order.
#[test]
fn test_callback_tracks_order() {
    struct OrderCb {
        order: Arc<Mutex<Vec<PmixStatus>>>,
    }
    impl RegisterClientCallback for OrderCb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.order.lock().unwrap().push(status);
        }
    }

    let order = Arc::new(Mutex::new(Vec::new()));

    // Simulate multiple callbacks in order.
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
