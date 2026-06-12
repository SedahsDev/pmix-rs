//! Integration tests for `PMIx_Initialized` via the safe `initialized()` wrapper.
//!
//! These tests call into the real PMIx library and do NOT require a running
//! PMIx daemon — `PMIx_Initialized` only checks a local atomic flag.

use pmix::utility::initialized;

/// Before `PMIx_Init` has been called, `initialized()` must return `false`.
///
/// The PMIx library initializes its internal `pmix_globals.initialized`
/// flag to `false` at load time. It is only set to `true` after a
/// successful `PMIx_Init` call.
#[test]
fn initialized_before_init_returns_false() {
    assert!(
        !initialized(),
        "PMIx_Initialized should return false before PMIx_Init"
    );
}

/// `initialized()` is idempotent — calling it multiple times returns the
/// same result with no side effects.
#[test]
fn initialized_is_idempotent() {
    let first_call = initialized();
    let second_call = initialized();
    assert_eq!(
        first_call, second_call,
        "PMIx_Initialized must be idempotent (no side effects)"
    );
}

/// `initialized()` returns a boolean, not an error code.
///
/// This is a compile-time type check — if the function signature changes
/// from `fn initialized() -> bool` to something else, this test will fail
/// to compile.
#[test]
fn initialized_returns_bool() {
    let _result: bool = initialized();
}
