//! Integration tests for `PMIx_Store_internal` via the safe `store_internal()` wrapper.
//!
//! `PMIx_Store_internal` stores data locally with internal scope — it will
//! never be "pushed" externally. Most tests require PMIx to be initialized
//! (via `PMIx_Init`), which requires a running PMIx daemon. Tests that need
//! a daemon are marked `#[ignore]`.

use pmix::{PmixStatus, Proc, data_ops::store_internal};

// ─────────────────────────────────────────────────────────────────────────────
// Helper to build a PmixOwnedValue from a builder
// ─────────────────────────────────────────────────────────────────────────────

fn build_value(builder: pmix::PmixValueBuilder) -> pmix::PmixOwnedValue {
    builder.build().expect("build owned value")
}

// ─────────────────────────────────────────────────────────────────────────────
// API surface tests (no PMIx daemon required)
// ─────────────────────────────────────────────────────────────────────────────

/// `store_internal` returns `Err(PMIX_ERR_INIT)` when PMIx is not initialized.
///
/// The C implementation checks `pmix_globals.init_cntr <= 0` and returns
/// `PMIX_ERR_INIT` (-31) if PMIx has not been initialized.
#[test]
fn store_internal_not_initialized_returns_err_init() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let value = build_value(
        pmix::PmixValueBuilder::new()
            .string("test_value")
            .expect("string value"),
    );

    let result = store_internal(&proc, "test_key", &value);
    assert!(
        result.is_err(),
        "store_internal should fail when PMIx is not initialized, got {:?}",
        result
    );
    let err = result.unwrap_err();
    assert_eq!(
        err,
        PmixStatus::from_raw(-31), // PMIX_ERR_INIT
        "Expected PMIX_ERR_INIT, got {:?}",
        err
    );
}

/// `store_internal` has the correct function signature.
///
/// Compile-time type check: `store_internal` takes `&Proc`, `&str`,
/// `&PmixOwnedValue` and returns `Result<(), PmixStatus>`.
#[test]
fn store_internal_signature_check() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let _result: Result<(), PmixStatus> = store_internal(&proc, "key", &value);
}

// ─────────────────────────────────────────────────────────────────────────────
// Value type tests (all require PMIx_Init → ignored)
// ─────────────────────────────────────────────────────────────────────────────

/// Store a string value internally.
///
/// Derived from `test/test_internal.c` which stores string values and later
/// retrieves them via `PMIx_Get`. Requires PMIx_Init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_string_value() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(
        pmix::PmixValueBuilder::new()
            .string("test_internal:test_namespace:0:0")
            .expect("string value"),
    );
    let result = store_internal(&proc, "test_key:0", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

/// Store an integer value internally.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_int_value() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let result = store_internal(&proc, "test_int_key", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

/// Store a bool value internally.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_bool_value() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().bool(true));
    let result = store_internal(&proc, "test_bool_key", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

/// Store a uint64 value internally.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_uint64_value() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().uint64(999_999));
    let result = store_internal(&proc, "test_uint64_key", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

/// Store a double value internally.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_double_value() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().double(3.14159));
    let result = store_internal(&proc, "test_double_key", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge case tests
// ─────────────────────────────────────────────────────────────────────────────

/// Store with a key containing NUL bytes should return an error.
///
/// PMIx keys must be valid C strings (no interior NUL).
#[test]
fn store_internal_key_with_nul_returns_error() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    // Key with embedded NUL — CString::new will fail, function returns Error.
    let result = store_internal(&proc, "test\0key", &value);
    assert!(
        result.is_err(),
        "store_internal with NUL in key should fail, got {:?}",
        result
    );
}

/// Store with an empty key should fail (not initialized, or BAD_PARAM with init).
#[test]
fn store_internal_empty_key() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let result = store_internal(&proc, "", &value);
    // Without PMIx init, this will return ERR_INIT regardless of key.
    assert!(
        result.is_err(),
        "store_internal should fail (not initialized), got {:?}",
        result
    );
}

/// Store with a long key (512 chars, at PMIX_MAX_KEYLEN boundary).
///
/// The C implementation returns PMIX_ERR_BAD_PARAM if key length >= PMIX_MAX_KEYLEN.
#[test]
fn store_internal_long_key() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let long_key: String = "k".repeat(512);
    let result = store_internal(&proc, &long_key, &value);
    // Without PMIx init, returns ERR_INIT. With init, should return ERR_BAD_PARAM.
    assert!(
        result.is_err(),
        "store_internal should fail, got {:?}",
        result
    );
}

/// Store with a valid-length key (511 chars, just under PMIX_MAX_KEYLEN).
#[test]
fn store_internal_max_key_length() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));
    let max_key: String = "k".repeat(511);
    let result = store_internal(&proc, &max_key, &value);
    // Without PMIx init, returns ERR_INIT. With init, should succeed.
    assert!(
        result.is_err(),
        "store_internal should fail (not initialized), got {:?}",
        result
    );
}

/// Store multiple values with different keys on the same proc.
///
/// Derived from test_internal.c which stores multiple values in a loop.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_multiple_keys() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_namespace", 0).unwrap();
    for i in 0..5 {
        let key = format!("test_key:{}", i);
        let value = build_value(
            pmix::PmixValueBuilder::new()
                .string(&format!("value_{}", i))
                .expect("string value"),
        );
        let result = store_internal(&proc, &key, &value);
        assert!(
            result.is_ok(),
            "store_internal should succeed for key {}, got {:?}",
            key,
            result
        );
    }
}

/// Store on different proc ranks.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_different_ranks() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let nspace = "test_namespace";
    for rank in 0..3u32 {
        let proc = Proc::new(nspace, rank).unwrap();
        let value = build_value(pmix::PmixValueBuilder::new().int(rank as i32));
        let result = store_internal(&proc, "rank_key", &value);
        assert!(
            result.is_ok(),
            "store_internal should succeed for rank {}, got {:?}",
            rank,
            result
        );
    }
}

/// Value is not consumed by store_internal — caller retains ownership.
///
/// PMIx_Store_internal copies the value internally (via PMIX_BFROPS_VALUE_XFER).
/// The PmixOwnedValue should still be usable after the call.
#[test]
fn store_internal_value_not_consumed() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));

    // Call store_internal — it will fail (not initialized) but should not
    // consume or modify the value.
    let _ = store_internal(&proc, "test_key", &value);

    // Value is still accessible and valid.
    let _ = value.as_raw();
}

/// Store_internal with wildcard rank.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn store_internal_wildcard_rank() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // PMIX_RANK_WILDCARD = -1 (stored as u32::MAX in the C lib).
    let proc = Proc::new("test_namespace", u32::MAX).unwrap();
    let value = build_value(
        pmix::PmixValueBuilder::new()
            .string("wildcard")
            .expect("string value"),
    );
    let result = store_internal(&proc, "wildcard_key", &value);
    assert!(
        result.is_ok(),
        "store_internal should succeed, got {:?}",
        result
    );
}

/// Deterministic behavior — calling store_internal twice with the same
/// parameters should produce the same result.
#[test]
fn store_internal_deterministic() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(42));

    let result1 = store_internal(&proc, "test_key", &value);
    let result2 = store_internal(&proc, "test_key", &value);

    assert_eq!(
        result1.is_err(),
        result2.is_err(),
        "store_internal should be deterministic"
    );
}

/// Store_internal with special characters in key.
#[test]
fn store_internal_special_key_chars() {
    let proc = Proc::new("test_namespace", 0).unwrap();
    let value = build_value(pmix::PmixValueBuilder::new().int(1));

    // Keys with dots, underscores, hyphens — all valid in PMIx.
    let special_keys = vec![
        "pmix.test_key",
        "test_key_with_underscore",
        "test-key-with-hyphen",
        "a.b.c.d.e.f.g.h",
    ];

    for key in special_keys {
        let result = store_internal(&proc, key, &value);
        // Without init, all should fail with ERR_INIT.
        assert!(
            result.is_err(),
            "store_internal({:?}) should fail (not initialized), got {:?}",
            key,
            result
        );
    }
}
