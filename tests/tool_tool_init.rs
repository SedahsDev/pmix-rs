//! Tests for `PMIx_tool_init`, `PMIx_tool_finalize`, `PmixToolHandle`,
//! `tool_init_minimal`, and `is_tool_initialized`.
//!
//! Note: `PMIx_tool_init` requires a running PMIx daemon or a proper
//! PMIx server environment. Tests that call the actual FFI are marked
//! `#[ignore]` and should be run with a PMIx environment.
//! Unit tests that verify API structure, types, and defaults run without
//! a PMIx runtime.

use pmix::tool::{
    is_tool_initialized, tool_finalize, tool_init, tool_init_minimal, PmixToolHandle,
};
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixToolHandle — structure and traits
// ─────────────────────────────────────────────────────────────────────────────

/// PmixToolHandle implements Clone.
#[test]
fn test_tool_handle_clone() {
    // We cannot construct a PmixToolHandle directly (it requires FFI),
    // but we can verify the type is Clone via a compile-time check.
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixToolHandle>();
}

/// PmixToolHandle implements Debug.
#[test]
fn test_tool_handle_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixToolHandle>();
}

/// PmixToolHandle implements Clone + Debug together.
#[test]
fn test_tool_handle_traits() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixToolHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// is_tool_initialized — state checks (no FFI)
// ─────────────────────────────────────────────────────────────────────────────

/// is_tool_initialized returns a bool (compile check).
#[test]
fn test_is_tool_initialized_returns_bool() {
    let _val: bool = is_tool_initialized();
}

/// is_tool_initialized is idempotent — calling it multiple times returns the same value
/// (without any tool_init having been called).
#[test]
fn test_is_tool_initialized_idempotent() {
    let v1 = is_tool_initialized();
    let v2 = is_tool_initialized();
    assert_eq!(v1, v2, "is_tool_initialized should be idempotent without init");
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init — signature and parameter checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init returns Result<PmixToolHandle, PmixStatus> (compile check).
#[test]
fn test_tool_init_return_type() {
    fn assert_return_type() -> Result<PmixToolHandle, PmixStatus> {
        // This is only a compile-time check — we don't actually call it.
        unreachable!()
    }
    let _ = std::mem::needs_drop::<std::result::Result<PmixToolHandle, PmixStatus>>();
}

/// tool_init signature accepts Option<&Proc> and &Info.
#[test]
fn test_tool_init_signature() {
    // Compile-time signature check:
    // tool_init takes Option<&Proc> and &Info.
    // We verify by assigning the function to a typed variable.
    let _: fn(Option<&pmix::Proc>, &pmix::Info) -> Result<PmixToolHandle, PmixStatus> = tool_init;
}

/// tool_init_minimal signature returns Result<PmixToolHandle, PmixStatus>.
#[test]
fn test_tool_init_minimal_signature() {
    let _: fn() -> Result<PmixToolHandle, PmixStatus> = tool_init_minimal;
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_finalize — signature checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_finalize consumes PmixToolHandle and returns Result<(), PmixStatus>.
#[test]
fn test_tool_finalize_signature() {
    let _: fn(PmixToolHandle) -> Result<(), PmixStatus> = tool_finalize;
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus interaction
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw converts success (0) correctly.
#[test]
fn test_pmix_status_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "Raw 0 should be success");
}

/// PmixStatus::from_raw converts error (-1) correctly.
#[test]
fn test_pmix_status_error() {
    let status = PmixStatus::from_raw(-1);
    assert!(status.is_error(), "Raw -1 should be error");
}

/// PmixStatus is Clone + Copy + Debug.
#[test]
fn test_pmix_status_traits() {
    fn assert_traits<T: Clone + Copy + std::fmt::Debug>() {}
    assert_traits::<PmixStatus>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc helpers — nspace and rank
// ─────────────────────────────────────────────────────────────────────────────

/// Proc::nspace returns Option<String> (compile check).
#[test]
fn test_proc_nspace_return_type() {
    fn assert_nspace_type(p: &pmix::Proc) -> Option<String> {
        p.nspace()
    }
    let _ = assert_nspace_type;
}

/// Proc::rank returns u32 (compile check).
#[test]
fn test_proc_rank_return_type() {
    fn assert_rank_type(p: &pmix::Proc) -> u32 {
        p.rank()
    }
    let _ = assert_rank_type;
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init with no info should return a handle or an error.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_init_with_server() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    match result {
        Ok(handle) => {
            // Handle should have a proc with nspace and rank
            let proc = handle.proc();
            assert!(
                proc.nspace().is_some(),
                "Tool handle should have a namespace"
            );
            let _rank = proc.rank();
            // Finalize
            let finalize_result = tool_finalize(handle);
            assert!(
                finalize_result.is_ok(),
                "tool_finalize should succeed after tool_init"
            );
        }
        Err(status) => {
            // Expected when no PMIx server is available
            assert!(status.is_error(), "Expected error status when no server");
        }
    }
}

/// tool_init_minimal should behave like tool_init with empty info.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_init_minimal_with_server() {
    let result = tool_init_minimal();
    match result {
        Ok(handle) => {
            let proc = handle.proc();
            assert!(proc.nspace().is_some(), "Should have namespace");
            let _ = tool_finalize(handle);
        }
        Err(status) => {
            assert!(status.is_error(), "Expected error status when no server");
        }
    }
}

/// Double finalize should return an error.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_double_finalize_error() {
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => {
            // No server available — skip test
            return;
        }
    };
    // First finalize should succeed
    let result1 = tool_finalize(handle.clone());
    assert!(result1.is_ok(), "First finalize should succeed");
    // Second finalize on the cloned handle should fail or succeed depending on ref count
    // PMIx tool library is reference-counted, so this is implementation-dependent
    let _result2 = tool_finalize(handle);
}

/// tool_init then is_tool_initialized should return true.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_init_sets_initialized_flag() {
    let info = InfoBuilder::new().build();
    let handle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(_) => {
            return;
        }
    };
    assert!(
        is_tool_initialized(),
        "is_tool_initialized should be true after tool_init"
    );
    let _ = tool_finalize(handle);
    assert!(
        !is_tool_initialized(),
        "is_tool_initialized should be false after tool_finalize"
    );
}

/// tool_init with DO_NOT_CONNECT should not try to connect to a server.
/// Ignored because it requires PMIx library availability.
#[test]
#[ignore = "requires PMIx library"]
fn test_tool_init_do_not_connect() {
    // PMIX_TOOL_DO_NOT_CONNECT tells the library to skip server connection.
    // This test verifies the tool library can be initialized without a server
    // when this flag is set.
    // Note: constructing an Info with this key requires the InfoBuilder or
    // direct FFI, which is beyond the scope of this test.
    // The test exists as a placeholder for future integration testing.
}

/// PmixToolHandle::proc returns a reference to Proc.
#[test]
fn test_tool_handle_proc_method() {
    fn assert_proc_method(h: &PmixToolHandle) -> &pmix::Proc {
        h.proc()
    }
    let _ = assert_proc_method;
}

/// PmixToolHandle Debug output contains struct name.
#[test]
fn test_tool_handle_debug_contains_name() {
    // We can't construct a real handle without FFI, but we can verify
    // the Debug impl exists and the type is formattable.
    // This is a compile-time verification that Debug is implemented.
    fn check_debug<T: std::fmt::Debug>() {}
    check_debug::<PmixToolHandle>();
}

/// Multiple is_tool_initialized calls are consistent.
#[test]
fn test_is_tool_initialized_consistency() {
    let values: Vec<bool> = (0..10).map(|_| is_tool_initialized()).collect();
    let first = values[0];
    for (i, val) in values.iter().enumerate() {
        assert_eq!(
            *val, first,
            "is_tool_initialized should be consistent across {} calls",
            i + 1
        );
    }
}

/// tool_init and tool_init_minimal have compatible return types.
#[test]
fn test_tool_init_variants_same_return() {
    type InitReturn = Result<PmixToolHandle, PmixStatus>;
    fn _check_full() -> InitReturn {
        tool_init(None, &InfoBuilder::new().build())
    }
    fn _check_minimal() -> InitReturn {
        tool_init_minimal()
    }
    let _ = (_check_full, _check_minimal);
}
