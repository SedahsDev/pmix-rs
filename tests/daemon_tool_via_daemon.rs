//! Round 8 — P7: tool.rs module via prte-beast daemon.
//!
//! Tests tool.rs FFI functions directly. Cannot use the shared tool handle
//! for testing tool_init/finalize themselves, but uses daemon_lock for serialization.
//!
//! Run:
//!   cargo test --test daemon_tool_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::tool::{
    tool_init, tool_init_minimal, tool_finalize, tool_is_connected,
    tool_attach_to_server, tool_disconnect, tool_connect_to_server,
    tool_get_servers, tool_set_server, is_tool_initialized,
};
use pmix::{InfoBuilder, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run, no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_tool_initialized_type() {
    let _f: fn() -> bool = is_tool_initialized;
}

#[test]
fn test_tool_init_type() {
    use pmix::tool::PmixToolHandle;
    let _f: fn(Option<&Proc>, &pmix::Info) -> Result<PmixToolHandle, PmixStatus> = tool_init;
}

#[test]
fn test_tool_init_minimal_type() {
    use pmix::tool::PmixToolHandle;
    let _f: fn() -> Result<PmixToolHandle, PmixStatus> = tool_init_minimal;
}

#[test]
fn test_tool_finalize_type() {
    use pmix::tool::PmixToolHandle;
    let _f: fn(PmixToolHandle) -> Result<(), PmixStatus> = tool_finalize;
}

#[test]
fn test_tool_is_connected_type() {
    let _f: fn() -> bool = tool_is_connected;
}

#[test]
fn test_tool_attach_to_server_type() {
    use pmix::tool::{PmixToolHandle, PmixServerHandle};
    let _f: fn(
        Option<&Proc>,
        bool,
        &pmix::Info,
    ) -> Result<(Option<PmixToolHandle>, Option<PmixServerHandle>), PmixStatus> =
        tool_attach_to_server;
}

#[test]
fn test_tool_disconnect_type() {
    let _f: fn(&Proc) -> Result<(), PmixStatus> = tool_disconnect;
}

#[test]
fn test_tool_connect_to_server_type() {
    use pmix::tool::PmixToolHandle;
    let _f: fn(Option<&Proc>, &pmix::Info) -> Result<PmixToolHandle, PmixStatus> =
        tool_connect_to_server;
}

#[test]
fn test_tool_get_servers_type() {
    let _f: fn() -> Result<Vec<Proc>, PmixStatus> = tool_get_servers;
}

#[test]
fn test_tool_set_server_type() {
    let _f: fn(&Proc, &pmix::Info) -> Result<(), PmixStatus> = tool_set_server;
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — single consolidated test to avoid PMIx state corruption.
// This tests tool.rs FFI functions. Uses shared handle to ensure daemon is
// connected, then tests tool operations.
// ─────────────────────────────────────────────────────────────────────────────

/// Full tool workflow: ensure shared handle is initialized, then exercise all tool APIs.
/// Consolidated into a single test to avoid multiple init/finalize cycles.
#[test]
#[ignore = "daemon isolation"]
fn test_tool_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    // Ensure shared handle is initialized — this gives us the daemon connection
    let _shared = daemon_helper::get_tool_handle().expect("shared tool handle");

    let info = InfoBuilder::new().build();

    // ── is_tool_initialized ──
    let initialized = is_tool_initialized();
    assert!(initialized, "tool should be initialized after shared handle init");

    // ── tool_is_connected ──
    let connected = tool_is_connected();
    let _ = connected;

    // ── tool_get_servers ──
    let servers_result = tool_get_servers();
    let _ = servers_result;

    // ── tool_set_server (with a dummy proc) ──
    let proc = Proc::new("test-nspace", 0).expect("proc");
    let set_result = tool_set_server(&proc, &info);
    let _ = set_result;

    // ── tool_disconnect ──
    let disc_result = tool_disconnect(&proc);
    let _ = disc_result;

    // NOTE: We do NOT call tool_init/tool_finalize here because the shared handle
    // already manages the init/finalize lifecycle. Calling tool_init again on top
    // of an existing init causes PMIx global state corruption.
    // tool_init_minimal, tool_connect_to_server, tool_attach_to_server are tested
    // via their type signatures above and in the daemon_server tests.
}
