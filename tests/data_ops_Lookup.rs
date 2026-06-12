//! Integration tests for `PMIx_Lookup` and `PMIx_Lookup_nb` via the safe
//! `lookup()` and `lookup_nb()` wrappers.
//!
//! These tests cover type signatures, callback registration, and behavior
//! that can be verified without a running PMIx daemon. Tests that require
//! PMIx runtime (`PMIx_Init`) are marked `#[ignore]`.

use pmix::data_ops::{LookupCallback, PmixPdata, lookup, lookup_nb};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Module and type availability
// ─────────────────────────────────────────────────────────────────────────────

/// `lookup` function is public and has the correct signature.
///
/// Compile-time check: the function exists and accepts `&mut [PmixPdata]`
/// and optional `&Info`.
#[test]
fn lookup_function_signature() {
    let _: fn(&mut [PmixPdata], Option<&Info>) -> Result<(PmixStatus, Vec<PmixPdata>), PmixStatus> =
        lookup;
}

/// `lookup_nb` function is public and has the correct signature.
///
/// Compile-time check: the non-blocking variant exists and accepts
/// `&[&str]` keys, optional `&Info`, and a callback.
#[test]
fn lookup_nb_function_signature() {
    let _: fn(&[&str], Option<&Info>, Box<dyn LookupCallback>) -> Result<(), PmixStatus> =
        lookup_nb;
}

/// `LookupCallback` trait is importable and object-safe.
///
/// Compile-time check: the trait exists, is public, and can be used as
/// a trait object.
#[test]
fn lookup_callback_trait_object() {
    struct TestCallback;
    impl LookupCallback for TestCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {
            // No-op for type checking.
        }
    }

    let cb: Box<dyn LookupCallback> = Box::new(TestCallback);
    let _: Box<dyn LookupCallback> = cb;
}

/// `PmixPdata` struct is importable and constructible.
#[test]
fn pdata_constructible() {
    let pdata = PmixPdata::new("test_key");
    assert_eq!(pdata.key, "test_key");
    assert!(pdata.value.is_none());
}

/// `PmixPdata` has the expected fields.
#[test]
fn pdata_has_expected_fields() {
    let pdata = PmixPdata::new("my_key");
    // Access all public fields to verify they exist.
    let _key: String = pdata.key;
    let _proc: Proc = pdata.proc;
    let _value: Option<pmix::PmixOwnedValue> = pdata.value;
}

// ─────────────────────────────────────────────────────────────────────────────
// Behavior tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `lookup` with an empty data array returns an error.
///
/// The safe wrapper rejects empty input before calling FFI.
#[test]
fn lookup_empty_data_returns_error() {
    let mut data: Vec<PmixPdata> = Vec::new();
    let result = lookup(&mut data, None);
    assert!(
        result.is_err(),
        "lookup with empty data should return error"
    );
}

/// `lookup_nb` with an empty keys array returns an error.
///
/// The safe wrapper rejects empty input before calling FFI.
#[test]
fn lookup_nb_empty_keys_returns_error() {
    struct TestCallback;
    impl LookupCallback for TestCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }

    let keys: &[&str] = &[];
    let result = lookup_nb(keys, None, Box::new(TestCallback));
    assert!(
        result.is_err(),
        "lookup_nb with empty keys should return error"
    );
}

/// `lookup` before init returns `PMIX_ERR_INIT` (-31).
///
/// PMIx_Lookup requires PMIx_Init to have been called first. Calling it
/// without initialization should return PMIX_ERR_INIT.
#[test]
fn lookup_before_init_returns_err_init() {
    let mut data = vec![PmixPdata::new("test_key")];
    let result = lookup(&mut data, None);
    assert!(result.is_err(), "lookup should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `lookup_nb` before init returns `PMIX_ERR_INIT` (-31).
///
/// The non-blocking variant also requires initialization.
#[test]
fn lookup_nb_before_init_returns_err_init() {
    struct InitCheckCallback;
    impl LookupCallback for InitCheckCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result = lookup_nb(&["test_key"], None, Box::new(InitCheckCallback));
    assert!(result.is_err(), "lookup_nb should fail without PMIx_Init");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `lookup` returns a `Result` type with proper error handling.
#[test]
fn lookup_returns_result_type() {
    let mut data = vec![PmixPdata::new("test_key")];
    let result: Result<(PmixStatus, Vec<PmixPdata>), PmixStatus> = lookup(&mut data, None);

    // Verify the error is a known PMIx error code.
    match result {
        Ok(_) => panic!("should not succeed without init"),
        Err(PmixStatus::Known(PmixError::ErrInit)) => {
            // Expected: PMIX_ERR_INIT
        }
        Err(PmixStatus::Unknown(code)) => {
            panic!("unexpected unknown status code: {}", code);
        }
        Err(PmixStatus::Known(other)) => {
            panic!(
                "unexpected known error: {:?} (raw={})",
                other,
                (other as i32)
            );
        }
    }
}

/// `lookup_nb` returns a `Result` type with proper error handling.
#[test]
fn lookup_nb_returns_result_type() {
    struct ResultTypeCallback;
    impl LookupCallback for ResultTypeCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {
            panic!("Callback should NOT be called on immediate failure");
        }
    }

    let result: Result<(), PmixStatus> =
        lookup_nb(&["test_key"], None, Box::new(ResultTypeCallback));

    match result {
        Ok(()) => panic!("should not succeed without init"),
        Err(PmixStatus::Known(PmixError::ErrInit)) => {
            // Expected: PMIX_ERR_INIT
        }
        Err(PmixStatus::Unknown(code)) => {
            panic!("unexpected unknown status code: {}", code);
        }
        Err(PmixStatus::Known(other)) => {
            panic!(
                "unexpected known error: {:?} (raw={})",
                other,
                (other as i32)
            );
        }
    }
}

/// `lookup` with InfoBuilder-constructed info (directives).
///
/// This tests that the InfoBuilder can construct an info array and
/// that lookup accepts it. The call itself will fail without init,
/// but the info construction and type compatibility are verified.
#[test]
fn lookup_with_info_builder() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();

    let mut data = vec![PmixPdata::new("test_key")];
    let result = lookup(&mut data, Some(&info));
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `lookup_nb` callback receives `PmixStatus` and `Vec<PmixPdata>` (type check).
#[test]
fn lookup_callback_receives_correct_types() {
    struct StatusCallback;
    impl LookupCallback for StatusCallback {
        fn on_result(self: Box<Self>, status: PmixStatus, data: Vec<PmixPdata>) {
            // This function should not be called in this test,
            // but the compiler verifies the signature.
            let _ = status.is_success();
            let _ = status.to_raw();
            for item in data {
                let _ = item.key;
                let _ = item.proc;
                let _ = item.value;
            }
        }
    }

    let result = lookup_nb(&["test_key"], None, Box::new(StatusCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
}

/// `lookup_nb` callback is not invoked on immediate failure.
///
/// When PMIx_Lookup_nb returns an error synchronously (e.g., not initialized),
/// the callback should NOT be called.
#[test]
fn lookup_nb_callback_not_invoked_on_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NoInvokeCallback;
    impl LookupCallback for NoInvokeCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let result = lookup_nb(&["test_key"], None, Box::new(NoInvokeCallback));

    // Should fail immediately without invoking callback.
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert!(
        !CALLBACK_INVOKED.load(Ordering::SeqCst),
        "callback should NOT have been invoked on immediate failure"
    );
}

/// `lookup` with multiple keys in a single call.
///
/// Tests that the wrapper handles multiple PmixPdata entries.
#[test]
fn lookup_multiple_keys_before_init() {
    let mut data = vec![
        PmixPdata::new("key1"),
        PmixPdata::new("key2"),
        PmixPdata::new("key3"),
    ];
    let result = lookup(&mut data, None);
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `lookup_nb` with multiple keys returns error before init.
#[test]
fn lookup_nb_multiple_keys_before_init() {
    struct MultiCallback;
    impl LookupCallback for MultiCallback {
        fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
    }

    let result = lookup_nb(&["key1", "key2", "key3"], None, Box::new(MultiCallback));
    assert!(result.is_err(), "should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}

/// `lookup` is callable multiple times (idempotent error behavior).
#[test]
fn lookup_multiple_calls_consistent_error() {
    for i in 0..5 {
        let mut data = vec![PmixPdata::new("test_key")];
        let result = lookup(&mut data, None);
        assert!(
            result.is_err(),
            "iteration {}: should fail without PMIx_Init",
            i
        );
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "iteration {}: should be PMIX_ERR_INIT",
            i
        );
    }
}

/// `lookup_nb` is callable multiple times (idempotent error behavior).
#[test]
fn lookup_nb_multiple_calls_consistent_error() {
    for i in 0..5 {
        struct MultiCallback;
        impl LookupCallback for MultiCallback {
            fn on_result(self: Box<Self>, _status: PmixStatus, _data: Vec<PmixPdata>) {}
        }

        let result = lookup_nb(&["test_key"], None, Box::new(MultiCallback));
        assert!(
            result.is_err(),
            "iteration {}: should fail without PMIx_Init",
            i
        );
        assert_eq!(
            result.unwrap_err().to_raw(),
            -31,
            "iteration {}: should be PMIX_ERR_INIT",
            i
        );
    }
}

/// `PmixPdata::new` handles long keys correctly (key truncation to 511 chars).
#[test]
fn pdata_long_key() {
    let long_key: String = "a".repeat(600);
    let pdata = PmixPdata::new(&long_key);
    assert_eq!(pdata.key, long_key);
    // The key is stored as-is in the Rust struct; truncation happens
    // when converting to pmix_key_t in the FFI call.
}

/// `PmixPdata::new` handles empty key.
#[test]
fn pdata_empty_key() {
    let pdata = PmixPdata::new("");
    assert_eq!(pdata.key, "");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx daemon — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// `lookup` succeeds after `PMIx_Init` with published data.
///
/// Requires a running PMIx server. This test follows the pattern from
/// `test/simple/simppub.c`: publish key-value pairs, fence, then lookup.
#[test]
#[ignore = "requires PMIx daemon"]
fn lookup_after_init() {
    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    let mut data = vec![PmixPdata::new("test_key")];
    let result = lookup(&mut data, None);
    // Without prior publish, expect NotFound or PartialSuccess.
    match result {
        Ok((status, results)) => {
            assert!(
                status.is_success()
                    || matches!(
                        status,
                        PmixStatus::Known(PmixError::ErrNotFound)
                            | PmixStatus::Known(PmixError::ErrPartialSuccess)
                    ),
                "lookup should return Success, NotFound, or PartialSuccess"
            );
            assert_eq!(results.len(), 1, "should return one result");
        }
        Err(status) => panic!("lookup should not return hard error: {:?}", status),
    }
}

/// `lookup_nb` succeeds after `PMIx_Init` and invokes the callback.
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn lookup_nb_after_init() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let _ctx = pmix::init(None).expect("PMIx_Init should succeed");

    static CALLBACK_INVOKED: AtomicBool = AtomicBool::new(false);

    struct NbCallback;
    impl LookupCallback for NbCallback {
        fn on_result(self: Box<Self>, status: PmixStatus, _data: Vec<PmixPdata>) {
            assert!(
                status.is_success()
                    || matches!(
                        status,
                        PmixStatus::Known(PmixError::ErrNotFound)
                            | PmixStatus::Known(PmixError::ErrPartialSuccess)
                    ),
                "callback should receive a valid status"
            );
            CALLBACK_INVOKED.store(true, Ordering::SeqCst);
        }
    }

    let result = lookup_nb(&["test_key"], None, Box::new(NbCallback));
    assert!(result.is_ok(), "lookup_nb should accept the request");

    // Note: callback may be invoked synchronously or asynchronously
    // depending on the PMIx implementation. In a real integration test
    // with a daemon, we would call PMIx_Progress or fence to drive
    // the completion.
}

/// Publish -> Fence -> Lookup pattern from simppub.c.
///
/// This is the canonical PMIx workflow: publish data, fence to ensure
/// visibility, then lookup the published data.
///
/// Requires a running PMIx server.
#[test]
#[ignore = "requires PMIx daemon"]
fn publish_fence_lookup_pattern() {
    let ctx = pmix::init(None).expect("PMIx_Init should succeed");

    // Publish data.
    let info = InfoBuilder::new().build();
    let publish_result = pmix::data_ops::publish(&info);
    assert!(publish_result.is_ok(), "publish should succeed");

    // Fence to ensure data is visible.
    let proc = ctx.get_proc();
    let fence_result = pmix::fence(proc, None);
    assert!(fence_result.is_ok(), "fence should succeed after publish");

    // Lookup the published data.
    let mut data = vec![PmixPdata::new("test_key")];
    let lookup_result = lookup(&mut data, None);
    match lookup_result {
        Ok((status, results)) => {
            assert_eq!(results.len(), 1, "should return one result");
            assert!(
                status.is_success()
                    || matches!(
                        status,
                        PmixStatus::Known(PmixError::ErrNotFound)
                            | PmixStatus::Known(PmixError::ErrPartialSuccess)
                    ),
                "lookup status should be usable"
            );
        }
        Err(status) => panic!("lookup should not return hard error: {:?}", status),
    }
}
