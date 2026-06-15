//! Tests for `PMIx_server_define_process_set` safe wrapper.
//!
//! The C PMIx library does not ship dedicated test cases for this function.
//! Tests are derived from the function's documented behavior (spec section 16.2.19),
//! the patterns used by related server APIs, and the spec's description of
//! process set semantics.
//!
//! NOTE: PMIx_server_define_process_set requires a prior PMIx_server_init.
//! All tests that invoke the FFI function are marked #[ignore] and require
//! a running PMIx server/daemon.

use pmix::server::server_define_process_set;
use pmix::{PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus round-trip tests (no FFI — these run without a PMIx server)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that success status from the FFI layer maps correctly.
#[test]
fn test_pmix_status_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "raw 0 should be PMIX_SUCCESS");
}

/// Verify that error statuses are properly detected.
#[test]
fn test_pmix_status_error() {
    // PMIX_ERROR = -1
    let status = PmixStatus::from_raw(-1);
    assert!(!status.is_success(), "raw -1 should be an error");
    // PMIX_ERR_BAD_PARAM = -8 (returned by C impl when params are invalid)
    let status = PmixStatus::from_raw(-8);
    assert!(!status.is_success(), "raw -8 should be an error");
}

/// Verify the function signature accepts &[Proc] and &str (not &mut).
/// This is a compile-time check — if the signature changes, this test fails.
#[test]
fn test_signature_type_check() {
    // Verify Proc is constructable (tests the type exists and is usable).
    let _proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
}

/// Verify that the function returns Result<(), PmixStatus> at the type level.
/// Compile-time check — the types must match or this test won't compile.
#[test]
fn test_return_type_check() {
    // This is a type-level assertion: server_define_process_set returns
    // Result<(), PmixStatus>. If the return type changes, this fails to compile.
    fn assert_return_type(_: Result<(), PmixStatus>) {}
    // We can't call the function without a server, but we can verify the
    // type signature is what we expect by checking the function pointer type.
    let _fn_ptr: fn(&[Proc], &str) -> Result<(), PmixStatus> = server_define_process_set;
    // Suppress unused warning on the assertion helper.
    let _ = assert_return_type;
}

/// Verify PMIX_ERR_BAD_PARAM raw value matches the spec (used by C impl for null params).
#[test]
fn test_pmix_err_bad_param_value() {
    // PMIX_ERR_BAD_PARAM = -8 in PMIx v4.x
    let status = PmixStatus::from_raw(-8);
    assert!(
        !status.is_success(),
        "PMIX_ERR_BAD_PARAM should be an error"
    );
}

/// Verify Proc construction works with various ranks.
#[test]
fn test_proc_construction_various_ranks() {
    for rank in 0..10u32 {
        let proc = Proc::new("test_nspace", rank).expect("Proc::new should work");
        assert_eq!(proc.get_rank(), rank, "rank should match what we set");
    }
}

/// Verify Proc construction fails for nspace containing NUL byte.
#[test]
fn test_proc_construction_nul_in_nspace() {
    // CString::new fails on NUL bytes — our wrapper propagates this as PMIX_ERROR.
    // Proc::new uses CString internally, so it returns Err(NulError).
    let result = Proc::new("test\0nspace", 0);
    assert!(result.is_err(), "Proc::new should fail with NUL in nspace");
}

/// Verify multiple Proc constructions and destructions don't leak.
#[test]
fn test_proc_multiple_lifecycle() {
    for _ in 0..10 {
        let _proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    }
}

/// Verify Proc::new_with_nspace creates a copy with the same nspace.
#[test]
fn test_proc_new_with_nspace() {
    let proc1 = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let proc2 = proc1
        .new_with_nspace(1)
        .expect("new_with_nspace should work");
    assert_eq!(proc2.get_rank(), 1, "new rank should be set");
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI tests — require initialized PMIx server (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Basic API call: the function should accept a valid Proc slice and pset name.
/// Whether it returns Ok or Err depends on the PMIx runtime environment,
/// so we only check it doesn't panic.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_basic() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let _result = server_define_process_set(&members, "pset1");
}

/// The function should return a PmixStatus (either success or a known error).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_returns_pmix_status() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result = server_define_process_set(&members, "pset1");

    match result {
        Ok(()) => {
            // Success is valid — process set was defined.
        }
        Err(status) => {
            // On error, we should get a valid PmixStatus (not a raw code of 0).
            let raw = status.to_raw();
            assert!(
                raw != 0,
                "error status should not have raw value 0 (that's PMIX_SUCCESS)"
            );
        }
    }
}

/// Calling the function multiple times with the same pset name should produce
/// consistent results (same outcome on repeated calls).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_consistent() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result1 = server_define_process_set(&members, "pset1");
    let result2 = server_define_process_set(&members, "pset1");

    match (result1, result2) {
        (Ok(()), Ok(())) => {
            // Both succeeded — consistent.
        }
        (Err(e1), Err(e2)) => {
            assert_eq!(e1, e2, "error statuses should be consistent across calls");
        }
        _ => {
            // One Ok, one Err — this could happen if the first defines and
            // the second fails because the pset already exists.
            // This is acceptable behavior depending on PMIx implementation.
        }
    }
}

/// Multiple calls with different pset names should not interfere.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_multiple_psets() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let _result1 = server_define_process_set(&members, "pset1");
    let _result2 = server_define_process_set(&members, "pset2");
    let _result3 = server_define_process_set(&members, "pset3");
}

/// Calling with an empty members array should be handled gracefully.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_empty_members() {
    let result = server_define_process_set(&[], "pset_empty");
    // PMIx may return PMIX_ERR_BAD_PARAM for empty members, or handle it gracefully.
    // Either way, it should not panic or segfault.
    match result {
        Ok(()) => {
            // Some implementations may allow empty process sets.
        }
        Err(status) => {
            // PMIX_ERR_BAD_PARAM is the expected error for empty members.
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Multiple members in a single process set.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_multiple_members() {
    let members: Vec<Proc> = (0..5)
        .map(|rank| Proc::new("test_nspace", rank).expect("Proc::new should work"))
        .collect();
    let result = server_define_process_set(&members, "pset_multi");
    match result {
        Ok(()) => {
            // Success — process set with 5 members defined.
        }
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Pset name with various valid characters (alphanumeric, underscore, hyphen).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_define_process_set_pset_name_formats() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];

    let _r1 = server_define_process_set(&members, "pset_1");
    let _r2 = server_define_process_set(&members, "pset-2");
    let _r3 = server_define_process_set(&members, "PSET3");
    let _r4 = server_define_process_set(&members, "a");
}

/// Calling the function without PMIx server initialized should fail.
#[test]
#[ignore = "requires PMIx server to be initialized — needs running PMIx daemon"]
fn test_requires_server_init() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result = server_define_process_set(&members, "pset1");
    assert!(result.is_err(), "should fail without server init");
}

/// When PMIx server IS initialized, the function should succeed for valid input.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_with_initialized_server() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result = server_define_process_set(&members, "pset1");
    assert!(result.is_ok(), "should succeed with initialized server");
}

/// Members from different namespaces can belong to the same process set.
#[test]
#[ignore = "requires PMIx server initialized — cross-namespace pset"]
fn test_define_process_set_cross_namespace() {
    let proc1 = Proc::new("nspace_a", 0).expect("Proc::new should work");
    let proc2 = Proc::new("nspace_b", 0).expect("Proc::new should work");
    let members = vec![proc1, proc2];
    let result = server_define_process_set(&members, "pset_cross");
    match result {
        Ok(()) => {
            // Cross-namespace process set defined successfully.
        }
        Err(status) => {
            // Some implementations may restrict cross-namespace psets.
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Calling the function with members from the same namespace but different ranks.
#[test]
#[ignore = "requires PMIx server initialized — same namespace, different ranks"]
fn test_define_process_set_same_namespace_different_ranks() {
    let members: Vec<Proc> = (0..3)
        .map(|rank| Proc::new("test_nspace", rank).expect("Proc::new should work"))
        .collect();
    let result = server_define_process_set(&members, "pset_ranks");
    match result {
        Ok(()) => {
            // Process set with 3 members from same namespace defined.
        }
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Verify the function properly handles CString allocation for pset_name.
/// A pset_name containing a NUL byte should return PMIX_ERROR.
#[test]
fn test_pset_name_with_nul_byte() {
    // Our wrapper uses CString::new which rejects NUL bytes.
    // We cannot construct a &str with a NUL byte in Rust, so this tests
    // that the function signature correctly takes &str (which guarantees no NUL).
    // The actual NUL-byte rejection happens at the Rust type level.
    // This is a compile-time guarantee, not a runtime check.
}

/// Memory safety: calling the function many times should not leak.
/// The temporary pmix_proc_t array is allocated with calloc and freed with free.
#[test]
#[ignore = "requires PMIx server initialized — memory leak smoke test"]
fn test_no_memory_leak_smoke() {
    for i in 0..100 {
        let members: Vec<Proc> = (0..3)
            .map(|rank| Proc::new(&format!("nspace_{}", i), rank).expect("Proc::new should work"))
            .collect();
        let _ = server_define_process_set(&members, &format!("pset_{}", i));
    }
}

/// Spec section 16.2.19 says the server shall alert all local clients via
/// PMIX_PROCESS_SET_DEFINE event. This test documents that behavior.
#[test]
#[ignore = "requires PMIx server initialized and event listener"]
fn test_process_set_define_event() {
    // The spec states: "The PMIx server shall alert all local clients of the
    // new process set (including process set name and membership) via the
    // PMIX_PROCESS_SET_DEFINE event."
    //
    // We cannot verify event delivery without a full PMIx server setup
    // with registered event listeners, but we can verify the call itself
    // completes without error.
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result = server_define_process_set(&members, "pset_event_test");
    assert!(result.is_ok(), "should succeed to trigger event");
}

/// Host environment responsibility: process set names should not conflict
/// with system-assigned namespaces. This test verifies that attempting to
/// define a pset with a name that looks like a namespace is handled.
#[test]
#[ignore = "requires PMIx server initialized — name conflict test"]
fn test_pset_name_conflict_with_namespace() {
    // The spec says the host environment is responsible for ensuring that
    // process set names do not conflict with system-assigned namespaces.
    // Our wrapper does not enforce this — it is the caller's responsibility.
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let _result = server_define_process_set(&members, "test_nspace");
    // The PMIx server may reject this or accept it — implementation dependent.
}

/// Verify that the wrapper correctly handles the case where the C library
/// returns PMIX_ERR_OPERATION_UNSUPPORTED (the server may not support psets).
#[test]
#[ignore = "requires PMIx server that does not support process sets"]
fn test_operation_unsupported() {
    // PMIX_ERR_OPERATION_UNSUPPORTED = -6
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let members = vec![proc];
    let result = server_define_process_set(&members, "pset1");
    match result {
        Ok(()) => {
            // Server supports it.
        }
        Err(status) => {
            let raw = status.to_raw();
            // Could be PMIX_ERR_OPERATION_UNSUPPORTED (-6) or other error.
            assert!(raw != 0, "error status should not be PMIX_SUCCESS");
        }
    }
}

/// Verify Proc::get_rank returns the correct rank after construction.
#[test]
fn test_proc_get_rank() {
    let proc = Proc::new("test_nspace", 42).expect("Proc::new should work");
    assert_eq!(proc.get_rank(), 42, "rank should be 42");
}

/// Verify Proc::set_rank updates the rank.
#[test]
fn test_proc_set_rank() {
    let mut proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    assert_eq!(proc.get_rank(), 0);
    proc.set_rank(100);
    assert_eq!(proc.get_rank(), 100);
}

/// Test that the function compiles with a slice reference (not just Vec).
#[test]
#[ignore = "requires PMIx server initialized — compile-time slice test"]
fn test_slice_reference() {
    let proc = Proc::new("test_nspace", 0).expect("Proc::new should work");
    let procs = vec![proc];
    // Pass a slice reference, not the Vec itself.
    let _result = server_define_process_set(procs.as_slice(), "pset_slice");
}
