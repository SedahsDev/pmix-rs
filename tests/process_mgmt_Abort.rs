//! Tests for `PMIx_Abort` via the safe `process_mgmt` module wrapper.
//!
//! Derived from C test patterns in:
//! - `test/simple/simpft.c` — rank 0 calls `PMIx_Abort(PMIX_ERR_OUT_OF_RESOURCE, "Eat rocks", &proc, 1)`
//!   after PMIx_Init, other ranks wait for notification.
//!
//! Tests that require `PMIx_Init` are marked `#[ignore]` because they need
//! a running PMIx daemon / server.

mod daemon_helper;

use pmix::process_mgmt::abort;
use pmix::{PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Abort without PMIx_Init (expected to fail gracefully)
// ─────────────────────────────────────────────────────────────────────────────

/// Calling `abort` without `PMIx_Init` must return an error rather
/// than panic or segfault — the library should detect that it is not
/// initialized and return an appropriate error code.
///
/// Derived from `test/simple/simpft.c` — the C test calls PMIx_Abort
/// only after PMIx_Init, so calling it without init is an error path.
#[test]
fn abort_without_init_fails() {
    let result = abort(
        PmixStatus::Known(PmixError::Error),
        Some("test abort message"),
        None,
    );
    assert!(
        result.is_err(),
        "abort without PMIx_Init should fail, got {:?}\n\
         NOTE: if this test passes (returns Ok), it means PMIx_Abort\n\
         succeeded without PMIx_Init — that would indicate a bug in\n\
         the PMIx library or an unexpected behavior.",
        result
    );
}

/// Abort with NULL message (no msg) without init — should also fail.
///
/// The spec says "Passing a NULL msg parameter is allowed."
#[test]
fn abort_without_msg_without_init_fails() {
    let result = abort(PmixStatus::Known(PmixError::Error), None, None);
    assert!(
        result.is_err(),
        "abort without msg and without init should fail"
    );
}

/// Abort with explicit proc targets without init — should fail.
///
/// Derived from `test/simple/simpft.c` pattern:
/// `PMIx_Abort(PMIX_ERR_OUT_OF_RESOURCE, "Eat rocks", &proc, 1)`
#[test]
fn abort_with_procs_without_init_fails() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let result = abort(
        PmixStatus::Known(PmixError::ErrOutOfResource),
        Some("Eat rocks"),
        Some(&[proc]),
    );
    assert!(
        result.is_err(),
        "abort with explicit procs without PMIx_Init should fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Error code variations (all without init, all expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Abort with PMIX_ERR_JOB_ABORTED status code.
#[test]
fn abort_with_job_aborted_status() {
    let result = abort(
        PmixStatus::Known(PmixError::ErrJobAborted),
        Some("job aborted"),
        None,
    );
    assert!(result.is_err(), "should fail without init");
}

/// Abort with PMIX_ERR_TIMEOUT status code.
#[test]
fn abort_with_timeout_status() {
    let result = abort(PmixStatus::Known(PmixError::ErrTimeout), None, None);
    assert!(result.is_err(), "should fail without init");
}

/// Abort with a generic unknown status code.
#[test]
fn abort_with_unknown_status() {
    let result = abort(PmixStatus::Unknown(-999), Some("unknown error"), None);
    assert!(result.is_err(), "should fail without init");
}

/// Abort with PMIX_SUCCESS as the status (some RMs treat 0 differently).
///
/// The spec notes: "some resource managers will not abort the application
/// if the provided status is zero unless specifically configured to do so".
#[test]
fn abort_with_success_status() {
    let result = abort(PmixStatus::Known(PmixError::Success), None, None);
    assert!(
        result.is_err(),
        "abort with status=0 without PMIx_Init should still fail"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc target variations (all without init, all expected to fail)
// ─────────────────────────────────────────────────────────────────────────────

/// Abort targeting multiple procs — should fail without init.
#[test]
fn abort_multiple_procs_without_init_fails() {
    let proc1 = Proc::new("test_ns", 0).expect("create proc1");
    let proc2 = Proc::new("test_ns", 1).expect("create proc2");
    let proc3 = Proc::new("test_ns", 2).expect("create proc3");
    let result = abort(
        PmixStatus::Known(PmixError::Error),
        Some("abort multiple"),
        Some(&[proc1, proc2, proc3]),
    );
    assert!(result.is_err(), "should fail without init");
}

/// Abort targeting procs from a different namespace — should fail without init.
#[test]
fn abort_cross_namespace_without_init_fails() {
    let proc = Proc::new("other_namespace", 0).expect("create proc");
    let result = abort(
        PmixStatus::Known(PmixError::ErrParamValueNotSupported),
        Some("cross-namespace abort"),
        Some(&[proc]),
    );
    assert!(result.is_err(), "should fail without init");
}

/// Abort with wildcard rank (PMIX_RANK_WILDCARD = 4294967295).
///
/// Equivalent to aborting all processes in the namespace.
#[test]
fn abort_wildcard_rank_without_init_fails() {
    let proc = Proc::new("test_ns", u32::MAX).expect("create wildcard proc");
    let result = abort(
        PmixStatus::Known(PmixError::Error),
        Some("abort all"),
        Some(&[proc]),
    );
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Abort with an empty string message — should still work (just fail
/// without init for the same reason as everything else).
#[test]
fn abort_empty_message_without_init_fails() {
    let result = abort(PmixStatus::Known(PmixError::Error), Some(""), None);
    assert!(result.is_err(), "should fail without init");
}

/// Abort with a long message — the C API takes `const char msg[]` with
/// no explicit length limit, so this should be fine (just fail without init).
#[test]
fn abort_long_message_without_init_fails() {
    let long_msg = "A".repeat(1000);
    let result = abort(PmixStatus::Known(PmixError::Error), Some(&long_msg), None);
    assert!(result.is_err(), "should fail without init");
}

/// Abort with empty proc slice — should behave like NULL procs (abort all).
#[test]
fn abort_empty_proc_slice_without_init_fails() {
    let result = abort(
        PmixStatus::Known(PmixError::Error),
        Some("empty procs"),
        Some(&[]),
    );
    assert!(result.is_err(), "should fail without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration test (requires PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Full integration test: Init → Abort → observe behavior.
///
/// This test mirrors `test/simple/simpft.c`:
/// 1. PMIx_Init
/// 2. Register error handler
/// 3. Rank 0 calls PMIx_Abort
/// 4. Other ranks wait for notification
///
/// NOTE: This test requires a running PMIx server and must be run
/// under `pmixrun` or equivalent. It is ignored by default.
///
/// ```sh
/// pmixrun -n 2 -- cargo test --test process_mgmt_Abort abort_integration -- --include-ignored
/// ```
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn abort_integration() {
    daemon_helper::ensure_pmix_init();
    // This would require PMIx_Init which needs a daemon.
    // The safe approach is to skip this in unit test mode.
    // In a real integration test environment, we would:
    //
    // 1. Call pmix::lifecycle::init(None).expect("init");
    // 2. Register an event handler for abort notifications.
    // 3. If rank 0, call abort(PmixStatus::Known(PmixError::ErrOutOfResource),
    //    Some("Eat rocks"), Some(&[my_proc])).
    // 4. Other ranks wait for the notification callback.
    // 5. Verify the abort status and message match.
    //
    // Because PMIx_Abort does not return when the caller is included
    // in the abort targets, we can only test the "abort others" path
    // or verify that the error handler receives the notification.
    unimplemented!("requires PMIx daemon — run under pmixrun");
}
