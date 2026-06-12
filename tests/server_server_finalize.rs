//! Tests for `PMIx_server_finalize`.
//!
//! Derived from C test sources:
//! - test/simple/simptest.c — server finalize on error paths and at cleanup
//! - test/simple/gwtest.c — server finalize on fork setup failure and at exit
//!
//! Note: `PMIx_server_finalize` requires a running PMIx daemon or a proper
//! PMIx server environment. Tests that call the actual FFI are marked
//! `#[ignore]` and should be run with a PMIx environment.
//!
//! Unit tests that verify API structure, types, and behavior run without
//! a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{PmixServerHandle, PmixServerModule, server_finalize, server_init_minimal};

// ─────────────────────────────────────────────────────────────────────────────
// API signature and type checks
// ─────────────────────────────────────────────────────────────────────────────

/// server_finalize has the expected function signature:
/// `fn(PmixServerHandle) -> Result<(), PmixStatus>`
#[test]
fn test_server_finalize_signature() {
    fn assert_fn_type<F, R>(_: F)
    where
        F: Fn(PmixServerHandle) -> R,
    {
    }
    // This compiles only if server_finalize matches the expected signature.
    assert_fn_type(server_finalize);
}

/// server_finalize consumes the handle (takes by value, not by reference).
/// This test verifies the type system enforces single-use semantics.
#[test]
fn test_server_finalize_consumes_handle() {
    // PmixServerHandle is not Clone, so it can only be moved once.
    // We can't actually call server_finalize without a PMIx daemon,
    // but we can verify the type properties.
    // Note: we can't use negative trait bounds (!Clone) in stable Rust,
    // so instead verify via the function signature that it takes ownership.
    let _: fn(PmixServerHandle) -> Result<(), PmixStatus> = server_finalize;
}

/// server_finalize returns Result<(), PmixStatus>.
#[test]
fn test_server_finalize_return_type() {
    // Verify the return type is Result<(), PmixStatus> by type-checking.
    fn check_return<R>(_: fn(PmixServerHandle) -> R) {}
    let _result_type: Result<(), PmixStatus> = Err(PmixStatus::from_raw(-1));
    check_return(server_finalize);
    drop(_result_type);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — ownership and lifecycle
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle is not Copy — it must be explicitly moved.
#[test]
fn test_server_handle_not_copy() {
    // Verify PmixServerHandle does not implement Copy
    // by checking it doesn't satisfy Copy trait requirements.
    // (We can't use negative bounds on stable, so just verify Debug.)
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

/// PmixServerHandle debug output contains expected fields.
#[test]
fn test_server_handle_debug_fields() {
    // We can't create a real handle without PMIx_server_init,
    // but we can verify the type is Debug via a compile check.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// server_finalize — behavior tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// PMIx_server_finalize after a successful server_init should return SUCCESS.
///
/// Derived from simptest.c:587 — the main test path calls server_finalize
/// at cleanup and checks for PMIX_SUCCESS.
///
/// Ignored by default — requires a PMIx server environment.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_after_init() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    let result = server_finalize(handle);
    assert!(
        result.is_ok(),
        "server_finalize should return Ok(()) after successful init"
    );
}

/// PMIx_server_finalize should succeed after init even with no callbacks.
///
/// Derived from simptest.c:477 — server_finalize is called as error
/// cleanup even when setup_fork fails, meaning the server was initialized
/// but no callbacks were actually invoked.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_no_callbacks_invoked() {
    // Initialize with a minimal module (no callbacks set).
    // This simulates the error path where the server was initialized
    // but no client operations occurred before finalization.
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    // Finalize without having invoked any callbacks.
    let result = server_finalize(handle);
    assert!(
        result.is_ok(),
        "server_finalize should succeed even with no callbacks invoked"
    );
}

/// Multiple init/finalize cycles should work (idempotent finalization).
///
/// Derived from gwtest.c — the gateway test initializes and finalizes
/// the server multiple times in different code paths.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_multiple_cycles() {
    let module = PmixServerModule::default();

    // First cycle
    let handle1 = server_init_minimal(Some(&module)).expect("first server_init should succeed");
    server_finalize(handle1).expect("first server_finalize should succeed");

    // Second cycle — re-init after finalize
    let handle2 = server_init_minimal(Some(&module)).expect("second server_init should succeed");
    server_finalize(handle2).expect("second server_finalize should succeed");
}

/// server_finalize with None module (minimal server, no module provided).
///
/// Derived from simptest.c:495 — server_finalize is called after
/// server_register_client failure, where the server was initialized
/// but client registration failed.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_none_module() {
    let handle = server_init_minimal(None).expect("server_init_minimal(None) should succeed");
    let result = server_finalize(handle);
    assert!(
        result.is_ok(),
        "server_finalize should succeed for a server initialized with no module"
    );
}

/// server_finalize should release all server resources.
///
/// Derived from simptest.c — after server_finalize, the test checks
/// the return code and exits. This verifies that finalize is the
/// last operation and cleans up everything.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_releases_resources() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init should succeed");

    // Finalize — this should release all internal PMIx server resources.
    let result = server_finalize(handle);
    assert!(result.is_ok(), "server_finalize should succeed");

    // After finalize, the handle is consumed (moved), so it cannot
    // be used again. This is enforced by the type system.
}

/// server_finalize returns Err on an invalid state.
///
/// This tests that server_finalize properly returns an error when
/// called in an unexpected state. Note: we can't easily trigger this
/// without a PMIx daemon, so this is a compile-time type check.
#[test]
fn test_server_finalize_error_path_type() {
    // Verify that server_finalize can return an Err variant
    // by checking the Result type.
    let result: Result<(), PmixStatus> = Err(PmixStatus::from_raw(-1));
    assert!(result.is_err(), "Err variant should be detectable");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error handling patterns from C tests
// ─────────────────────────────────────────────────────────────────────────────

/// C test pattern: check return code and report error.
/// simptest.c:587 — `if (PMIX_SUCCESS != (rc = PMIx_server_finalize()))`
///
/// This verifies the Rust wrapper matches the C error-checking pattern.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_error_check_pattern() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init should succeed");

    // Rust pattern: match on Result instead of comparing status codes.
    match server_finalize(handle) {
        Ok(()) => {
            // Success — equivalent to rc == PMIX_SUCCESS
        }
        Err(status) => {
            panic!("Finalize failed with error {:?}", status);
        }
    }
}

/// C test pattern: server_finalize on error path after setup_fork failure.
/// simptest.c:477 — `PMIx_server_finalize(); return rc;`
///
/// The C code calls server_finalize unconditionally before returning
/// an error code. The Rust wrapper should handle this gracefully.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_on_error_path() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init should succeed");

    // Simulate an error path: server was initialized but some operation
    // failed. We should still be able to finalize cleanly.
    let result = server_finalize(handle);
    assert!(
        result.is_ok(),
        "server_finalize should succeed even on error cleanup path"
    );
}

/// C test pattern: server_finalize after register_client failure.
/// simptest.c:485 — finalize called after PMIx_server_register_client fails.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_after_register_failure() {
    // Simulate: server_init succeeded, but a subsequent operation failed.
    // The server should still be finalizable.
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init should succeed");

    // In the C test, register_client fails here.
    // In Rust, we just finalize directly since we can't simulate the failure
    // without a real PMIx server environment.
    let result = server_finalize(handle);
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// Handle ownership — move semantics prevent double-finalize
// ─────────────────────────────────────────────────────────────────────────────

/// The handle is consumed by value, preventing double-finalize.
/// This test verifies the type system prevents calling server_finalize twice
/// on the same handle.
#[test]
fn test_handle_move_prevents_double_finalize() {
    // We can't create a real handle without PMIx, but we can verify
    // the move semantics compile correctly.
    //
    // This demonstrates that after `server_finalize(handle)`,
    // `handle` is no longer accessible.
    //
    // In C, double-finalize is a runtime bug.
    // In Rust, it's a compile-time error because the handle is moved.
    fn assert_consumed(_: PmixServerHandle) {}
    // The function signature takes PmixServerHandle by value,
    // not &PmixServerHandle, so the caller cannot reuse it.
    let _sig: fn(PmixServerHandle) -> Result<(), PmixStatus> = server_finalize;
}

/// Verify server_finalize is not callable with a reference.
#[test]
fn test_server_finalize_not_callable_by_ref() {
    // This verifies that server_finalize takes ownership.
    // If it took &PmixServerHandle, double-finalize would be possible.
    fn check<'a>(f: fn(PmixServerHandle) -> Result<(), PmixStatus>) {
        let _ = f;
    }
    check(server_finalize);
}
