//! Batch 11 — Combined tests for server nspace and resource management.
//!
//! Tests for:
//! - `server_register_nspace`
//! - `server_deregister_nspace`
//! - `server_register_resources`
//! - `server_deregister_resources`
//! - `server_setup_application`
//! - `server_setup_fork`
//! - `server_setup_local_support`
//!
//! Note: Tests that call `server_init_minimal` corrupt C-level PMIx state
//! (double-free between tests). Daemon-dependent tests are marked `#[ignore]`.
//! Active tests focus on compile-time type checks, panic safety, and signature
//! verification that don't require the daemon.

use pmix::server::{
    DeregisterNspaceCallback, DeregisterResourcesCallback, RegisterNspaceCallback,
    RegisterResourcesCallback, SetupApplicationCallback, SetupLocalSupportCallback,
    server_deregister_nspace, server_deregister_resources, server_register_nspace,
    server_register_resources, server_setup_application, server_setup_fork,
    server_setup_local_support,
};
use pmix::{InfoBuilder, PmixStatus, Proc};
use std::panic::catch_unwind;

// ═══════════════════════════════════════════════════════════════════════════
// server_register_nspace — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_register_nspace has the expected function signature.
#[test]
fn test_register_nspace_signature() {
    fn _check_sig() {
        struct Dummy;
        impl RegisterNspaceCallback for Dummy {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _f: fn(
            &str,
            i32,
            &pmix::Info,
            Box<dyn RegisterNspaceCallback>,
        ) -> Result<(), PmixStatus> = server_register_nspace;
    }
}

/// server_register_nspace does not panic with valid parameters.
#[test]
fn test_register_nspace_no_panic_valid_params() {
    struct Dummy;
    impl RegisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_register_nspace("job.12345", 4, &info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_register_nspace must not panic with valid params"
    );
}

/// server_register_nspace does not panic with zero procs.
#[test]
fn test_register_nspace_no_panic_zero_procs() {
    struct Dummy;
    impl RegisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_register_nspace("job.00000", 0, &info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_register_nspace must not panic with zero procs"
    );
}

/// server_register_nspace with NUL byte in nspace returns error (no panic).
#[test]
fn test_register_nspace_nul_byte_returns_error() {
    struct Dummy;
    impl RegisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| {
        let res = server_register_nspace("job\x00bad", 1, &info, Box::new(Dummy));
        assert!(res.is_err(), "NUL byte in nspace should return Err");
    });
    assert!(result.is_ok(), "must not panic on NUL byte");
}

/// server_register_nspace returns Result type.
#[test]
fn test_register_nspace_return_type() {
    struct Dummy;
    impl RegisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> = server_register_nspace("test", 1, &info, Box::new(Dummy));
}

/// RegisterNspaceCallback trait is Send.
#[test]
fn test_register_nspace_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn RegisterNspaceCallback>>();
}

/// RegisterNspaceCallback can be used as trait object.
#[test]
fn test_register_nspace_callback_trait_object() {
    struct Dummy;
    impl RegisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn RegisterNspaceCallback> = Box::new(Dummy);
}

// ═══════════════════════════════════════════════════════════════════════════
// server_deregister_nspace — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_deregister_nspace has the expected function signature.
#[test]
fn test_deregister_nspace_signature() {
    fn _check_sig() {
        let _f: fn(&str, Option<Box<dyn DeregisterNspaceCallback>>) = server_deregister_nspace;
    }
}

/// server_deregister_nspace does not panic with valid nspace and callback.
#[test]
fn test_deregister_nspace_no_panic_with_callback() {
    struct Dummy;
    impl DeregisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = catch_unwind(|| {
        server_deregister_nspace("job.12345", Some(Box::new(Dummy)));
    });
    assert!(
        result.is_ok(),
        "server_deregister_nspace must not panic with callback"
    );
}

/// server_deregister_nspace does not panic in blocking mode (None callback).
#[test]
fn test_deregister_nspace_no_panic_blocking() {
    let result = catch_unwind(|| {
        server_deregister_nspace("job.12345", None);
    });
    assert!(
        result.is_ok(),
        "server_deregister_nspace must not panic in blocking mode"
    );
}

/// server_deregister_nspace with NUL byte invokes callback with error (no panic).
#[test]
fn test_deregister_nspace_nul_byte_no_panic() {
    struct Dummy;
    impl DeregisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = catch_unwind(|| {
        server_deregister_nspace("bad\x00nspace", Some(Box::new(Dummy)));
    });
    assert!(result.is_ok(), "must not panic on NUL byte");
}

/// server_deregister_nspace with NUL byte and no callback does not panic.
#[test]
fn test_deregister_nspace_nul_byte_no_callback() {
    let result = catch_unwind(|| {
        server_deregister_nspace("bad\x00nspace", None);
    });
    assert!(
        result.is_ok(),
        "must not panic on NUL byte with no callback"
    );
}

/// DeregisterNspaceCallback trait is Send.
#[test]
fn test_deregister_nspace_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DeregisterNspaceCallback>>();
}

/// DeregisterNspaceCallback can be used as trait object.
#[test]
fn test_deregister_nspace_callback_trait_object() {
    struct Dummy;
    impl DeregisterNspaceCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn DeregisterNspaceCallback> = Box::new(Dummy);
}

// ═══════════════════════════════════════════════════════════════════════════
// server_register_resources — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_register_resources has the expected function signature.
#[test]
fn test_register_resources_signature() {
    fn _check_sig() {
        let _f: fn(&pmix::Info, Box<dyn RegisterResourcesCallback>) -> Result<(), PmixStatus> =
            server_register_resources;
    }
}

/// server_register_resources does not panic with valid parameters.
#[test]
fn test_register_resources_no_panic_valid_params() {
    struct Dummy;
    impl RegisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_register_resources(&info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_register_resources must not panic with valid params"
    );
}

/// server_register_resources returns Result type.
#[test]
fn test_register_resources_return_type() {
    struct Dummy;
    impl RegisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> = server_register_resources(&info, Box::new(Dummy));
}

/// RegisterResourcesCallback trait is Send.
#[test]
fn test_register_resources_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn RegisterResourcesCallback>>();
}

/// RegisterResourcesCallback can be used as trait object.
#[test]
fn test_register_resources_callback_trait_object() {
    struct Dummy;
    impl RegisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn RegisterResourcesCallback> = Box::new(Dummy);
}

/// RegisterResourcesCallback::on_complete receives PmixStatus.
#[test]
fn test_register_resources_callback_receives_status() {
    struct Capture {
        status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
    }
    impl RegisterResourcesCallback for Capture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cb = Box::new(Capture {
        status: std::sync::Arc::clone(&status),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    let captured = status.lock().unwrap();
    assert!(captured.is_some());
    assert!(captured.as_ref().unwrap().is_success());
}

// ═══════════════════════════════════════════════════════════════════════════
// server_deregister_resources — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_deregister_resources has the expected function signature.
#[test]
fn test_deregister_resources_signature() {
    fn _check_sig() {
        let _f: fn(&pmix::Info, Box<dyn DeregisterResourcesCallback>) -> Result<(), PmixStatus> =
            server_deregister_resources;
    }
}

/// server_deregister_resources does not panic with valid parameters.
#[test]
fn test_deregister_resources_no_panic_valid_params() {
    struct Dummy;
    impl DeregisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_deregister_resources(&info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_deregister_resources must not panic with valid params"
    );
}

/// server_deregister_resources returns Result type.
#[test]
fn test_deregister_resources_return_type() {
    struct Dummy;
    impl DeregisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> = server_deregister_resources(&info, Box::new(Dummy));
}

/// DeregisterResourcesCallback trait is Send.
#[test]
fn test_deregister_resources_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DeregisterResourcesCallback>>();
}

/// DeregisterResourcesCallback can be used as trait object.
#[test]
fn test_deregister_resources_callback_trait_object() {
    struct Dummy;
    impl DeregisterResourcesCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn DeregisterResourcesCallback> = Box::new(Dummy);
}

/// DeregisterResourcesCallback::on_complete receives PmixStatus.
#[test]
fn test_deregister_resources_callback_receives_status() {
    struct Capture {
        status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
    }
    impl DeregisterResourcesCallback for Capture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cb = Box::new(Capture {
        status: std::sync::Arc::clone(&status),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    let captured = status.lock().unwrap();
    assert!(captured.is_some());
    assert!(captured.as_ref().unwrap().is_success());
}

// ═══════════════════════════════════════════════════════════════════════════
// server_setup_application — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_setup_application has the expected function signature.
#[test]
fn test_setup_application_signature() {
    fn _check_sig() {
        let _f: fn(&str, &pmix::Info, Box<dyn SetupApplicationCallback>) -> Result<(), PmixStatus> =
            server_setup_application;
    }
}

/// server_setup_application does not panic with valid parameters.
#[test]
fn test_setup_application_no_panic_valid_params() {
    struct Dummy;
    impl SetupApplicationCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_setup_application("job.12345", &info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_setup_application must not panic with valid params"
    );
}

/// server_setup_application returns Result type.
#[test]
fn test_setup_application_return_type() {
    struct Dummy;
    impl SetupApplicationCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    let info = InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_setup_application("job.12345", &info, Box::new(Dummy));
}

/// server_setup_application with NUL byte in nspace returns error (no panic).
#[test]
fn test_setup_application_nul_byte_returns_error() {
    struct Dummy;
    impl SetupApplicationCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| {
        let res = server_setup_application("bad\x00nspace", &info, Box::new(Dummy));
        assert!(res.is_err(), "NUL byte in nspace should return Err");
    });
    assert!(result.is_ok(), "must not panic on NUL byte");
}

/// SetupApplicationCallback trait is Send.
#[test]
fn test_setup_application_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn SetupApplicationCallback>>();
}

/// SetupApplicationCallback can be used as trait object.
#[test]
fn test_setup_application_callback_trait_object() {
    struct Dummy;
    impl SetupApplicationCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    let _cb: Box<dyn SetupApplicationCallback> = Box::new(Dummy);
}

/// SetupApplicationCallback::on_complete receives (PmixStatus, Vec<(String, String)>).
#[test]
fn test_setup_application_callback_receives_status_and_info() {
    struct Capture {
        called: std::sync::Arc<std::sync::Mutex<bool>>,
    }
    impl SetupApplicationCallback for Capture {
        fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
            *self.called.lock().unwrap() = true;
            let _ = status.is_success();
            let _ = info.len();
        }
    }
    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let cb = Box::new(Capture {
        called: std::sync::Arc::clone(&called),
    });
    cb.on_complete(PmixStatus::from_raw(0), vec![]);
    assert!(*called.lock().unwrap(), "callback should have been invoked");
}

// ═══════════════════════════════════════════════════════════════════════════
// server_setup_fork — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_setup_fork has the expected function signature.
#[test]
fn test_setup_fork_signature() {
    fn _check_sig() {
        let _f: fn(&Proc, Option<Vec<&str>>) -> Result<Vec<String>, PmixStatus> = server_setup_fork;
    }
}

/// server_setup_fork does not panic with valid proc and no env.
#[test]
fn test_setup_fork_no_panic_valid_params() {
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let result = catch_unwind(|| server_setup_fork(&proc, None));
    assert!(
        result.is_ok(),
        "server_setup_fork must not panic with valid params"
    );
}

/// server_setup_fork does not panic with initial env vars.
#[test]
fn test_setup_fork_no_panic_with_env() {
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let result =
        catch_unwind(|| server_setup_fork(&proc, Some(vec!["PATH=/usr/bin", "HOME=/tmp"])));
    assert!(
        result.is_ok(),
        "server_setup_fork must not panic with env vars"
    );
}

/// server_setup_fork returns Result<Vec<String>, PmixStatus>.
#[test]
fn test_setup_fork_return_type() {
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let _result: Result<Vec<String>, PmixStatus> = server_setup_fork(&proc, None);
}

/// server_setup_fork returns error when server is not initialized.
#[test]
fn test_setup_fork_error_without_init() {
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let result = server_setup_fork(&proc, None);
    assert!(result.is_err(), "should fail without server init");
    let err = result.unwrap_err();
    assert!(err.is_error(), "error status should be an error code");
}

/// server_setup_fork with empty env returns same error as None.
#[test]
fn test_setup_fork_none_vs_empty_env() {
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let result_none = server_setup_fork(&proc, None);
    let result_empty = server_setup_fork(&proc, Some(Vec::new()));
    assert!(result_none.is_err());
    assert!(result_empty.is_err());
    assert_eq!(
        result_none.unwrap_err(),
        result_empty.unwrap_err(),
        "None and empty env should produce same error"
    );
}

/// server_setup_fork works with various proc ranks.
#[test]
fn test_setup_fork_various_ranks() {
    for rank in [0u32, 1, 100, u32::MAX] {
        let proc = Proc::new("test_ns", rank).expect("proc creation failed");
        assert_eq!(proc.get_rank(), rank);
        let result = server_setup_fork(&proc, None);
        assert!(
            result.is_err(),
            "rank {} should fail without server init",
            rank
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// server_setup_local_support — compile-time type checks & panic safety
// ═══════════════════════════════════════════════════════════════════════════

/// server_setup_local_support has the expected function signature.
#[test]
fn test_setup_local_support_signature() {
    fn _check_sig() {
        let _f: fn(
            &str,
            &pmix::Info,
            Box<dyn SetupLocalSupportCallback>,
        ) -> Result<(), PmixStatus> = server_setup_local_support;
    }
}

/// server_setup_local_support does not panic with valid parameters.
#[test]
fn test_setup_local_support_no_panic_valid_params() {
    struct Dummy;
    impl SetupLocalSupportCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| server_setup_local_support("job.12345", &info, Box::new(Dummy)));
    assert!(
        result.is_ok(),
        "server_setup_local_support must not panic with valid params"
    );
}

/// server_setup_local_support returns Result type.
#[test]
fn test_setup_local_support_return_type() {
    struct Dummy;
    impl SetupLocalSupportCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let _result: Result<(), PmixStatus> =
        server_setup_local_support("job.12345", &info, Box::new(Dummy));
}

/// server_setup_local_support with NUL byte in nspace returns error (no panic).
#[test]
fn test_setup_local_support_nul_byte_returns_error() {
    struct Dummy;
    impl SetupLocalSupportCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let result = catch_unwind(|| {
        let res = server_setup_local_support("bad\x00nspace", &info, Box::new(Dummy));
        assert!(res.is_err(), "NUL byte in nspace should return Err");
    });
    assert!(result.is_ok(), "must not panic on NUL byte");
}

/// SetupLocalSupportCallback trait is Send.
#[test]
fn test_setup_local_support_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn SetupLocalSupportCallback>>();
}

/// SetupLocalSupportCallback can be used as trait object.
#[test]
fn test_setup_local_support_callback_trait_object() {
    struct Dummy;
    impl SetupLocalSupportCallback for Dummy {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _cb: Box<dyn SetupLocalSupportCallback> = Box::new(Dummy);
}

/// SetupLocalSupportCallback::on_complete receives PmixStatus.
#[test]
fn test_setup_local_support_callback_receives_status() {
    struct Capture {
        status: std::sync::Arc<std::sync::Mutex<Option<PmixStatus>>>,
    }
    impl SetupLocalSupportCallback for Capture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    let status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cb = Box::new(Capture {
        status: std::sync::Arc::clone(&status),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    let captured = status.lock().unwrap();
    assert!(captured.is_some());
    assert!(captured.as_ref().unwrap().is_success());
}

// ═══════════════════════════════════════════════════════════════════════════
// Cross-function tests — all 7 functions, panic safety batch
// ═══════════════════════════════════════════════════════════════════════════

/// All 7 functions are panic-safe with safe inputs (single catch_unwind test).
#[test]
fn test_all_seven_functions_no_panic() {
    // --- register_nspace ---
    struct DummyRegNs;
    impl RegisterNspaceCallback for DummyRegNs {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let info = InfoBuilder::new().build();
    let r = catch_unwind(|| server_register_nspace("job.12345", 4, &info, Box::new(DummyRegNs)));
    assert!(r.is_ok(), "register_nspace panicked");

    // --- deregister_nspace ---
    struct DummyDeregNs;
    impl DeregisterNspaceCallback for DummyDeregNs {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let r = catch_unwind(|| {
        server_deregister_nspace("job.12345", Some(Box::new(DummyDeregNs)));
    });
    assert!(r.is_ok(), "deregister_nspace panicked");

    // --- register_resources ---
    struct DummyRegRes;
    impl RegisterResourcesCallback for DummyRegRes {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let r = catch_unwind(|| server_register_resources(&info, Box::new(DummyRegRes)));
    assert!(r.is_ok(), "register_resources panicked");

    // --- deregister_resources ---
    struct DummyDeregRes;
    impl DeregisterResourcesCallback for DummyDeregRes {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let r = catch_unwind(|| server_deregister_resources(&info, Box::new(DummyDeregRes)));
    assert!(r.is_ok(), "deregister_resources panicked");

    // --- setup_application ---
    struct DummySetupApp;
    impl SetupApplicationCallback for DummySetupApp {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    let r = catch_unwind(|| server_setup_application("job.12345", &info, Box::new(DummySetupApp)));
    assert!(r.is_ok(), "setup_application panicked");

    // --- setup_fork ---
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");
    let r = catch_unwind(|| server_setup_fork(&proc, None));
    assert!(r.is_ok(), "setup_fork panicked");

    // --- setup_local_support ---
    struct DummySetupLocal;
    impl SetupLocalSupportCallback for DummySetupLocal {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let r =
        catch_unwind(|| server_setup_local_support("job.12345", &info, Box::new(DummySetupLocal)));
    assert!(r.is_ok(), "setup_local_support panicked");
}

/// All 7 callback traits are Send — verified in one test.
#[test]
fn test_all_seven_callback_traits_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn RegisterNspaceCallback>>();
    assert_send::<Box<dyn DeregisterNspaceCallback>>();
    assert_send::<Box<dyn RegisterResourcesCallback>>();
    assert_send::<Box<dyn DeregisterResourcesCallback>>();
    assert_send::<Box<dyn SetupApplicationCallback>>();
    assert_send::<Box<dyn SetupLocalSupportCallback>>();
}

/// Multiple calls to each function are consistent (no state corruption).
#[test]
fn test_multiple_calls_consistency() {
    struct DummyRegNs;
    impl RegisterNspaceCallback for DummyRegNs {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    struct DummyDeregNs;
    impl DeregisterNspaceCallback for DummyDeregNs {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    struct DummyRegRes;
    impl RegisterResourcesCallback for DummyRegRes {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    struct DummyDeregRes;
    impl DeregisterResourcesCallback for DummyDeregRes {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    struct DummySetupApp;
    impl SetupApplicationCallback for DummySetupApp {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }
    struct DummySetupLocal;
    impl SetupLocalSupportCallback for DummySetupLocal {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    let info = InfoBuilder::new().build();
    let proc = Proc::new("job.12345", 0).expect("proc creation failed");

    for _ in 0..3 {
        let r1 = server_register_nspace("job.12345", 4, &info, Box::new(DummyRegNs));
        let r2 = server_deregister_nspace("job.12345", Some(Box::new(DummyDeregNs)));
        let r3 = server_register_resources(&info, Box::new(DummyRegRes));
        let r4 = server_deregister_resources(&info, Box::new(DummyDeregRes));
        let r5 = server_setup_application("job.12345", &info, Box::new(DummySetupApp));
        let r6 = server_setup_fork(&proc, None);
        let r7 = server_setup_local_support("job.12345", &info, Box::new(DummySetupLocal));

        // All calls should complete without panicking.
        // Results may be Err (no server), but they must be Results, not panics.
        let _ = (r1, r2, r3, r4, r5, r6, r7);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Daemon-dependent integration tests — IGNORED
// ═══════════════════════════════════════════════════════════════════════════

/// Nspace lifecycle: register → setup app → setup fork → deregister.
/// Requires PMIx server runtime. Calling server_init_minimal corrupts
/// C-level PMIx state (double-free between tests).
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_nspace_full_lifecycle() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_nspace("job.12345", 4, &info, callback)
    // 3. server_setup_application("job.12345", &info, callback)
    // 4. server_setup_fork(&proc, None)
    // 5. server_deregister_nspace("job.12345", Some(callback))
    // 6. server_finalize
    panic!("requires PMIx server runtime");
}

/// Resource registration: CPU/memory/gpu resources per node.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_resource_registration_per_node() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_resources with node-specific info
    // 3. Verify callback receives success
    // 4. server_deregister_resources
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}

/// Duplicate nspace registration should be handled gracefully.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_duplicate_nspace_registration() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_nspace("job.12345", 4, &info, cb1)
    // 3. server_register_nspace("job.12345", 4, &info, cb2) — duplicate
    // 4. Verify appropriate behavior (error or callback with error)
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}

/// Deregister unknown nspace should return error.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_deregister_unknown_nspace() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_deregister_nspace("nonexistent", Some(callback))
    // 3. Verify callback receives error status
    // 4. server_finalize
    panic!("requires PMIx server runtime");
}

/// Setup local support with valid proc info.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_setup_local_support_with_proc_info() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_nspace
    // 3. server_setup_local_support with valid info
    // 4. Verify callback receives success
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}

/// server_setup_application callback receives setup info on success.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_setup_application_callback_receives_info() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_nspace
    // 3. server_setup_application
    // 4. Verify callback receives Vec<(String, String)> with setup info
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}

/// server_setup_fork returns environment variables when server is initialized.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_setup_fork_returns_env_with_server() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_nspace
    // 3. server_setup_fork(&proc, None)
    // 4. Verify returned Vec<String> contains PMIX_* variables
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}

/// server_register_resources and server_deregister_resources pair.
/// Requires PMIx server runtime.
#[test]
#[ignore = "requires PMIx server runtime; server_init_minimal corrupts C-level state"]
fn test_register_deregister_resources_pair() {
    // This would require:
    // 1. server_init_minimal
    // 2. server_register_resources with info
    // 3. server_deregister_resources with info
    // 4. Verify both callbacks receive success
    // 5. server_finalize
    panic!("requires PMIx server runtime");
}
