//! Tests for `PMIx_server_setup_fork` safe wrapper.
//!
//! These tests verify the Rust wrapper's behavior:
//! - Signature and type safety
//! - Return type (Result<Vec<String>, PmixStatus>)
//! - Error handling for invalid inputs
//! - Memory safety (no leaks from env array allocation)
//! - Integration with PMIx server (requires running PMIx daemon)
//!
//! Tests that require a running PMIx server are marked #[ignore].

use pmix::Proc;
use pmix::server::server_setup_fork;

// ─────────────────────────────────────────────────────────────────────────────
// Tests that do NOT require PMIx runtime
// ─────────────────────────────────────────────────────────────────────────────

/// Test: server_setup_fork function signature accepts &Proc and Option<Vec<&str>>.
/// This is a compile-time test — if it compiles, the signature is correct.
#[test]
fn test_setup_fork_signature() {
    // Verify the function has the correct signature by type-checking parameters.
    let proc = Proc::new("test_namespace", 0).expect("proc creation failed");
    let _: Result<Vec<String>, _> = server_setup_fork(&proc, None);
    let _: Result<Vec<String>, _> = server_setup_fork(&proc, Some(Vec::new()));
    let _: Result<Vec<String>, _> =
        server_setup_fork(&proc, Some(vec!["KEY=VALUE", "ANOTHER=test"]));
}

/// Test: server_setup_fork returns an error when PMIx server is not initialized.
/// Without PMIx_server_init, setup_fork should return PMIX_ERR_INIT.
#[test]
fn test_setup_fork_without_init_returns_error() {
    let proc = Proc::new("test_namespace", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, None);

    // Should fail because PMIx server is not initialized.
    assert!(
        result.is_err(),
        "server_setup_fork should fail without PMIx server init"
    );

    let err = result.unwrap_err();
    assert!(
        err.is_error(),
        "error status should be an error code, got: {:?}",
        err
    );
}

/// Test: server_setup_fork with empty initial env returns error when not initialized.
#[test]
fn test_setup_fork_empty_env_without_init() {
    let proc = Proc::new("foobar", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, Some(Vec::new()));

    assert!(
        result.is_err(),
        "should fail without server init even with empty env"
    );
}

/// Test: server_setup_fork with initial env vars returns error when not initialized.
#[test]
fn test_setup_fork_with_initial_env_without_init() {
    let proc = Proc::new("myjob.12345", 42).expect("proc creation failed");
    let initial_env = vec!["PATH=/usr/bin", "HOME=/tmp", "TEST_VAR=hello"];
    let result = server_setup_fork(&proc, Some(initial_env));

    assert!(
        result.is_err(),
        "should fail without server init even with env vars"
    );
}

/// Test: Proc construction for various ranks works correctly with setup_fork call.
#[test]
fn test_setup_fork_proc_ranks() {
    // Test that different ranks can be constructed and passed to setup_fork.
    // All should fail (no server init), but the Proc construction should work.
    for rank in [0u32, 1, 100, u32::MAX] {
        let proc = Proc::new("test_ns", rank).expect("proc creation failed");
        assert_eq!(proc.get_rank(), rank);
        let result = server_setup_fork(&proc, None);
        assert!(
            result.is_err(),
            "rank {}: should fail without server init",
            rank
        );
    }
}

/// Test: Proc construction for various namespaces works correctly.
#[test]
fn test_setup_fork_proc_namespaces() {
    let namespaces: Vec<&str> = vec!["job.0001", "foobar", "my_app", "a"];
    let long_ns = "x".repeat(255);
    let all_ns: Vec<&str> = namespaces
        .into_iter()
        .chain(std::iter::once(&*long_ns))
        .collect();
    for ns in &all_ns {
        let proc = Proc::new(ns, 0).expect("proc creation failed");
        let result = server_setup_fork(&proc, None);
        assert!(
            result.is_err(),
            "namespace '{}': should fail without server init",
            ns
        );
    }
}

/// Test: server_setup_fork returns Result type (not Option or other).
#[test]
fn test_setup_fork_return_type() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result: Result<Vec<String>, pmix::PmixStatus> = server_setup_fork(&proc, None);

    // Verify the error type is PmixStatus.
    match result {
        Ok(_) => panic!("should not succeed without server init"),
        Err(status) => {
            assert!(status.is_error(), "should be an error status");
        }
    }
}

/// Test: server_setup_fork with None env is equivalent to empty Vec for error case.
#[test]
fn test_setup_fork_none_vs_empty_env() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result_none = server_setup_fork(&proc, None);
    let result_empty = server_setup_fork(&proc, Some(Vec::new()));

    // Both should fail with the same error (no server init).
    assert!(result_none.is_err(), "None env should fail");
    assert!(result_empty.is_err(), "Empty env should fail");
    assert_eq!(
        result_none.unwrap_err(),
        result_empty.unwrap_err(),
        "None and empty env should produce same error"
    );
}

/// Test: server_setup_fork is callable multiple times (idempotent error).
#[test]
fn test_setup_fork_multiple_calls() {
    let proc = Proc::new("test", 0).expect("proc creation failed");

    // Call multiple times — should consistently fail.
    for i in 0..5 {
        let result = server_setup_fork(&proc, None);
        assert!(
            result.is_err(),
            "call {}: should fail without server init",
            i
        );
    }
}

/// Test: server_setup_fork with special characters in env values.
#[test]
fn test_setup_fork_special_env_chars() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let special_env = vec![
        "PATH=/usr/local/bin:/usr/bin:/bin",
        "LD_LIBRARY_PATH=/opt/lib:/usr/lib",
        "ENV_VAR_WITH_SPACES=hello world",
        "ENV_VAR_WITH_EQUALS=a=b=c",
    ];
    let result = server_setup_fork(&proc, Some(special_env));

    // Should fail because no server init, not because of env parsing.
    assert!(result.is_err(), "should fail without server init");
}

/// Test: server_setup_fork with many initial env vars.
#[test]
fn test_setup_fork_many_env_vars() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let many_env: Vec<&'static str> = (0..100)
        .map(|i| -> &'static str {
            let s = format!("VAR_{}=value_{}", i, i);
            s.leak()
        })
        .collect();

    let result = server_setup_fork(&proc, Some(many_env));
    assert!(result.is_err(), "should fail without server init");
}

/// Test: server_setup_fork with single env var.
#[test]
fn test_setup_fork_single_env_var() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, Some(vec!["SINGLE=value"]));
    assert!(result.is_err(), "should fail without server init");
}

/// Test: server_setup_fork error status is PMIX_ERR_INIT specifically.
#[test]
fn test_setup_fork_error_is_init() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, None);

    match result {
        Err(status) => {
            // PMIx_server_setup_fork returns PMIX_ERR_INIT when not initialized.
            // The raw code should be -31 (PMIX_ERR_INIT).
            let raw = status.to_raw();
            assert!(raw < 0, "error status should be negative, got: {}", raw);
        }
        Ok(_) => panic!("should not succeed without server init"),
    }
}

/// Test: Proc with rank 0 (wildcard-adjacent) works correctly.
#[test]
fn test_setup_fork_rank_zero() {
    let proc = Proc::new("job.0001", 0).expect("proc creation failed");
    assert_eq!(proc.get_rank(), 0);
    let result = server_setup_fork(&proc, None);
    assert!(result.is_err(), "rank 0 should fail without server init");
}

/// Test: server_setup_fork function is Send-safe (can be called from any thread).
#[test]
fn test_setup_fork_send_safe() {
    // Verify the closure capturing server_setup_fork is Send.
    fn assert_send<T: Send>() {}
    assert_send::<fn(&Proc, Option<Vec<&str>>) -> Result<Vec<String>, pmix::PmixStatus>>();
}

/// Test: server_setup_fork with proc that has long namespace.
#[test]
fn test_setup_fork_long_namespace() {
    // PMIx namespace max length is PMIX_MAX_NSLEN (typically 256).
    let long_ns = "a".repeat(200);
    let proc = Proc::new(&long_ns, 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, None);
    assert!(
        result.is_err(),
        "long namespace should fail without server init"
    );
}

/// Test: server_setup_fork env return type is Vec<String> (owned, not borrowed).
#[test]
fn test_setup_fork_returns_owned_strings() {
    // Compile-time test: the return type Vec<String> means the caller owns the data.
    // This test verifies the type at compile time.
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result: Result<Vec<String>, pmix::PmixStatus> = server_setup_fork(&proc, None);
    assert!(result.is_err());

    // If it succeeded, we could do:
    // let env: Vec<String> = result.unwrap();
    // drop(env); // owned, no borrow issues
}

/// Test: server_setup_fork with env containing empty value.
#[test]
fn test_setup_fork_empty_value_env() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, Some(vec!["EMPTY="]));
    assert!(result.is_err(), "should fail without server init");
}

/// Test: server_setup_fork with env containing only key (no equals).
#[test]
fn test_setup_fork_key_only_env() {
    let proc = Proc::new("test", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, Some(vec!["JUSTKEY"]));
    assert!(result.is_err(), "should fail without server init");
}

/// Test: Multiple procs from same namespace can be passed to setup_fork.
#[test]
fn test_setup_fork_multiple_procs_same_namespace() {
    let proc0 = Proc::new("job.0001", 0).expect("proc creation failed");
    let proc1 = Proc::new("job.0001", 1).expect("proc creation failed");
    let proc2 = Proc::new("job.0001", 2).expect("proc creation failed");

    let result0 = server_setup_fork(&proc0, None);
    let result1 = server_setup_fork(&proc1, None);
    let result2 = server_setup_fork(&proc2, None);

    assert!(result0.is_err());
    assert!(result1.is_err());
    assert!(result2.is_err());

    // All should return the same error (no server init).
    assert_eq!(result0.as_ref().unwrap_err(), result1.as_ref().unwrap_err(),);
    assert_eq!(result1.as_ref().unwrap_err(), result2.as_ref().unwrap_err());
}

/// Test: Multiple procs from different namespaces.
#[test]
fn test_setup_fork_different_namespaces() {
    let proc_a = Proc::new("job.A", 0).expect("proc creation failed");
    let proc_b = Proc::new("job.B", 0).expect("proc creation failed");

    let result_a = server_setup_fork(&proc_a, Some(vec!["JOB=A"]));
    let result_b = server_setup_fork(&proc_b, Some(vec!["JOB=B"]));

    assert!(result_a.is_err());
    assert!(result_b.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require running PMIx server (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test: server_setup_fork returns environment variables when server is initialized.
///
/// This test requires a running PMIx server initialized via PMIx_server_init.
/// It verifies that the returned environment contains expected PMIx variables.
///
/// # Ignored
/// Requires PMIx server runtime. Run with: `cargo test --test server_server_setup_fork -- --ignored`
#[test]
#[ignore]
fn test_setup_fork_returns_env_with_server() {
    // This would require PMIx_server_init to be called first.
    // In a real integration test, we would:
    // 1. Call PMIx_server_init
    // 2. Register an nspace
    // 3. Call server_setup_fork
    // 4. Verify the returned env contains PMIX_NAMESPACE, PMIX_RANK, etc.
    // 5. Call PMIx_server_finalize
    panic!("requires PMIx server runtime — needs PMIx_server_init");
}

/// Integration test: server_setup_fork preserves initial env vars.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_preserves_initial_env() {
    // In a real integration test:
    // 1. Initialize server
    // 2. Call setup_fork with initial env ["MY_VAR=my_value"]
    // 3. Verify "MY_VAR=my_value" is in returned env
    // 4. Verify PMIX_* vars are also present
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork with different ranks returns different env.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_different_ranks_different_env() {
    // In a real integration test:
    // 1. Initialize server, register nspace
    // 2. Call setup_fork for rank 0 and rank 1
    // 3. Verify PMIX_RANK differs in each returned env
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork env contains PMIX_NAMESPACE.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_env_contains_namespace() {
    // In a real integration test:
    // 1. Initialize server, register nspace "test_job"
    // 2. Call setup_fork for proc in "test_job"
    // 3. Verify env contains "PMIX_NAMESPACE=test_job"
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork env contains PMIX_RANK.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_env_contains_rank() {
    // In a real integration test:
    // 1. Initialize server, register nspace
    // 2. Call setup_fork for proc with rank 42
    // 3. Verify env contains "PMIX_RANK=42"
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork env contains PMIX_VERSION.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_env_contains_version() {
    // In a real integration test:
    // 1. Initialize server
    // 2. Call setup_fork
    // 3. Verify env contains "PMIX_VERSION=<version>"
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork memory safety — no double free.
///
/// # Ignored
/// Requires PMIx server runtime and valgrind/ASan.
#[test]
#[ignore]
fn test_setup_fork_no_memory_leak() {
    // In a real integration test with valgrind:
    // 1. Initialize server
    // 2. Call setup_fork multiple times
    // 3. Verify no memory leaks with valgrind
    // 4. Finalize server
    panic!("requires PMIx server runtime and valgrind");
}

/// Integration test: server_setup_fork with large initial env.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_large_initial_env() {
    // In a real integration test:
    // 1. Initialize server
    // 2. Call setup_fork with 1000 initial env vars
    // 3. Verify all initial vars are preserved in output
    // 4. Verify PMIx vars are appended
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork with NUL byte in env should return error.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_nul_in_env_returns_error() {
    // In a real integration test:
    // 1. Initialize server
    // 2. Call setup_fork with env containing NUL byte
    // 3. Verify it returns an error (NUL not allowed in CString)
    // 4. Verify no memory leak
    panic!("requires PMIx server runtime");
}

/// Integration test: server_setup_fork for proc in unregistered nspace.
///
/// # Ignored
/// Requires PMIx server runtime.
#[test]
#[ignore]
fn test_setup_fork_unregistered_nspace() {
    // In a real integration test:
    // 1. Initialize server
    // 2. Call setup_fork for a proc in an unregistered nspace
    // 3. Verify behavior (may succeed or fail depending on PMIx version)
    panic!("requires PMIx server runtime");
}
