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
use std::thread;
use std::time::Duration;

use pmix::Info;
use pmix::info_with_string_key;
use pmix::tool::{PmixToolHandle, tool_init};

/// Default timeout for `tool_init` FFI calls (in seconds).
///
/// PMIx_tool_init can block indefinitely if the PRTE daemon is running but
/// not accepting tool connections (e.g., stale URI file, port mismatch after
/// daemon restart, or the daemon being in a broken state). This timeout
/// prevents the test runner from hanging forever.
const TOOL_INIT_TIMEOUT_SECS: u64 = 10;

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

/// Read the PRTE URI from the URI file or `PMIX_SERVER_URIv61` env var.
///
/// openpmix 6.1.0 checks versioned env vars (PMIX_SERVER_URIv61, PMIX_SERVER_URIv51,
/// PMIX_SERVER_URI3, etc.) via `pmix_ptl_base_check_server_uris()`. The
/// unversioned `PMIX_SERVER_URI` is NOT checked by the library.
pub fn read_uri() -> Result<String, String> {
    // Try versioned env var first (set by test harness or PRRTE)
    // PMIX_SERVER_URIv61 matches our PRRTE 4.1.0 / openpmix 6.1.0 setup
    for key in &[
        "PMIX_SERVER_URIv61",
        "PMIX_SERVER_URIv51",
        "PMIX_SERVER_URIv41",
    ] {
        if let Ok(uri) = env::var(key) {
            if !uri.is_empty() {
                return Ok(uri);
            }
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

/// Return an `Info` array containing the `pmix.srvr.uri` key with the
/// daemon's URI value. This bypasses the 13-byte key limit of `InfoBuilder::add()`
/// by using `info_with_string_key()` which accepts arbitrary-length string keys.
///
/// Use this for `tool_init()` calls that need to connect to the running daemon.
pub fn get_tool_init_info() -> Info {
    let uri = read_uri().unwrap_or_else(|e| {
        panic!("Cannot read PRTE URI for tool_init: {}", e);
    });
    info_with_string_key("pmix.srvr.uri", &uri)
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

/// Call `tool_init` in a background thread with a timeout.
///
/// `PMIx_tool_init` can block indefinitely if the PRTE daemon is running
/// but not accepting tool connections (e.g., stale URI file, port mismatch
/// after daemon restart, or the daemon being in a broken state). This
/// wrapper spawns the FFI call in a separate thread and kills it after
/// `TOOL_INIT_TIMEOUT_SECS` seconds, returning `Err` instead of hanging.
///
/// We pass the URI string (not the Info struct) across the thread boundary,
/// then construct the Info inside the thread. This avoids any issues with
/// Info's internal pointer not being Send.
///
/// Returns `Ok(PmixToolHandle)` on success, `Err(String)` on timeout or FFI error.
fn tool_init_with_timeout(uri: &str) -> Result<PmixToolHandle, String> {
    let uri = uri.to_string();

    let (tx, rx) = std::sync::mpsc::channel();

    let handle = thread::spawn(move || {
        // Construct the Info inside the thread to avoid cross-thread pointer issues
        let info = info_with_string_key("pmix.srvr.uri", &uri);
        match tool_init(None, &info) {
            Ok(h) => tx.send(Ok(h)),
            Err(e) => tx.send(Err(format!("tool_init returned error: {:?}", e))),
        }
    });

    match rx.recv_timeout(Duration::from_secs(TOOL_INIT_TIMEOUT_SECS)) {
        Ok(result) => {
            // Detach the thread — it's already done sending
            let _ = handle.join();
            result
        }
        Err(_recv_timeout) => {
            // Timeout — the thread is still blocked in the FFI call.
            // We can't kill it cleanly, but we can mark init as failed
            // so subsequent calls don't retry. In practice the thread
            // will be cleaned up when the test process exits.
            eprintln!(
                "[daemon_helper] tool_init timed out after {} seconds — \
                 PRTE daemon may not be accepting tool connections",
                TOOL_INIT_TIMEOUT_SECS
            );
            Err(format!(
                "tool_init timed out after {} seconds (PRTE daemon not accepting tool connections)",
                TOOL_INIT_TIMEOUT_SECS
            ))
        }
    }
}

/// Get the shared tool handle.
///
/// Returns `Err` if the daemon is unavailable or `tool_init` failed.
/// The handle is initialized lazily on first call and reused for all
/// subsequent calls in the same test binary.
///
/// Thread-safe: uses the global daemon mutex to serialize initialization,
/// preventing the race where multiple threads all see `SHARED_TOOL` as None
/// and try to call `tool_init` concurrently.
///
/// Uses a timeout wrapper around `tool_init` to prevent indefinite hangs
/// when the PRTE daemon is running but not accepting tool connections.
pub fn get_tool_handle() -> Result<&'static PmixToolHandle, String> {
    // Fast path: already initialized (lock-free)
    if let Some(handle) = SHARED_TOOL.get() {
        return Ok(handle);
    }

    // Slow path: acquire lock to serialize initialization
    let _lock = daemon_lock()?;

    // Double-check after acquiring lock
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

    // Use timeout wrapper to prevent indefinite hangs
    match tool_init_with_timeout(&uri) {
        Ok(handle) => {
            eprintln!(
                "[daemon_helper] tool_init succeeded (uri={})",
                uri.chars().take(30).collect::<String>()
            );
            // Store it — Drop will call tool_finalize at process exit
            SHARED_TOOL.set(handle).ok();
            SHARED_TOOL
                .get()
                .ok_or_else(|| "tool_init did not store handle".to_string())
        }
        Err(e) => {
            INIT_FAILED.set(true).ok();
            Err(format!("tool_init failed: {}", e))
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

/// Guard that restores `PMIX_SERVER_URIv61` to its previous value on drop.
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
                Some(val) => env::set_var("PMIX_SERVER_URIv61", val),
                None => env::remove_var("PMIX_SERVER_URIv61"),
            }
        }
    }
}

/// Read the PRTE URI file and set `PMIX_SERVER_URIv61`. Returns a guard that
/// restores the previous value on drop.
///
/// openpmix 6.1.0 checks versioned env vars (PMIX_SERVER_URIv61, PMIX_SERVER_URIv51,
/// etc.) via `pmix_ptl_base_check_server_uris()`. The unversioned
/// `PMIX_SERVER_URI` is NOT checked by the library.
///
/// Deprecated: use `get_tool_handle()` instead.
pub fn connect_to_daemon() -> Result<DaemonGuard, String> {
    let uri = read_uri()?;

    let previous;
    unsafe {
        previous = env::var("PMIX_SERVER_URIv61").ok();
        env::set_var("PMIX_SERVER_URIv61", &uri);
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

/// Ensures PMIx is initialized exactly once per test binary (DVM/prterun tests).
/// Returns a reference to the Context.
/// Call this instead of direct pmix::init(None) to avoid multiple init/finalize.
/// Multiple cycles are not supported.
pub fn ensure_pmix_init() -> &'static pmix::Context {
    use std::sync::OnceLock;
    static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();
    PMIX_CTX.get_or_init(|| pmix::init(None).expect("PMIx_Init failed — run under prterun"))
}
