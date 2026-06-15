//! Integration tests for `PMIx_tool_get_servers`.
//!
//! NOTE: `PMIx_tool_get_servers` requires `PMIx_tool_init` first. Tests
//! that call the function are marked `#[ignore]`.

use pmix::tool::tool_get_servers;

/// `tool_get_servers` returns an error when called without `tool_init`.
///
/// Without initialization, the PMIx library has no server connection,
/// so this should return an error (typically PMIX_ERR_INIT).
#[test]
fn tool_get_servers_without_init_returns_error() {
    let result = tool_get_servers();
    // Without tool_init, this should fail — not crash.
    assert!(
        result.is_err(),
        "tool_get_servers without tool_init should return Err"
    );
}

/// Compile-time type check: returns `Result<Vec<Proc>, PmixStatus>`.
#[test]
fn tool_get_servers_return_type() {
    // Verify the function signature compiles.
    let _: fn() -> Result<Vec<pmix::Proc>, pmix::PmixStatus> = tool_get_servers;
}
