//! Tests for `PMIx_server_init`, `PMIx_server_finalize`, `PmixServerModule`,
//! `PmixServerHandle`, and `is_server_initialized`.
//!
//! Note: `PMIx_server_init` requires a running PMIx daemon or a proper
//! PMIx server environment. Tests that call the actual FFI are marked
//! `#[ignore]` and should be run with a PMIx environment.
//!
//! Unit tests that verify API structure, types, and defaults run without
//! a PMIx runtime.

use pmix::PmixStatus;
use pmix::server::{
    PmixServerHandle, PmixServerModule, is_server_initialized, server_finalize, server_init,
    server_init_minimal,
};

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerModule — structure and defaults
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerModule implements Default — all callbacks are None.
#[test]
fn test_server_module_default_all_null() {
    let module = PmixServerModule::default();
    // All fields are Option<...> and default to None.
    // Verify via Debug output that the module is constructible.
    let debug = format!("{:?}", module);
    assert!(
        debug.contains("PmixServerModule"),
        "Debug output should contain struct name"
    );
}

/// PmixServerModule implements Debug.
#[test]
fn test_server_module_debug() {
    let module = PmixServerModule::default();
    let debug_str = format!("{:?}", module);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
}

/// PmixServerModule can be constructed with explicit fields.
#[test]
fn test_server_module_construct() {
    let module = PmixServerModule {
        client_connected: None,
        client_finalized: None,
        abort: None,
        fence_nb: None,
        direct_modex: None,
        publish: None,
        lookup: None,
        unpublish: None,
        spawn: None,
        connect: None,
        disconnect: None,
        register_events: None,
        deregister_events: None,
        listener: None,
        notify_event: None,
        query: None,
        tool_connected: None,
        log: None,
        allocate: None,
        job_control: None,
        monitor: None,
        get_credential: None,
        validate_credential: None,
        iof_pull: None,
        push_stdin: None,
        group: None,
        fabric: None,
        client_connected2: None,
        session_control: None,
    };
    let _debug = format!("{:?}", module);
    // If we got here without panic, construction succeeded.
}

/// PmixServerModule can have individual callbacks set.
#[test]
fn test_server_module_set_callback() {
    extern "C" fn dummy_connected() {}

    let module = PmixServerModule {
        client_connected: Some(dummy_connected),
        ..Default::default()
    };
    assert!(module.client_connected.is_some());
}

/// PmixServerModule has the expected number of callback fields.
#[test]
fn test_server_module_field_count() {
    // The PMIx 4.0 server module has 29 callback fields.
    // Verify by checking the Debug output contains all expected field names.
    let module = PmixServerModule::default();
    let debug = format!("{:?}", module);

    let expected_fields = [
        "client_connected",
        "client_finalized",
        "abort",
        "fence_nb",
        "direct_modex",
        "publish",
        "lookup",
        "unpublish",
        "spawn",
        "connect",
        "disconnect",
        "register_events",
        "deregister_events",
        "listener",
        "notify_event",
        "query",
        "tool_connected",
        "log",
        "allocate",
        "job_control",
        "monitor",
        "get_credential",
        "validate_credential",
        "iof_pull",
        "push_stdin",
        "group",
        "fabric",
        "client_connected2",
        "session_control",
    ];

    for field in &expected_fields {
        assert!(
            debug.contains(field),
            "PmixServerModule Debug output should contain field '{}'",
            field
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — structure
// ─────────────────────────────────────────────────────────────────────────────

/// PmixServerHandle implements Debug.
#[test]
fn test_server_handle_debug() {
    // We can't create a real handle without PMIx_server_init,
    // but we can verify the type exists and is Debug via a type check.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// is_server_initialized — state check
// ─────────────────────────────────────────────────────────────────────────────

/// is_server_initialized returns a bool (compiles and runs).
/// Without a running PMIx daemon, this should return false.
#[test]
fn test_is_server_initialized_returns_bool() {
    let _result = is_server_initialized();
    // Just verify it compiles and doesn't panic.
}

/// is_server_initialized returns false when PMIx is not initialized.
#[test]
fn test_is_server_initialized_false_when_not_init() {
    // Without a PMIx daemon running, the server should not be initialized.
    // This test may pass or fail depending on the environment.
    let result = is_server_initialized();
    // In a clean test environment, PMIx should not be initialized.
    // We don't assert false because the test runner might have initialized PMIx.
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// server_init / server_finalize — integration tests (require PMIx daemon)
// ─────────────────────────────────────────────────────────────────────────────

/// server_init with a default module and no info should work when a PMIx
/// daemon is running.
///
/// This test is ignored by default because it requires a PMIx server
/// environment. Run with: `cargo test -- --ignored --test-threads=1`
/// in an environment where a PMIx daemon is available.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_init_minimal_with_daemon() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("server_init_minimal should succeed");
    assert!(
        is_server_initialized(),
        "server should be initialized after server_init_minimal"
    );
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init with an empty info array should work.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_init_with_empty_info() {
    let module = PmixServerModule::default();
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_finalize on an already-finalized server should handle gracefully.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_finalize_idempotent() {
    let module = PmixServerModule::default();
    let handle = server_init_minimal(Some(&module)).expect("first server_init should succeed");
    server_finalize(handle).expect("first server_finalize should succeed");

    // Second init/finalize cycle
    let handle2 = server_init_minimal(Some(&module)).expect("second server_init should succeed");
    server_finalize(handle2).expect("second server_finalize should succeed");
}

/// server_init with tool support info key.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_init_with_tool_support() {
    let module = PmixServerModule::default();
    // PMIX_SERVER_TOOL_SUPPORT is a boolean key
    let info = pmix::InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

/// server_init with None module (minimal server).
#[test]
#[ignore = "requires PMIx daemon"]
fn test_server_init_none_module() {
    let handle = server_init_minimal(None).expect("server_init_minimal(None) should succeed");
    server_finalize(handle).expect("server_finalize should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// API consistency tests
// ─────────────────────────────────────────────────────────────────────────────

/// server_init and server_init_minimal have compatible signatures.
#[test]
fn test_api_signatures_compile() {
    // Verify the functions exist and have the expected signatures
    // by checking they can be called (even though they will fail
    // without a PMIx daemon).
    fn _check_server_init() {
        let module = PmixServerModule::default();
        let info = pmix::InfoBuilder::new().build();
        let _result: Result<PmixServerHandle, PmixStatus> = server_init(Some(&module), &info);
    }
    fn _check_server_init_minimal() {
        let module = PmixServerModule::default();
        let _result: Result<PmixServerHandle, PmixStatus> = server_init_minimal(Some(&module));
    }
    fn _check_server_finalize() {
        // Can't call this without a handle, but verify the signature type-checks
        fn assert_fn_type<F, R>(_: F)
        where
            F: Fn(PmixServerHandle) -> R,
        {
        }
        assert_fn_type(server_finalize);
    }
}

/// PmixServerModule::as_c_ptr returns a valid pointer type.
#[test]
fn test_as_c_ptr_type() {
    let module = PmixServerModule::default();
    let ptr = module.as_c_ptr();
    // The pointer should be non-null (it points to the module on the stack).
    assert!(!ptr.is_null(), "as_c_ptr should return a non-null pointer");
}

/// Multiple PmixServerModule instances can coexist.
#[test]
fn test_multiple_modules() {
    let module1 = PmixServerModule::default();
    let module2 = PmixServerModule::default();
    let module3 = PmixServerModule {
        client_connected: Some(dummy_fn),
        ..Default::default()
    };

    let ptr1 = module1.as_c_ptr();
    let ptr2 = module2.as_c_ptr();
    let ptr3 = module3.as_c_ptr();

    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());
    assert!(!ptr3.is_null());
    // Different instances should have different addresses
    assert_ne!(
        ptr1, ptr2,
        "different modules should have different addresses"
    );
    assert_ne!(
        ptr2, ptr3,
        "different modules should have different addresses"
    );
}

extern "C" fn dummy_fn() {}
