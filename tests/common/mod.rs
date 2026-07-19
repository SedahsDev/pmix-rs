//! Shared integration-test harness for pmix-rs.
//!
//! Integration tests under `tests/*.rs` are separate crates, so each file that
//! needs these helpers should include:
//!
//! ```ignore
//! #[path = "common/mod.rs"]
//! mod common;
//! use common::{require_server, skip_without_server};
//! ```
//!
//! ## Helpers
//!
//! - [`require_server`] — fail-fast; use with `#[ignore]` daemon tests
//! - [`skip_without_server`] — returns `false` when server init is unavailable
//! - [`with_server`] — run a closure only when server init succeeds

use pmix::PmixStatus;
use pmix::server::PmixServerHandle;

/// Initialize a minimal PMIx server or panic.
///
/// Prefer this for `#[ignore]` tests that already require a daemon/runtime.
/// Panicking makes missing setup loud instead of silently skipping.
pub fn require_server() -> PmixServerHandle {
    pmix::server::server_init_minimal(None).unwrap_or_else(|e| {
        panic!(
            "server_init_minimal required for this test but failed: {:?}",
            e
        )
    })
}

/// Try to initialize a minimal PMIx server.
///
/// Returns `Some(handle)` on success, `None` if init failed (e.g. not running
/// as a server process). Callers that want graceful skip should return early
/// when this is `None`.
pub fn try_server() -> Option<PmixServerHandle> {
    pmix::server::server_init_minimal(None).ok()
}

/// Returns `true` if a minimal server was initialized successfully.
///
/// Convenience for the common `if !skip_without_server() { return; }` pattern.
pub fn skip_without_server() -> bool {
    try_server().is_some()
}

/// Run `f` only when `server_init_minimal` succeeds.
///
/// On init failure, prints a skip message and returns `Ok(())` so the test
/// does not fail when a server runtime is unavailable.
pub fn with_server<F>(test_name: &str, f: F) -> Result<(), PmixStatus>
where
    F: FnOnce(&PmixServerHandle) -> Result<(), PmixStatus>,
{
    match try_server() {
        Some(handle) => f(&handle),
        None => {
            eprintln!("{test_name}: server_init failed, skipping");
            Ok(())
        }
    }
}

/// Finalize a server handle, ignoring finalize errors (best-effort cleanup).
pub fn finalize_server(handle: PmixServerHandle) {
    let _ = pmix::server::server_finalize(handle);
}
