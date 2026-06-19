//! Helper module for tests that require a running PMIx daemon.
//!
//! Provides a **single shared tool handle** that is initialized once at the
//! start of the test binary and finalized when the process exits. This avoids
//! the PMIx global state corruption that occurs from multiple
//! `tool_init`/`tool_finalize` cycles.
//!
//! Also provides a global mutex to serialize daemon-dependent tests, since
//! PMIx tool APIs use global C state that cannot be accessed concurrently.
//!
//! Usage in test files:
//! ```rust
//! mod daemon_helper;
//!
//! #[test]
//! fn my_daemon_test() {
//!     let handle = daemon_helper::get_tool_handle().expect("daemon not available");
//!     // Now use handle directly — no need for tool_init/finalize
//!     let results = pmix::query_log::query_info(handle, &[PmixQueryKey::Version]).expect("query");
//!     // ... test code ...
//! }
//! ```

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use pmix::tool::{tool_init, PmixToolHandle};
use pmix::InfoBuilder;

/// Default path where the systemd PRTE service writes its URI.
const DEFAULT_URI_FILE: &str = "/run/user/1000/prte/uri";

/// Resolve the URI file path. Checks `PMIX_TEST_URI_FILE` environment variable
/// first (useful for CI environments), falling back to the default systemd path.
fn resolve_uri_file() -> String {
    env::var("PMIX_TEST_URI_FILE").unwrap_or_else(|_| DEFAULT_URI_FILE.to_string())
}

/// Global mutex to serialize all daemon-dependent tests.
/// PMIx tool APIs use global C state that cannot be accessed concurrently.
fn global_daemon_mutex() -> &'static Mutex<()> {
    use std::sync::OnceLock;
    static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| Mutex::new(()))
}

/// Acquire the global daemon lock. Returns a guard that releases on drop.
pub fn daemon_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    global_daemon_mutex()
        .lock()
        .map_err(|e| format!("Failed to acquire daemon lock: {}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared tool handle (initialized once, finalized at process exit)
// ─────────────────────────────────────────────────────────────────────────────

/// Read the PRTE URI from the URI file or `PMIX_SERVER_URI` env var.
fn read_uri() -> Result<String, String> {
    // Try env var first (set by test harness)
    if let Ok(uri) = env::var("PMIX_SERVER_URI") {
        if !uri.is_empty() {
            return Ok(uri);
        }
    }

    // Fall back to URI file
    let uri_file = resolve_uri_file();
    let uri_path = Path::new(&uri_file);

    if !uri_path.exists() {
        return Err(format!(
            "PRTE URI file not found at {}. Is the PRTE daemon running?",
            uri_path.display()
        ));
    }

    let uri_content =
        fs::read_to_string(uri_path).map_err(|e| format!("Failed to read URI file: {}", e))?;

    let uri = uri_content
        .lines()
        .next()
        .ok_or_else(|| "URI file is empty".to_string())?
        .trim()
        .to_string();

    if uri.is_empty() {
        return Err("URI file contains no valid URI".to_string());
    }

    Ok(uri)
}

/// Singleton tool handle, initialized once for the entire test binary.
///
/// The handle is created on first call and lives until process exit,
/// where `Drop` automatically calls `tool_finalize`.
///
/// Callers must hold the daemon lock (via `daemon_lock()`) before using
/// the handle, since PMIx C APIs access global state.
static SHARED_TOOL: std::sync::OnceLock<PmixToolHandle> = std::sync::OnceLock::new();

/// Whether initialization was attempted but failed.
static INIT_FAILED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Get the shared tool handle.
///
/// Returns `Err` if the daemon is unavailable or `tool_init` failed.
/// The handle is initialized lazily on first call and reused for all
/// subsequent calls in the same test binary.
pub fn get_tool_handle() -> Result<&'static PmixToolHandle, String> {
    // Fast path: already initialized
    if let Some(handle) = SHARED_TOOL.get() {
        return Ok(handle);
    }

    // Try to initialize
    let uri = match read_uri() {
        Ok(u) => u,
        Err(e) => {
            INIT_FAILED.set(true).ok();
            return Err(format!("Cannot read PRTE URI: {}", e));
        }
    };

    let info = InfoBuilder::new().build();

    match tool_init(None, &info) {
        Ok(handle) => {
            eprintln!(
                "[daemon_helper] tool_init succeeded (uri={})",
                uri.chars().take(30).collect::<String>()
            );
            // Store it — Drop will call tool_finalize at process exit
            SHARED_TOOL.set(handle).ok();
            SHARED_TOOL.get().ok_or_else(|| "tool_init did not store handle".to_string())
        }
        Err(e) => {
            INIT_FAILED.set(true).ok();
            Err(format!("tool_init failed: {:?}", e))
        }
    }
}

/// Check if a PRTE daemon is available (URI file exists and is readable).
pub fn daemon_available() -> bool {
    read_uri().is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Legacy API (kept for backward compatibility, but prefer get_tool_handle)
// ─────────────────────────────────────────────────────────────────────────────

/// Guard that restores `PMIX_SERVER_URI` to its previous value on drop.
///
/// Deprecated: use `get_tool_handle()` instead, which manages the URI
/// internally.
pub struct DaemonGuard {
    previous: Option<String>,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(val) => env::set_var("PMIX_SERVER_URI", val),
                None => env::remove_var("PMIX_SERVER_URI"),
            }
        }
    }
}

/// Read the PRTE URI file and set `PMIX_SERVER_URI`. Returns a guard that
/// restores the previous value on drop.
///
/// Deprecated: use `get_tool_handle()` instead.
pub fn connect_to_daemon() -> Result<DaemonGuard, String> {
    let uri = read_uri()?;

    let previous;
    unsafe {
        previous = env::var("PMIX_SERVER_URI").ok();
        env::set_var("PMIX_SERVER_URI", &uri);
    }

    Ok(DaemonGuard { previous })
}

// ─────────────────────────────────────────────────────────────────────────────
// Test utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Assert that a `PmixStatus` is success.
pub fn assert_success(status: pmix::PmixStatus) {
    assert_eq!(
        status,
        pmix::PmixStatus::Known(pmix::PmixError::Success),
        "Expected PMIX_SUCCESS, got {:?}",
        status
    );
}

/// Assert that a `PmixStatus` is an error (not success).
pub fn assert_error(status: pmix::PmixStatus) {
    assert_ne!(
        status,
        pmix::PmixStatus::Known(pmix::PmixError::Success),
        "Expected error, got success"
    );
}
