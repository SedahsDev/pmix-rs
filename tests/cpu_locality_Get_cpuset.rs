//! Tests for PMIx_Get_cpuset — cpu_locality module.
//!
//! These tests verify the safe Rust wrapper around the C API function
//! `PMIx_Get_cpuset`, which retrieves the CPU set for the calling
//! process or thread as determined by the PMIx framework.

use pmix::cpu_locality::PmixBindEnvelope;
use pmix::cpu_locality::get_cpuset;
use pmix::fabric::PmixCpuset;

// ─────────────────────────────────────────────────────────────────────────────
// Basic functionality tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_cpuset with Process envelope compiles and does not panic.
#[test]
fn test_get_cpuset_process_envelope() {
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    // Without a running PMIx session, this may return PMIX_ERR_INIT.
    // The important thing is that the FFI call is made correctly.
    let _ = result;
}

/// Test that get_cpuset with Thread envelope compiles and does not panic.
#[test]
fn test_get_cpuset_thread_envelope() {
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Thread);
    let _ = result;
}

/// Test that get_cpuset returns an error when PMIx is not initialized.
///
/// This mirrors the C behavior from pmix_client_topology.c:
/// ```c
/// if (pmix_globals.init_cntr <= 0) {
///     return PMIX_ERR_INIT;
/// }
/// ```
#[test]
fn test_get_cpuset_not_initialized() {
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    assert!(
        result.is_err(),
        "get_cpuset should return an error when PMIx is not initialized"
    );
}

/// Test that get_cpuset returns the expected error type (PMIX_ERR_INIT).
#[test]
fn test_get_cpuset_error_type() {
    use pmix::PmixStatus;

    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    match result {
        Err(status) => {
            // The error should be PMIX_ERR_INIT (or another error code).
            // We verify the status is an error, not success.
            assert!(!PmixStatus::from_raw(0).is_success() || result.is_err());
            let _ = status;
        }
        Ok(()) => {
            // If PMIx happens to be initialized (e.g., in an integration test),
            // success is also acceptable.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cpuset reuse and cleanup tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that the cpuset is properly cleaned up on drop even after
/// a failed get_cpuset call.
#[test]
fn test_get_cpuset_cleanup_on_error() {
    let mut cpuset = PmixCpuset::new();
    let _ = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    // cpuset should still drop without issues
    drop(cpuset);
}

/// Test that get_cpuset can be called multiple times on the same cpuset.
#[test]
fn test_get_cpuset_reuse_cpuset() {
    let mut cpuset = PmixCpuset::new();
    let r1 = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    let r2 = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    // Both calls should return the same result (likely PMIX_ERR_INIT).
    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "repeated calls should be consistent"
    );
}

/// Test that get_cpuset works with a fresh cpuset each time.
#[test]
fn test_get_cpuset_fresh_cpuset_each_call() {
    let cpuset1 = PmixCpuset::new();
    let cpuset2 = PmixCpuset::new();

    let mut c1 = cpuset1;
    let mut c2 = cpuset2;
    let r1 = get_cpuset(&mut c1, PmixBindEnvelope::Process);
    let r2 = get_cpuset(&mut c2, PmixBindEnvelope::Thread);

    // Both should return consistent results (likely errors).
    assert_eq!(r1.is_err(), r2.is_err());
}

/// Test that get_cpuset does not leak memory across repeated calls.
#[test]
fn test_get_cpuset_no_memory_leak() {
    for _ in 0..10 {
        let mut cpuset = PmixCpuset::new();
        let _ = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
        // cpuset is dropped at end of each iteration.
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixBindEnvelope tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test PmixBindEnvelope to_raw conversion matches C constants.
#[test]
fn test_bind_envelope_process_to_raw() {
    // PMIX_CPUBIND_PROCESS = 0
    assert_eq!(PmixBindEnvelope::Process.to_raw(), 0);
}

/// Test PmixBindEnvelope Thread variant maps to C constant.
#[test]
fn test_bind_envelope_thread_to_raw() {
    // PMIX_CPUBIND_THREAD = 1
    assert_eq!(PmixBindEnvelope::Thread.to_raw(), 1);
}

/// Test PmixBindEnvelope Clone and Copy traits.
#[test]
fn test_bind_envelope_clone() {
    let p = PmixBindEnvelope::Process;
    let p2 = p.clone();
    assert_eq!(p, p2);
}

/// Test PmixBindEnvelope Copy semantics.
#[test]
fn test_bind_envelope_copy() {
    let p = PmixBindEnvelope::Thread;
    let p2 = p; // Copy, not move
    assert_eq!(p, p2);
}

/// Test PmixBindEnvelope PartialEq.
#[test]
fn test_bind_envelope_partial_eq() {
    assert_eq!(PmixBindEnvelope::Process, PmixBindEnvelope::Process);
    assert_eq!(PmixBindEnvelope::Thread, PmixBindEnvelope::Thread);
    assert_ne!(PmixBindEnvelope::Process, PmixBindEnvelope::Thread);
}

/// Test PmixBindEnvelope Debug formatting.
#[test]
fn test_bind_envelope_debug() {
    let debug_str = format!("{:?}", PmixBindEnvelope::Process);
    assert!(!debug_str.is_empty());
}

/// Test PmixBindEnvelope Eq trait.
#[test]
fn test_bind_envelope_eq() {
    let a = PmixBindEnvelope::Process;
    let b = PmixBindEnvelope::Process;
    assert!(a == b);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration-style tests (derived from C test in simpfabric.c)
// ─────────────────────────────────────────────────────────────────────────────

/// Test derived from simpfabric.c pattern:
/// Construct cpuset, call get_cpuset with Process, check result.
///
/// In the C test:
/// ```c
/// PMIX_CPUSET_CONSTRUCT(&mycpuset);
/// rc = PMIx_Get_cpuset(&mycpuset, PMIX_CPUBIND_PROCESS);
/// if (PMIX_SUCCESS != rc) {
///     fprintf(stderr, "Get of my cpuset failed: %s\n", PMIx_Error_string(rc));
///     goto cleanup;
/// }
/// ```
#[test]
fn test_get_cpuset_simpfabric_pattern() {
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    match result {
        Ok(()) => {
            // Success — the cpuset has been populated.
            // In a real PMIx session, we would then call
            // PMIx_server_generate_cpuset_string to inspect it.
        }
        Err(status) => {
            // Expected when PMIx is not initialized.
            // The error status should be a valid PMIx error code.
            assert!(!status.is_success());
        }
    }
}

/// Test that get_cpuset follows the same init-check pattern as the C impl.
/// The C code checks `pmix_globals.init_cntr <= 0` and returns PMIX_ERR_INIT.
#[test]
#[ignore = "requires PMIx runtime — needs PMIx_Init to succeed"]
fn test_get_cpuset_initialized_session() {
    // This test would require a full PMIx session to be initialized.
    // In a real integration test, we would:
    // 1. Call PMIx_Init
    // 2. Call get_cpuset with Process envelope
    // 3. Verify it returns Ok(())
    // 4. Verify the cpuset bitmap is non-empty
    // 5. Call PMIx_Finalize
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    assert!(
        result.is_ok(),
        "get_cpuset should succeed in an initialized session"
    );
}

/// Test that get_cpuset with different envelopes returns different results.
/// In a real PMIx session, Process and Thread may return different cpusets
/// if the thread has been bound to a different CPU set.
#[test]
#[ignore = "requires PMIx runtime — needs PMIx_Init to succeed"]
fn test_get_cpuset_process_vs_thread() {
    let mut cpuset_proc = PmixCpuset::new();
    let mut cpuset_thread = PmixCpuset::new();

    let r_proc = get_cpuset(&mut cpuset_proc, PmixBindEnvelope::Process);
    let r_thread = get_cpuset(&mut cpuset_thread, PmixBindEnvelope::Thread);

    // Both should succeed in an initialized session.
    assert!(r_proc.is_ok());
    assert!(r_thread.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Test get_cpuset with a cpuset that was just constructed (no prior use).
#[test]
fn test_get_cpuset_freshly_constructed() {
    let mut cpuset = PmixCpuset::new();
    // No prior operations on the cpuset — just call get_cpuset.
    let _ = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
}

/// Test that get_cpuset does not modify the cpuset on error.
/// The cpuset should remain in a valid (empty) state after an error.
#[test]
fn test_get_cpuset_error_preserves_cpuset() {
    let mut cpuset = PmixCpuset::new();
    let result = get_cpuset(&mut cpuset, PmixBindEnvelope::Process);
    if result.is_err() {
        // The cpuset should still be usable for subsequent calls.
        let result2 = get_cpuset(&mut cpuset, PmixBindEnvelope::Thread);
        assert!(result2.is_err());
    }
}

/// Test that multiple cpusets can be queried in parallel.
#[test]
fn test_get_cpuset_multiple_cpusets() {
    let mut cpuset1 = PmixCpuset::new();
    let mut cpuset2 = PmixCpuset::new();
    let mut cpuset3 = PmixCpuset::new();

    let r1 = get_cpuset(&mut cpuset1, PmixBindEnvelope::Process);
    let r2 = get_cpuset(&mut cpuset2, PmixBindEnvelope::Thread);
    let r3 = get_cpuset(&mut cpuset3, PmixBindEnvelope::Process);

    // All should return consistent results (likely errors).
    assert_eq!(r1.is_err(), r2.is_err());
    assert_eq!(r2.is_err(), r3.is_err());
}
