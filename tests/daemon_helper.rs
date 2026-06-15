//! Helper module for tests that require a running PMIx daemon.
//!
//! Reads the PRTE server URI from the systemd-managed service and sets
//! `PMIX_SERVER_URI` so that `PMIx_tool_init` can connect.
//!
//! Also provides a global mutex to serialize daemon-dependent tests, since
//! PMIx tool_init/finalize use global C state that cannot be accessed
//! concurrently.
//!
//! Usage in test files:
//! ```rust
//! mod daemon_helper;
//!
//! #[test]
//! fn my_daemon_test() {
//!     let _lock = daemon_helper::daemon_lock().expect("another test holds the lock");
//!     let _guard = daemon_helper::connect_to_daemon().expect("daemon not available");
//!     // Now tool_init will work
//!     let handle = pmix_rs::tool::tool_init(None, &pmix_rs::InfoBuilder::new().build())
//!         .expect("tool_init failed");
//!     // ... test code ...
//! }
//! ```

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Default path where the systemd PRTE service writes its URI.
const DEFAULT_URI_FILE: &str = "/run/user/1000/prte/uri";

/// Resolve the URI file path. Checks `PMIX_TEST_URI_FILE` environment variable
/// first (useful for CI environments), falling back to the default systemd path.
fn resolve_uri_file() -> String {
    env::var("PMIX_TEST_URI_FILE").unwrap_or_else(|_| DEFAULT_URI_FILE.to_string())
}

/// Global mutex to serialize all daemon-dependent tests.
/// PMIx tool_init/finalize use global C state that cannot be accessed concurrently.
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

/// Guard that restores `PMIX_SERVER_URI` to its previous value on drop.
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
/// Returns `Err` if the URI file doesn't exist, is empty, or the PRTE service
/// is not running.
pub fn connect_to_daemon() -> Result<DaemonGuard, String> {
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

    let previous;
    unsafe {
        // Save previous value
        previous = env::var("PMIX_SERVER_URI").ok();

        // Set the new URI
        env::set_var("PMIX_SERVER_URI", &uri);
    }

    Ok(DaemonGuard { previous })
}

/// Check if a PRTE daemon is available (URI file exists and is readable).
pub fn daemon_available() -> bool {
    connect_to_daemon().is_ok()
}
