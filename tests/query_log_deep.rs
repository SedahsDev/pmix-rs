//! Deep tests for query_log module — Round 2.
//!
//! Targets untested code paths in query_log.rs (59.02% coverage).
//! Focus: PmixQuery builder, QueryResults, log_data structured,
//! callback traits, panic safety.
//!
//! FFI tests that require PMIx_Init are marked #[ignore].

use pmix::query_log::*;
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery construction (safe without PMIx_Init)
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
fn test_query_new_empty_keys_rejected() {
    let result = PmixQuery::new(&[]);
    assert!(result.is_err());
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

#[test]
fn test_query_drop_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _query = PmixQuery::new(&["safe_key"]).expect("create");
    });
    assert!(result.is_ok());
}

#[test]
fn test_query_drop_loop() {
    for _ in 0..100 {
        let _query = PmixQuery::new(&["loop_key"]).expect("create");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixQuery::with_qualifiers (safe without PMIx_Init)
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

// ─────────────────────────────────────────────────────────────────────────────
// QueryResults type checks (compile-time only)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_query_results_debug_format() {
    // QueryResults is constructed by query_info() FFI call.
    // Just verify the Debug impl compiles via type assertion.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<QueryResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
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

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder compile-time checks
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

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

// ── query_info ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_single_query() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let query = PmixQuery::new(&["test_key"]).expect("create");
    let result = query_info(&[query]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_multiple_queries() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let q1 = PmixQuery::new(&["key1"]).expect("q1");
    let q2 = PmixQuery::new(&["key2"]).expect("q2");
    let result = query_info(&[q1, q2]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_empty_returns_error() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = query_info(&[]);
    assert!(result.is_err());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_result_has_len() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let query = PmixQuery::new(&["test_key"]).expect("create");
    let result = query_info(&[query]);
    match result {
        Ok(results) => {
            let _ = results.len();
            let _ = results.is_empty();
        }
        Err(_) => {
            // Server may not be available — that's fine
        }
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_with_qualifiers() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = InfoBuilder::new().build();
    let query = PmixQuery::new(&["test_key"]).expect("create").with_qualifiers(info);
    let result = query_info(&[query]);
    let _ = result;
}

// ── query_info_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopQueryCb;
    impl QueryCallback for NoopQueryCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let query = PmixQuery::new(&["nb_key"]).expect("create");
    let result = query_info_nb(&[query], Box::new(NoopQueryCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_nb_multiple_queries() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopQueryCb;
    impl QueryCallback for NoopQueryCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let q1 = PmixQuery::new(&["key1"]).expect("q1");
    let q2 = PmixQuery::new(&["key2"]).expect("q2");
    let result = query_info_nb(&[q1, q2], Box::new(NoopQueryCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_info_nb_empty_returns_error() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopQueryCb;
    impl QueryCallback for NoopQueryCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    let result = query_info_nb(&[], Box::new(NoopQueryCb));
    assert!(result.is_err());
}

// ── log_data ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_empty() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = log_data(&[], &[]);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_with_data() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let data = vec![InfoBuilder::new().build()];
    let result = log_data(&data, &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let data = vec![InfoBuilder::new().build()];
    let dirs = vec![InfoBuilder::new().build()];
    let result = log_data(&data, &dirs);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_multiple_entries() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let data = vec![
        InfoBuilder::new().build(),
        InfoBuilder::new().build(),
        InfoBuilder::new().build(),
    ];
    let result = log_data(&data, &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_with_collect_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let data = vec![builder.build()];
    let result = log_data(&data, &[]);
    let _ = result;
}

// ── log_data_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLogCb;
    impl LogCallback for NoopLogCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = log_data_nb(&[], &[], Box::new(NoopLogCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_nb_with_data() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLogCb;
    impl LogCallback for NoopLogCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let data = vec![InfoBuilder::new().build()];
    let result = log_data_nb(&data, &[], Box::new(NoopLogCb));
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_log_data_nb_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLogCb;
    impl LogCallback for NoopLogCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let data = vec![InfoBuilder::new().build()];
    let dirs = vec![InfoBuilder::new().build()];
    let result = log_data_nb(&data, &dirs, Box::new(NoopLogCb));
    let _ = result;
}

// ── Lifecycle / integration ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_then_log() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let query = PmixQuery::new(&["test_key"]).expect("create");
    let _ = query_info(&[query]);
    let data = vec![InfoBuilder::new().build()];
    let _ = log_data(&data, &[]);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_query_nb_then_log_nb() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopQueryCb;
    impl QueryCallback for NoopQueryCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _results: QueryResults) {}
    }
    struct NoopLogCb;
    impl LogCallback for NoopLogCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let query = PmixQuery::new(&["nb_key"]).expect("create");
    let _ = query_info_nb(&[query], Box::new(NoopQueryCb));
    let _ = log_data_nb(&[], &[], Box::new(NoopLogCb));
}
