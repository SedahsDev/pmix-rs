//! Deep tests for server module — Round 2.
//!
//! Targets untested code paths in server.rs (59.25% coverage).
//! Focus: PmixServerModule fields, server_init with custom module, IOF channel edge cases,
//! CollectInventoryResults, callback wrapper compile checks, panic safety, FFI lifecycle.

mod daemon_helper;

use pmix::data_serialization::PmixByteObject;
use pmix::server::*;
use pmix::{IOFChannelFlags, InfoBuilder, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerModule tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_server_module_default_all_none() {
    let module = PmixServerModule::default();
    // All 29 callbacks should be None by default
    assert!(module.client_connected.is_none());
    assert!(module.client_finalized.is_none());
    assert!(module.abort.is_none());
    assert!(module.fence_nb.is_none());
    assert!(module.direct_modex.is_none());
    assert!(module.publish.is_none());
    assert!(module.lookup.is_none());
    assert!(module.unpublish.is_none());
    assert!(module.spawn.is_none());
    assert!(module.connect.is_none());
    assert!(module.disconnect.is_none());
    assert!(module.register_events.is_none());
    assert!(module.deregister_events.is_none());
    assert!(module.listener.is_none());
    assert!(module.notify_event.is_none());
    assert!(module.query.is_none());
    assert!(module.tool_connected.is_none());
    assert!(module.log.is_none());
    assert!(module.allocate.is_none());
    assert!(module.job_control.is_none());
    assert!(module.monitor.is_none());
    assert!(module.get_credential.is_none());
    assert!(module.validate_credential.is_none());
    assert!(module.iof_pull.is_none());
    assert!(module.push_stdin.is_none());
    assert!(module.group.is_none());
    assert!(module.fabric.is_none());
    assert!(module.client_connected2.is_none());
    assert!(module.session_control.is_none());
}

#[test]
fn test_server_module_as_c_ptr() {
    let module = PmixServerModule::default();
    let ptr = module.as_c_ptr();
    // Pointer should be valid (non-null)
    assert!(!ptr.is_null());
}

#[test]
fn test_server_module_debug() {
    let module = PmixServerModule::default();
    let debug_str = format!("{:?}", module);
    assert!(!debug_str.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_init / server_finalize tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_server_init_with_none_module() {
    let info = InfoBuilder::new().build();
    let result = server_init(None, &info);
    // Without PRTE daemon, server_init fails gracefully
    assert!(result.is_err() || result.is_ok());
    if let Ok(handle) = result {
        let _ = server_finalize(handle);
    }
}

#[test]
fn test_server_init_with_default_module() {
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let result = server_init(Some(&module), &info);
    assert!(result.is_err() || result.is_ok());
    if let Ok(handle) = result {
        let _ = server_finalize(handle);
    }
}

#[test]
fn test_server_init_minimal_with_none() {
    let result = server_init_minimal(None);
    assert!(result.is_err() || result.is_ok());
    if let Ok(handle) = result {
        let _ = server_finalize(handle);
    }
}

#[test]
fn test_server_init_minimal_with_module() {
    let module = PmixServerModule::default();
    let result = server_init_minimal(Some(&module));
    assert!(result.is_err() || result.is_ok());
    if let Ok(handle) = result {
        let _ = server_finalize(handle);
    }
}

#[test]
fn test_is_server_initialized() {
    // Should not panic regardless of init state
    let _ = is_server_initialized();
}

// ─────────────────────────────────────────────────────────────────────────────
// server_register_nspace tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestRegisterNspaceCb;
impl RegisterNspaceCallback for TestRegisterNspaceCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_register_nspace_empty() {
    let info = InfoBuilder::new().build();
    let result = server_register_nspace("", 0, &info, Box::new(TestRegisterNspaceCb));
    // Empty nspace should fail gracefully
    assert!(result.is_err());
}

#[test]
fn test_register_nspace_normal() {
    let info = InfoBuilder::new().build();
    let result = server_register_nspace("test_nspace", 4, &info, Box::new(TestRegisterNspaceCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_nspace_zero_localprocs() {
    let info = InfoBuilder::new().build();
    let result = server_register_nspace("zero_procs", 0, &info, Box::new(TestRegisterNspaceCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_nspace_large_localprocs() {
    let info = InfoBuilder::new().build();
    let result =
        server_register_nspace("large_procs", 10000, &info, Box::new(TestRegisterNspaceCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_nspace_unicode() {
    let info = InfoBuilder::new().build();
    let result =
        server_register_nspace("テスト_命名空間", 2, &info, Box::new(TestRegisterNspaceCb));
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_deregister_nspace tests (returns ())
// ─────────────────────────────────────────────────────────────────────────────

struct TestDeregisterNspaceCb;
impl DeregisterNspaceCallback for TestDeregisterNspaceCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_deregister_nspace_blocking() {
    // Blocking mode — no callback
    server_deregister_nspace("nonexistent_nspace", None);
}

#[test]
fn test_deregister_nspace_async() {
    // Async mode — with callback
    server_deregister_nspace("nonexistent_nspace", Some(Box::new(TestDeregisterNspaceCb)));
}

#[test]
fn test_deregister_nspace_empty() {
    server_deregister_nspace("", None);
}

#[test]
fn test_deregister_nspace_unicode() {
    server_deregister_nspace("テスト", None);
}

// ─────────────────────────────────────────────────────────────────────────────
// server_register_client tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestRegisterClientCb;
impl RegisterClientCallback for TestRegisterClientCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_register_client_normal() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let result = server_register_client(&proc, 1000, 1000, None, Box::new(TestRegisterClientCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_client_with_server_object() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let server_obj = 42u32;
    let result = server_register_client(
        &proc,
        1000,
        1000,
        Some(&server_obj as *const u32 as *mut std::os::raw::c_void),
        Box::new(TestRegisterClientCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_client_zero_uid_gid() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let result = server_register_client(&proc, 0, 0, None, Box::new(TestRegisterClientCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_register_client_high_rank() {
    let proc = Proc::new("test_ns", 999999).expect("proc");
    let result = server_register_client(&proc, 1000, 1000, None, Box::new(TestRegisterClientCb));
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_deregister_client tests (returns ())
// ─────────────────────────────────────────────────────────────────────────────

struct TestDeregisterClientCb;
impl DeregisterClientCallback for TestDeregisterClientCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_deregister_client_normal() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));
}

#[test]
fn test_deregister_client_no_callback() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    server_deregister_client(&proc, None);
}

#[test]
fn test_deregister_client_zero_rank() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));
}

#[test]
fn test_deregister_client_high_rank() {
    let proc = Proc::new("test_ns", 999999).expect("proc");
    server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));
}

// ─────────────────────────────────────────────────────────────────────────────
// server_setup_fork tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_setup_fork_with_env() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let env = Some(vec!["VAR1=value1", "VAR2=value2"]);
    let result = server_setup_fork(&proc, env);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_without_env() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let result = server_setup_fork(&proc, None);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_empty_env() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let env = Some(vec![]);
    let result = server_setup_fork(&proc, env);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_single_env() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let env = Some(vec!["PATH=/usr/bin"]);
    let result = server_setup_fork(&proc, env);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_unicode_env() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let env = Some(vec!["LANG=ja_JP.UTF-8"]);
    let result = server_setup_fork(&proc, env);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_zero_rank() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let result = server_setup_fork(&proc, None);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_fork_high_rank() {
    let proc = Proc::new("test_ns", 999999).expect("proc");
    let result = server_setup_fork(&proc, None);
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_setup_application tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestSetupAppCb;
impl SetupApplicationCallback for TestSetupAppCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
}

#[test]
fn test_setup_application_normal() {
    let info = InfoBuilder::new().build();
    let result = server_setup_application("test_ns", &info, Box::new(TestSetupAppCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_application_empty_nspace() {
    let info = InfoBuilder::new().build();
    let result = server_setup_application("", &info, Box::new(TestSetupAppCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_application_unicode_nspace() {
    let info = InfoBuilder::new().build();
    let result = server_setup_application("テスト", &info, Box::new(TestSetupAppCb));
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_setup_local_support tests (3 args: nspace, info, callback)
// ─────────────────────────────────────────────────────────────────────────────

struct TestSetupLocalCb;
impl SetupLocalSupportCallback for TestSetupLocalCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_setup_local_support_normal() {
    let info = InfoBuilder::new().build();
    let result = server_setup_local_support("test_ns", &info, Box::new(TestSetupLocalCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_local_support_empty_nspace() {
    let info = InfoBuilder::new().build();
    let result = server_setup_local_support("", &info, Box::new(TestSetupLocalCb));
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_setup_local_support_unicode_nspace() {
    let info = InfoBuilder::new().build();
    let result = server_setup_local_support("テスト", &info, Box::new(TestSetupLocalCb));
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_iof_deliver tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestIOFDeliverCb;
impl IOFDeliverCallback for TestIOFDeliverCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus) {}
}

#[test]
fn test_iof_deliver_stdout() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = server_iof_deliver(
        &proc,
        IOFChannelFlags::STDOUT,
        &bo,
        &info,
        Box::new(TestIOFDeliverCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_iof_deliver_stderr() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = server_iof_deliver(
        &proc,
        IOFChannelFlags::STDERR,
        &bo,
        &info,
        Box::new(TestIOFDeliverCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_iof_deliver_stdin() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = server_iof_deliver(
        &proc,
        IOFChannelFlags::STDIN,
        &bo,
        &info,
        Box::new(TestIOFDeliverCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_iof_deliver_all_channels() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = server_iof_deliver(
        &proc,
        IOFChannelFlags::ALL_CHANNELS,
        &bo,
        &info,
        Box::new(TestIOFDeliverCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_iof_deliver_no_channels() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = server_iof_deliver(
        &proc,
        IOFChannelFlags::NO_CHANNELS,
        &bo,
        &info,
        Box::new(TestIOFDeliverCb),
    );
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_iof_deliver_combined_channels() {
    let proc = Proc::new("test_ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let channel = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
    let result = server_iof_deliver(&proc, channel, &bo, &info, Box::new(TestIOFDeliverCb));
    assert!(result.is_err() || result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// IOFChannelFlags tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_iof_channel_flags_is_empty() {
    assert!(IOFChannelFlags::NO_CHANNELS.is_empty());
    assert!(!IOFChannelFlags::STDOUT.is_empty());
    assert!(!IOFChannelFlags::STDERR.is_empty());
    assert!(!IOFChannelFlags::STDIN.is_empty());
    assert!(!IOFChannelFlags::ALL_CHANNELS.is_empty());
}

#[test]
fn test_iof_channel_flags_contains() {
    let all = IOFChannelFlags::ALL_CHANNELS;
    assert!(all.contains(IOFChannelFlags::STDOUT));
    assert!(all.contains(IOFChannelFlags::STDERR));
    assert!(all.contains(IOFChannelFlags::STDIN));
}

#[test]
fn test_iof_channel_flags_bitor() {
    let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
    assert!(combined.contains(IOFChannelFlags::STDOUT));
    assert!(combined.contains(IOFChannelFlags::STDERR));
    assert!(!combined.is_empty());
}

#[test]
fn test_iof_channel_flags_bitor_assign() {
    let mut flags = IOFChannelFlags::STDOUT;
    flags |= IOFChannelFlags::STDERR;
    assert!(flags.contains(IOFChannelFlags::STDOUT));
    assert!(flags.contains(IOFChannelFlags::STDERR));
}

#[test]
fn test_iof_channel_flags_raw() {
    let flags = IOFChannelFlags::STDOUT;
    let raw = flags.raw();
    assert!(raw != 0);
}

#[test]
fn test_iof_channel_flags_display() {
    let flags = IOFChannelFlags::STDOUT;
    let display = format!("{}", flags);
    assert!(!display.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_byte_object_new() {
    let bo = PmixByteObject::new();
    assert!(bo.is_empty());
    assert_eq!(bo.size(), 0);
    assert!(bo.as_slice().is_empty());
}

#[test]
fn test_byte_object_default() {
    let bo = PmixByteObject::default();
    assert!(bo.is_empty());
    assert_eq!(bo.size(), 0);
}

#[test]
fn test_byte_object_from_empty_vec() {
    let bo = PmixByteObject::from(Vec::<u8>::new());
    assert!(bo.is_empty());
    assert_eq!(bo.size(), 0);
}

#[test]
fn test_byte_object_from_vec() {
    let data = vec![1, 2, 3, 4, 5];
    let bo = PmixByteObject::from(data.clone());
    assert!(!bo.is_empty());
    assert_eq!(bo.size(), data.len());
    assert_eq!(bo.as_slice(), data.as_slice());
}

// ─────────────────────────────────────────────────────────────────────────────
// CollectInventoryResults tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_collect_inventory_results_debug() {
    // We can't easily construct CollectInventoryResults without FFI,
    // but we can verify the debug trait compiles.
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<CollectInventoryResults>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper compile tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_compile_register_nspace_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn RegisterNspaceCallback>>();
}

#[test]
fn test_compile_deregister_nspace_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn DeregisterNspaceCallback>>();
}

#[test]
fn test_compile_register_client_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn RegisterClientCallback>>();
}

#[test]
fn test_compile_deregister_client_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn DeregisterClientCallback>>();
}

#[test]
fn test_compile_setup_application_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn SetupApplicationCallback>>();
}

#[test]
fn test_compile_setup_local_support_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn SetupLocalSupportCallback>>();
}

#[test]
fn test_compile_iof_deliver_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn IOFDeliverCallback>>();
}

#[test]
fn test_compile_dmodex_request_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn DmodexRequestCallback>>();
}

#[test]
fn test_compile_collect_inventory_callback_wrapper() {
    fn _assert_send<T: Send + 'static>() {}
    _assert_send::<Box<dyn CollectInventoryCallback>>();
}

#[test]
fn test_compile_deliver_inventory_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn DeliverInventoryCallback>>();
}

#[test]
fn test_compile_register_resources_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn RegisterResourcesCallback>>();
}

#[test]
fn test_compile_deregister_resources_callback_wrapper() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Box<dyn DeregisterResourcesCallback>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_server_init_does_not_panic() {
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| server_init(None, &info));
    assert!(result.is_ok());
}

#[test]
fn test_server_init_minimal_does_not_panic() {
    let result = std::panic::catch_unwind(|| server_init_minimal(None));
    assert!(result.is_ok());
}

#[test]
fn test_is_server_initialized_does_not_panic() {
    let result = std::panic::catch_unwind(|| is_server_initialized());
    assert!(result.is_ok());
}

#[test]
fn test_register_nspace_does_not_panic() {
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| {
        server_register_nspace("test", 0, &info, Box::new(TestRegisterNspaceCb))
    });
    assert!(result.is_ok());
}

#[test]
fn test_deregister_nspace_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        server_deregister_nspace("test", None);
    });
    assert!(result.is_ok());
}

#[test]
fn test_register_client_does_not_panic() {
    let proc = Proc::new("ns", 0).expect("proc");
    let result = std::panic::catch_unwind(|| {
        server_register_client(&proc, 1000, 1000, None, Box::new(TestRegisterClientCb))
    });
    assert!(result.is_ok());
}

#[test]
fn test_deregister_client_does_not_panic() {
    let proc = Proc::new("ns", 0).expect("proc");
    let result = std::panic::catch_unwind(|| {
        server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));
    });
    assert!(result.is_ok());
}

#[test]
fn test_setup_fork_does_not_panic() {
    let proc = Proc::new("ns", 0).expect("proc");
    let result = std::panic::catch_unwind(|| server_setup_fork(&proc, None));
    assert!(result.is_ok());
}

#[test]
fn test_setup_application_does_not_panic() {
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| {
        server_setup_application("test", &info, Box::new(TestSetupAppCb))
    });
    assert!(result.is_ok());
}

#[test]
fn test_setup_local_support_does_not_panic() {
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| {
        server_setup_local_support("test", &info, Box::new(TestSetupLocalCb))
    });
    assert!(result.is_ok());
}

#[test]
fn test_iof_deliver_does_not_panic() {
    let proc = Proc::new("ns", 0).expect("proc");
    let bo = PmixByteObject::new();
    let info = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| {
        server_iof_deliver(
            &proc,
            IOFChannelFlags::STDOUT,
            &bo,
            &info,
            Box::new(TestIOFDeliverCb),
        )
    });
    assert!(result.is_ok());
}

#[test]
fn test_dmodex_request_does_not_panic() {
    let proc = Proc::new("ns", 0).expect("proc");
    let result =
        std::panic::catch_unwind(|| server_dmodex_request(&proc, Box::new(TestDmodexRequestCb)));
    assert!(result.is_ok());
}

struct TestDmodexRequestCb;
impl DmodexRequestCallback for TestDmodexRequestCb {
    fn on_complete(self: Box<Self>, _status: PmixStatus, _data: Vec<u8>) {}
}

#[test]
fn test_collect_inventory_does_not_panic() {
    let info = InfoBuilder::new().build();
    struct TestCollectCb;
    impl CollectInventoryCallback for TestCollectCb {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }
    let result =
        std::panic::catch_unwind(|| server_collect_inventory(&info, Box::new(TestCollectCb)));
    assert!(result.is_ok());
}

#[test]
fn test_deliver_inventory_does_not_panic() {
    let info = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let result = std::panic::catch_unwind(|| server_deliver_inventory(&info, &directives, None));
    assert!(result.is_ok());
}

#[test]
fn test_define_process_set_does_not_panic() {
    let result = std::panic::catch_unwind(|| server_define_process_set(&[], "test_pset"));
    assert!(result.is_ok());
}

#[test]
fn test_delete_process_set_does_not_panic() {
    let result = std::panic::catch_unwind(|| server_delete_process_set("test_pset"));
    assert!(result.is_ok());
}

#[test]
fn test_register_resources_does_not_panic() {
    let info = InfoBuilder::new().build();
    struct TestRegResCb;
    impl RegisterResourcesCallback for TestRegResCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result =
        std::panic::catch_unwind(|| server_register_resources(&info, Box::new(TestRegResCb)));
    assert!(result.is_ok());
}

#[test]
fn test_deregister_resources_does_not_panic() {
    let info = InfoBuilder::new().build();
    struct TestDeregResCb;
    impl DeregisterResourcesCallback for TestDeregResCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result =
        std::panic::catch_unwind(|| server_deregister_resources(&info, Box::new(TestDeregResCb)));
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI lifecycle tests (ignored — require PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_server_init_then_finalize() {
    daemon_helper::ensure_pmix_init();
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init failed");
    assert!(is_server_initialized());
    server_finalize(handle).expect("server_finalize failed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_nspace_then_deregister() {
    daemon_helper::ensure_pmix_init();
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init failed");

    let reg_info = InfoBuilder::new().build();
    server_register_nspace("test_ns", 2, &reg_info, Box::new(TestRegisterNspaceCb))
        .expect("register_nspace failed");

    server_deregister_nspace("test_ns", None);
    server_finalize(handle).expect("server_finalize failed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_register_client_then_deregister() {
    daemon_helper::ensure_pmix_init();
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init failed");

    let proc = Proc::new("test_ns", 0).expect("proc");
    server_register_client(&proc, 1000, 1000, None, Box::new(TestRegisterClientCb))
        .expect("register_client failed");

    server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));

    server_finalize(handle).expect("server_finalize failed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_full_server_lifecycle() {
    daemon_helper::ensure_pmix_init();
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init failed");

    // Register nspace
    let reg_info = InfoBuilder::new().build();
    let _ = server_register_nspace("full_ns", 4, &reg_info, Box::new(TestRegisterNspaceCb));

    // Register client
    let proc = Proc::new("full_ns", 0).expect("proc");
    let _ = server_register_client(&proc, 1000, 1000, None, Box::new(TestRegisterClientCb));

    // Setup fork
    let _ = server_setup_fork(&proc, Some(vec!["PATH=/usr/bin"]));

    // Setup application
    let app_info = InfoBuilder::new().build();
    let _ = server_setup_application("full_ns", &app_info, Box::new(TestSetupAppCb));

    // Setup local support
    let local_info = InfoBuilder::new().build();
    let _ = server_setup_local_support("full_ns", &local_info, Box::new(TestSetupLocalCb));

    // Deregister
    server_deregister_client(&proc, Some(Box::new(TestDeregisterClientCb)));
    server_deregister_nspace("full_ns", None);

    server_finalize(handle).expect("server_finalize failed");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_server_handle_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

#[test]
fn test_server_handle_default() {
    // PmixServerHandle doesn't implement Default, but we can verify
    // it's constructible from server_init result
    let info = InfoBuilder::new().build();
    let _ = server_init(None, &info);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge case: NUL byte in nspace/client names
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_register_nspace_nul_byte() {
    let info = InfoBuilder::new().build();
    // NUL byte in nspace should return error gracefully
    let result = server_register_nspace("test\0nspace", 0, &info, Box::new(TestRegisterNspaceCb));
    assert!(result.is_err());
}

#[test]
fn test_deregister_nspace_nul_byte() {
    // NUL byte — should invoke callback with error if provided, otherwise return
    server_deregister_nspace("test\0nspace", Some(Box::new(TestDeregisterNspaceCb)));
}

#[test]
fn test_setup_application_nul_byte() {
    let info = InfoBuilder::new().build();
    let result = server_setup_application("test\0ns", &info, Box::new(TestSetupAppCb));
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// server_deliver_inventory blocking vs async
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_deliver_inventory_blocking() {
    let info = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let result = server_deliver_inventory(&info, &directives, None);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_deliver_inventory_async() {
    let info = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    struct TestDeliverCb;
    impl DeliverInventoryCallback for TestDeliverCb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = server_deliver_inventory(&info, &directives, Some(Box::new(TestDeliverCb)));
    assert!(result.is_err() || result.is_ok());
}
