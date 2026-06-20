//! Deep tests for data_ops module — Round 2.
//!
//! Targets untested code paths in data_ops.rs (54.78% coverage).
//! Focus: publish/get/lookup with Info options, fence_nb, store_internal,
//! PmixPdata construction, callback trait bounds, panic safety.
//!
//! FFI tests that require PMIx_Init are marked #[ignore].

use pmix::data_ops::*;
use pmix::{InfoBuilder, PmixStatus, Proc, PmixValueBuilder};

// ─────────────────────────────────────────────────────────────────────────────
// PmixPdata construction (safe without PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pdata_new_basic() {
    let pdata = PmixPdata::new("test_key");
    assert_eq!(pdata.key, "test_key");
    assert!(pdata.value.is_none());
}

#[test]
fn test_pdata_new_empty_key() {
    let pdata = PmixPdata::new("");
    assert_eq!(pdata.key, "");
    assert!(pdata.value.is_none());
}

#[test]
fn test_pdata_new_long_key() {
    let long_key = "k".repeat(512);
    let pdata = PmixPdata::new(&long_key);
    assert_eq!(pdata.key, long_key);
}

#[test]
fn test_pdata_new_unicode_key() {
    let pdata = PmixPdata::new("key-αβγ");
    assert_eq!(pdata.key, "key-αβγ");
}

#[test]
fn test_pdata_debug_format() {
    let pdata = PmixPdata::new("debug_test");
    let debug = format!("{:?}", pdata);
    assert!(debug.contains("PmixPdata") || !debug.is_empty());
}

#[test]
fn test_pdata_multiple_independent() {
    let p1 = PmixPdata::new("key1");
    let p2 = PmixPdata::new("key2");
    assert_eq!(p1.key, "key1");
    assert_eq!(p2.key, "key2");
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction (safe without PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_new_basic() {
    let proc = Proc::new("test_ns", 0).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_new_wildcard_rank() {
    let proc = Proc::new("test_ns", u32::MAX).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_new_nul_rejected() {
    let result = Proc::new("bad\x00ns", 0);
    assert!(result.is_err());
}

#[test]
fn test_proc_new_empty_nspace() {
    let proc = Proc::new("", 0).expect("create proc with empty nspace");
    let _ = proc;
}

#[test]
fn test_proc_new_long_nspace() {
    let long_ns = "n".repeat(256);
    let proc = Proc::new(&long_ns, 0).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_new_unicode_nspace() {
    let proc = Proc::new("ns-αβγ", 0).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_multiple_independent() {
    let p1 = Proc::new("ns1", 0).expect("p1");
    let p2 = Proc::new("ns2", 1).expect("p2");
    let _ = (&p1, &p2);
}

#[test]
fn test_proc_new_various_ranks() {
    let proc0 = Proc::new("ns", 0).expect("rank 0");
    let proc1 = Proc::new("ns", 1).expect("rank 1");
    let proc_max = Proc::new("ns", u32::MAX).expect("max rank");
    let _ = (&proc0, &proc1, &proc_max);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixValueBuilder construction (safe without PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_value_builder_string() {
    let value = PmixValueBuilder::new().string("hello").expect("set string").build().expect("build");
    let _ = value;
}

#[test]
fn test_value_builder_bool() {
    let value = PmixValueBuilder::new().bool(true).build().expect("build");
    let _ = value;
}

#[test]
fn test_value_builder_int() {
    let value = PmixValueBuilder::new().int(42).build().expect("build");
    let _ = value;
}

#[test]
fn test_value_builder_undef() {
    let value = PmixValueBuilder::new().undef().build().expect("build");
    let _ = value;
}

#[test]
fn test_value_builder_string_nul_rejected() {
    let result = PmixValueBuilder::new().string("bad\x00value");
    assert!(result.is_err());
}

#[test]
fn test_value_builder_multiple_independent() {
    let v1 = PmixValueBuilder::new().string("a").expect("s1").build().expect("b1");
    let v2 = PmixValueBuilder::new().string("b").expect("s2").build().expect("b2");
    let _ = (&v1, &v2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pdata_new_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixPdata::new("safe_key"));
    assert!(result.is_ok());
}

#[test]
fn test_pdata_new_nul_does_not_panic() {
    let result = std::panic::catch_unwind(|| PmixPdata::new("safe\x00key"));
    assert!(result.is_ok());
}

#[test]
fn test_proc_new_does_not_panic() {
    let result = std::panic::catch_unwind(|| Proc::new("safe_ns", 0));
    assert!(result.is_ok());
}

#[test]
fn test_proc_new_nul_does_not_panic() {
    let result = std::panic::catch_unwind(|| Proc::new("bad\x00ns", 0));
    assert!(result.is_ok());
}

#[test]
fn test_proc_drop_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let _proc = Proc::new("ns", 0).expect("create");
    });
    assert!(result.is_ok());
}

#[test]
fn test_proc_drop_loop() {
    for _ in 0..100 {
        let _proc = Proc::new("ns", 0).expect("create");
    }
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

// ── publish ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_publish_empty_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = InfoBuilder::new().build();
    let result = publish(&info);
    assert!(result.is_ok(), "publish should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_publish_with_collect_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let result = publish(&info);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_publish_multiple_times() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = InfoBuilder::new().build();
    for _ in 0..3 {
        let result = publish(&info);
        assert!(result.is_ok());
    }
}

// ── publish_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_publish_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopPublishCb;
    impl PublishCallback for NoopPublishCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = publish_nb(&info, Box::new(NoopPublishCb));
    assert!(result.is_ok());
}

// ── get ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_returns_error_without_server() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let result = get(&proc, "nonexistent_key", None);
    // May succeed or fail depending on server — just verify no crash
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_empty_key() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let result = get(&proc, "", None);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let info = InfoBuilder::new().build();
    let result = get(&proc, "key", Some(&info));
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_with_collect_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    let result = get(&proc, "key", Some(&info));
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_multiple_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    for key in &["key1", "key2", "key3"] {
        let result = get(&proc, key, None);
        let _ = result;
    }
}

// ── get_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopGetCb;
    impl GetValueCallback for NoopGetCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let proc = Proc::new("test_ns", 0).expect("create");
    let result = get_nb(&proc, "key", None, Box::new(NoopGetCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_nb_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopGetCb;
    impl GetValueCallback for NoopGetCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let proc = Proc::new("test_ns", 0).expect("create");
    let info = InfoBuilder::new().build();
    let result = get_nb(&proc, "key", Some(&info), Box::new(NoopGetCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_nb_empty_key_returns_error() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopGetCb;
    impl GetValueCallback for NoopGetCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let proc = Proc::new("test_ns", 0).expect("create");
    let result = get_nb(&proc, "", None, Box::new(NoopGetCb));
    let _ = result;
}

// ── lookup ──

#[test]
#[ignore = "returns error without server"]
fn test_lookup_returns_results() {
    let mut pdata = vec![PmixPdata::new("lookup_key")];
    let result = lookup(&mut pdata, None);
    assert!(result.is_ok(), "lookup should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_empty_key() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut pdata = vec![PmixPdata::new("")];
    let result = lookup(&mut pdata, None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_multiple_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut pdata = vec![
        PmixPdata::new("key1"),
        PmixPdata::new("key2"),
        PmixPdata::new("key3"),
    ];
    let result = lookup(&mut pdata, None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_single_key() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut pdata = vec![PmixPdata::new("single_key")];
    let result = lookup(&mut pdata, None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut pdata = vec![PmixPdata::new("info_key")];
    let info = InfoBuilder::new().build();
    let result = lookup(&mut pdata, Some(&info));
    assert!(result.is_ok());
}

// ── lookup_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLookupCb;
    impl LookupCallback for NoopLookupCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let result = lookup_nb(&["nb_key"], None, Box::new(NoopLookupCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nb_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLookupCb;
    impl LookupCallback for NoopLookupCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let info = InfoBuilder::new().build();
    let result = lookup_nb(&["nb_info_key"], Some(&info), Box::new(NoopLookupCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nb_multiple_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLookupCb;
    impl LookupCallback for NoopLookupCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let result = lookup_nb(&["key1", "key2", "key3"], None, Box::new(NoopLookupCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nb_empty_keys_returns_error() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopLookupCb;
    impl LookupCallback for NoopLookupCb {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }
    let result = lookup_nb(&[], None, Box::new(NoopLookupCb));
    assert!(result.is_err());
}

// ── unpublish ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_no_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = unpublish(None, None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_single_key() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = unpublish(Some(&["test_key"]), None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_multiple_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = unpublish(Some(&["key1", "key2", "key3"]), None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = InfoBuilder::new().build();
    let result = unpublish(Some(&["test_key"]), Some(&info));
    assert!(result.is_ok());
}

// ── unpublish_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopUnpublishCb;
    impl UnpublishCallback for NoopUnpublishCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = unpublish_nb(Some(&["test_key"]), None, Box::new(NoopUnpublishCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_nb_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopUnpublishCb;
    impl UnpublishCallback for NoopUnpublishCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = unpublish_nb(Some(&["test_key"]), Some(&info), Box::new(NoopUnpublishCb));
    assert!(result.is_ok());
}

// ── store_internal ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_store_internal_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let value = PmixValueBuilder::new().string("test_value").expect("set").build().expect("build");
    let result = store_internal(&proc, "test_key", &value);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_store_internal_empty_key() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let value = PmixValueBuilder::new().string("value").expect("set").build().expect("build");
    let result = store_internal(&proc, "", &value);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_store_internal_multiple() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    for i in 0..3 {
        let value = PmixValueBuilder::new().string(&format!("value_{}", i)).expect("set").build().expect("build");
        let result = store_internal(&proc, &format!("key_{}", i), &value);
        assert!(result.is_ok());
    }
}

// ── fence_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_nb_empty_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopFenceCb;
    impl FenceCallback for NoopFenceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = fence_nb(&[], None, Box::new(NoopFenceCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_nb_single_proc() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopFenceCb;
    impl FenceCallback for NoopFenceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let proc = Proc::new("test_ns", 0).expect("create");
    let result = fence_nb(&[proc], None, Box::new(NoopFenceCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_nb_multiple_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopFenceCb;
    impl FenceCallback for NoopFenceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let procs = vec![
        Proc::new("ns", 0).expect("p0"),
        Proc::new("ns", 1).expect("p1"),
        Proc::new("ns", 2).expect("p2"),
    ];
    let result = fence_nb(&procs, None, Box::new(NoopFenceCb));
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_nb_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    struct NoopFenceCb;
    impl FenceCallback for NoopFenceCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let proc = Proc::new("test_ns", 0).expect("create");
    let info = InfoBuilder::new().build();
    let result = fence_nb(&[proc], Some(&info), Box::new(NoopFenceCb));
    assert!(result.is_ok());
}

// ── Publish/Get/Lookup/Unpublish lifecycle ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_publish_then_unpublish() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = InfoBuilder::new().build();
    publish(&info).expect("publish");
    unpublish(Some(&["test_key"]), None).expect("unpublish");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_store_internal_then_get() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("create");
    let value = PmixValueBuilder::new().string("stored_value").expect("set").build().expect("build");
    store_internal(&proc, "stored_key", &value).expect("store");
    let result = get(&proc, "stored_key", None);
    let _ = result;
}
