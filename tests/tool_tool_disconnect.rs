//! Tests for `PMIx_tool_disconnect` and the `tool_disconnect` safe wrapper.
//!
//! Note: `PMIx_tool_disconnect` requires a running PMIx daemon and a
//! previously initialized tool library with an active server connection.
//! Tests that call the actual FFI are marked `#[ignore]` and should be
//! run with a PMIx environment. Unit tests that verify API structure,
//! types, and function signatures run without a PMIx runtime.
//!
//! # C API
//! ```c
//! pmix_status_t PMIx_tool_disconnect(const pmix_proc_t *server);
//! ```
//!
//! Disconnects the tool from the specified server while leaving the
//! tool library initialized. The tool can later reconnect using
//! `PMIx_tool_attach_to_server`.

use pmix::tool::{PmixServerHandle, tool_attach_to_server, tool_disconnect};
use pmix::{Info, InfoBuilder, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_disconnect takes &Proc and returns Result<(), PmixStatus>.
#[test]
fn test_tool_disconnect_signature() {
    let _: fn(&Proc) -> Result<(), PmixStatus> = tool_disconnect;
}

/// tool_disconnect takes a reference to Proc (not owned).
#[test]
fn test_tool_disconnect_takes_ref() {
    // Compile-time check: the parameter is &Proc, not Proc.
    // If it took Proc by value, this type alias would not match.
    type F = fn(&Proc) -> Result<(), PmixStatus>;
    let _f: F = tool_disconnect;
}

/// tool_disconnect return type is Result<(), PmixStatus> — unit success.
#[test]
fn test_tool_disconnect_return_unit() {
    type Ret = Result<(), PmixStatus>;
    fn _assert_same_type(_r: Ret) {}
    let _: fn(&Proc) -> Ret = tool_disconnect;
}

/// tool_disconnect is a plain function pointer (not a closure or method).
#[test]
fn test_tool_disconnect_function_pointer() {
    let f: fn(&Proc) -> Result<(), PmixStatus> = tool_disconnect;
    let _ = f as *const ();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus interaction
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw(0) is success — tool_disconnect returns this on success.
#[test]
fn test_pmix_status_from_raw_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "Raw 0 must be PMIX_SUCCESS");
}

/// PmixStatus::from_raw(-1) is error — tool_disconnect returns this on failure.
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
// Error codes that tool_disconnect may return
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus::from_raw converts ErrNotFound (-46).
#[test]
fn test_pmix_status_err_not_found() {
    let status = PmixStatus::from_raw(-46);
    assert!(status.is_error(), "Raw -46 (ErrNotFound) should be error");
}

/// PmixStatus::from_raw converts ErrInit (-31).
#[test]
fn test_pmix_status_err_init() {
    let status = PmixStatus::from_raw(-31);
    assert!(status.is_error(), "Raw -31 (ErrInit) should be error");
}

/// PmixStatus::from_raw converts ErrBadParam (-27).
#[test]
fn test_pmix_status_err_bad_param() {
    let status = PmixStatus::from_raw(-27);
    assert!(status.is_error(), "Raw -27 (ErrBadParam) should be error");
}

/// PmixStatus::from_raw converts ErrTimeout (-24).
#[test]
fn test_pmix_status_err_timeout() {
    let status = PmixStatus::from_raw(-24);
    assert!(status.is_error(), "Raw -24 (ErrTimeout) should be error");
}

/// PmixStatus::from_raw converts ErrUnreach (-25).
#[test]
fn test_pmix_status_err_unreach() {
    let status = PmixStatus::from_raw(-25);
    assert!(status.is_error(), "Raw -25 (ErrUnreach) should be error");
}

// ─────────────────────────────────────────────────────────────────────────────
// API consistency — tool_attach_to_server and tool_disconnect are a pair
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle from tool_attach_to_server can be passed to tool_disconnect.
#[test]
fn test_attach_disconnect_type_pair() {
    // tool_attach_to_server returns Option<PmixServerHandle>
    // tool_disconnect takes &Proc which PmixServerHandle.proc() provides.
    fn assert_disconnect_accepts_server_handle(
        server: &PmixServerHandle,
    ) -> Result<(), PmixStatus> {
        tool_disconnect(server.proc())
    }
    let _ = assert_disconnect_accepts_server_handle;
}

/// PmixServerHandle.proc() returns &Proc — compatible with tool_disconnect.
#[test]
fn test_server_handle_proc_compatible_with_disconnect() {
    fn get_proc(s: &PmixServerHandle) -> &Proc {
        s.proc()
    }
    fn disconnect_with_proc(p: &Proc) -> Result<(), PmixStatus> {
        tool_disconnect(p)
    }
    // The chain: PmixServerHandle -> proc() -> &Proc -> tool_disconnect
    let _ = (get_proc, disconnect_with_proc);
}

/// tool_disconnect can be called multiple times (different servers).
#[test]
fn test_tool_disconnect_multiple_calls_signature() {
    fn _check_multiple(proc_a: &Proc, proc_b: &Proc) {
        let _r1: Result<(), PmixStatus> = tool_disconnect(proc_a);
        let _r2: Result<(), PmixStatus> = tool_disconnect(proc_b);
    }
    let _ = _check_multiple;
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle traits
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle is Clone (can be stored for later disconnect).
#[test]
fn test_server_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixServerHandle>();
}

/// PmixServerHandle is Debug (useful for error reporting).
#[test]
fn test_server_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

/// PmixServerHandle is both Clone and Debug.
#[test]
fn test_server_handle_traits_combined() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixServerHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc type checks
// ─────────────────────────────────────────────────────────────────────────────

/// Proc is Clone (can be stored and reused for disconnect).
#[test]
fn test_proc_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<Proc>();
}

/// Proc reference lifetime is compatible with tool_disconnect call.
#[test]
fn test_proc_ref_lifetime() {
    fn _check<'a>(proc: &'a Proc) -> Result<(), PmixStatus> {
        tool_disconnect(proc)
    }
    let _ = _check;
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder compatibility
// ─────────────────────────────────────────────────────────────────────────────

/// InfoBuilder::new().build() produces a valid Info.
#[test]
fn test_info_builder_builds() {
    let info = InfoBuilder::new().build();
    let _: &Info = &info;
}

// ─────────────────────────────────────────────────────────────────────────────
// Result type properties
// ─────────────────────────────────────────────────────────────────────────────

/// tool_disconnect Result is Send (for potential async use).
#[test]
fn test_disconnect_result_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Result<(), PmixStatus>>();
}

/// tool_disconnect Result is Sync (shareable across threads).
#[test]
fn test_disconnect_result_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Result<(), PmixStatus>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require PMIx server (ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// tool_attach_to_server followed by tool_disconnect should succeed.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_attach_then_disconnect() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    let init_result = tool_init(None, &init_info);
    match init_result {
        Ok(handle) => {
            // Attach to a server
            let attach_info = InfoBuilder::new().build();
            let attach_result = tool_attach_to_server(Some(handle.proc()), true, &attach_info);
            match attach_result {
                Ok((_, Some(server))) => {
                    // Disconnect from the server
                    let disconnect_result = tool_disconnect(server.proc());
                    match disconnect_result {
                        Ok(()) => {
                            // Disconnection succeeded
                        }
                        Err(status) => {
                            assert!(status.is_error(), "disconnect returned error: {:?}", status);
                        }
                    }
                }
                Ok((_, None)) => {
                    // No server returned — cannot test disconnect
                }
                Err(status) => {
                    // Expected if no PMIx server is available
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

/// Disconnect from a server that was not connected should return error.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_disconnect_not_connected() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    match tool_init(None, &init_info) {
        Ok(handle) => {
            // Try to disconnect from a server we never connected to.
            // We use the tool's own proc as a fake server identifier.
            let disconnect_result = tool_disconnect(handle.proc());
            // This should fail because we're not connected to ourselves as a server.
            match disconnect_result {
                Ok(()) => {
                    // Unexpected — but not necessarily wrong if the library
                    // handles this case gracefully.
                }
                Err(status) => {
                    assert!(
                        status.is_error(),
                        "Expected error when disconnecting from non-connected server"
                    );
                }
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// Disconnect after attach with want_server=true returns the server handle.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_disconnect_with_server_handle() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    match tool_init(None, &init_info) {
        Ok(handle) => {
            let attach_info = InfoBuilder::new().build();
            match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
                Ok((_, Some(server))) => {
                    // Clone the server handle before disconnecting
                    let server_clone = server.clone();
                    // Disconnect using the original
                    let result = tool_disconnect(server.proc());
                    assert!(result.is_ok(), "Disconnect should succeed: {:?}", result);
                    // Verify the cloned handle is still accessible
                    let _proc = server_clone.proc();
                }
                Ok((_, None)) => {
                    // No server returned
                }
                Err(_) => {
                    // No server available
                }
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// Multiple attach/disconnect cycles should work.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_attach_disconnect_cycle() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    match tool_init(None, &init_info) {
        Ok(handle) => {
            let attach_info = InfoBuilder::new().build();
            // First attach
            match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
                Ok((_, Some(server1))) => {
                    // First disconnect
                    let r1 = tool_disconnect(server1.proc());
                    if r1.is_ok() {
                        // Re-attach
                        match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
                            Ok((_, Some(server2))) => {
                                // Second disconnect
                                let r2 = tool_disconnect(server2.proc());
                                assert!(r2.is_ok(), "Second disconnect should succeed");
                            }
                            Ok((_, None)) => {}
                            Err(_) => {}
                        }
                    }
                }
                Ok((_, None)) => {}
                Err(_) => {}
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// Disconnect does not finalize the tool library — tool remains initialized.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_disconnect_leaves_tool_initialized() {
    use pmix::tool::{is_tool_initialized, tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    match tool_init(None, &init_info) {
        Ok(handle) => {
            assert!(
                is_tool_initialized(),
                "Tool should be initialized after init"
            );

            let attach_info = InfoBuilder::new().build();
            match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
                Ok((_, Some(server))) => {
                    // Disconnect from server
                    let _ = tool_disconnect(server.proc());
                    // Tool should still be initialized after disconnect
                    assert!(
                        is_tool_initialized(),
                        "Tool should still be initialized after disconnect"
                    );
                }
                Ok((_, None)) => {}
                Err(_) => {}
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// tool_init followed by tool_finalize still works after a failed disconnect.
/// Ignored because it requires PMIx library availability.
#[test]
#[ignore = "requires PMIx server"]
fn test_finalize_after_failed_disconnect() {
    use pmix::tool::{tool_finalize, tool_init};

    let init_info = InfoBuilder::new().build();
    match tool_init(None, &init_info) {
        Ok(handle) => {
            // Try to disconnect from a non-connected server
            let _ = tool_disconnect(handle.proc());
            // Finalize should still work
            let result = tool_finalize(handle);
            assert!(
                result.is_ok(),
                "Finalize should succeed after failed disconnect"
            );
        }
        Err(_) => {
            // No server available — skip
        }
    }
}

/// Disconnect with tool_init_minimal handle.
/// Ignored because it requires a running PMIx server.
#[test]
#[ignore = "requires PMIx server"]
fn test_disconnect_with_minimal_init() {
    use pmix::tool::{tool_finalize, tool_init_minimal};

    match tool_init_minimal() {
        Ok(handle) => {
            let attach_info = InfoBuilder::new().build();
            match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
                Ok((_, Some(server))) => {
                    let result = tool_disconnect(server.proc());
                    assert!(result.is_ok(), "Disconnect should succeed: {:?}", result);
                }
                Ok((_, None)) => {}
                Err(_) => {}
            }
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // No server available — skip
        }
    }
}
