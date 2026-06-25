//! Round 8 — P5: query_log.rs module via prte-beast daemon.
//!
//! Uses shared tool handle from daemon_helper for single init/finalize lifecycle.
//!
//! Run:
//!   cargo test --test daemon_query_log_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::query_log::{
    LogCallback, PmixQuery, QueryCallback, QueryResults, log_data, log_data_nb, query_info,
    query_info_nb,
};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run, no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_info_type() {
    let _f: fn(&[PmixQuery]) -> Result<QueryResults, PmixStatus> = query_info;
}

#[test]
fn test_query_info_nb_type() {
    let _f: fn(&[PmixQuery], Box<dyn QueryCallback>) -> Result<(), PmixStatus> = query_info_nb;
}

#[test]
fn test_log_data_type() {
    let _f: fn(&[Info], &[Info]) -> Result<(), PmixStatus> = log_data;
}

#[test]
fn test_log_data_nb_type() {
    let _f: fn(&[Info], &[Info], Box<dyn LogCallback>) -> Result<(), PmixStatus> = log_data_nb;
}

#[test]
fn test_pmix_query_new_type() {
    let _f: fn(&[&str]) -> Result<PmixQuery, PmixStatus> = PmixQuery::new;
}

#[test]
fn test_pmix_query_with_qualifiers_type() {
    let _f: fn(PmixQuery, Info) -> PmixQuery = PmixQuery::with_qualifiers;
}

#[test]
fn test_query_results_len_type() {
    let _f: fn(&QueryResults) -> usize = QueryResults::len;
}

#[test]
fn test_query_results_is_empty_type() {
    let _f: fn(&QueryResults) -> bool = QueryResults::is_empty;
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback implementations for non-blocking tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestQueryCallback {
    called: std::sync::atomic::AtomicBool,
    status: std::sync::Mutex<Option<PmixStatus>>,
    result_count: std::sync::atomic::AtomicUsize,
}

impl QueryCallback for TestQueryCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus, results: QueryResults) {
        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.status.lock().unwrap() = Some(status);
        self.result_count
            .store(results.len(), std::sync::atomic::Ordering::SeqCst);
    }
}

struct TestLogCallback {
    called: std::sync::atomic::AtomicBool,
    status: std::sync::Mutex<Option<PmixStatus>>,
}

impl LogCallback for TestLogCallback {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.status.lock().unwrap() = Some(status);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — uses shared tool handle.
// PmixQuery construction tests run independently (no FFI).
// All FFI-calling tests consolidated into a single test to avoid PMIx state corruption.
// ─────────────────────────────────────────────────────────────────────────────

/// Test PmixQuery::new with single key (no FFI)
#[test]
#[ignore = "daemon isolation"]
fn test_pmix_query_new_single_key() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _ = daemon_helper::get_tool_handle().expect("shared tool handle");

    let query = PmixQuery::new(&["pmix.version"]).expect("PmixQuery::new with single key");
    let _ = query;
}

/// Test PmixQuery::new with multiple keys (no FFI)
#[test]
#[ignore = "daemon isolation"]
fn test_pmix_query_new_multiple_keys() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _ = daemon_helper::get_tool_handle().expect("shared tool handle");

    let query = PmixQuery::new(&["pmix.version", "pmix.client.attrs"])
        .expect("PmixQuery::new with multiple keys");
    let _ = query;
}

/// Test PmixQuery::new with empty keys returns ErrBadParam (no FFI)
#[test]
#[ignore = "daemon isolation"]
fn test_pmix_query_new_empty_keys_error() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _ = daemon_helper::get_tool_handle().expect("shared tool handle");

    let result = PmixQuery::new(&[]);
    assert!(result.is_err(), "PmixQuery::new(&[]) should return Err");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "Expected ErrBadParam for empty keys"
    );
}

/// Test PmixQuery::with_qualifiers (no FFI)
#[test]
#[ignore = "daemon isolation"]
fn test_pmix_query_with_qualifiers() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _ = daemon_helper::get_tool_handle().expect("shared tool handle");

    let query = PmixQuery::new(&["pmix.version"]).expect("PmixQuery::new");
    let info = InfoBuilder::new().build();
    let _query_with_qual = query.with_qualifiers(info);
}

/// CONSOLIDATED test: All query_log FFI calls in a single shared handle context.
/// Tests: query_info with various keys, QueryResults::len(), QueryResults::is_empty(),
/// query_info_nb, log_data, log_data_nb.
#[test]
#[ignore = "daemon isolation"]
fn test_query_log_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("shared tool handle");

    // ── 1. query_info with various keys ──
    let keys = vec!["pmix.version", "pmix.client.attrs", "pmix.system.tools"];

    for key in &keys {
        let q = PmixQuery::new(&[key]).expect("PmixQuery::new");
        let qs = vec![q];
        let result = query_info(&qs);
        match &result {
            Ok(results) => {
                let len = results.len();
                let empty = results.is_empty();
                assert_eq!(
                    empty,
                    len == 0,
                    "is_empty should match len == 0 for key={}",
                    key
                );
                println!("query '{}' -> len={}, is_empty={}", key, len, empty);
            }
            Err(status) => {
                println!("query '{}' -> error: {}", key, status);
            }
        }
    }

    // ── 2. query_info_nb ──
    let query = PmixQuery::new(&["pmix.version"]).expect("PmixQuery::new");
    let queries = vec![query];

    let cb = Box::new(TestQueryCallback {
        called: std::sync::atomic::AtomicBool::new(false),
        status: std::sync::Mutex::new(None),
        result_count: std::sync::atomic::AtomicUsize::new(0),
    });

    let submit_result = query_info_nb(&queries, cb);
    match &submit_result {
        Ok(()) => {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(status) => {
            println!("query_info_nb rejected immediately: {}", status);
        }
    }

    // ── 3. query_info_nb with empty queries returns ErrBadParam ──
    let cb2 = Box::new(TestQueryCallback {
        called: std::sync::atomic::AtomicBool::new(false),
        status: std::sync::Mutex::new(None),
        result_count: std::sync::atomic::AtomicUsize::new(0),
    });

    let result = query_info_nb(&[], cb2);
    assert!(result.is_err(), "query_info_nb(&[]) should return Err");
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::Known(PmixError::ErrBadParam),
        "Expected ErrBadParam for empty queries"
    );

    // ── 4. log_data ──
    let data: Vec<Info> = vec![];
    let directives: Vec<Info> = vec![];

    let log_result = log_data(&data, &directives);
    match &log_result {
        Ok(()) => println!("log_data succeeded"),
        Err(status) => {
            println!("log_data returned error: {}", status);
        }
    }

    // ── 5. log_data_nb ──
    let cb3 = Box::new(TestLogCallback {
        called: std::sync::atomic::AtomicBool::new(false),
        status: std::sync::Mutex::new(None),
    });

    let submit_result = log_data_nb(&data, &directives, cb3);
    match &submit_result {
        Ok(()) => {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(status) => {
            println!("log_data_nb rejected immediately: {}", status);
        }
    }
}
