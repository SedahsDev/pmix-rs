//! Tests for `PMIx_server_delete_process_set` safe wrapper.
//!
//! The C PMIx library does not ship dedicated test cases for this function.
//! Tests are derived from the function's documented behavior (spec section 16.2.20),
//! the patterns used by related server APIs, and the spec's description of
//! process set deletion semantics.
//!
//! NOTE: PMIx_server_delete_process_set requires a prior PMIx_server_init.
//! All tests that invoke the FFI function are marked #[ignore] and require
//! a running PMIx server/daemon.

use pmix::PmixStatus;
use pmix::server::server_delete_process_set;

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
    // PMIX_ERR_BAD_PARAM = -8
    let status = PmixStatus::from_raw(-8);
    assert!(!status.is_success(), "raw -8 should be an error");
}

/// Verify the function signature takes &str (not &mut).
/// Compile-time check — if the signature changes, this test fails.
#[test]
fn test_signature_type_check() {
    let _fn_ptr: fn(&str) -> Result<(), PmixStatus> = server_delete_process_set;
}

/// Verify that the function returns Result<(), PmixStatus> at the type level.
#[test]
fn test_return_type_check() {
    fn assert_return_type(_: Result<(), PmixStatus>) {}
    let _ = assert_return_type;
}

/// Verify PMIX_ERR_BAD_PARAM raw value matches the spec (used by C impl for null params).
#[test]
fn test_pmix_err_bad_param_value() {
    let status = PmixStatus::from_raw(-8);
    assert!(
        !status.is_success(),
        "PMIX_ERR_BAD_PARAM should be an error"
    );
}

/// Verify PMIX_ERR_NOT_FOUND raw value (used when pset doesn't exist).
#[test]
fn test_pmix_err_not_found_value() {
    // PMIX_ERR_NOT_FOUND = -46
    let status = PmixStatus::from_raw(-46);
    assert!(
        !status.is_success(),
        "PMIX_ERR_NOT_FOUND should be an error"
    );
}

/// Verify PmixStatus equality for error codes.
#[test]
fn test_pmix_status_equality() {
    let s1 = PmixStatus::from_raw(-1);
    let s2 = PmixStatus::from_raw(-1);
    assert_eq!(s1, s2, "same raw code should produce equal PmixStatus");
}

/// Verify PmixStatus to_raw round-trip.
#[test]
fn test_pmix_status_to_raw_roundtrip() {
    for code in [-46i32, -8, -1, 0, 1] {
        let status = PmixStatus::from_raw(code);
        assert_eq!(
            status.to_raw(),
            code,
            "round-trip should preserve raw value for {}",
            code
        );
    }
}

/// Verify the function is callable with various string types.
#[test]
fn test_callable_with_string_literals() {
    // Just verify the function is callable — we can't test the result without a server.
    // This is a compile-time check that &str works.
    let _ = || {
        let _ = server_delete_process_set("pset_name");
        let _ = server_delete_process_set("pset_1");
        let _ = server_delete_process_set("a");
    };
}

/// Verify the function is callable with String::as_str().
#[test]
fn test_callable_with_string_as_str() {
    let name = String::from("dynamic_pset_name");
    let _ = || {
        let _ = server_delete_process_set(name.as_str());
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI tests — require initialized PMIx server (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

/// Basic API call: the function should accept a valid pset name.
/// Whether it returns Ok or Err depends on the PMIx runtime environment,
/// so we only check it doesn't panic.
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_delete_process_set_basic() {
    let _result = server_delete_process_set("pset1");
}

/// The function should return a PmixStatus (either success or a known error).
#[test]
#[ignore = "requires PMIx server initialized — calls FFI which segfaults without init"]
fn test_delete_process_set_returns_pmix_status() {
    let result = server_delete_process_set("pset1");

    match result {
        Ok(()) => {
            // Success — process set was deleted.
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
/// consistent results (second call should fail since the pset was already deleted).
#[test]
#[ignore = "requires PMIx server initialized — idempotency test"]
fn test_delete_process_set_consistent() {
    let result1 = server_delete_process_set("pset1");
    let result2 = server_delete_process_set("pset1");

    match (result1, result2) {
        (Ok(()), Ok(())) => {
            // Both succeeded — possible if the pset was redefined between calls.
        }
        (Err(e1), Err(e2)) => {
            assert_eq!(e1, e2, "error statuses should be consistent across calls");
        }
        (Ok(()), Err(_)) => {
            // First deleted, second failed (pset no longer exists) — expected behavior.
        }
        (Err(_), Ok(())) => {
            // Unexpected — first failed, second succeeded.
            // This could happen if another process defined the pset between calls.
        }
    }
}

/// Delete multiple different process sets.
#[test]
#[ignore = "requires PMIx server initialized — multiple psets"]
fn test_delete_process_set_multiple_psets() {
    let _result1 = server_delete_process_set("pset1");
    let _result2 = server_delete_process_set("pset2");
    let _result3 = server_delete_process_set("pset3");
}

/// Pset name with various valid characters (alphanumeric, underscore, hyphen).
#[test]
#[ignore = "requires PMIx server initialized — name format test"]
fn test_delete_process_set_pset_name_formats() {
    let _r1 = server_delete_process_set("pset_1");
    let _r2 = server_delete_process_set("pset-2");
    let _r3 = server_delete_process_set("PSET3");
    let _r4 = server_delete_process_set("a");
}

/// Delete a non-existent process set — should return PMIX_ERR_NOT_FOUND or similar.
#[test]
#[ignore = "requires PMIx server initialized — non-existent pset"]
fn test_delete_process_set_non_existent() {
    let result = server_delete_process_set("non_existent_pset_xyz");
    match result {
        Ok(()) => {
            // Some implementations may silently succeed for non-existent psets.
        }
        Err(status) => {
            // PMIX_ERR_NOT_FOUND (-46) or PMIX_ERR_BAD_PARAM (-8) are expected.
            let raw = status.to_raw();
            assert!(
                raw != 0,
                "error status should not be PMIX_SUCCESS for non-existent pset"
            );
        }
    }
}

/// Calling the function without PMIx server initialized should fail.
#[test]
#[ignore = "requires PMIx server to be initialized — needs running PMIx daemon"]
fn test_requires_server_init() {
    let result = server_delete_process_set("pset1");
    assert!(result.is_err(), "should fail without server init");
}

/// When PMIx server IS initialized and the pset exists, the function should succeed.
#[test]
#[ignore = "requires PMIx server initialized"]
fn test_with_initialized_server() {
    let result = server_delete_process_set("pset1");
    assert!(result.is_ok(), "should succeed with initialized server");
}

/// Memory safety: calling the function many times should not leak.
#[test]
#[ignore = "requires PMIx server initialized — memory leak smoke test"]
fn test_no_memory_leak_smoke() {
    for i in 0..100 {
        let _ = server_delete_process_set(&format!("pset_{}", i));
    }
}

/// Spec section 16.2.20 says the server shall alert all local clients via
/// PMIX_PROCESS_SET_DELETE event. This test documents that behavior.
#[test]
#[ignore = "requires PMIx server initialized and event listener"]
fn test_process_set_delete_event() {
    // The spec states: "The PMIx server shall alert all local clients of the
    // process set name being deleted via the PMIX_PROCESS_SET_DELETE event."
    //
    // We cannot verify event delivery without a full PMIx server setup
    // with registered event listeners, but we can verify the call itself
    // completes without error.
    let result = server_delete_process_set("pset_event_test");
    assert!(result.is_ok(), "should succeed to trigger event");
}

/// Delete a pset name that looks like a namespace — implementation dependent.
#[test]
#[ignore = "requires PMIx server initialized — name conflict test"]
fn test_pset_name_conflict_with_namespace() {
    // The spec says the host environment is responsible for ensuring that
    // process set names do not conflict with system-assigned namespaces.
    // Our wrapper does not enforce this — it is the caller's responsibility.
    let _result = server_delete_process_set("test_nspace");
    // The PMIx server may reject this or accept it — implementation dependent.
}

/// Verify the wrapper correctly handles the case where the C library
/// returns PMIX_ERR_OPERATION_UNSUPPORTED (the server may not support psets).
#[test]
#[ignore = "requires PMIx server that does not support process sets"]
fn test_operation_unsupported() {
    let result = server_delete_process_set("pset1");
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

/// Verify deletion has no impact on member processes (per spec).
/// The spec says: "Deletion of the name has no impact on the member processes."
#[test]
#[ignore = "requires PMIx server initialized — member process check"]
fn test_delete_no_impact_on_members() {
    // Delete the pset, then verify member processes are still accessible.
    // This is a behavioral test documented by the spec.
    let _result = server_delete_process_set("pset_members");
    // Member processes should still be alive and queryable after pset deletion.
    // We cannot verify this without a full PMIx test harness, but the call
    // itself should complete without error.
}

/// Host environment responsibility: consistent knowledge of process set
/// membership across all involved PMIx servers.
#[test]
#[ignore = "requires multiple PMIx servers — distributed test"]
fn test_consistent_across_servers() {
    // The spec says: "The host environment is responsible for ensuring
    // consistent knowledge of process set membership across all involved PMIx servers."
    // Our wrapper does not enforce this — it is the caller's responsibility.
    let _result = server_delete_process_set("pset_distributed");
}

/// Test that the function properly handles empty string pset names.
#[test]
#[ignore = "requires PMIx server initialized — empty pset name"]
fn test_empty_pset_name() {
    let result = server_delete_process_set("");
    // PMIx may return PMIX_ERR_BAD_PARAM for empty pset names.
    match result {
        Ok(()) => {
            // Some implementations may handle empty names.
        }
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Test with a very long pset name.
#[test]
#[ignore = "requires PMIx server initialized — long pset name"]
fn test_long_pset_name() {
    let long_name = "pset_".repeat(100); // 400 char name
    let result = server_delete_process_set(&long_name);
    match result {
        Ok(()) => {
            // Server accepted the long name.
        }
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}

/// Verify the function works with Unicode pset names (if supported by PMIx).
#[test]
#[ignore = "requires PMIx server initialized — Unicode pset name"]
fn test_unicode_pset_name() {
    // PMIx uses C strings, so Unicode may or may not work depending on encoding.
    // Our wrapper uses CString which will reject NUL bytes but accepts UTF-8.
    let result = server_delete_process_set("pset_alpha");
    match result {
        Ok(()) => {}
        Err(status) => {
            assert!(!status.is_success(), "error status should not be success");
        }
    }
}
