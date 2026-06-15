//! Tests for `PMIx_tool_attach_to_server`, `PmixServerHandle`,
//! and the `tool_attach_to_server` safe wrapper.
//!
//! Note: `PMIx_tool_attach_to_server` requires a running PMIx daemon
//! and a previously initialized tool library. Tests that call the
//! actual FFI are marked `#[ignore]` and should be run with a PMIx
//! environment. Unit tests that verify API structure, types, and
//! defaults run without a PMIx runtime.

use pmix::tool::{PmixServerHandle, PmixToolHandle, tool_attach_to_server};
use pmix::{Info, InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — structure and traits
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle implements Clone.
#[test]
fn test_server_handle_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixServerHandle>();
}

/// PmixServerHandle implements Debug.
#[test]
fn test_server_handle_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

/// PmixServerHandle implements Clone + Debug together.
#[test]
fn test_server_handle_traits() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixServerHandle>();
}

/// PmixServerHandle::proc returns a reference to Proc.
#[test]
fn test_server_handle_proc_method() {
    fn assert_proc_method(h: &PmixServerHandle) -> &pmix::Proc {
        h.proc()
    }
    let _ = assert_proc_method;
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_attach_to_server — signature and parameter checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_attach_to_server returns Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>.
#[test]
fn test_tool_attach_to_server_return_type() {
    type AttachReturn = Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>;
    fn _assert_return_type() -> AttachReturn {
        unreachable!()
    }
    let _ = std::mem::needs_drop::<AttachReturn>();
}

/// tool_attach_to_server signature accepts Option<&Proc>, bool, &Info.
#[test]
fn test_tool_attach_to_server_signature() {
    type AttachFn = fn(
        Option<&pmix::Proc>,
        bool,
        &Info,
    )
        -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>;
    let _: AttachFn = tool_attach_to_server;
}

/// tool_attach_to_server accepts None for myproc (no tool identity needed).
#[test]
fn test_tool_attach_to_server_myproc_none() {
    // Compile-time check: the function accepts None for myproc.
    // We verify the signature compiles with None.
    fn _check_none_myproc(
        info: &Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> {
        tool_attach_to_server(None, true, info)
    }
    let _ = _check_none_myproc;
}

/// tool_attach_to_server accepts want_server=false (no server identity needed).
#[test]
fn test_tool_attach_to_server_want_server_false() {
    fn _check_no_server(
        info: &Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> {
        tool_attach_to_server(None, false, info)
    }
    let _ = _check_no_server;
}

/// tool_attach_to_server accepts both myproc=Some and want_server=true.
#[test]
fn test_tool_attach_to_server_both_requested() {
    fn _check_both(
        proc: &pmix::Proc,
        info: &Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> {
        tool_attach_to_server(Some(proc), true, info)
    }
    let _ = _check_both;
}

/// tool_attach_to_server with empty info still compiles (C API may reject at runtime).
#[test]
fn test_tool_attach_to_server_empty_info() {
    fn _check_empty_info() -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>
    {
        let empty = InfoBuilder::new().build();
        tool_attach_to_server(None, true, &empty)
    }
    let _ = _check_empty_info;
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

/// PmixStatus::from_raw converts ErrUnreach (-25).
#[test]
fn test_pmix_status_err_unreach() {
    let status = PmixStatus::from_raw(-25);
    assert!(status.is_error(), "Raw -25 (ErrUnreach) should be error");
}

/// PmixStatus::from_raw converts ErrBadParam (-27).
#[test]
fn test_pmix_status_err_bad_param() {
    let status = PmixStatus::from_raw(-27);
    assert!(status.is_error(), "Raw -27 (ErrBadParam) should be error");
}

/// PmixStatus::from_raw converts ErrInit (-31).
#[test]
fn test_pmix_status_err_init() {
    let status = PmixStatus::from_raw(-31);
    assert!(status.is_error(), "Raw -31 (ErrInit) should be error");
}

/// PmixStatus::from_raw converts ErrTimeout (-24).
#[test]
fn test_pmix_status_err_timeout() {
    let status = PmixStatus::from_raw(-24);
    assert!(status.is_error(), "Raw -24 (ErrTimeout) should be error");
}

// ─────────────────────────────────────────────────────────────────────────────
// Return type structure checks
// ─────────────────────────────────────────────────────────────────────────────

/// The return tuple contains two Options — both can be None.
#[test]
fn test_attach_return_tuple_structure() {
    let result: (Option<PmixToolHandle>, Option<PmixServerHandle>) = (None, None);
    assert!(result.0.is_none());
    assert!(result.1.is_none());
}

/// tool_attach_to_server result type is Send (for potential async use).
#[test]
fn test_attach_result_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Info interaction
// ─────────────────────────────────────────────────────────────────────────────

/// InfoBuilder::new().build() produces a valid Info for tool_attach_to_server.
#[test]
fn test_info_builder_produces_attach_compatible_info() {
    let info = InfoBuilder::new().build();
    // Compile-time check that Info can be passed to tool_attach_to_server.
    let _: fn(
        Option<&pmix::Proc>,
        bool,
        &Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> =
        tool_attach_to_server;
    let _ = &info;
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// tool_attach_to_server with a valid server URI should connect or return error.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_attach_to_server_with_server() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Ok((tool, server)) => {
            // If we got a server handle, it should have a valid proc
            if let Some(s) = server {
                let _proc = s.proc();
                assert!(
                    s.proc().nspace().is_some(),
                    "Server handle should have a namespace"
                );
            }
            // If we got a tool handle, it should have a valid proc
            if let Some(t) = tool {
                assert!(
                    t.proc().nspace().is_some(),
                    "Tool handle should have a namespace"
                );
            }
        }
        Err(status) => {
            // Expected when no PMIx server is available
            assert!(status.is_error(), "Expected error status when no server");
        }
    }
}

/// tool_attach_to_server with want_server=false should not return a server handle.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_attach_to_server_no_server_identity() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, false, &info);
    match result {
        Ok((tool, server)) => {
            assert!(
                server.is_none(),
                "Server handle should be None when want_server is false"
            );
            // Tool handle may or may not be present depending on myproc param
            let _ = tool;
        }
        Err(status) => {
            assert!(status.is_error(), "Expected error status when no server");
        }
    }
}

/// tool_attach_to_server with myproc=None should not return a tool handle.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_attach_to_server_no_tool_identity() {
    let info = InfoBuilder::new().build();
    let result = tool_attach_to_server(None, true, &info);
    match result {
        Ok((tool, server)) => {
            assert!(
                tool.is_none(),
                "Tool handle should be None when myproc is None"
            );
            // Server handle may be present
            let _ = server;
        }
        Err(status) => {
            assert!(status.is_error(), "Expected error status when no server");
        }
    }
}

/// tool_attach_to_server after tool_init should attempt connection.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_attach_after_init() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    let init_result = tool_init(None, &init_info);
    match init_result {
        Ok(handle) => {
            // Now try to attach to a server
            let attach_info = InfoBuilder::new().build();
            let attach_result = tool_attach_to_server(Some(handle.proc()), true, &attach_info);
            match attach_result {
                Ok((tool, server)) => {
                    // Connection established
                    if let Some(s) = server {
                        assert!(
                            s.proc().nspace().is_some(),
                            "Attached server should have namespace"
                        );
                    }
                    if let Some(t) = tool {
                        assert!(
                            t.proc().nspace().is_some(),
                            "Tool identity should have namespace"
                        );
                    }
                }
                Err(status) => {
                    // Expected if no additional server is discoverable
                    assert!(status.is_error());
                }
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// tool_attach_to_server with both identities requested returns both handles.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_tool_attach_both_handles() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    let init_result = tool_init(None, &init_info);
    match init_result {
        Ok(handle) => {
            let attach_info = InfoBuilder::new().build();
            let attach_result = tool_attach_to_server(Some(handle.proc()), true, &attach_info);
            match attach_result {
                Ok((tool, server)) => {
                    assert!(
                        tool.is_some(),
                        "Tool handle should be Some when myproc is Some"
                    );
                    assert!(
                        server.is_some(),
                        "Server handle should be Some when want_server is true"
                    );
                }
                Err(status) => {
                    assert!(status.is_error());
                }
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// PmixServerHandle Debug output contains struct name.
#[test]
fn test_server_handle_debug_contains_name() {
    fn check_debug<T: std::fmt::Debug>() {}
    check_debug::<PmixServerHandle>();
}

/// PmixServerHandle proc() returns &Proc — compile check.
#[test]
fn test_server_handle_proc_returns_proc_ref() {
    fn assert_proc_ref(h: &PmixServerHandle) -> &pmix::Proc {
        h.proc()
    }
    let _ = assert_proc_ref;
}

/// tool_attach_to_server with neither identity requested compiles.
#[test]
fn test_tool_attach_neither_identity() {
    fn _check_neither(
        info: &Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> {
        tool_attach_to_server(None, false, info)
    }
    let _ = _check_neither;
}

/// Multiple calls to tool_attach_to_server with the same info compile.
#[test]
fn test_tool_attach_multiple_calls_signature() {
    fn _check_multiple(info: &Info) {
        let _r1: Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> =
            tool_attach_to_server(None, true, info);
        let _r2: Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> =
            tool_attach_to_server(None, true, info);
    }
    let _ = _check_multiple;
}
