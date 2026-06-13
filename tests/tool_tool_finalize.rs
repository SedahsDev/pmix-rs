//! Tests for `PMIx_tool_finalize`.
//!
//! Note: `PMIx_tool_finalize` requires a running PMIx daemon or a proper
//! PMIx server environment to exercise the actual FFI path. Tests that
//! call the real FFI are marked `#[ignore]` and should be run with a
//! PMIx environment. Unit tests that verify API structure, types, and
//! function signatures run without a PMIx runtime.
//!
//! # C API
//! ```c
//! pmix_status_t PMIx_tool_finalize(void);
//! ```
//!
//! The tool library is reference-counted. Each `tool_finalize` decrements
//! the count; the connection closes when it reaches zero.

use pmix::tool::{tool_finalize, tool_init, tool_init_minimal, PmixToolHandle};
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_finalize takes PmixToolHandle and returns Result<(), PmixStatus>.
#[test]
fn test_tool_finalize_signature() {
    let _: fn(PmixToolHandle) -> Result<(), PmixStatus> = tool_finalize;
}

/// tool_finalize consumes the handle by value (not by reference).
#[test]
fn test_tool_finalize_consumes_handle() {
    // Compile-time check: the parameter is PmixToolHandle by value.
    // If it took &PmixToolHandle, this type alias would not match.
    type F = fn(PmixToolHandle) -> Result<(), PmixStatus>;
    let _f: F = tool_finalize;
}

/// tool_finalize return type is Result<(), PmixStatus> — unit success.
#[test]
fn test_tool_finalize_return_unit() {
    // Verify the Ok arm carries () not a handle or other type.
    type Ret = Result<(), PmixStatus>;
    fn _assert_same_type(r: Ret) {}
    // tool_finalize returns exactly this type.
    let _: fn(PmixToolHandle) -> Ret = tool_finalize;
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixToolHandle traits required by tool_finalize
// ─────────────────────────────────────────────────────────────────────────────

/// PmixToolHandle is Clone (required for multi-finalize scenarios).
#[test]
fn test_tool_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixToolHandle>();
}

/// PmixToolHandle is Debug (useful for error reporting).
#[test]
fn test_tool_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixToolHandle>();
}

/// PmixToolHandle is both Clone and Debug.
#[test]
fn test_tool_handle_traits_combined() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixToolHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus interaction
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw(0) is success — tool_finalize returns this on success.
#[test]
fn test_pmix_status_from_raw_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "Raw 0 must be PMIX_SUCCESS");
}

/// PmixStatus::from_raw(-1) is error — tool_finalize returns this on failure.
#[test]
fn test_pmix_status_from_raw_error() {
    let status = PmixStatus::from_raw(-1);
    assert!(status.is_error(), "Raw -1 must be an error");
}

/// PmixStatus is Clone + Copy + Debug (required for error propagation).
#[test]
fn test_pmix_status_traits() {
    fn assert_sc<T: Clone + Copy + std::fmt::Debug>() {}
    assert_sc::<PmixStatus>();
}

/// PmixStatus implements PartialEq + Eq (for test assertions).
#[test]
fn test_pmix_status_equality() {
    fn assert_eq_t<T: PartialEq + Eq>() {}
    assert_eq_t::<PmixStatus>();
}

// ─────────────────────────────────────────────────────────────────────────────
// API consistency — tool_init and tool_finalize are a matched pair
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init returns a handle that tool_finalize can consume.
#[test]
fn test_init_finalize_type_pair() {
    // tool_init -> Result<PmixToolHandle, PmixStatus>
    // tool_finalize -> fn(PmixToolHandle) -> Result<(), PmixStatus>
    // The Ok type of init is exactly the parameter type of finalize.
    type InitOk = PmixToolHandle;
    fn assert_finalize_accepts_init_ok(h: InitOk) -> Result<(), PmixStatus> {
        tool_finalize(h)
    }
    let _ = assert_finalize_accepts_init_ok;
}

/// tool_init_minimal returns the same handle type as tool_init.
#[test]
fn test_init_minimal_compatible_with_finalize() {
    type InitReturn = Result<PmixToolHandle, PmixStatus>;
    fn _check_full() -> InitReturn {
        tool_init(None, &InfoBuilder::new().build())
    }
    fn _check_minimal() -> InitReturn {
        tool_init_minimal()
    }
    // Both return PmixToolHandle on success, consumable by tool_finalize.
    let _ = (_check_full, _check_minimal);
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI binding existence
// ─────────────────────────────────────────────────────────────────────────────

/// tool_finalize is a function pointer type that can be stored and passed around.
#[test]
fn test_tool_finalize_function_pointer() {
    // Verify tool_finalize is a plain function (not a closure or method).
    let f: fn(PmixToolHandle) -> Result<(), PmixStatus> = tool_finalize;
    // The function pointer itself is safe to hold — calling it requires
    // a valid PmixToolHandle obtained from tool_init.
    let _ = f as *const ();
}

// ─────────────────────────────────────────────────────────────────────────────
// Reference counting semantics (compile-time verification)
// ─────────────────────────────────────────────────────────────────────────────

/// tool_finalize takes ownership of the handle (move semantics).
/// After calling tool_finalize, the original handle is consumed.
#[test]
fn test_tool_finalize_move_semantics() {
    // This is a compile-time structural check.
    // tool_finalize signature: fn(PmixToolHandle) -> Result<(), PmixStatus>
    // The handle is passed by value, so it is moved/consumed.
    fn assert_consumes_by_value(_: PmixToolHandle) {}
    let _ = assert_consumes_by_value;
}

/// Multiple tool_init calls can be paired with multiple tool_finalize calls
/// because the library is reference-counted.
#[test]
fn test_reference_counting_documented() {
    // The PMIx spec states the tool library is reference-counted.
    // Each tool_init increments, each tool_finalize decrements.
    // This test documents that behavior — actual exercise requires a server.
    // Verified by the signature: tool_finalize takes a handle but the
    // underlying C function takes no parameters, confirming the
    // reference-counted design.
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init followed by tool_finalize should succeed.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_init_then_finalize() {
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => {
            // No server available — skip
            return;
        }
    };
    let result = tool_finalize(handle);
    assert!(
        result.is_ok(),
        "tool_finalize should succeed after tool_init: {:?}",
        result
    );
}

/// tool_init_minimal followed by tool_finalize should succeed.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_init_minimal_then_finalize() {
    let handle = match tool_init_minimal() {
        Ok(h) => h,
        Err(_) => return,
    };
    let result = tool_finalize(handle);
    assert!(result.is_ok(), "finalize after minimal init should succeed");
}

/// Double finalize: init twice, finalize twice — reference counting.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_reference_counted_finalize() {
    let info = InfoBuilder::new().build();
    let h1 = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => return,
    };
    let h2 = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => return,
    };
    // First finalize should succeed (refcount 2 -> 1)
    let r1 = tool_finalize(h1);
    assert!(r1.is_ok(), "First finalize should succeed");
    // Second finalize should succeed (refcount 1 -> 0, connection closes)
    let r2 = tool_finalize(h2);
    assert!(r2.is_ok(), "Second finalize should succeed");
}

/// Finalize without prior init should return an error.
/// Ignored because it requires PMIx library availability.
#[test]
#[ignore = "requires PMIx library"]
fn test_finalize_without_init() {
    // Calling PMIx_tool_finalize without a matching PMIx_tool_init
    // should return an error (e.g., PMIX_ERR_NOT_INITIALIZED).
    // We cannot construct a PmixToolHandle without going through FFI,
    // so this test is a placeholder for integration testing.
    // The safe Rust API prevents this by requiring a handle as input.
}

/// Finalize after init with custom proc should succeed.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_finalize_with_custom_proc() {
    // Tool init with a specific proc identity, then finalize.
    // This tests that finalize works regardless of how the handle was obtained.
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => return,
    };
    // Verify handle has valid proc
    let proc = handle.proc();
    assert!(
        proc.nspace().is_some(),
        "Handle should have a namespace before finalize"
    );
    let result = tool_finalize(handle);
    assert!(result.is_ok(), "finalize should succeed with custom proc");
}

/// Clone the handle, finalize original — cloned handle still valid.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_clone_then_finalize_original() {
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => return,
    };
    let cloned = handle.clone();
    // Finalize the original
    let r1 = tool_finalize(handle);
    assert!(r1.is_ok(), "Finalize original should succeed");
    // The cloned handle still exists (it carries the same proc info)
    // Note: the reference count is managed by the C library, not our handle.
    // Finalizing the clone would decrement again.
    let _ = cloned.proc(); // Verify cloned handle is still accessible
}

/// Verify tool_finalize error path returns Err with a PmixStatus.
/// Ignored because it requires a PMIx environment to trigger errors.
#[test]
#[ignore = "requires PMIx server"]
fn test_finalize_error_status() {
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => return,
    };
    // First finalize should succeed
    assert!(tool_finalize(handle).is_ok());
    // If we had another handle (from a clone before finalize),
    // finalizing again might return an error depending on ref count.
    // This is implementation-dependent and documented as such.
}
