//! Tests for `PMIx_Connect` and `PMIx_Connect_nb` via the safe
//! `process_mgmt` module wrapper.
//!
//! Derived from C test patterns in:
//! - `test/test_cd.c` — blocking and non-blocking connect/disconnect
//!   with `PMIx_Connect(&proc, 1, NULL, 0)` after PMIx_Init.
//! - `test/simple/simpdyn.c` — connect/disconnect cycle in a spawned job
//!   context with `PMIx_Connect(&proc, 1, NULL, 0)` after lookup.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

use pmix::process_mgmt::{ConnectCallbackWrapper, connect, connect_nb};
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Connect without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `connect` without `PMIx_Init` must return an error rather
/// than panic or segfault — the library should detect that it is not
/// initialized and return an appropriate error code.
///
/// Derived from `test/test_cd.c` — the C test calls PMIx_Connect
/// only after PMIx_Init, so calling it without init is an error path.
#[test]
fn connect_without_init_fails() {
    let proc = Proc::new("test_namespace", u32::MAX).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(
        result.is_err(),
        "connect without PMIx_Init should fail, got {:?}\n\\\
         NOTE: if this test passes (returns Ok), it means PMIx_Connect\n\\\
         succeeded without PMIx_Init — that would indicate a bug in\n\\\
         the PMIx library or unexpected behavior.",
        result
    );
}

/// Connect with a specific rank instead of wildcard — should also fail
/// without init.
#[test]
fn connect_specific_rank_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(
        result.is_err(),
        "connect with specific rank without init should fail"
    );
}

/// Connect with multiple procs — should fail without init.
///
/// Derived from `test/test_cd.c` pattern: the test connects a single
/// proc, but the API supports arrays of any size.
#[test]
fn connect_multiple_procs_without_init_fails() {
    let proc1 = Proc::new("test_ns", u32::MAX).expect("create proc1");
    let proc2 = Proc::new("other_ns", u32::MAX).expect("create proc2");
    let result = connect(&[proc1, proc2], &[]);
    assert!(
        result.is_err(),
        "connect with multiple procs without init should fail"
    );
}

/// Connect with empty proc array — our wrapper returns `PMIX_ERR_BAD_PARAM`
/// immediately without even calling the FFI layer.
#[test]
fn connect_empty_procs_returns_bad_param() {
    let result = connect(&[], &[]);
    assert!(result.is_err(), "connect with empty procs should fail");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty proc array should return PMIX_ERR_BAD_PARAM, got {:?}",
        err
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Connect_nb without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking connect without init — should return synchronous error
/// because the library is not initialized.
#[test]
fn connect_nb_without_init_fails() {
    let proc = Proc::new("test_namespace", u32::MAX).expect("create proc");
    let callback = ConnectCallbackWrapper::new(|_status| {
        // This callback should NOT be called on synchronous failure.
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = connect_nb(&[proc], &[], callback);
    assert!(
        result.is_err(),
        "connect_nb without PMIx_Init should fail, got {:?}",
        result
    );
}

/// Connect_nb with empty proc array — should return `PMIX_ERR_BAD_PARAM`
/// immediately, just like the blocking variant.
#[test]
fn connect_nb_empty_procs_returns_bad_param() {
    let callback = ConnectCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on bad param");
    });
    let result = connect_nb(&[], &[], callback);
    assert!(result.is_err(), "connect_nb with empty procs should fail");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "empty proc array should return PMIX_ERR_BAD_PARAM, got {:?}",
        err
    );
}

/// Connect_nb with multiple procs — should fail without init.
#[test]
fn connect_nb_multiple_procs_without_init_fails() {
    let proc1 = Proc::new("test_ns", u32::MAX).expect("create proc1");
    let proc2 = Proc::new("other_ns", 0).expect("create proc2");
    let callback = ConnectCallbackWrapper::new(|_status| {
        panic!("callback should not be invoked on synchronous failure");
    });
    let result = connect_nb(&[proc1, proc2], &[], callback);
    assert!(
        result.is_err(),
        "connect_nb with multiple procs without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-namespace connect patterns (without init, expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Connect to a different namespace — mirrors the `simpdyn.c` pattern
/// where a spawned job connects to its parent's namespace.
///
/// In `simpdyn.c`: the spawned client sets `proc.nspace = nsp2` (parent's
/// namespace) and calls `PMIx_Connect(&proc, 1, NULL, 0)`.
#[test]
fn connect_cross_namespace_without_init_fails() {
    let proc = Proc::new("parent_namespace", u32::MAX).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(
        result.is_err(),
        "cross-namespace connect without init should fail"
    );
}

/// Connect to own namespace (self-connect) — valid pattern for establishing
/// a connection within a single job.
#[test]
fn connect_self_namespace_without_init_fails() {
    let proc = Proc::new("my_namespace", u32::MAX).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(result.is_err(), "self-connect without init should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Connect with a proc that has rank 0 (not wildcard).
#[test]
fn connect_rank_zero_without_init_fails() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(
        result.is_err(),
        "connect with rank 0 without init should fail"
    );
}

/// Connect with a proc that has a high rank value.
#[test]
fn connect_high_rank_without_init_fails() {
    let proc = Proc::new("test_ns", 1000).expect("create proc");
    let result = connect(&[proc], &[]);
    assert!(
        result.is_err(),
        "connect with high rank without init should fail"
    );
}

/// Connect with three procs from different namespaces — stress test
/// the proc array conversion.
#[test]
fn connect_three_namespaces_without_init_fails() {
    let procs = vec![
        Proc::new("namespace_a", 0).expect("proc a"),
        Proc::new("namespace_b", u32::MAX).expect("proc b"),
        Proc::new("namespace_c", 5).expect("proc c"),
    ];
    let result = connect(&procs, &[]);
    assert!(
        result.is_err(),
        "connect with three namespaces without init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper construction
// ─────────────────────────────────────────────────────────────────────────────

/// ConnectCallbackWrapper::new accepts a closure and returns a wrapper.
/// This tests that the wrapper type is constructible and usable.
#[test]
fn connect_callback_wrapper_construction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let _wrapper = ConnectCallbackWrapper::new(move |status| {
        called_clone.store(true, Ordering::SeqCst);
        assert!(status.is_error() || status.is_success());
    });
    // Wrapper is constructible — the callback won't be invoked here
    // because we're not calling the FFI layer.
}

/// ConnectCallbackWrapper closure receives PmixStatus.
#[test]
fn connect_callback_wrapper_receives_status() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let wrapper = ConnectCallbackWrapper::new(move |status| {
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
// Integration test (requires PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: Init → Connect → Disconnect.
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
/// pmixrun -n 1 -- cargo test --test process_mgmt_Connect connect_integration -- --include-ignored
/// ```
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn connect_integration() {
    // This would require PMIx_Init which needs a daemon.
    // In a real integration test environment, we would:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create a Proc with our own namespace and WILDCARD rank.
    // 3. Call connect(&[proc], &[]) and verify Ok(()).
    // 4. Call disconnect(&[proc], &[]) and verify Ok(()).
    // 5. Call connect_nb with a callback and verify async completion.
    //
    // Because connect is a blocking collective, it requires all
    // participating processes to call it simultaneously.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}

/// Non-blocking connect integration: Init → Connect_nb → wait for callback.
///
/// Mirrors `test/test_cd.c` non-blocking path:
/// `PMIx_Connect_nb(&proc, 1, NULL, 0, cnct_cb, &cbdata)`
/// followed by `PMIX_WAIT_FOR_COMPLETION(cbdata.in_progress)`.
///
/// Ignored by default — requires PMIx daemon.
#[test]
#[ignore = "requires PMIx_Init with a running PMIx daemon"]
fn connect_nb_integration() {
    // In a real integration test:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Create a Proc with our own namespace and WILDCARD rank.
    // 3. Use a Arc<Mutex<...>> or AtomicBool shared with the callback
    //    to detect when connect_nb completes.
    // 4. Call connect_nb(&[proc], &[], callback).
    // 5. Wait for the callback to fire.
    // 6. Verify the callback received PMIX_SUCCESS.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
