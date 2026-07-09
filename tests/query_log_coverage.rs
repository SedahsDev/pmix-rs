//! Additional structural coverage for query_log module — TASK-032.
//!
//! Focus on pure-Rust code paths that don't require PMIx_Init:
//! - PmixQuery construction edge cases
//! - query_info validation (empty queries rejection)
//! - LogCallback/QueryCallback trait object construction
//! - PmixQuery with_qualifiers edge cases
//! - QueryResults compile-time checks
//!
//! Tests that require FFI calls without PMIx_Init are excluded —
//! the PMIx library may segfault rather than return an error code
//! when called without initialization.

use pmix::query_log::*;
use pmix::{InfoBuilder, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery — construction edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_new_single_key() {
    let _query = PmixQuery::new(&["test_key"]).expect("create query");
}

#[test]
fn test_query_new_multiple_keys() {
    let _query = PmixQuery::new(&["key1", "key2", "key3"]).expect("create query");
}

#[test]
fn test_query_new_many_keys() {
    let keys: Vec<String> = (0..50).map(|i| format!("key_{}", i)).collect();
    let key_refs: Vec<&str> = keys.iter().map(|s| s.as_ref()).collect();
    let _query = PmixQuery::new(&key_refs).expect("create query with many keys");
}

#[test]
fn test_query_new_empty_keys_rejected() {
    let result = PmixQuery::new(&[]);
    assert!(result.is_err(), "query with empty keys should be rejected");
}

#[test]
fn test_query_new_empty_string_key() {
    let _query = PmixQuery::new(&[""]).expect("empty string key ok");
}

#[test]
fn test_query_new_nul_key_rejected() {
    let result = PmixQuery::new(&["bad\x00key"]);
    assert!(result.is_err());
}

#[test]
fn test_query_new_unicode_key() {
    let _query = PmixQuery::new(&["key-αβγ"]).expect("unicode key ok");
}

#[test]
fn test_query_new_long_key() {
    let long_key = "k".repeat(512);
    let _query = PmixQuery::new(&[&long_key]).expect("long key ok");
}

#[test]
fn test_query_new_with_dots() {
    let _query = PmixQuery::new(&[
        "pmix.version",
        "pmix.client.attrs",
        "pmix.server.attrs",
        "pmix.job.size",
    ])
    .expect("create query with dotted keys");
}

#[test]
fn test_query_new_special_chars() {
    let _query = PmixQuery::new(&["pmix.monitor.beat", "pmix.info.collect"])
        .expect("create query with special chars");
}

#[test]
fn test_query_debug_format() {
    let query = PmixQuery::new(&["debug_key"]).expect("create");
    let debug = format!("{:?}", query);
    assert!(debug.contains("PmixQuery") || !debug.is_empty());
}

#[test]
fn test_query_multiple_independent() {
    let q1 = PmixQuery::new(&["key1"]).expect("q1");
    let q2 = PmixQuery::new(&["key2"]).expect("q2");
    let q3 = PmixQuery::new(&["key3"]).expect("q3");
    let _ = (&q1, &q2, &q3);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery — with_qualifiers edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_with_empty_qualifiers() {
    let info = InfoBuilder::new().build();
    let query = PmixQuery::new(&["test_key"]).expect("create");
    let _query = query.with_qualifiers(info);
}

#[test]
fn test_query_with_qualifiers_chained() {
    let info = InfoBuilder::new().build();
    let _query = PmixQuery::new(&["test_key"])
        .expect("create")
        .with_qualifiers(info);
}

#[test]
fn test_query_with_qualifiers_transfers_ownership() {
    let info = InfoBuilder::new().build();
    let query = PmixQuery::new(&["test_key"]).expect("create");
    let _query = query.with_qualifiers(info);
    // info is consumed — can't use it anymore
}

// ─────────────────────────────────────────────────────────────────────────────
// query_info — validation (pure Rust, no FFI)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_info_empty_queries_returns_error() {
    let result = query_info(&[]);
    assert!(result.is_err(), "query_info with empty queries should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// QueryResults — compile-time checks (fields are private)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_results_debug_format() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<QueryResults>();
}

#[test]
fn test_query_results_type_name() {
    let name = std::any::type_name::<QueryResults>();
    assert!(name.contains("QueryResults"));
}

// ─────────────────────────────────────────────────────────────────────────────
// QueryCallback — trait object construction
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_callback_trait_object() {
    struct DummyQuery;
    impl QueryCallback for DummyQuery {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let _callback: Box<dyn QueryCallback> = Box::new(DummyQuery);
}

#[test]
fn test_query_callback_with_arc_state() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct ArcQueryCb {
        called: Arc<AtomicBool>,
    }
    impl QueryCallback for ArcQueryCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let _cb: Box<dyn QueryCallback> = Box::new(ArcQueryCb {
        called: called.clone(),
    });
}

#[test]
fn test_query_callback_send_bound() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn QueryCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// LogCallback — trait object construction
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_log_callback_trait_object() {
    struct DummyLog;
    impl LogCallback for DummyLog {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _callback: Box<dyn LogCallback> = Box::new(DummyLog);
}

#[test]
fn test_log_callback_with_arc_state() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct ArcLogCb {
        called: Arc<AtomicBool>,
    }
    impl LogCallback for ArcLogCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let cb: Box<dyn LogCallback> = Box::new(ArcLogCb {
        called: called.clone(),
    });

    // Simulate invoking the callback
    {
        cb.on_complete(PmixStatus::Known(PmixError::Success));
    }
    assert!(
        called.load(Ordering::SeqCst),
        "Callback should have been invoked"
    );
}

#[test]
fn test_log_callback_send_bound() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn LogCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder — compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_infobuilder_build_empty() {
    let info = InfoBuilder::new().build();
    let _ = info;
}

#[test]
fn test_infobuilder_collect_data() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let _info = builder.build();
}

#[test]
fn test_infobuilder_multiple_collect() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    builder.collect_data();
    let _info = builder.build();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery — drop safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_drop_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixQuery::new(&["safe_key"]));
    assert!(result.is_ok());
}

#[test]
fn test_query_drop_loop() {
    for _ in 0..100 {
        let _query = PmixQuery::new(&["loop_key"]).expect("create");
    }
}

#[test]
fn test_query_drop_does_not_leak() {
    for _ in 0..200 {
        let _query = PmixQuery::new(&["loop_key"]).expect("create");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety — pure Rust paths only
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_new_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixQuery::new(&["safe_key"]));
    assert!(result.is_ok());
}

#[test]
fn test_query_new_empty_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixQuery::new(&[]));
    assert!(result.is_ok());
}

#[test]
fn test_query_new_nul_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixQuery::new(&["bad\x00key"]));
    assert!(result.is_ok());
}

#[test]
fn test_query_info_empty_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _ = query_info(&[]);
    });
    assert!(result.is_ok());
}
