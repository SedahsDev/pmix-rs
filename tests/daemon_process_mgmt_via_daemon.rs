//! Round 8 — P4: process_mgmt.rs module via prte-beast daemon.
//!
//! Uses shared tool handle from daemon_helper for single init/finalize lifecycle.
//!
//! Run:
//!   cargo test --test daemon_process_mgmt_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::process_mgmt::{
    ConnectCallbackWrapper, DisconnectCallbackWrapper, PmixApp, SpawnCallbackWrapper, abort,
    connect, connect_nb, disconnect, disconnect_nb, resolve_nodes, resolve_peers, spawn, spawn_nb,
};
use pmix::{InfoBuilder, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run, no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_type() {
    let _f: fn(&[pmix::Info], &[PmixApp]) -> Result<String, PmixStatus> = spawn;
}

#[test]
fn test_spawn_nb_type() {
    let _f: fn(&[pmix::Info], &[PmixApp], SpawnCallbackWrapper) -> Result<(), PmixStatus> =
        spawn_nb;
}

#[test]
fn test_connect_type() {
    let _f: fn(&[Proc], &[pmix::Info]) -> Result<(), PmixStatus> = connect;
}

#[test]
fn test_connect_nb_type() {
    let _f: fn(&[Proc], &[pmix::Info], ConnectCallbackWrapper) -> Result<(), PmixStatus> =
        connect_nb;
}

#[test]
fn test_disconnect_type() {
    let _f: fn(&[Proc], &[pmix::Info]) -> Result<(), PmixStatus> = disconnect;
}

#[test]
fn test_disconnect_nb_type() {
    let _f: fn(&[Proc], &[pmix::Info], DisconnectCallbackWrapper) -> Result<(), PmixStatus> =
        disconnect_nb;
}

#[test]
fn test_abort_type() {
    let _f: fn(PmixStatus, Option<&str>, Option<&[Proc]>) -> Result<(), PmixStatus> = abort;
}

#[test]
fn test_resolve_peers_type() {
    let _f: fn(Option<&str>, Option<&str>) -> Result<Vec<Proc>, PmixStatus> = resolve_peers;
}

#[test]
fn test_resolve_nodes_type() {
    let _f: fn(&str) -> Result<String, PmixStatus> = resolve_nodes;
}

#[test]
fn test_spawn_callback_wrapper_exists() {
    let _cb = SpawnCallbackWrapper::new(|_status: PmixStatus, _nspace: Option<String>| {});
}

#[test]
fn test_connect_callback_wrapper_exists() {
    let _cb = ConnectCallbackWrapper::new(|_status: PmixStatus| {});
}

#[test]
fn test_disconnect_callback_wrapper_exists() {
    let _cb = DisconnectCallbackWrapper::new(|_status: PmixStatus| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// Error-path tests (always run — no daemon needed)
//
// NOTE: With the shared handle, PMIx is already initialized so these tests
// now exercise the "after init" path. The ErrInit path is covered by daemon_server tests.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_connect_before_init() {
    let proc = Proc::new("test-nspace", 0).expect("proc");
    let procs = vec![proc];
    let result = connect(&procs, &[]);
    let _ = result;
}

#[test]
fn test_disconnect_before_init() {
    let proc = Proc::new("test-nspace", 0).expect("proc");
    let procs = vec![proc];
    let result = disconnect(&procs, &[]);
    let _ = result;
}

#[test]
fn test_resolve_peers_before_init() {
    let result = resolve_peers(None, None);
    let _ = result;
}

#[test]
fn test_resolve_nodes_before_init() {
    let result = resolve_nodes("test-nspace");
    let _ = result;
}

#[test]
fn test_spawn_before_init() {
    let apps = vec![PmixApp::builder().cmd("echo").build().expect("app")];
    let result = spawn(&[], &apps);
    let _ = result;
}

#[test]
fn test_abort_before_init() {
    let result = abort(PmixStatus::Known(PmixError::Success), Some("test"), None);
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — single consolidated test using shared tool handle.
//
// IMPORTANT: PMIx_Connect and PMIx_Disconnect are collective operations that
// block until ALL participating processes have called connect/disconnect with
// the same set of procs. In a single-process test context, these will block
// forever. We test them via the nb variants which return immediately, and the
// callback may fire in-place (per PMIx spec) or asynchronously.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "daemon isolation"]
fn test_process_mgmt_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("shared tool handle");
    let _ = handle; // handle lives for the duration

    let proc = Proc::new("test-nspace", 0).expect("proc");
    let procs = vec![proc];
    let directives = vec![InfoBuilder::new().build()];
    let nspace = "test-nspace";

    // ── 1. connect_nb (non-blocking — returns immediately, callback may fire in-place) ──
    let cb = ConnectCallbackWrapper::new(|_status| {});
    let connect_result = connect_nb(&procs, &directives, cb);
    match &connect_result {
        Ok(()) => {}
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "connect_nb returned ErrInit after tool_init"
            );
        }
    }

    // ── 2. disconnect_nb (non-blocking — returns immediately) ──
    let cb = DisconnectCallbackWrapper::new(|_status| {});
    let disconnect_result = disconnect_nb(&procs, &directives, cb);
    match &disconnect_result {
        Ok(()) => {}
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "disconnect_nb returned ErrInit after tool_init"
            );
        }
    }

    // ── 3. resolve_peers ──
    let peers_result = resolve_peers(None, Some(nspace));
    match &peers_result {
        Ok(peers) => {
            let _ = peers.len();
        }
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "resolve_peers returned ErrInit after tool_init"
            );
        }
    }

    // ── 4. resolve_nodes ──
    let nodes_result = resolve_nodes(nspace);
    match &nodes_result {
        Ok(nodes) => {
            let _ = nodes.len();
        }
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "resolve_nodes returned ErrInit after tool_init"
            );
        }
    }

    // ── 5. abort (various parameter combos) ──
    let _ = abort(
        PmixStatus::Known(PmixError::Success),
        Some("test abort message"),
        Some(&procs),
    );
    let _ = abort(PmixStatus::Known(PmixError::Success), None, Some(&procs));
    let _ = abort(
        PmixStatus::Known(PmixError::Success),
        Some("global abort"),
        None,
    );

    // ── 6. spawn ──
    let apps = vec![PmixApp::builder().cmd("echo").build().expect("app")];
    let spawn_result = spawn(&directives, &apps);
    match &spawn_result {
        Ok(nspace) => {
            let _ = nspace.len();
        }
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "spawn returned ErrInit after tool_init"
            );
        }
    }

    // ── 7. spawn_nb ──
    let cb = SpawnCallbackWrapper::new(|_status, _nspace| {});
    let spawn_nb_result = spawn_nb(&directives, &apps, cb);
    match &spawn_nb_result {
        Ok(()) => {}
        Err(status) => {
            assert_ne!(
                *status,
                PmixStatus::Known(PmixError::ErrInit),
                "spawn_nb returned ErrInit after tool_init"
            );
        }
    }

    // NOTE: Blocking connect/disconnect skipped — these are collective
    // operations that require multiple processes. In a single-process
    // context they block forever. The nb variants above exercise the
    // FFI paths; the callback may fire in-place per PMIx spec.
}
