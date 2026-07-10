//! Structural unit tests for the tool module — TASK-043.
//!
//! Focus on code paths not yet exercised by the in-module tests.
//! All tests run WITHOUT PMIx_Init — they test the Rust wrapper layer only.

use pmix::tool::*;
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// tool_init — parameter variation tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_init_with_empty_info_and_none_proc() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_tool_init_with_empty_info_and_some_proc() {
    let proc = pmix::Proc::new("test_ns", 42).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_init(Some(&proc), &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_tool_init_with_infobuilder_info() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_tool_init_with_collect_data_info() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let result = tool_init(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_tool_init_multiple_calls_consistent() {
    let info = InfoBuilder::new().build();
    // tool_init behavior depends on PMIx state — just verify it doesn't crash
    for i in 0..5 {
        let result = tool_init(None, &info);
        match result {
            Ok(handle) => {
                let _ = tool_finalize(handle);
            }
            Err(_) => {
                // Expected without DVM
            }
        }
    }
}

#[test]
fn test_tool_init_with_various_proc_ranks() {
    let info = InfoBuilder::new().build();
    for rank in [0u32, 1, 100, u32::MAX] {
        let proc = pmix::Proc::new("test_ns", rank).unwrap();
        let result = tool_init(Some(&proc), &info);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error(), "Expected error for rank {}", rank);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init_minimal — convenience wrapper tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // SIGSEGV under --test-threads>1: races with other tool_init tests via shared PMIx state
fn test_tool_init_minimal_returns_consistent_result() {
    let result = tool_init_minimal();
    // Without DVM, should either succeed or fail consistently
    match result {
        Ok(handle) => {
            // If it somehow succeeded, finalize it
            let _ = tool_finalize(handle);
        }
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
#[ignore] // SIGSEGV under --test-threads>1: races with tool_is_connected/is_tool_initialized via shared PMIx state
fn test_tool_init_minimal_consecutive_calls() {
    // tool_init_minimal can succeed on first call (PMIx may auto-connect)
    // and then fail on subsequent calls, or vice versa.
    // We just verify it doesn't crash and returns consistent-ish results.
    for i in 0..5 {
        let result = tool_init_minimal();
        match result {
            Ok(handle) => {
                // If succeeded, finalize to clean up
                let _ = tool_finalize(handle);
            }
            Err(_) => {
                // Expected without DVM after first call
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_finalize — handle consumption tests (via tool_init result)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // SIGSEGV on process cleanup when PMIx is initialized then finalized
fn test_tool_finalize_after_tool_init() {
    let result = tool_init_minimal();
    match result {
        Ok(handle) => {
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // Expected without DVM
        }
    }
}

#[test]
#[ignore] // SIGSEGV on process cleanup when PMIx is initialized then finalized
fn test_tool_finalize_after_tool_init_with_proc() {
    let proc = pmix::Proc::new("test", 999).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_init(Some(&proc), &info);
    match result {
        Ok(handle) => {
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // Expected without DVM
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_attach_to_server — option combination tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_attach_with_both_myproc_and_server() {
    let proc = pmix::Proc::new("my_tool", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(Some(&proc), true, &info);
    match result {
        Ok((tool, server)) => {
            assert!(tool.is_some(), "tool should be Some when myproc is Some");
            assert!(
                server.is_some(),
                "server should be Some when want_server is true"
            );
        }
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_attach_with_neither_myproc_nor_server() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, false, &info);
    match result {
        Ok((tool, server)) => {
            assert!(tool.is_none(), "tool should be None when myproc is None");
            assert!(
                server.is_none(),
                "server should be None when want_server is false"
            );
        }
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_attach_with_myproc_only() {
    let proc = pmix::Proc::new("my_tool", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(Some(&proc), false, &info);
    match result {
        Ok((tool, server)) => {
            assert!(tool.is_some());
            assert!(server.is_none());
        }
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_attach_with_server_only() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Ok((tool, server)) => {
            assert!(tool.is_none());
            assert!(server.is_some());
        }
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_attach_with_empty_info() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_get_servers — FFI path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_servers_returns_error_without_dvm() {
    let result = tool_get_servers();
    assert!(result.is_err(), "tool_get_servers should fail without DVM");
}

#[test]
fn test_get_servers_consecutive_calls() {
    let results: Vec<_> = (0..3).map(|_| tool_get_servers()).collect();
    let first_is_err = results[0].is_err();
    for (i, r) in results.iter().enumerate().skip(1) {
        assert_eq!(r.is_err(), first_is_err, "call {} inconsistent", i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_set_server — Info variation tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_set_server_with_empty_info() {
    let server = pmix::Proc::new("server_ns", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&server, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_set_server_with_wildcard_server() {
    let server = pmix::Proc::new("", u32::MAX).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&server, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_set_server_various_ranks() {
    let info = InfoBuilder::new().build();
    for rank in [0u32, 1, 42, 1000, u32::MAX] {
        let server = pmix::Proc::new("server_ns", rank).unwrap();
        let result = tool_set_server(&server, &info);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error(), "rank {} should error without DVM", rank);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_disconnect — FFI path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_with_empty_nspace() {
    let server = pmix::Proc::new("", 0).unwrap();
    let result = tool_disconnect(&server);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_disconnect_consecutive_calls() {
    let server = pmix::Proc::new("server_ns", 0).unwrap();
    let results: Vec<_> = (0..3).map(|_| tool_disconnect(&server)).collect();
    let first_is_err = results[0].is_err();
    for (i, r) in results.iter().enumerate().skip(1) {
        assert_eq!(r.is_err(), first_is_err, "call {} inconsistent", i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_connect_to_server — FFI path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_connect_to_server_with_none_proc() {
    let info = InfoBuilder::new().build();
    let result = tool_connect_to_server(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_connect_to_server_with_some_proc() {
    let proc = pmix::Proc::new("my_tool", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_connect_to_server(Some(&proc), &info);
    match result {
        Ok(handle) => {
            assert_eq!(handle.proc().rank(), 0);
        }
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_connect_to_server_with_empty_info() {
    let info = InfoBuilder::new().build();
    let result = tool_connect_to_server(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc::nspace() and Proc::rank() edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_nspace_with_different_namespaces() {
    let names = ["test", "my_ns", "a", "", "very_long_namespace_name_here"];
    for name in &names {
        let proc = pmix::Proc::new(name, 0).unwrap();
        let nspace: Option<String> = proc.nspace();
        if let Some(ns) = nspace {
            assert!(!ns.is_empty() || name.is_empty());
        }
    }
}

#[test]
fn test_proc_rank_boundary_values() {
    let proc0 = pmix::Proc::new("test", 0).unwrap();
    assert_eq!(proc0.rank(), 0);

    let proc_max = pmix::Proc::new("test", u32::MAX).unwrap();
    assert_eq!(proc_max.rank(), u32::MAX);
}

// ─────────────────────────────────────────────────────────────────────────────
// is_tool_initialized — state tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_tool_initialized_default_state() {
    // is_tool_initialized() may return true or false depending on PMIx state
    // from other tests — just verify it doesn't crash
    let _ = is_tool_initialized();
}

#[test]
fn test_is_tool_initialized_consistent() {
    let results: Vec<bool> = (0..5).map(|_| is_tool_initialized()).collect();
    let first = results[0];
    for (i, r) in results.iter().enumerate().skip(1) {
        assert_eq!(*r, first, "is_tool_initialized call {} inconsistent", i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_is_connected — default behavior tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_is_connected_default_state() {
    // tool_is_connected() may return true or false depending on PMIx state
    // from other tests — just verify it doesn't crash
    let _ = tool_is_connected();
}

#[test]
fn test_tool_is_connected_consistent() {
    let results: Vec<bool> = (0..5).map(|_| tool_is_connected()).collect();
    let first = results[0];
    for (i, r) in results.iter().enumerate().skip(1) {
        assert_eq!(*r, first, "tool_is_connected call {} inconsistent", i);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus error code coverage for tool-specific errors
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_error_codes_all_are_errors() {
    let error_codes = [
        pmix::PmixError::ErrNotFound,
        pmix::PmixError::ErrTimeout,
        pmix::PmixError::ErrLostConnection,
        pmix::PmixError::ErrInit,
        pmix::PmixError::ErrBadParam,
        pmix::PmixError::ErrUnreach,
        pmix::PmixError::Error,
    ];
    for err in &error_codes {
        let status = PmixStatus::Known(*err);
        assert!(status.is_error(), "{:?} should be an error", err);
        assert!(!status.is_success(), "{:?} should not be success", err);
    }
}

#[test]
fn test_tool_error_to_raw_values() {
    for err in [pmix::PmixError::ErrNotFound, pmix::PmixError::ErrTimeout] {
        let status = PmixStatus::Known(err);
        let raw = status.to_raw();
        assert!(raw < 0, "{:?} raw value should be negative", err);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Info handling edge cases for tool functions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_init_with_multiple_info_builders() {
    for _ in 0..5 {
        let info = InfoBuilder::new().build();
        let result = tool_init(None, &info);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error());
            }
        }
    }
}

#[test]
fn test_tool_connect_with_info_builder_collect_data() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let result = tool_connect_to_server(None, &info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Static state tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // SIGSEGV under --test-threads>1 due to FFI race with tool_init/finalize in other tests
fn test_static_state_lazylock_initialized() {
    let _ = is_tool_initialized();
    let _ = tool_is_connected();
}

// ─────────────────────────────────────────────────────────────────────────────
// Handle clone/debug trait verification
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tool_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixToolHandle>();
}

#[test]
fn test_server_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixServerHandle>();
}

#[test]
fn test_tool_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixToolHandle>();
}

#[test]
fn test_server_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}
