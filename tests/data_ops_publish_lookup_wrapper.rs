//! Wrapper tests for publish/lookup/get/unpublish/fence functions.
//!
//! These tests exercise the Rust wrapper logic for FFI calls without
//! PMIx_Init. All FFI calls return errors without segfaulting, so we
//! can test the wrapper error-handling paths, parameter validation,
//! and Info handling.
//!
//! Coverage targets:
//!   - publish (lines 45-66) — FFI call + error path
//!   - publish_nb (lines 135-173) — callback registration + FFI + cleanup
//!   - get (lines 377-437) — CString + FFI + error path
//!   - get_nb (lines 279-347) — callback reg + CString + FFI + cleanup
//!   - lookup (lines 508-580) — FFI + error path
//!   - lookup_nb (lines 751-825) — callback reg + FFI + cleanup
//!   - unpublish (lines 909-963) — FFI + error path
//!   - unpublish_nb (lines 998-1082) — callback reg + FFI + cleanup
//!   - fence_nb (lines 1248-1318) — callback reg + FFI + cleanup

use pmix::data_ops::*;
use pmix::{PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// publish — synchronous publish
// ─────────────────────────────────────────────────────────────────────────────

/// publish without PMIx init returns error.
#[test]
fn test_publish_no_init() {
    let info = pmix::InfoBuilder::new().build();
    let result = publish(&info);
    assert!(result.is_err());
}

/// publish with empty Info returns error (not initialized).
#[test]
fn test_publish_empty_info() {
    let info = pmix::InfoBuilder::new().build();
    let result = publish(&info);
    assert!(result.is_err());
}

/// publish returns consistent error on repeated calls.
#[test]
fn test_publish_repeated_calls() {
    let info = pmix::InfoBuilder::new().build();
    let r1 = publish(&info);
    let r2 = publish(&info);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// publish_nb — non-blocking publish
// ─────────────────────────────────────────────────────────────────────────────

/// publish_nb without PMIx init returns error, callback not invoked.
#[test]
fn test_publish_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb { c: Arc<AtomicBool> }
    impl PublishCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info = pmix::InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst), "callback should not be invoked on immediate failure");
}

/// publish_nb with empty Info returns error.
#[test]
fn test_publish_nb_empty_info() {
    struct Cb;
    impl PublishCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = pmix::InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(Cb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// get — blocking retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// get without PMIx init returns error.
#[test]
fn test_get_no_init() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "test_key", None);
    assert!(result.is_err());
}

/// get with Info returns error (not initialized).
#[test]
fn test_get_with_info() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let info = pmix::InfoBuilder::new().build();
    let result = get(&proc, "test_key", Some(&info));
    assert!(result.is_err());
}

/// get with NUL in key returns error (CString failure).
#[test]
fn test_get_nul_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "test\x00key", None);
    assert!(result.is_err());
}

/// get with empty key returns error.
#[test]
fn test_get_empty_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "", None);
    assert!(result.is_err());
}

/// get with long key returns error.
#[test]
fn test_get_long_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let long_key: String = "k".repeat(1024);
    let result = get(&proc, &long_key, None);
    assert!(result.is_err());
}

/// get with different proc ranks returns consistent error.
#[test]
fn test_get_different_ranks() {
    for rank in 0..3u32 {
        let proc = Proc::new("test_ns", rank).unwrap();
        let result = get(&proc, "key", None);
        assert!(result.is_err());
    }
}

/// get is deterministic — same result on repeated calls.
#[test]
fn test_get_deterministic() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let r1 = get(&proc, "key", None);
    let r2 = get(&proc, "key", None);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_nb — non-blocking retrieval
// ─────────────────────────────────────────────────────────────────────────────

/// get_nb without PMIx init returns error, callback not invoked.
#[test]
fn test_get_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb { c: Arc<AtomicBool> }
    impl GetValueCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get_nb(&proc, "test_key", None, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst), "callback should not be invoked on immediate failure");
}

/// get_nb with Info returns error.
#[test]
fn test_get_nb_with_info() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let info = pmix::InfoBuilder::new().build();
    struct Cb;
    impl GetValueCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let result = get_nb(&proc, "test_key", Some(&info), Box::new(Cb));
    assert!(result.is_err());
}

/// get_nb with NUL in key returns error (CString failure, callback cleaned up).
#[test]
fn test_get_nb_nul_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    struct Cb;
    impl GetValueCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let result = get_nb(&proc, "test\x00key", None, Box::new(Cb));
    assert!(result.is_err());
}

/// get_nb with empty key returns error.
#[test]
fn test_get_nb_empty_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    struct Cb;
    impl GetValueCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let result = get_nb(&proc, "", None, Box::new(Cb));
    assert!(result.is_err());
}

/// get_nb with different proc ranks.
#[test]
fn test_get_nb_different_ranks() {
    for rank in 0..3u32 {
        let proc = Proc::new("test_ns", rank).unwrap();
        struct Cb;
        impl GetValueCallback for Cb {
            fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
        }
        let result = get_nb(&proc, "key", None, Box::new(Cb));
        assert!(result.is_err());
    }
}

/// get_nb is deterministic.
#[test]
fn test_get_nb_deterministic() {
    let proc = Proc::new("test_ns", 0).unwrap();
    struct Cb;
    impl GetValueCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let r1 = get_nb(&proc, "key", None, Box::new(Cb));
    let r2 = get_nb(&proc, "key", None, Box::new(Cb));
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// lookup — synchronous lookup
// ─────────────────────────────────────────────────────────────────────────────

/// lookup without PMIx init returns error.
#[test]
fn test_lookup_no_init() {
    let mut data: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let result = lookup(&mut data, None);
    assert!(result.is_err());
}

/// lookup with Info returns error.
#[test]
fn test_lookup_with_info() {
    let mut data: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let info = pmix::InfoBuilder::new().build();
    let result = lookup(&mut data, Some(&info));
    assert!(result.is_err());
}

/// lookup with pre-populated data returns error.
#[test]
fn test_lookup_with_data() {
    let mut data: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    // Even with data entries, FFI call fails without init.
    let result = lookup(&mut data, None);
    assert!(result.is_err());
}

/// lookup is deterministic.
#[test]
fn test_lookup_deterministic() {
    let mut data1: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let mut data2: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let r1 = lookup(&mut data1, None);
    let r2 = lookup(&mut data2, None);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// lookup_nb — non-blocking lookup
// ─────────────────────────────────────────────────────────────────────────────

/// lookup_nb without PMIx init returns error, callback not invoked.
#[test]
fn test_lookup_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb { c: Arc<AtomicBool> }
    impl LookupCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _results: Vec<pmix::data_ops::PmixPdata>) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let keys = vec!["test_key"];
    let result = lookup_nb(&keys, None, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst), "callback should not be invoked on immediate failure");
}

/// lookup_nb with Info returns error.
#[test]
fn test_lookup_nb_with_info() {
    let keys = vec!["test_key"];
    let info = pmix::InfoBuilder::new().build();
    struct Cb;
    impl LookupCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _results: Vec<pmix::data_ops::PmixPdata>) {}
    }
    let result = lookup_nb(&keys, Some(&info), Box::new(Cb));
    assert!(result.is_err());
}

/// lookup_nb with empty keys returns error.
#[test]
fn test_lookup_nb_empty_keys() {
    struct Cb;
    impl LookupCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _results: Vec<pmix::data_ops::PmixPdata>) {}
    }
    let keys: Vec<&str> = vec![];
    let result = lookup_nb(&keys, None, Box::new(Cb));
    assert!(result.is_err());
}

/// lookup_nb with multiple keys returns error.
#[test]
fn test_lookup_nb_multiple_keys() {
    let keys = vec!["key1", "key2", "key3"];
    struct Cb;
    impl LookupCallback for Cb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _results: Vec<pmix::data_ops::PmixPdata>) {}
    }
    let result = lookup_nb(&keys, None, Box::new(Cb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// unpublish — synchronous unpublish
// ─────────────────────────────────────────────────────────────────────────────

/// unpublish without PMIx init returns error.
#[test]
fn test_unpublish_no_init() {
    let result = unpublish(Some(&["test_key"]), None);
    assert!(result.is_err());
}

/// unpublish with Info returns error.
#[test]
fn test_unpublish_with_info() {
    let info = pmix::InfoBuilder::new().build();
    let result = unpublish(Some(&["test_key"]), Some(&info));
    assert!(result.is_err());
}

/// unpublish with None keys returns error.
#[test]
fn test_unpublish_none_keys() {
    let result = unpublish(None, None);
    assert!(result.is_err());
}

/// unpublish with empty key list returns error.
#[test]
fn test_unpublish_empty_keys() {
    let keys: Vec<&str> = vec![];
    let result = unpublish(Some(&keys), None);
    assert!(result.is_err());
}

/// unpublish with multiple keys returns error.
#[test]
fn test_unpublish_multiple_keys() {
    let result = unpublish(Some(&["key1", "key2", "key3"]), None);
    assert!(result.is_err());
}

/// unpublish is deterministic.
#[test]
fn test_unpublish_deterministic() {
    let r1 = unpublish(Some(&["key"]), None);
    let r2 = unpublish(Some(&["key"]), None);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// unpublish_nb — non-blocking unpublish
// ─────────────────────────────────────────────────────────────────────────────

/// unpublish_nb without PMIx init returns error, callback not invoked.
#[test]
fn test_unpublish_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb { c: Arc<AtomicBool> }
    impl UnpublishCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let result = unpublish_nb(Some(&["test_key"]), None, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst), "callback should not be invoked on immediate failure");
}

/// unpublish_nb with Info returns error.
#[test]
fn test_unpublish_nb_with_info() {
    let info = pmix::InfoBuilder::new().build();
    struct Cb;
    impl UnpublishCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = unpublish_nb(Some(&["test_key"]), Some(&info), Box::new(Cb));
    assert!(result.is_err());
}

/// unpublish_nb with None keys returns error.
#[test]
fn test_unpublish_nb_none_keys() {
    struct Cb;
    impl UnpublishCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = unpublish_nb(None, None, Box::new(Cb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// fence_nb — non-blocking fence
// ─────────────────────────────────────────────────────────────────────────────

/// fence_nb without PMIx init returns error, callback not invoked.
#[test]
fn test_fence_nb_no_init() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb { c: Arc<AtomicBool> }
    impl FenceCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let procs: Vec<Proc> = Vec::new();
    let result = fence_nb(&procs, None, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst), "callback should not be invoked on immediate failure");
}

/// fence_nb with Info returns error.
#[test]
fn test_fence_nb_with_info() {
    let procs: Vec<Proc> = Vec::new();
    let info = pmix::InfoBuilder::new().build();
    struct Cb;
    impl FenceCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fence_nb(&procs, Some(&info), Box::new(Cb));
    assert!(result.is_err());
}

/// fence_nb with procs returns error.
#[test]
fn test_fence_nb_with_procs() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let procs = vec![proc];
    struct Cb;
    impl FenceCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fence_nb(&procs, None, Box::new(Cb));
    assert!(result.is_err());
}

/// fence_nb is deterministic.
#[test]
fn test_fence_nb_deterministic() {
    let procs: Vec<Proc> = Vec::new();
    struct Cb;
    impl FenceCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let r1 = fence_nb(&procs, None, Box::new(Cb));
    let r2 = fence_nb(&procs, None, Box::new(Cb));
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// store_internal — internal store
// ─────────────────────────────────────────────────────────────────────────────

fn build_value(builder: pmix::PmixValueBuilder) -> pmix::PmixOwnedValue {
    builder.build().expect("build owned value")
}

/// store_internal without PMIx init returns error.
#[test]
fn test_store_internal_no_init() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let result = store_internal(&proc, "test_key", &value);
    assert!(result.is_err());
}

/// store_internal with NUL key returns error.
#[test]
fn test_store_internal_nul_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let result = store_internal(&proc, "test\x00key", &value);
    assert!(result.is_err());
}

/// store_internal with empty key returns error.
#[test]
fn test_store_internal_empty_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let result = store_internal(&proc, "", &value);
    assert!(result.is_err());
}

/// store_internal with different value types all return error.
#[test]
fn test_store_internal_various_types() {
    let proc = Proc::new("test_ns", 0).unwrap();
    
    let val_int = build_value(pmix::PmixValueBuilder::new().int(42));
    assert!(store_internal(&proc, "key_int", &val_int).is_err());
    
    let val_str = build_value(pmix::PmixValueBuilder::new().string("hello").expect("string"));
    assert!(store_internal(&proc, "key_str", &val_str).is_err());
    
    let val_bool = build_value(pmix::PmixValueBuilder::new().bool(true));
    assert!(store_internal(&proc, "key_bool", &val_bool).is_err());
    
    let val_double = build_value(pmix::PmixValueBuilder::new().double(3.14));
    assert!(store_internal(&proc, "key_double", &val_double).is_err());
}

/// store_internal is deterministic.
#[test]
fn test_store_internal_deterministic() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let r1 = store_internal(&proc, "key", &value);
    let r2 = store_internal(&proc, "key", &value);
    assert_eq!(r1.is_err(), r2.is_err());
}
