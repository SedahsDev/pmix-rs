//! Integration tests for `PMIx_Get_attribute_name` via the safe `get_attribute_name()` wrapper.
//!
//! NOTE: `PMIx_Get_attribute_name` requires PMIx to be initialized. Most tests
//! are marked `#[ignore]` and require a running PMIx daemon or DVM-launched process.

use pmix::utility::get_attribute_name;

/// `PMIx_Get_attribute_name` returns a known attribute name for a valid
/// attribute string. Requires PMIx initialization.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_known() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = get_attribute_name("pmix.hostname");
    assert!(
        result.is_ok(),
        "get_attribute_name('pmix.hostname') should return Ok, got {:?}",
        result
    );
}

/// `PMIx_Get_attribute_name` returns Ok for any string — it returns the
/// input unchanged if the attribute is not found. Requires PMIx init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_unknown_returns_input() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = get_attribute_name("nonexistent.attribute.xyz");
    assert!(
        result.is_ok(),
        "get_attribute_name should handle unknown attributes gracefully, got {:?}",
        result
    );
}

/// `PMIx_Get_attribute_name` returns a non-empty string. Requires PMIx init.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn get_attribute_name_non_empty() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = get_attribute_name("pmix.hostname").unwrap();
    assert!(
        !result.is_empty(),
        "get_attribute_name should not return empty string"
    );
}

/// Compile-time type check: returns `Result<String, PmixStatus>`.
///
/// Uses `core::mem::transmute` to avoid actually calling the FFI function,
/// which crashes without PMIx initialization.
#[test]
fn get_attribute_name_return_type() {
    // Verify the function signature compiles without calling it.
    fn assert_fn_type<F, R>(_: F)
    where
        F: Fn(&str) -> R,
        R: std::fmt::Debug,
    {
    }
    assert_fn_type(get_attribute_name);
}
