//! Comprehensive tests for PMIx tool server interaction APIs.
//!
//! Tests `tool_attach_to_server`, `tool_disconnect`, `tool_get_servers`,
//! and `tool_set_server` — covering error paths (no init), success paths
//! (with daemon), and full state-machine lifecycles.
//!
//! # State machine
//!
//! init → attach → disconnect → finalize
//! init → get_servers → finalize
//! init → set_server → finalize

mod daemon_helper;

use pmix::tool::{
    PmixServerHandle, PmixToolHandle, is_tool_initialized, tool_attach_to_server, tool_disconnect,
    tool_finalize, tool_get_servers, tool_init, tool_set_server,
};
use pmix::{Info, InfoBuilder, PmixStatus, Proc};

// ═══════════════════════════════════════════════════════════════════════════
// Section 1: tool_attach_to_server — error paths without init
// ═══════════════════════════════════════════════════════════════════════════

/// tool_attach_to_server returns Err when called without tool_init.
#[test]
fn test_attach_to_server_without_init_returns_err() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    assert!(
        result.is_err(),
        "tool_attach_to_server without tool_init should return Err"
    );
}

/// tool_attach_to_server error is not PMIX_SUCCESS.
#[test]
fn test_attach_to_server_without_init_error_not_success() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Err(status) => {
            assert!(
                status.is_error(),
                "Error status should be an error, not success: {:?}",
                status
            );
        }
        Ok(_) => panic!("Expected Err, got Ok"),
    }
}

/// tool_attach_to_server without init does not panic.
#[test]
fn test_attach_to_server_without_init_no_panic() {
    let info = InfoBuilder::new().build();
    // If we reach here, no panic occurred.
    let _result = tool_attach_to_server(None, true, &info);
}

/// tool_attach_to_server with want_server=false also errors without init.
#[test]
fn test_attach_to_server_without_init_no_server_flag() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, false, &info);
    assert!(
        result.is_err(),
        "tool_attach_to_server without init should fail regardless of want_server"
    );
}

/// tool_attach_to_server with myproc=Some also errors without init.
#[test]
fn test_attach_to_server_without_init_with_myproc() {
    let info = InfoBuilder::new().build();
    let proc = Proc::new("test-nspace", 0).unwrap();
    let result = tool_attach_to_server(Some(&proc), true, &info);
    assert!(
        result.is_err(),
        "tool_attach_to_server without init should fail even with myproc"
    );
}

/// tool_attach_to_server with empty info errors without init.
#[test]
fn test_attach_to_server_without_init_empty_info() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    assert!(
        result.is_err(),
        "tool_attach_to_server with empty info should fail without init"
    );
}
// ═══════════════════════════════════════════════════════════════════════════

/// tool_attach_to_server has the correct function signature.
#[test]
fn test_attach_to_server_signature() {
    type AttachFn = fn(
        Option<&Proc>,
        bool,
        &Info,
    )
        -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>;
    let _: AttachFn = tool_attach_to_server;
}

/// tool_attach_to_server return type is Send.
#[test]
fn test_attach_to_server_result_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>>();
}

/// PmixToolHandle is Clone + Debug (needed for attach return values).
#[test]
fn test_tool_handle_traits_for_attach() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixToolHandle>();
}

/// PmixServerHandle is Clone + Debug (needed for attach return values).
#[test]
fn test_server_handle_traits_for_attach() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixServerHandle>();
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 3: tool_attach_to_server — with init (daemon running)
// ═══════════════════════════════════════════════════════════════════════════

/// tool_attach_to_server after tool_init — may succeed or fail, but doesn't panic.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_attach_to_server_with_init_no_panic() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let attach_info = InfoBuilder::new().build();
    // This may succeed or fail depending on server config, but must not panic.
    let _result = tool_attach_to_server(Some(handle.proc()), true, &attach_info);
}

/// tool_attach_to_server with want_server=true returns Option<PmixServerHandle>.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_attach_to_server_want_server_returns_option() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let attach_info = InfoBuilder::new().build();
    let result = tool_attach_to_server(Some(handle.proc()), true, &attach_info);
    match result {
        Ok((tool_handle, server_handle)) => {
            // Both are Options — may be Some or None depending on server
            if let Some(sh) = &server_handle {
                // If we got a server handle, it should be debuggable
                let _debug = format!("{:?}", sh);
            }
            if let Some(th) = &tool_handle {
                // If we got a tool handle, it should be debuggable
                let _debug = format!("{:?}", th);
            }
        }
        Err(_) => {
            // Expected if no additional server is discoverable
        }
    }
}

/// tool_attach_to_server with want_server=false returns None for server handle.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_attach_to_server_no_server_flag_returns_none() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let attach_info = InfoBuilder::new().build();
    let result = tool_attach_to_server(Some(handle.proc()), false, &attach_info);
    match result {
        Ok((_, server_handle)) => {
            assert!(
                server_handle.is_none(),
                "Server handle should be None when want_server is false"
            );
        }
        Err(_) => {
            // Expected if no additional server is discoverable
        }
    }
}

/// tool_attach_to_server with myproc=None returns None for tool handle.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_attach_to_server_no_myproc_returns_none_tool() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let attach_info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &attach_info);
    match result {
        Ok((tool_handle, _)) => {
            assert!(
                tool_handle.is_none(),
                "Tool handle should be None when myproc is None"
            );
        }
        Err(_) => {
            // Expected if no additional server is discoverable
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 4: tool_disconnect — error paths without init
// ═══════════════════════════════════════════════════════════════════════════

/// tool_disconnect returns Err when called without tool_init.
#[test]
fn test_disconnect_without_init_returns_err() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let result = tool_disconnect(&proc);
    assert!(
        result.is_err(),
        "tool_disconnect without tool_init should return Err"
    );
}

/// tool_disconnect error without init is not PMIX_SUCCESS.
#[test]
fn test_disconnect_without_init_error_not_success() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let result = tool_disconnect(&proc);
    match result {
        Err(status) => {
            assert!(
                status.is_error(),
                "Error status should be an error, not success: {:?}",
                status
            );
        }
        Ok(_) => panic!("Expected Err, got Ok"),
    }
}

/// tool_disconnect without init does not panic.
#[test]
fn test_disconnect_without_init_no_panic() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    // If we reach here, no panic occurred.
    let _result = tool_disconnect(&proc);
}

/// tool_disconnect with different Proc ranks also errors without init.
#[test]
fn test_disconnect_without_init_various_ranks() {
    for rank in [0u32, 1, 999, u32::MAX] {
        let proc = Proc::new("test-nspace", rank).unwrap();
        let result = tool_disconnect(&proc);
        assert!(
            result.is_err(),
            "tool_disconnect without init should fail for rank {}",
            rank
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 5: tool_disconnect — signature and type checks
// ═══════════════════════════════════════════════════════════════════════════

/// tool_disconnect has the correct function signature.
#[test]
fn test_disconnect_signature() {
    let _: fn(&Proc) -> Result<(), PmixStatus> = tool_disconnect;
}

/// tool_disconnect Result is Send + Sync.
#[test]
fn test_disconnect_result_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Result<(), PmixStatus>>();
}

/// PmixServerHandle.proc() returns &Proc compatible with tool_disconnect.
#[test]
fn test_server_handle_proc_compatible_with_disconnect() {
    fn disconnect_server(s: &PmixServerHandle) -> Result<(), PmixStatus> {
        tool_disconnect(s.proc())
    }
    let _ = disconnect_server;
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 6: tool_get_servers — error paths without init
// ═══════════════════════════════════════════════════════════════════════════

/// tool_get_servers returns Err when called without tool_init.
///
/// Ignored: conflicts with the shared tool handle singleton which
/// initializes PMIx globally, making "without init" calls succeed.
#[test]
#[ignore]
fn test_get_servers_without_init_returns_err() {
    let result = tool_get_servers();
    assert!(
        result.is_err(),
        "tool_get_servers without tool_init should return Err"
    );
}

/// tool_get_servers error without init is not PMIX_SUCCESS.
///
/// Ignored: conflicts with the shared tool handle singleton which
/// initializes PMIx globally, making "without init" calls succeed.
#[test]
#[ignore]
fn test_get_servers_without_init_error_not_success() {
    let result = tool_get_servers();
    match result {
        Err(status) => {
            assert!(
                status.is_error(),
                "Error status should be an error, not success: {:?}",
                status
            );
        }
        Ok(_) => panic!("Expected Err, got Ok"),
    }
}

/// tool_get_servers without init does not panic.
#[test]
fn test_get_servers_without_init_no_panic() {
    // If we reach here, no panic occurred.
    let _result = tool_get_servers();
}

/// Multiple tool_get_servers calls without init all return Err.
///
/// Ignored: conflicts with the shared tool handle singleton which
/// initializes PMIx globally, making "without init" calls succeed.
#[test]
#[ignore]
fn test_get_servers_without_init_multiple_calls() {
    for _ in 0..5 {
        let result = tool_get_servers();
        assert!(result.is_err(), "Each call should return Err without init");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 7: tool_get_servers — signature and type checks
// ═══════════════════════════════════════════════════════════════════════════

/// tool_get_servers has the correct function signature.
#[test]
fn test_get_servers_signature() {
    let _: fn() -> Result<Vec<Proc>, PmixStatus> = tool_get_servers;
}

/// tool_get_servers Result is Send.
#[test]
fn test_get_servers_result_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<Vec<Proc>, PmixStatus>>();
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 8: tool_set_server — error paths without init
// ═══════════════════════════════════════════════════════════════════════════

/// tool_set_server returns Err when called without tool_init.
#[test]
fn test_set_server_without_init_returns_err() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&proc, &info);
    assert!(
        result.is_err(),
        "tool_set_server without tool_init should return Err"
    );
}

/// tool_set_server error without init is not PMIX_SUCCESS.
#[test]
fn test_set_server_without_init_error_not_success() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&proc, &info);
    match result {
        Err(status) => {
            assert!(
                status.is_error(),
                "Error status should be an error, not success: {:?}",
                status
            );
        }
        Ok(_) => panic!("Expected Err, got Ok"),
    }
}

/// tool_set_server without init does not panic.
#[test]
fn test_set_server_without_init_no_panic() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    // If we reach here, no panic occurred.
    let _result = tool_set_server(&proc, &info);
}

/// tool_set_server with empty info also errors without init.
#[test]
fn test_set_server_without_init_empty_info() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&proc, &info);
    assert!(
        result.is_err(),
        "tool_set_server with empty info should fail without init"
    );
}
/// tool_set_server with different Proc values all error without init.
#[test]
fn test_set_server_without_init_various_procs() {
    let procs = vec![
        Proc::new("test-nspace", 0).unwrap(),
        Proc::new("other-nspace", 1).unwrap(),
        Proc::new("prte-beast-1519", 0).unwrap(),
    ];
    let info = InfoBuilder::new().build();
    for proc in procs {
        let result = tool_set_server(&proc, &info);
        assert!(
            result.is_err(),
            "tool_set_server without init should fail for any proc"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 9: tool_set_server — signature and type checks
// ═══════════════════════════════════════════════════════════════════════════

/// tool_set_server has the correct function signature.
#[test]
fn test_set_server_signature() {
    let _: fn(&Proc, &Info) -> Result<(), PmixStatus> = tool_set_server;
}

/// tool_set_server Result is Send + Sync.
#[test]
fn test_set_server_result_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Result<(), PmixStatus>>();
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 10: tool_get_servers — with init (daemon running)
// ═══════════════════════════════════════════════════════════════════════════

/// tool_get_servers after tool_init returns Ok (may be empty or populated).
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_get_servers_with_init_succeeds() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let result = tool_get_servers();
    assert!(
        result.is_ok(),
        "tool_get_servers after tool_init should succeed"
    );
    let servers = result.unwrap();
    // May be empty or have servers — both are valid
    let _ = servers.len();
}

/// tool_get_servers returns Vec<Proc> where each Proc has a namespace.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_get_servers_with_init_returns_valid_procs() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");
    let servers = tool_get_servers().expect("tool_get_servers failed");
    for server in &servers {
        // Each server proc should have a namespace
        let nspace = server.nspace();
        assert!(nspace.is_some(), "Server proc should have a namespace");
    }
}

/// Multiple tool_get_servers calls after init all succeed.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_get_servers_with_init_multiple_calls() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");
    for _ in 0..3 {
        let result = tool_get_servers();
        assert!(
            result.is_ok(),
            "tool_get_servers should succeed multiple times"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 11: tool_set_server — with init (daemon running)
// ═══════════════════════════════════════════════════════════════════════════

/// tool_set_server after tool_init — may succeed or fail, but doesn't panic.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_set_server_with_init_no_panic() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Use the tool's own proc as the server proc — may or may not work
    let info = InfoBuilder::new().build();
    let result = tool_set_server(handle.proc(), &info);
    // We accept either success or error — the key is no panic
    let _ = result;
}

/// tool_set_server with server from get_servers after init.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_set_server_with_server_from_get_servers() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");

    let servers = tool_get_servers().expect("tool_get_servers failed");
    if let Some(server) = servers.first() {
        // Try to set the first server as primary
        let info = InfoBuilder::new().build();
        let result = tool_set_server(server, &info);
        // May succeed or fail depending on server config
        let _ = result;
    }
    // If no servers, we can't test set_server — that's fine
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 12: State machine — init → attach → disconnect → finalize
// ═══════════════════════════════════════════════════════════════════════════

/// Full lifecycle: init → attach → disconnect → finalize (if attach succeeds).
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_lifecycle_init_attach_disconnect_finalize() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Step 1: attach (may succeed or fail)
    let attach_info = InfoBuilder::new().build();
    match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
        Ok((_, Some(server))) => {
            // Step 2: disconnect from the server we attached to
            let disconnect_result = tool_disconnect(server.proc());
            match disconnect_result {
                Ok(()) => {
                    // Disconnection succeeded
                }
                Err(_) => {
                    // Disconnect may fail if the server was already auto-disconnected
                }
            }
        }
        Ok((_, None)) => {
            // Attach succeeded but no server handle returned — skip disconnect
        }
        Err(_) => {
            // Attach failed — skip disconnect
        }
    }

    // Step 3: finalize — singleton handle, Drop handles it at process exit
}

/// State machine: init → get_servers → finalize.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_lifecycle_init_get_servers_finalize() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Get servers
    let servers = tool_get_servers().expect("tool_get_servers failed");
    let _ = servers.len();

    // Finalize — singleton handle, Drop handles it at process exit
}

/// State machine: init → set_server → finalize (using tool's own proc).
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_lifecycle_init_set_server_finalize() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Step 1: set_server (using tool's own proc as target)
    let info = InfoBuilder::new().build();
    let _set_result = tool_set_server(handle.proc(), &info);

    // Step 2: finalize — singleton handle, Drop handles it at process exit
}

/// Full combined lifecycle: init → get_servers → set_server → attach → disconnect → finalize.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_lifecycle_full_combined() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Step 1: get_servers
    let servers = tool_get_servers().expect("tool_get_servers failed");

    // Step 2: set_server (if we have servers)
    if let Some(server) = servers.first() {
        let info = InfoBuilder::new().build();
        let _ = tool_set_server(server, &info);
    }

    // Step 3: attach (may succeed or fail)
    let attach_info = InfoBuilder::new().build();
    match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
        Ok((_, Some(server))) => {
            // Step 4: disconnect
            let _ = tool_disconnect(server.proc());
        }
        Ok((_, None)) | Err(_) => {
            // Attach didn't give us a server — skip disconnect
        }
    }

    // Step 5: finalize — singleton handle, Drop handles it at process exit
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 13: State machine — tool remains initialized after disconnect
// ═══════════════════════════════════════════════════════════════════════════

/// After disconnect, tool is still initialized until finalize.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_tool_initialized_after_disconnect() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");
    assert!(is_tool_initialized(), "Should be initialized after init");

    // Try disconnect (even from a non-connected server)
    let proc = Proc::new("nonexistent", 0).unwrap();
    let _ = tool_disconnect(&proc);

    // Tool should still be initialized
    assert!(
        is_tool_initialized(),
        "Should still be initialized after disconnect"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 14: State machine — finalize works after failed operations
// ═══════════════════════════════════════════════════════════════════════════

/// finalize succeeds after failed attach.
/// NOTE: This test calls tool_finalize on the shared handle, so it must be #[ignore]
/// to avoid breaking other tests that depend on the singleton.
#[test]
#[ignore]
fn test_finalize_after_failed_attach() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Try attach with empty info (likely to fail)
    let empty_info = InfoBuilder::new().build();
    let _ = tool_attach_to_server(Some(handle.proc()), true, &empty_info);

    // Finalize should still work — this test specifically tests finalize behavior
    let finalize_result = tool_finalize(handle.clone());
    assert!(
        finalize_result.is_ok(),
        "tool_finalize should succeed after failed attach: {:?}",
        finalize_result
    );
}

/// finalize succeeds after failed disconnect.
/// NOTE: This test calls tool_finalize on the shared handle, so it must be #[ignore].
#[test]
#[ignore]
fn test_finalize_after_failed_disconnect() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Try disconnect from a non-connected server
    let fake_server = Proc::new("nonexistent-server", 0).unwrap();
    let _ = tool_disconnect(&fake_server);

    // Finalize should still work
    let finalize_result = tool_finalize(handle.clone());
    assert!(
        finalize_result.is_ok(),
        "tool_finalize should succeed after failed disconnect: {:?}",
        finalize_result
    );
}

/// finalize succeeds after failed set_server.
/// NOTE: This test calls tool_finalize on the shared handle, so it must be #[ignore].
#[test]
#[ignore]
fn test_finalize_after_failed_set_server() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("daemon not available");

    // Try set_server with a fake server proc
    let fake_server = Proc::new("nonexistent-server", 0).unwrap();
    let info = InfoBuilder::new().build();
    let _ = tool_set_server(&fake_server, &info);

    // Finalize should still work
    let finalize_result = tool_finalize(handle.clone());
    assert!(
        finalize_result.is_ok(),
        "tool_finalize should succeed after failed set_server: {:?}",
        finalize_result
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 15: Thread safety — concurrent get_servers after init
// ═══════════════════════════════════════════════════════════════════════════

/// Concurrent tool_get_servers calls are safe after tool_init.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_concurrent_get_servers_safe() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");

    const NUM_THREADS: usize = 4;
    let mut threads = Vec::new();

    for _ in 0..NUM_THREADS {
        threads.push(std::thread::spawn(|| {
            // tool_get_servers is safe to call concurrently after init
            let result = tool_get_servers();
            assert!(result.is_ok(), "Concurrent tool_get_servers should succeed");
            result
        }));
    }

    // All threads should complete successfully
    for t in threads {
        let result = t.join().expect("thread panicked");
        assert!(result.is_ok(), "thread returned error");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 16: PmixStatus error code verification
// ═══════════════════════════════════════════════════════════════════════════

/// Error from tool_attach_to_server without init is a known PMIx error.
#[test]
fn test_attach_without_init_error_code() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Err(status) => {
            let raw = status.to_raw();
            assert!(raw < 0, "Error should be negative (raw={})", raw);
        }
        Ok(_) => panic!("Expected Err"),
    }
}

/// Error from tool_get_servers without init is a known PMIx error.
///
/// Ignored: conflicts with the shared tool handle singleton which
/// initializes PMIx globally, making "without init" calls succeed.
#[test]
#[ignore]
fn test_get_servers_without_init_error_code() {
    let result = tool_get_servers();
    match result {
        Err(status) => {
            let raw = status.to_raw();
            assert!(raw < 0, "Error should be negative (raw={})", raw);
        }
        Ok(_) => panic!("Expected Err"),
    }
}

/// Error from tool_set_server without init is a known PMIx error.
#[test]
fn test_set_server_without_init_error_code() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&proc, &info);
    match result {
        Err(status) => {
            let raw = status.to_raw();
            assert!(raw < 0, "Error should be negative (raw={})", raw);
        }
        Ok(_) => panic!("Expected Err"),
    }
}

/// Error from tool_disconnect without init is a known PMIx error.
#[test]
fn test_disconnect_without_init_error_code() {
    let proc = Proc::new("test-nspace", 0).unwrap();
    let result = tool_disconnect(&proc);
    match result {
        Err(status) => {
            let raw = status.to_raw();
            assert!(raw < 0, "Error should be negative (raw={})", raw);
        }
        Ok(_) => panic!("Expected Err"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 17: PmixStatus trait checks
// ═══════════════════════════════════════════════════════════════════════════

/// PmixStatus is Clone + Copy + Debug + PartialEq + Eq.
#[test]
fn test_pmix_status_full_traits() {
    fn assert_traits<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    assert_traits::<PmixStatus>();
}

/// PmixStatus implements std::error::Error.
#[test]
fn test_pmix_status_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<PmixStatus>();
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 18: Proc type checks
// ═══════════════════════════════════════════════════════════════════════════

/// Proc is Clone (needed for server interaction).
#[test]
fn test_proc_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<Proc>();
}

/// Proc::new creates valid procs for various nspace/rank combos.
#[test]
fn test_proc_new_various() {
    let p1 = Proc::new("test", 0).unwrap();
    assert_eq!(p1.rank(), 0);
    assert_eq!(p1.nspace(), Some("test".to_string()));

    let p2 = Proc::new("my-nspace", 42).unwrap();
    assert_eq!(p2.rank(), 42);
    assert_eq!(p2.nspace(), Some("my-nspace".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 19: Info type checks
// ═══════════════════════════════════════════════════════════════════════════

/// InfoBuilder produces Info compatible with all server interaction functions.
#[test]
fn test_info_compatible_with_all_server_funcs() {
    let info = InfoBuilder::new().build();
    let proc = Proc::new("test", 0).unwrap();

    // All these compile — proving Info is compatible
    let _: Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> =
        tool_attach_to_server(Some(&proc), true, &info);
    let _: Result<(), PmixStatus> = tool_set_server(&proc, &info);
    // tool_disconnect doesn't take Info
    let _: Result<(), PmixStatus> = tool_disconnect(&proc);
    // tool_get_servers doesn't take Info
    let _: Result<Vec<Proc>, PmixStatus> = tool_get_servers();
}

// ═══════════════════════════════════════════════════════════════════════════
// Section 20: Integration — multi-server interaction
// ═══════════════════════════════════════════════════════════════════════════

/// After init, enumerate servers and try to set each as primary.
/// Ignored: requires PRTE daemon accepting tool connections (PMIx_tool_init blocks indefinitely).
#[test]
#[ignore]
fn test_get_servers_then_set_each_as_primary() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("daemon not available");

    let servers = tool_get_servers().expect("tool_get_servers failed");
    for server in &servers {
        // Try to set each server as primary
        let info = InfoBuilder::new().build();
        let result = tool_set_server(server, &info);
        // May succeed or fail — the key is no panic
        let _ = result;
    }
}

/// Verify is_tool_initialized state transitions correctly through the lifecycle.
/// NOTE: This test does its own init/finalize cycle to test the counter behavior,
/// so it must be #[ignore] since it conflicts with the singleton.
#[test]
#[ignore]
fn test_is_tool_initialized_transitions() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");

    let info = daemon_helper::get_tool_init_info();
    let handle = tool_init(None, &info).expect("tool_init failed");
    assert!(is_tool_initialized(), "Should be true after init");

    // Do some operations
    let _ = tool_get_servers();
    assert!(
        is_tool_initialized(),
        "Should still be true after get_servers"
    );

    let proc = Proc::new("test", 0).unwrap();
    let _ = tool_disconnect(&proc);
    assert!(
        is_tool_initialized(),
        "Should still be true after disconnect"
    );

    let _ = tool_set_server(handle.proc(), &info);
    assert!(
        is_tool_initialized(),
        "Should still be true after set_server"
    );

    tool_finalize(handle);
}
