//! Integration tests for `PMIx_tool_set_server`.
//!
//! NOTE: `PMIx_tool_set_server` requires `PMIx_tool_init` first. Tests
//! that call the function are marked `#[ignore]`.

use pmix::{Info, InfoBuilder, Proc, tool::tool_set_server};

/// `tool_set_server` returns an error when called without `tool_init`.
#[test]
fn tool_set_server_without_init_returns_error() {
    let server = Proc::new("test-nspace", 0).unwrap();
    let info = InfoBuilder::new().build();
    let result = tool_set_server(&server, &info);
    assert!(
        result.is_err(),
        "tool_set_server without tool_init should return Err"
    );
}

/// Compile-time type check.
#[test]
fn tool_set_server_return_type() {
    let _: fn(&Proc, &Info) -> Result<(), pmix::PmixStatus> = tool_set_server;
}
