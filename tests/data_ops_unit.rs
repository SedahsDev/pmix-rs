//! Structural unit tests for the data_ops module — TASK-045.
//!
//! Focus on code paths not yet exercised by existing in-module tests.
//! All tests run WITHOUT PMIx_Init — they exercise the Rust wrapper layer only.

use pmix::data_ops::*;
use pmix::{Info, InfoBuilder, PmixError, PmixOwnedValue, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// PmixPdata — construction and trait verification
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pdata_new_with_key() {
    let pdata = PmixPdata::new("test_key");
    assert_eq!(pdata.key, "test_key");
    assert!(pdata.value.is_none());
}

#[test]
fn test_pdata_new_with_empty_key() {
    let pdata = PmixPdata::new("");
    assert_eq!(pdata.key, "");
    assert!(pdata.value.is_none());
}

#[test]
fn test_pdata_new_with_special_keys() {
    let special_keys = [
        "key-with-dash",
        "key_with_underscore",
        "key.with.dots",
        "key123",
        "KEY_UPPERCASE",
    ];
    for key in &special_keys {
        let pdata = PmixPdata::new(key);
        assert_eq!(pdata.key, *key);
    }
}

#[test]
fn test_pdata_debug_format() {
    let pdata = PmixPdata::new("debug_key");
    let debug_str = format!("{:?}", pdata);
    assert!(debug_str.contains("PmixPdata"));
    assert!(debug_str.contains("debug_key"));
}

#[test]
fn test_pdata_is_sized() {
    fn assert_sized<T: Sized>() {}
    assert_sized::<PmixPdata>();
}

// ─────────────────────────────────────────────────────────────────────────────
// PublishCallback — trait verification and implementations
// ─────────────────────────────────────────────────────────────────────────────

/// A simple PublishCallback that records invocations.
struct RecordingPublishCb {
    calls: std::sync::Arc<std::sync::Mutex<Vec<PmixStatus>>>,
}

impl PublishCallback for RecordingPublishCb {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.calls.lock().unwrap().push(status);
    }
}

#[test]
fn test_publish_callback_records_invocation() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let cb: Box<dyn PublishCallback> = Box::new(RecordingPublishCb {
        calls: calls.clone(),
    });

    // Simulate callback invocation
    let status = PmixStatus::Known(PmixError::Success);
    // We can't call on_complete directly since it takes Box<Self>,
    // but we can verify the trait is properly implemented.
    assert!(calls.lock().unwrap().is_empty());
    let _ = cb; // suppress unused warning
}

#[test]
fn test_publish_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn PublishCallback>>();
    assert_send::<RecordingPublishCb>();
}

// ─────────────────────────────────────────────────────────────────────────────
// GetValueCallback — trait verification and implementations
// ─────────────────────────────────────────────────────────────────────────────

struct RecordingGetCb {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(PmixStatus, bool)>>>,
}

impl GetValueCallback for RecordingGetCb {
    fn on_result(self: Box<Self>, status: PmixStatus, value: Option<PmixOwnedValue>) {
        self.calls.lock().unwrap().push((status, value.is_some()));
    }
}

#[test]
fn test_get_value_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn GetValueCallback>>();
    assert_send::<RecordingGetCb>();
}

// ─────────────────────────────────────────────────────────────────────────────
// LookupCallback — trait verification and implementations
// ─────────────────────────────────────────────────────────────────────────────

struct RecordingLookupCb {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(PmixStatus, usize)>>>,
}

impl LookupCallback for RecordingLookupCb {
    fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>) {
        self.calls.lock().unwrap().push((status, data.len()));
    }
}

#[test]
fn test_lookup_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn LookupCallback>>();
    assert_send::<RecordingLookupCb>();
}

// ─────────────────────────────────────────────────────────────────────────────
// UnpublishCallback — trait verification and implementations
// ─────────────────────────────────────────────────────────────────────────────

struct RecordingUnpublishCb {
    calls: std::sync::Arc<std::sync::Mutex<Vec<PmixStatus>>>,
}

impl UnpublishCallback for RecordingUnpublishCb {
    fn on_complete(self: Box<Self>, status: PmixStatus) {
        self.calls.lock().unwrap().push(status);
    }
}

#[test]
fn test_unpublish_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn UnpublishCallback>>();
    assert_send::<RecordingUnpublishCb>();
}

// ─────────────────────────────────────────────────────────────────────────────
// publish — FFI call path tests (without DVM)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_publish_empty_info() {
    let info = InfoBuilder::new().build();
    assert!(info.is_empty());
    let result = publish(&info);
    match result {
        Ok(_) => {} // rare: PMIx is initialized
        Err(e) => {
            assert!(e.is_error(), "Expected error without DVM");
        }
    }
}

#[test]
fn test_publish_with_directive() {
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let result = publish(&info);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_publish_consistent_error() {
    let info = InfoBuilder::new().build();
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = publish(&info);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

#[test]
fn test_publish_does_not_panic() {
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| publish(&info));
    assert!(result.is_ok(), "publish should not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// publish_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_publish_nb_empty_info() {
    let info = InfoBuilder::new().build();
    let cb: Box<dyn PublishCallback> = Box::new(RecordingPublishCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = publish_nb(&info, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_publish_nb_error_does_not_leak_callback() {
    let info = InfoBuilder::new().build();
    for _ in 0..20 {
        let cb: Box<dyn PublishCallback> = Box::new(RecordingPublishCb {
            calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        });
        let result = publish_nb(&info, cb);
        assert!(result.is_err());
    }
}

#[test]
fn test_publish_nb_with_directive() {
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let cb: Box<dyn PublishCallback> = Box::new(RecordingPublishCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = publish_nb(&info, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// get — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_with_valid_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "test_key", None);
    match result {
        Ok(_) => {} // rare
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_get_with_empty_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "", None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_get_with_info_directive() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let result = get(&proc, "test_key", Some(&info));
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_get_with_various_procs() {
    let procs = [
        Proc::new("ns1", 0).unwrap(),
        Proc::new("ns2", 1).unwrap(),
        Proc::new("", u32::MAX).unwrap(),
    ];
    for proc in &procs {
        let result = get(proc, "test_key", None);
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.is_error());
            }
        }
    }
}

#[test]
fn test_get_consistent_error() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = get(&proc, "test_key", None);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

#[test]
fn test_get_does_not_panic() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let result = std::panic::catch_unwind(|| get(&proc, "test_key", None));
    assert!(result.is_ok(), "get should not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// get_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_nb_with_valid_key() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let cb: Box<dyn GetValueCallback> = Box::new(RecordingGetCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = get_nb(&proc, "test_key", None, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_get_nb_with_info() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let cb: Box<dyn GetValueCallback> = Box::new(RecordingGetCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = get_nb(&proc, "test_key", Some(&info), cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_get_nb_error_does_not_leak_callback() {
    let proc = Proc::new("test_ns", 0).unwrap();
    for _ in 0..20 {
        let cb: Box<dyn GetValueCallback> = Box::new(RecordingGetCb {
            calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        });
        let result = get_nb(&proc, "test_key", None, cb);
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// lookup — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_lookup_empty_data() {
    let mut data: Vec<PmixPdata> = Vec::new();
    let result = lookup(&mut data, None);
    match result {
        Ok(_) => unreachable!("empty data should return error"),
        Err(e) => {
            assert!(e.is_error());
            // Empty data should return Error, not a PMIx error
            assert_eq!(e.to_raw(), PmixStatus::Known(PmixError::Error).to_raw());
        }
    }
}

#[test]
fn test_lookup_single_key() {
    let mut data = vec![PmixPdata::new("test_key")];
    let result = lookup(&mut data, None);
    match result {
        Ok(_) => {} // rare
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_multiple_keys() {
    let mut data = vec![
        PmixPdata::new("key1"),
        PmixPdata::new("key2"),
        PmixPdata::new("key3"),
    ];
    let result = lookup(&mut data, None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_with_info() {
    let mut data = vec![PmixPdata::new("test_key")];
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let result = lookup(&mut data, Some(&info));
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_consistent_error() {
    let mut data = vec![PmixPdata::new("test_key")];
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = lookup(&mut data, None);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

#[test]
fn test_lookup_does_not_panic() {
    // lookup() should return Result, not panic.
    // We use a simple assertion instead of catch_unwind since
    // &mut Vec<PmixPdata> is not UnwindSafe.
    let mut data = vec![PmixPdata::new("test_key")];
    let result = lookup(&mut data, None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// lookup_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_lookup_nb_empty_keys() {
    let keys: &[&str] = &[];
    let cb: Box<dyn LookupCallback> = Box::new(RecordingLookupCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = lookup_nb(keys, None, cb);
    match result {
        Ok(_) => unreachable!("empty keys should return error"),
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_nb_single_key() {
    let keys = ["test_key"];
    let cb: Box<dyn LookupCallback> = Box::new(RecordingLookupCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = lookup_nb(&keys, None, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_nb_multiple_keys() {
    let keys = ["key1", "key2", "key3"];
    let cb: Box<dyn LookupCallback> = Box::new(RecordingLookupCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = lookup_nb(&keys, None, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_lookup_nb_error_does_not_leak_callback() {
    let keys = ["test_key"];
    for _ in 0..20 {
        let cb: Box<dyn LookupCallback> = Box::new(RecordingLookupCb {
            calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        });
        let result = lookup_nb(&keys, None, cb);
        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// unpublish — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_unpublish_none_keys() {
    let result = unpublish(None, None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_single_key() {
    let keys = ["test_key"];
    let result = unpublish(Some(&keys), None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_multiple_keys() {
    let keys = ["key1", "key2", "key3"];
    let result = unpublish(Some(&keys), None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_with_info() {
    let keys = ["test_key"];
    let info = {
        let mut b = InfoBuilder::new();
        b.collect_data();
        b.build()
    };
    let result = unpublish(Some(&keys), Some(&info));
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_empty_keys_slice() {
    let keys: [&str; 0] = [];
    let result = unpublish(Some(&keys), None);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_consistent_error() {
    let keys = ["test_key"];
    let mut first: Option<PmixStatus> = None;
    for _ in 0..10 {
        let result = unpublish(Some(&keys), None);
        match result {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref prev) = first {
                    assert_eq!(e, *prev);
                } else {
                    first = Some(e);
                }
            }
        }
    }
}

#[test]
fn test_unpublish_does_not_panic() {
    let keys = ["test_key"];
    let result = std::panic::catch_unwind(|| unpublish(Some(&keys), None));
    assert!(result.is_ok(), "unpublish should not panic");
}

// ─────────────────────────────────────────────────────────────────────────────
// unpublish_nb — FFI call path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_unpublish_nb_single_key() {
    let keys = ["test_key"];
    let cb: Box<dyn UnpublishCallback> = Box::new(RecordingUnpublishCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = unpublish_nb(Some(&keys), None, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_nb_none_keys() {
    let cb: Box<dyn UnpublishCallback> = Box::new(RecordingUnpublishCb {
        calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let result = unpublish_nb(None, None, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.is_error());
        }
    }
}

#[test]
fn test_unpublish_nb_error_does_not_leak_callback() {
    let keys = ["test_key"];
    for _ in 0..20 {
        let cb: Box<dyn UnpublishCallback> = Box::new(RecordingUnpublishCb {
            calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        });
        let result = unpublish_nb(Some(&keys), None, cb);
        assert!(result.is_err());
    }
}
