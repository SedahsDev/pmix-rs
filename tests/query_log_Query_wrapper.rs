//! Wrapper tests for query_log.rs — Query/Core Operations.
//!
//! Tests exercise Rust wrapper logic without PMIx_Init. FFI calls return
//! errors gracefully (no segfault), so we can test error paths, parameter
//! validation, and callback registration/cleanup logic.
//!
//! Coverage targets:
//!   - PmixQuery::new (lines 89-180) — key validation, CString, FFI alloc
//!   - PmixQuery::with_qualifiers (lines 162-171) — qualifier assignment
//!   - query_info (lines 285-314) — FFI call + error path
//!   - query_info_nb (lines 414-457) — callback reg + FFI + cleanup

use pmix::query_log::*;
use pmix::{PmixStatus, InfoBuilder};

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery::new — constructor
// ─────────────────────────────────────────────────────────────────────────────

/// PmixQuery::new with single key succeeds.
#[test]
fn test_query_new_single_key() {
    let query = PmixQuery::new(&["test_key"]);
    assert!(query.is_ok());
}

/// PmixQuery::new with multiple keys succeeds.
#[test]
fn test_query_new_multiple_keys() {
    let query = PmixQuery::new(&["key1", "key2", "key3"]);
    assert!(query.is_ok());
}

/// PmixQuery::new with empty keys returns ErrBadParam.
#[test]
fn test_query_new_empty_keys() {
    let result = PmixQuery::new(&[]);
    assert!(result.is_err());
}

/// PmixQuery::new with key containing NUL returns ErrBadParam (CString failure).
#[test]
fn test_query_new_nul_key() {
    let result = PmixQuery::new(&["test\x00key"]);
    assert!(result.is_err());
}

/// PmixQuery::new with long key succeeds (CString handles it).
#[test]
fn test_query_new_long_key() {
    let long_key: String = "k".repeat(256);
    let result = PmixQuery::new(&[&long_key]);
    assert!(result.is_ok());
}

/// PmixQuery::new with special characters in key.
#[test]
fn test_query_new_special_chars() {
    let result = PmixQuery::new(&["pmix:key:name", "key_with-dash", "key.with.dots"]);
    assert!(result.is_ok());
}

/// PmixQuery::new is deterministic.
#[test]
fn test_query_new_deterministic() {
    let r1 = PmixQuery::new(&["test_key"]);
    let r2 = PmixQuery::new(&["test_key"]);
    assert_eq!(r1.is_ok(), r2.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery::with_qualifiers — qualifier assignment
// ─────────────────────────────────────────────────────────────────────────────

/// with_qualifiers on a valid query succeeds.
#[test]
fn test_query_with_qualifiers() {
    let query = PmixQuery::new(&["test_key"]).unwrap();
    let info = InfoBuilder::new().build();
    let _query_with_qual = query.with_qualifiers(info);
}

/// with_qualifiers with empty Info.
#[test]
fn test_query_with_empty_qualifiers() {
    let query = PmixQuery::new(&["test_key"]).unwrap();
    let info = InfoBuilder::new().build();
    let _query_with_qual = query.with_qualifiers(info);
}

// ─────────────────────────────────────────────────────────────────────────────
// query_info — synchronous query
// ─────────────────────────────────────────────────────────────────────────────

/// query_info with empty queries returns error.
#[test]
fn test_query_info_empty() {
    let queries: Vec<PmixQuery> = vec![];
    let result = query_info(&queries);
    assert!(result.is_err());
}

/// query_info with valid queries returns error (not initialized).
#[test]
fn test_query_info_with_queries() {
    let query = PmixQuery::new(&["test_key"]).unwrap();
    let queries = vec![query];
    let result = query_info(&queries);
    assert!(result.is_err());
}

/// query_info with multiple queries returns error.
#[test]
fn test_query_info_multiple_queries() {
    let q1 = PmixQuery::new(&["key1"]).unwrap();
    let q2 = PmixQuery::new(&["key2"]).unwrap();
    let queries = vec![q1, q2];
    let result = query_info(&queries);
    assert!(result.is_err());
}

/// query_info is deterministic.
#[test]
fn test_query_info_deterministic() {
    let queries: Vec<PmixQuery> = vec![];
    let r1 = query_info(&queries);
    let r2 = query_info(&queries);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// query_info_nb — non-blocking query
// ─────────────────────────────────────────────────────────────────────────────

/// query_info_nb with empty queries returns error, callback not invoked.
#[test]
fn test_query_info_nb_empty() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl QueryCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let queries: Vec<PmixQuery> = vec![];
    let result = query_info_nb(&queries, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// query_info_nb with valid queries returns error, callback not invoked.
#[test]
fn test_query_info_nb_with_queries() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl QueryCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let query = PmixQuery::new(&["test_key"]).unwrap();
    let queries = vec![query];
    let result = query_info_nb(&queries, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// query_info_nb is deterministic.
#[test]
fn test_query_info_nb_deterministic() {
    struct Cb;
    impl QueryCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let queries: Vec<PmixQuery> = vec![];
    let r1 = query_info_nb(&queries, Box::new(Cb));
    let r2 = query_info_nb(&queries, Box::new(Cb));
    assert_eq!(r1.is_err(), r2.is_err());
}
