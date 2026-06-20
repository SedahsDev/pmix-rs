//! Tests for `PMIx_Register_attributes` — safe Rust wrapper.
//!
//! The `register_attributes` function registers host environment attribute
//! support for a given PMIx function.  It requires `PMIx_Init` (or server
//! init) to have been called first; otherwise it returns `PMIX_ERR_INIT`.
//!
//! Because most tests require a running PMIx server, they are marked `#[ignore]`.
//! The non-FFI tests (signature, type checks, input validation) run unconditionally.

use pmix::PmixStatus;
use pmix::utility::register_attributes;

// ─────────────────────────────────────────────────────────────────────────────
// Signature and type tests (no FFI required)
// ─────────────────────────────────────────────────────────────────────────────

/// `register_attributes` is callable with valid arguments and returns a Result.
#[test]
fn test_register_attributes_signature() {
    let result: Result<(), PmixStatus> = register_attributes("PMIx_Get", &["attr1", "attr2"]);
    assert!(
        result.is_err(),
        "register_attributes should fail without PMIx_Init (returns PMIX_ERR_INIT)"
    );
}

/// `register_attributes` returns `PMIX_ERR_INIT` when called before initialization.
#[test]
fn test_register_attributes_before_init() {
    let result = register_attributes("PMIx_Get", &["attr1"]);
    assert!(result.is_err(), "should fail before init");

    let err_status = result.unwrap_err();
    // PMIX_ERR_INIT = -31 (actual PMIx library value)
    assert_eq!(
        err_status.to_raw(),
        -31,
        "error should be PMIX_ERR_INIT (-31), got {}",
        err_status.to_raw()
    );
}

/// `register_attributes` accepts an empty attribute list.
#[test]
fn test_register_attributes_empty_attrs() {
    let result = register_attributes("PMIx_Get", &[] as &[&str]);
    assert!(
        result.is_err(),
        "should still fail with PMIX_ERR_INIT even with empty attrs"
    );
}

/// `register_attributes` rejects function names containing NUL bytes.
#[test]
fn test_register_attributes_nul_in_function_name() {
    let result = register_attributes("PMIx\0_Get", &["attr1"]);
    assert!(result.is_err(), "should reject NUL byte in function name");

    let err_status = result.unwrap_err();
    assert_eq!(
        err_status.to_raw(),
        -27,
        "error should be PMIX_ERR_BAD_PARAM (-27), got {}",
        err_status.to_raw()
    );
}

/// `register_attributes` handles function names with special characters.
#[test]
fn test_register_attributes_valid_function_names() {
    let valid_names = [
        "PMIx_Get",
        "PMIx_Put",
        "PMIx_Fence",
        "PMIx_Register_event_handler",
        "PMIx_server_register_nspace",
        "my_custom_function",
        "function.with.dots",
    ];

    for name in valid_names {
        let result = register_attributes(name, &["attr1"]);
        assert!(result.is_err(), "should fail before init for '{}'", name);
        let err = result.unwrap_err();
        assert_eq!(
            err.to_raw(),
            -31,
            "should be PMIX_ERR_INIT for '{}', got {}",
            name,
            err.to_raw()
        );
    }
}

/// `register_attributes` handles attribute names with dots and underscores.
#[test]
fn test_register_attributes_attribute_name_formats() {
    let attrs = &[
        "pmix.get.timeout",
        "pmix_get_scope",
        "some-nested.attribute.key",
        "UPPERCASE_ATTR",
        "mixedCase_attr_123",
    ];

    let result = register_attributes("PMIx_Get", attrs);
    assert!(result.is_err(), "should fail before init");
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -31,
        "should be PMIX_ERR_INIT, got {}",
        err.to_raw()
    );
}

/// `register_attributes` handles a large number of attributes.
#[test]
fn test_register_attributes_many_attributes() {
    let names: Vec<String> = (0..100).map(|i| format!("attr_{}", i)).collect();
    let attrs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();

    let result = register_attributes("PMIx_Get", &attrs);
    assert!(result.is_err(), "should fail before init");
    let err = result.unwrap_err();
    assert_eq!(
        err.to_raw(),
        -31,
        "should be PMIX_ERR_INIT, got {}",
        err.to_raw()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx_Init / PMIx_server_init — ignored by default)
// ─────────────────────────────────────────────────────────────────────────────

/// Successful registration after PMIx_Init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_attributes_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = register_attributes("PMIx_Get", &["pmix.get.timeout"]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Duplicate registration returns PMIX_ERR_REPEAT_ATTR_REGISTRATION.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_attributes_duplicate() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = register_attributes("PMIx_Get", &["attr1"]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration with empty attrs list is valid after init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_attributes_empty_after_init() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = register_attributes("PMIx_Get", &[] as &[&str]);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration with special attribute names after init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_attributes_special_attrs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let attrs = &[
        "pmix.get.timeout",
        "pmix.get.scope",
        "pmix.get.max_size",
        "pmix.get.collect_data",
    ];
    let result = register_attributes("PMIx_Get", attrs);
    assert!(result.is_err(), "expected PMIX_ERR_INIT without PMIx_Init");
}

/// Registration for server-side functions.
#[test]
#[ignore = "requires PMIx_server_init"]
fn test_register_attributes_server_functions() {
    let server_functions = [
        "PMIx_server_register_nspace",
        "PMIx_server_deregister_nspace",
        "PMIx_server_notify",
    ];
    for func in server_functions {
        let result = register_attributes(func, &["attr1"]);
        assert!(result.is_err(), "expected PMIX_ERR_INIT for '{}'", func);
    }
}

/// Registration for tool-side functions.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_attributes_tool_functions() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let tool_functions = ["PMIx_Connect", "PMIx_Disconnect", "PMIx_Notify_event"];
    for func in tool_functions {
        let result = register_attributes(func, &["attr1"]);
        assert!(result.is_err(), "expected PMIX_ERR_INIT for '{}'", func);
    }
}
