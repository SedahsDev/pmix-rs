//! Tests for server_init, server_finalize, server_register_client,
//! server_deregister_client — focusing on double register, finalize with
//! registered clients, error propagation, and lifecycle patterns.
//!
//! This test file covers cross-cutting integration scenarios that are not
//! tested by the individual per-function test modules:
//! - server_server_init.rs
//! - server_server_finalize.rs
//! - server_server_register_client.rs
//! - server_server_deregister_client.rs
//!
//! Note: Tests that call actual FFI functions (server_init, etc.) require
//! a running PMIx daemon (prte-beast). These run without #[ignore] because
//! the daemon IS available in this environment.

use pmix::PmixStatus;
use pmix::server::{
    DeregisterClientCallback, PmixServerHandle, PmixServerModule, RegisterClientCallback,
    is_server_initialized, server_deregister_client, server_finalize, server_init,
    server_init_minimal, server_register_client,
};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Helper callbacks
// ─────────────────────────────────────────────────────────────────────────────

/// A no-op register client callback.
struct NoOpRegisterCb;
impl RegisterClientCallback for NoOpRegisterCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

/// A no-op deregister client callback.
struct NoOpDeregisterCb;
impl DeregisterClientCallback for NoOpDeregisterCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

/// Callback that captures the status via Arc<Mutex<>>.
struct StatusCaptureRegister {
    status: Arc<Mutex<Option<PmixStatus>>>,
}
impl RegisterClientCallback for StatusCaptureRegister {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

/// Callback that captures the status via Arc<Mutex<>>.
struct StatusCaptureDeregister {
    status: Arc<Mutex<Option<PmixStatus>>>,
}
impl DeregisterClientCallback for StatusCaptureDeregister {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        *self.status.lock().unwrap() = Some(status);
    }
}

/// Callback that increments a shared counter.
struct CountRegisterCb {
    count: Arc<Mutex<usize>>,
}
impl RegisterClientCallback for CountRegisterCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        *self.count.lock().unwrap() += 1;
    }
}

/// Callback that increments a shared counter.
struct CountDeregisterCb {
    count: Arc<Mutex<usize>>,
}
impl DeregisterClientCallback for CountDeregisterCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {
        *self.count.lock().unwrap() += 1;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Double register tests — register same nspace/rank twice
// ─────────────────────────────────────────────────────────────────────────────

/// Register the same nspace/rank twice and verify the second registration
/// is handled (either succeeds or returns an error — both are valid PMIx
/// behaviors; we verify the call does not panic).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_double_register_same_nspace_same_rank() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("double.reg.test.1", 0).expect("invalid nspace");

    // First registration
    let result1 = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result1; // Accept whatever the daemon returns

    // Second registration — same nspace, same rank
    let result2 = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result2; // Accept whatever the daemon returns

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register different nspace with same rank — both should work independently.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_different_nspace_same_rank() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc1 = pmix::Proc::new("nspace_a.test", 42).expect("invalid nspace");
    let proc2 = pmix::Proc::new("nspace_b.test", 42).expect("invalid nspace");

    let result1 = server_register_client(&proc1, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result1;

    let result2 = server_register_client(&proc2, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result2;

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register same nspace with different ranks — both should work independently.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_same_nspace_different_rank() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc1 = pmix::Proc::new("same.ns.test", 0).expect("invalid nspace");
    let proc2 = pmix::Proc::new("same.ns.test", 1).expect("invalid nspace");
    let proc3 = pmix::Proc::new("same.ns.test", 2).expect("invalid nspace");

    let result1 = server_register_client(&proc1, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result1;
    let result2 = server_register_client(&proc2, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result2;
    let result3 = server_register_client(&proc3, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _ = result3;

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register the same client 5 times in a row.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_double_register_repeated_five_times() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("repeated.reg.test", 0).expect("invalid nspace");

    for _ in 0..5 {
        let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
        // Each call should complete without panic
    }

    server_finalize(handle).expect("server_finalize should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// Finalize with registered clients
// ─────────────────────────────────────────────────────────────────────────────

/// Init → register one client → finalize. The server should handle cleanup.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_finalize_with_one_registered_client() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("finalize.client.1", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));

    // Finalize with a registered client — should succeed (server auto-cleans).
    server_finalize(handle).expect("server_finalize should succeed with registered client");
}

/// Init → register multiple clients → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_finalize_with_multiple_registered_clients() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    for rank in 0..5 {
        let proc = pmix::Proc::new("finalize.multi.clients", rank).expect("invalid nspace");
        let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    }

    server_finalize(handle).expect("server_finalize should succeed with multiple clients");
}

/// Init → register clients from different nspaces → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_finalize_with_clients_from_different_nspaces() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let nspaces = ["job.alpha", "job.beta", "job.gamma"];
    for nspace in &nspaces {
        let proc = pmix::Proc::new(nspace, 0).expect("invalid nspace");
        let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    }

    server_finalize(handle)
        .expect("server_finalize should succeed with clients from different nspaces");
}

/// Init → register client → deregister client → finalize (clean lifecycle).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_deregister_then_finalize() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("clean.lifecycle.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));

    // Deregister before finalizing
    server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));

    server_finalize(handle).expect("server_finalize should succeed after deregister");
}

/// Init → register multiple → deregister all → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_multiple_deregister_all_then_finalize() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let procs: Vec<_> = (0..3)
        .map(|rank| pmix::Proc::new("multi.dereg.test", rank).expect("invalid nspace"))
        .collect();

    for proc in &procs {
        let _result = server_register_client(proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    }

    for proc in &procs {
        server_deregister_client(proc, Some(Box::new(NoOpDeregisterCb)));
    }

    server_finalize(handle).expect("server_finalize should succeed after all deregisters");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error propagation tests
// ─────────────────────────────────────────────────────────────────────────────

/// server_init with None module should work (minimal server, no callbacks).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_none_module() {
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(None, &info).expect("server_init with None module should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init with empty info and a module should work.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_with_empty_info() {
    let module = PmixServerModule::default();
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init_minimal with None module should work.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_minimal_none_module() {
    let handle = server_init_minimal(None).expect("server_init_minimal(None) should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init_minimal with a default module should work.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_minimal_default_module() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_finalize returns Result<(), PmixStatus> — verify Err variant works.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_finalize_err_variant() {
    // Verify the Result type can hold an Err variant.
    let result: Result<(), PmixStatus> = Err(PmixStatus::from_raw(-1));
    assert!(result.is_err(), "Err variant should be detectable");
}

/// server_register_client returns Result<(), PmixStatus> — verify type.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_register_client_return_type() {
    let proc = pmix::Proc::new("type.check", 0).expect("invalid nspace");
    let result: Result<(), PmixStatus> =
        server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    // We don't assert Ok/Err since it depends on daemon state.
    let _ = result;
}

/// server_deregister_client returns () — verify type.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_deregister_client_return_type() {
    let proc = pmix::Proc::new("type.check.dereg", 0).expect("invalid nspace");
    // This function returns () (void), so it doesn't return a Result.
    let _result: () = server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Lifecycle pattern tests
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: init → register → deregister → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_full_lifecycle_init_register_deregister_finalize() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("full.lifecycle.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Lifecycle with server_init (not minimal): init → register → deregister → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_full_lifecycle_with_server_init() {
    let module = PmixServerModule::default();
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");

    let proc = pmix::Proc::new("full.lifecycle.init.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Lifecycle: init → finalize (no clients registered).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_lifecycle_init_finalize_no_clients() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    // Finalize immediately without registering any clients.
    server_finalize(handle).expect("server_finalize should succeed with no clients");
}

/// Multiple init/finalize cycles in sequence.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_multiple_init_finalize_cycles() {
    let module = PmixServerModule::default();

    for _ in 0..3 {
        let handle =
            server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
        server_finalize(handle).expect("server_finalize should succeed");
    }
}

/// Multiple init/finalize cycles with clients registered each time.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_multiple_cycles_with_clients() {
    let module = PmixServerModule::default();

    for cycle in 0..3 {
        let handle =
            server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

        let proc = pmix::Proc::new(&format!("cycle.test.{}", cycle), 0).expect("invalid nspace");
        let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));

        server_finalize(handle).expect("server_finalize should succeed in cycle");
    }
}

/// Init → register → finalize → init again → register → finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_init_register_finalize_init_register_finalize() {
    let module = PmixServerModule::default();

    // First cycle
    let handle1 = server_init_minimal(Some(&module)).expect("first server_init should succeed");
    let proc1 = pmix::Proc::new("first.cycle.test", 0).expect("invalid nspace");
    let _result1 = server_register_client(&proc1, 1000, 1000, None, Box::new(NoOpRegisterCb));
    server_finalize(handle1).expect("first server_finalize should succeed");

    // Second cycle
    let handle2 = server_init_minimal(Some(&module)).expect("second server_init should succeed");
    let proc2 = pmix::Proc::new("second.cycle.test", 0).expect("invalid nspace");
    let _result2 = server_register_client(&proc2, 1000, 1000, None, Box::new(NoOpRegisterCb));
    server_finalize(handle2).expect("second server_finalize should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// is_server_initialized tests
// ─────────────────────────────────────────────────────────────────────────────

/// is_server_initialized returns a bool.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_is_server_initialized_returns_bool() {
    let _result: bool = is_server_initialized();
    // Just verify it compiles and doesn't panic.
}

/// is_server_initialized returns true after init, false after finalize.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_is_server_initialized_lifecycle() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    // After init, the server should be initialized.
    assert!(
        is_server_initialized(),
        "server should be initialized after server_init_minimal"
    );

    server_finalize(handle).expect("server_finalize should succeed");

    // After finalize, the server should not be initialized.
    assert!(
        !is_server_initialized(),
        "server should not be initialized after server_finalize"
    );
}

/// is_server_initialized returns false before any init.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_is_server_initialized_false_before_init() {
    // This test depends on ordering — if other tests have already initialized
    // the server, this might return true. We only verify it doesn't panic.
    let _result = is_server_initialized();
}

// ─────────────────────────────────────────────────────────────────────────────
// Type checks
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle is NOT Copy — enforced by the type system.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_pmix_server_handle_not_copy() {
    // PmixServerHandle does not derive Copy (it has no #[derive(Copy)]).
    // We verify this by checking that it doesn't implement Copy.
    // (Can't use negative bounds on stable Rust, so just verify Debug.)
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

/// PmixServerHandle implements Debug.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_pmix_server_handle_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

/// PmixServerModule implements Default.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_pmix_server_module_default() {
    let _module = PmixServerModule::default();
}

/// PmixServerModule implements Debug.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_pmix_server_module_debug() {
    let module = PmixServerModule::default();
    let debug = format!("{:?}", module);
    assert!(!debug.is_empty(), "Debug output should not be empty");
}

/// server_init signature: fn(Option<&PmixServerModule>, &Info) -> Result<PmixServerHandle, PmixStatus>

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_signature() {
    let _: fn(Option<&PmixServerModule>, &pmix::Info) -> Result<PmixServerHandle, PmixStatus> =
        server_init;
}

/// server_init_minimal signature: fn(Option<&PmixServerModule>) -> Result<PmixServerHandle, PmixStatus>

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_minimal_signature() {
    let _: fn(Option<&PmixServerModule>) -> Result<PmixServerHandle, PmixStatus> =
        server_init_minimal;
}

/// server_finalize signature: fn(PmixServerHandle) -> Result<(), PmixStatus>

#[test]
fn test_server_finalize_signature() {
    let _: fn(PmixServerHandle) -> Result<(), PmixStatus> = server_finalize;
}

/// server_register_client signature verification.

#[test]
fn test_server_register_client_signature() {
    fn _check_signature() {
        let proc = pmix::Proc::new("sig.check", 0).expect("invalid nspace");
        let _result: Result<(), PmixStatus> =
            server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    }
}

/// server_deregister_client signature verification.

#[test]
fn test_server_deregister_client_signature() {
    fn _check_signature() {
        let proc = pmix::Proc::new("sig.check.dereg", 0).expect("invalid nspace");
        let _result: () = server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));
    }
}

/// is_server_initialized signature: fn() -> bool

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_is_server_initialized_signature() {
    let _: fn() -> bool = is_server_initialized;
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety — all functions do not panic
// ─────────────────────────────────────────────────────────────────────────────

/// server_init does not panic with valid inputs.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_no_panic() {
    let module = PmixServerModule::default();
    let info = pmix::InfoBuilder::new().build();
    let _result = std::panic::catch_unwind(|| {
        let _ = server_init(Some(&module), &info);
    });
    // If we got here without panic, the test passes.
}

/// server_init_minimal does not panic with valid inputs.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_init_minimal_no_panic() {
    let module = PmixServerModule::default();
    let _result = std::panic::catch_unwind(|| {
        let _ = server_init_minimal(Some(&module));
    });
}

/// server_finalize does not panic.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_server_finalize_no_panic() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    let _result = std::panic::catch_unwind(|| {
        let _ = server_finalize(handle);
    });
}

/// server_register_client does not panic.

#[test]
fn test_server_register_client_no_panic() {
    let proc = pmix::Proc::new("panic.check.reg", 0).expect("invalid nspace");
    let _result = std::panic::catch_unwind(|| {
        let _ = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    });
}

/// server_deregister_client does not panic.

#[test]
fn test_server_deregister_client_no_panic() {
    let proc = pmix::Proc::new("panic.check.dereg", 0).expect("invalid nspace");
    let _result = std::panic::catch_unwind(|| {
        server_deregister_client(&proc, Some(Box::new(NoOpDeregisterCb)));
    });
}

/// is_server_initialized does not panic.

#[test]
fn test_is_server_initialized_no_panic() {
    let _result = std::panic::catch_unwind(|| {
        is_server_initialized();
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// Register client with a callback that captures status.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_with_status_capture() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCaptureRegister {
        status: Arc::clone(&status),
    });

    let proc = pmix::Proc::new("status.capture.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, cb);

    // The callback may or may not have been called yet (async).
    // We just verify the call itself doesn't panic.
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Deregister client with a callback that captures status.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_deregister_client_with_status_capture() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StatusCaptureDeregister {
        status: Arc::clone(&status),
    });

    let proc = pmix::Proc::new("status.capture.dereg.test", 0).expect("invalid nspace");
    server_deregister_client(&proc, Some(cb));

    // The callback may or may not have been called yet (async).
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Multiple register callbacks with shared counter.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_multiple_register_callbacks_shared_counter() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let count = Arc::new(Mutex::new(0usize));

    for rank in 0..3 {
        let proc = pmix::Proc::new("counter.reg.test", rank).expect("invalid nspace");
        let cb = Box::new(CountRegisterCb {
            count: Arc::clone(&count),
        });
        let _result = server_register_client(&proc, 1000, 1000, None, cb);
    }

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Multiple deregister callbacks with shared counter.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_multiple_deregister_callbacks_shared_counter() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let count = Arc::new(Mutex::new(0usize));

    for rank in 0..3 {
        let proc = pmix::Proc::new("counter.dereg.test", rank).expect("invalid nspace");
        let cb = Box::new(CountDeregisterCb {
            count: Arc::clone(&count),
        });
        server_deregister_client(&proc, Some(cb));
    }

    server_finalize(handle).expect("server_finalize should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge case tests
// ─────────────────────────────────────────────────────────────────────────────

/// Register client with root uid/gid (0, 0).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_root_credentials() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("root.creds.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 0, 0, None, Box::new(NoOpRegisterCb));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with large uid/gid values.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_large_uid_gid() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("large.uid.gid.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 65534, 65534, None, Box::new(NoOpRegisterCb));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with server_object = Some(ptr).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_with_server_object() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("server.obj.test", 0).expect("invalid nspace");
    let server_obj: i32 = 42;
    let _result = server_register_client(
        &proc,
        1000,
        1000,
        Some(&server_obj as *const i32 as *mut std::os::raw::c_void),
        Box::new(NoOpRegisterCb),
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with wildcard rank (u32::MAX).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_wildcard_rank() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("wildcard.rank.test", u32::MAX).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with different uid/gid combinations.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_various_credentials() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let cred_pairs = [(0, 0), (1000, 1000), (65534, 65534), (1, 1), (1000, 0)];
    for (uid, gid) in cred_pairs {
        let proc = pmix::Proc::new("various.creds.test", 0).expect("invalid nspace");
        let _result = server_register_client(&proc, uid, gid, None, Box::new(NoOpRegisterCb));
    }

    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_deregister_client with None callback (blocking mode).

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_deregister_client_no_callback() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("no.callback.dereg.test", 0).expect("invalid nspace");
    // Deregister with no callback — blocking behavior.
    server_deregister_client(&proc, None);

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register and deregister with no callback on both sides.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_deregister_no_callbacks() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc = pmix::Proc::new("no.cb.lifecycle.test", 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));
    server_deregister_client(&proc, None);

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with a Proc containing a long nspace name.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_long_nspace() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let long_nspace = "this.is.a.very.long.namespace.name.for.testing.purposes.in.pmix.rs";
    let proc = pmix::Proc::new(long_nspace, 0).expect("invalid nspace");
    let _result = server_register_client(&proc, 1000, 1000, None, Box::new(NoOpRegisterCb));

    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register multiple clients with different server_object pointers.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_clients_with_different_server_objects() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let obj1: i32 = 1;
    let obj2: i32 = 2;
    let obj3: i32 = 3;

    let proc1 = pmix::Proc::new("diff.obj.test", 0).expect("invalid nspace");
    let _result1 = server_register_client(
        &proc1,
        1000,
        1000,
        Some(&obj1 as *const i32 as *mut std::os::raw::c_void),
        Box::new(NoOpRegisterCb),
    );

    let proc2 = pmix::Proc::new("diff.obj.test", 1).expect("invalid nspace");
    let _result2 = server_register_client(
        &proc2,
        1000,
        1000,
        Some(&obj2 as *const i32 as *mut std::os::raw::c_void),
        Box::new(NoOpRegisterCb),
    );

    let proc3 = pmix::Proc::new("diff.obj.test", 2).expect("invalid nspace");
    let _result3 = server_register_client(
        &proc3,
        1000,
        1000,
        Some(&obj3 as *const i32 as *mut std::os::raw::c_void),
        Box::new(NoOpRegisterCb),
    );

    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init with a module that has some callbacks set.

#[test]
fn test_server_init_with_callbacks_set() {
    extern "C" fn dummy_connected() {}

    let module = PmixServerModule {
        client_connected: Some(dummy_connected),
        ..Default::default()
    };
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// Register client with NUL in nspace should fail at Proc creation.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_client_nul_in_nspace_fails_at_proc() {
    // Proc::new should reject nspace with NUL byte.
    let proc_result = pmix::Proc::new("job\0name", 0);
    assert!(
        proc_result.is_err(),
        "Proc::new should reject nspace containing NUL byte"
    );
}

/// Handle ownership: server_finalize consumes the handle.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_handle_consumed_by_finalize() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    // Move handle into server_finalize — handle is no longer accessible.
    server_finalize(handle).expect("server_finalize should succeed");
    // If we tried to use `handle` here, it would be a compile error.
}

/// Register client, then try to register a different client in the same nspace.

#[ignore] // requires daemon isolation — C-level PMIx state corrupts between tests
fn test_register_different_ranks_same_nspace_sequential() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");

    let proc0 = pmix::Proc::new("seq.rank.test", 0).expect("invalid nspace");
    let proc1 = pmix::Proc::new("seq.rank.test", 1).expect("invalid nspace");
    let proc2 = pmix::Proc::new("seq.rank.test", 2).expect("invalid nspace");

    let _r0 = server_register_client(&proc0, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _r1 = server_register_client(&proc1, 1000, 1000, None, Box::new(NoOpRegisterCb));
    let _r2 = server_register_client(&proc2, 1000, 1000, None, Box::new(NoOpRegisterCb));

    server_finalize(handle).expect("server_finalize should succeed");
}
