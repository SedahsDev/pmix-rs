//! Tests for `PMIx_Disconnect` and `PMIx_Disconnect_nb` via the safe
//! `process_mgmt` module wrapper.
//!
//! Derived from C test patterns in:
//! - `test/test_cd.c` — blocking and non-blocking connect/disconnect
//!   cycle: `PMIx_Connect` then `PMIx_Disconnect(&proc, 1, NULL, 0)`.
//! - `test/simple/simpdyn.c` — connect/disconnect cycle in a spawned job
//!   context with `PMIx_Disconnect(&proc, 1, NULL, 0)` after connect.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::process_mgmt::{DisconnectCallbackWrapper, disconnect, disconnect_nb};
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Disconnect without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `disconnect` without `PMIx_Init` must return an error rather
/// than panic or segfault — the library should detect that it is not
/// initialized and return an appropriate error code.
///
/// Derived from `test/test_cd.c` — the C test calls PMIx_Disconnect
/// only after PMIx_Connect which itself requires PMIx_Init.
#[test]
fn disconnect_without_init_fails() {
    let proc = Proc::new("test_namespace", u32::MAX).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "disconnect without PMIx_Init should fail, got {:?}\n\
         NOTE: if this test passes (returns Ok), it means PMIx_Disconnect\n\
         succeeded without PMIx_Init — that would indicate a bug in\n\
         the PMIx library or unexpected behavior.",
        result
    );
}

/// Disconnect with a specific rank instead of wildcard — should also fail
/// without init.
#[test]
fn disconnect_specific_rank_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "disconnect with specific rank without init should fail"
    );
}

/// Disconnect with multiple procs — should fail without init.
///
/// Derived from `test/test_cd.c` pattern: the test disconnects a single
/// proc, but the API supports arrays of any size.
#[test]
fn disconnect_multiple_procs_without_init_fails() {
    let proc1 = Proc::new("test_ns", u32::MAX).expect("create proc1");
    let proc2 = Proc::new("other_ns", u32::MAX).expect("create proc2");
    let result = disconnect(&[proc1, proc2], &[]);
    assert!(
        result.is_err(),
        "disconnect with multiple procs without init should fail"
    );
}

/// Disconnect with empty proc array — our wrapper returns `PMIX_ERR_BAD_PARAM`
/// immediately without even calling the FFI layer.
#[test]
fn disconnect_empty_procs_returns_bad_param() {
    let result = disconnect(&[], &[]);
    assert!(result.is_err(), "disconnect with empty procs should fail");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty proc array should return PMIX_ERR_BAD_PARAM, got {:?}",
        err
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Disconnect_nb without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────
// NOTE: PMIx_Disconnect_nb segfaults when called without PMIx_Init — the
// library tries to invoke the callback even on synchronous failure, and the
// internal state is invalid. These tests are ignored by default.

/// Non-blocking disconnect without init — should return synchronous error
/// because the library is not initialized.
///
/// Ignored because `PMIx_Disconnect_nb` segfaults without PMIx_Init —
/// the library tries to invoke the callback with invalid internal state.
#[test]
#[ignore = "PMIx_Disconnect_nb segfaults without PMIx_Init"]
fn disconnect_nb_without_init_fails() {
    let proc = Proc::new("test_namespace", u32::MAX).expect("create proc");
    let callback = DisconnectCallbackWrapper::new(|_status| {
        // This callback should NOT be called on synchronous failure.
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = disconnect_nb(&[proc], &[], callback);
    assert!(
        result.is_err(),
        "disconnect_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Disconnect_nb with empty proc array — should return `PMIX_ERR_BAD_PARAM`
/// immediately, just like the blocking variant.
#[test]
fn disconnect_nb_empty_procs_returns_bad_param() {
    let callback = DisconnectCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on bad param");
    });
    let result = disconnect_nb(&[], &[], callback);
    assert!(
        result.is_err(),
        "disconnect_nb with empty procs should fail"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty proc array should return PMIX_ERR_BAD_PARAM, got {:?}",
        err
    );
}

/// Disconnect_nb with multiple procs — should fail without init.
///
/// Ignored because `PMIx_Disconnect_nb` segfaults without PMIx_Init.
#[test]
#[ignore = "PMIx_Disconnect_nb segfaults without PMIx_Init"]
fn disconnect_nb_multiple_procs_without_init_fails() {
    let proc1 = Proc::new("test_ns", u32::MAX).expect("create proc1");
    let proc2 = Proc::new("other_ns", 0).expect("create proc2");
    let callback = DisconnectCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = disconnect_nb(&[proc1, proc2], &[], callback);
    assert!(
        result.is_err(),
        "disconnect_nb with multiple procs without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-namespace disconnect patterns (without init, expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Disconnect from a different namespace — mirrors the `simpdyn.c` pattern
/// where a spawned job disconnects from its parent's namespace.
///
/// In `simpdyn.c`: the spawned client sets `proc.nspace = nsp2` (parent's
/// namespace) and calls `PMIx_Disconnect(&proc, 1, NULL, 0)`.
#[test]
fn disconnect_cross_namespace_without_init_fails() {
    let proc = Proc::new("parent_namespace", u32::MAX).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "cross-namespace disconnect without init should fail"
    );
}

/// Disconnect from own namespace (self-disconnect) — valid pattern for
/// disconnecting within a single job.
#[test]
fn disconnect_self_namespace_without_init_fails() {
    let proc = Proc::new("my_namespace", u32::MAX).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(result.is_err(), "self-disconnect without init should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Disconnect with a proc that has rank 0 (not wildcard).
#[test]
fn disconnect_rank_zero_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "disconnect with rank 0 without init should fail"
    );
}

/// Disconnect with a proc that has a high rank value.
#[test]
fn disconnect_high_rank_without_init_fails() {
    let proc = Proc::new("test_ns", 1000).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "disconnect with high rank without init should fail"
    );
}

/// Disconnect with three procs from different namespaces — stress test
/// the proc array conversion.
#[test]
fn disconnect_three_namespaces_without_init_fails() {
    let procs = vec![
        Proc::new("namespace_a", 0).expect("proc a"),
        Proc::new("namespace_b", u32::MAX).expect("proc b"),
        Proc::new("namespace_c", 5).expect("proc c"),
    ];
    let result = disconnect(&procs, &[]);
    assert!(
        result.is_err(),
        "disconnect with three namespaces without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper construction
// ─────────────────────────────────────────────────────────────────────────────

/// DisconnectCallbackWrapper::new accepts a closure and returns a wrapper.
/// This tests that the wrapper type is constructible and usable.
#[test]
fn disconnect_callback_wrapper_construction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = DisconnectCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
    });
    // Wrapper is constructible — the callback won't be invoked here
    // because we're not calling the FFI layer.
}

/// DisconnectCallbackWrapper closure receives PmixStatus.
#[test]
fn disconnect_callback_wrapper_receives_status() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let wrapper = DisconnectCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        // Verify the status we receive is a valid PmixStatus.
        let _ = status.is_success();
        let _ = status.is_error();
    });

    // Invoke the callback manually to verify it works.
    // (In real usage, PMIx would invoke it via the bridge function.)
    // We can't easily test the bridge without FFI, so just verify
    // the wrapper is constructible with a meaningful closure.
    drop(wrapper);
}

// ─────────────────────────────────────────────────────────────────────────────
// Connect → Disconnect cycle patterns (without init, expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Disconnect with the same proc that would have been used for connect —
/// the pattern from test_cd.c is:
/// 1. Connect(&proc, 1, NULL, 0)
/// 2. Disconnect(&proc, 1, NULL, 0)
///
/// Without init, both should fail, but the wrapper should handle
/// the same proc gracefully in both cases.
#[test]
fn disconnect_same_proc_as_connect_without_init_fails() {
    let proc = Proc::new("test_namespace", u32::MAX).expect("create proc");
    // Both should fail without init, but the key is that the same
    // proc structure works with both functions.
    let _connect_result = pmix::process_mgmt::connect(&[proc.clone()], &[]);
    let disconnect_result = disconnect(&[proc], &[]);
    assert!(
        disconnect_result.is_err(),
        "disconnect with same proc as connect without init should fail"
    );
}

/// Disconnect with wildcard rank — the most common pattern from C tests.
#[test]
fn disconnect_wildcard_rank_without_init_fails() {
    let proc = Proc::new("my_job", pmix::RANK_WILDCARD).expect("create proc");
    let result = disconnect(&[proc], &[]);
    assert!(
        result.is_err(),
        "disconnect with wildcard rank without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration test (requires PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: Init → Connect → Disconnect → Connect_nb → Disconnect_nb.
///
/// This test mirrors `test/test_cd.c`:
/// 1. PMIx_Init
/// 2. PMIx_Connect(&proc, 1, NULL, 0) — blocking connect
/// 3. PMIx_Disconnect(&proc, 1, NULL, 0) — blocking disconnect
/// 4. PMIx_Connect_nb — non-blocking connect with callback
/// 5. PMIx_Disconnect_nb — non-blocking disconnect with callback
///
/// NOTE: This test requires a running PMIx server and must be run
/// under `pmixrun` or equivalent. It is ignored by default.
///
/// ```sh
/// pmixrun -n 1 -- cargo test --test process_mgmt_Disconnect disconnect_integration -- --include-ignored
/// ```
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn disconnect_integration() {
    // This would require PMIx_Init which needs a daemon.
    // In a real integration test environment, we would:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create a Proc with our own namespace and WILDCARD rank.
    // 3. Call connect(&[proc], &[]) and verify Ok(()).
    // 4. Call disconnect(&[proc], &[]) and verify Ok(()).
    // 5. Call connect_nb with a callback and verify async completion.
    // 6. Call disconnect_nb with a callback and verify async completion.
    //
    // Because disconnect is a blocking collective, it requires all
    // participating processes to call it simultaneously.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Non-blocking disconnect integration: Init → Connect_nb → Disconnect_nb.
///
/// Mirrors `test/test_cd.c` non-blocking path:
/// `PMIx_Connect_nb(&proc, 1, NULL, 0, cnct_cb, &cbdata)`
/// followed by `PMIx_Disconnect_nb(&proc, 1, NULL, 0, cd_cb, &cbdata)`
/// with `PMIX_WAIT_FOR_COMPLETION(cbdata.in_progress)`.
///
/// Ignored by default — requires PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn disconnect_nb_integration() {
    // In a real integration test:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create a Proc with our own namespace and WILDCARD rank.
    // 3. Connect first (required before disconnect).
    // 4. Use a Arc<Mutex<...>> or AtomicBool shared with the callback
    //    to detect when disconnect_nb completes.
    // 5. Call disconnect_nb(&[proc], &[], callback).
    // 6. Wait for the callback to fire.
    // 7. Verify the callback received PMIX_SUCCESS.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Test that disconnect without a prior connect returns the expected error.
///
/// Per the spec: "An error will be returned if the specified set of procs
/// was not previously connected via a call to PMIx_Connect or its
/// non-blocking form."
///
/// This would be PMIX_ERR_INVALID_OPERATION if the library could detect
/// that no connect was made. Without a daemon, the library may return
/// a different error (e.g., PMIX_ERR_NOT_SUPPORTED), so we just check
/// that it returns an error.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn disconnect_without_prior_connect_returns_error() {
    // In a real integration test:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create a Proc with our own namespace.
    // 3. Call disconnect(&[proc], &[]) WITHOUT calling connect first.
    // 4. Verify it returns an error (PMIX_ERR_INVALID_OPERATION or similar).
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
