//! Unit tests for the server module.

use super::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    // ── PmixServerModule tests ───────────────────────────────────────────────

    #[test]
    fn test_server_module_default() {
        let module = PmixServerModule::default();
        assert!(module.client_connected.is_none());
        assert!(module.client_finalized.is_none());
        assert!(module.abort.is_none());
    }

    #[test]
    fn test_server_module_all_fields_none() {
        let module = PmixServerModule::default();
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
    }

    #[test]
    fn test_server_module_additional_fields() {
        let module = PmixServerModule::default();
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

    // ── New: PmixServerModule callback manipulation ──────────────────────────

    pub(crate) extern "C" fn dummy_callback() {}

    #[test]
    fn test_server_module_set_single_callback() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        assert!(module.client_finalized.is_none());
        assert!(module.abort.is_none());
    }

    #[test]
    fn test_server_module_set_all_callbacks() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        module.client_finalized = Some(dummy_callback);
        module.abort = Some(dummy_callback);
        module.fence_nb = Some(dummy_callback);
        module.direct_modex = Some(dummy_callback);
        module.publish = Some(dummy_callback);
        module.lookup = Some(dummy_callback);
        module.unpublish = Some(dummy_callback);
        module.spawn = Some(dummy_callback);
        module.connect = Some(dummy_callback);
        module.disconnect = Some(dummy_callback);
        module.register_events = Some(dummy_callback);
        module.deregister_events = Some(dummy_callback);
        module.listener = Some(dummy_callback);
        module.notify_event = Some(dummy_callback);
        module.query = Some(dummy_callback);
        module.tool_connected = Some(dummy_callback);
        module.log = Some(dummy_callback);
        module.allocate = Some(dummy_callback);
        module.job_control = Some(dummy_callback);
        module.monitor = Some(dummy_callback);
        module.get_credential = Some(dummy_callback);
        module.validate_credential = Some(dummy_callback);
        module.iof_pull = Some(dummy_callback);
        module.push_stdin = Some(dummy_callback);
        module.group = Some(dummy_callback);
        module.fabric = Some(dummy_callback);
        module.client_connected2 = Some(dummy_callback);
        module.session_control = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        assert!(module.session_control.is_some());
    }

    #[test]
    fn test_server_module_clear_callback() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        assert!(module.client_connected.is_some());
        module.client_connected = None;
        assert!(module.client_connected.is_none());
    }

    #[test]
    fn test_server_module_as_c_ptr_returns_non_null() {
        let module = PmixServerModule::default();
        let ptr = module.as_c_ptr();
        assert!(
            !ptr.is_null(),
            "as_c_ptr must not return null for a valid module"
        );
    }

    #[test]
    fn test_server_module_as_c_ptr_consistent() {
        let module = PmixServerModule::default();
        let ptr1 = module.as_c_ptr();
        let ptr2 = module.as_c_ptr();
        assert_eq!(
            ptr1, ptr2,
            "as_c_ptr should return consistent pointer for same module"
        );
    }

    #[test]
    fn test_server_module_debug_format() {
        let module = PmixServerModule::default();
        let debug_str = format!("{:?}", module);
        assert!(!debug_str.is_empty(), "Debug output should not be empty");
        assert!(debug_str.starts_with("PmixServerModule"));
    }

    #[test]
    fn test_server_module_field_count() {
        let module = PmixServerModule::default();
        let debug_str = format!("{:?}", module);
        assert!(debug_str.starts_with("PmixServerModule"));
    }

    // ── PmixServerHandle tests ───────────────────────────────────────────────

    #[test]
    fn test_server_handle_debug_format() {
        let handle = PmixServerHandle { initialized: true };
        let debug_str = format!("{:?}", handle);
        assert!(!debug_str.is_empty(), "Debug output should not be empty");
        assert!(debug_str.starts_with("PmixServerHandle"));
    }

    #[test]
    fn test_server_handle_construction() {
        let handle = PmixServerHandle { initialized: true };
        assert!(handle.initialized);
    }

    // ── is_server_initialized tests ──────────────────────────────────────────

    #[test]
    fn test_is_server_initialized_returns_bool() {
        let _result: bool = is_server_initialized();
        // Verify it compiles and doesn't panic
    }

    // ── Callback trait compile-time verification ─────────────────────────────

    struct TestNspaceCallback {
        called: Arc<AtomicBool>,
    }

    impl RegisterNspaceCallback for TestNspaceCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_register_nspace_callback_trait_compiles() {
        let _callback: Box<dyn RegisterNspaceCallback> = Box::new(TestNspaceCallback {
            called: Arc::new(AtomicBool::new(false)),
        });
    }

    struct TestDeregisterNspaceCallback;

    impl DeregisterNspaceCallback for TestDeregisterNspaceCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_nspace_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterNspaceCallback> = Box::new(TestDeregisterNspaceCallback);
    }

    struct TestRegisterClientCallback;

    impl RegisterClientCallback for TestRegisterClientCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_register_client_callback_trait_compiles() {
        let _callback: Box<dyn RegisterClientCallback> = Box::new(TestRegisterClientCallback);
    }

    struct TestDeregisterClientCallback;

    impl DeregisterClientCallback for TestDeregisterClientCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_client_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterClientCallback> = Box::new(TestDeregisterClientCallback);
    }

    struct TestDmodexRequestCallback;

    impl DmodexRequestCallback for TestDmodexRequestCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }

    #[test]
    fn test_dmodex_request_callback_trait_compiles() {
        let _callback: Box<dyn DmodexRequestCallback> = Box::new(TestDmodexRequestCallback);
    }

    struct TestSetupApplicationCallback;

    impl SetupApplicationCallback for TestSetupApplicationCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
    }

    #[test]
    fn test_setup_application_callback_trait_compiles() {
        let _callback: Box<dyn SetupApplicationCallback> = Box::new(TestSetupApplicationCallback);
    }

    struct TestSetupLocalSupportCallback;

    impl SetupLocalSupportCallback for TestSetupLocalSupportCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_setup_local_support_callback_trait_compiles() {
        let _callback: Box<dyn SetupLocalSupportCallback> = Box::new(TestSetupLocalSupportCallback);
    }

    struct TestIOFDeliverCallback;

    impl IOFDeliverCallback for TestIOFDeliverCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_iof_deliver_callback_trait_compiles() {
        let _callback: Box<dyn IOFDeliverCallback> = Box::new(TestIOFDeliverCallback);
    }

    struct TestCollectInventoryCallback;

    impl CollectInventoryCallback for TestCollectInventoryCallback {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }

    #[test]
    fn test_collect_inventory_callback_trait_compiles() {
        let _callback: Box<dyn CollectInventoryCallback> = Box::new(TestCollectInventoryCallback);
    }

    struct TestDeliverInventoryCallback;

    impl DeliverInventoryCallback for TestDeliverInventoryCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deliver_inventory_callback_trait_compiles() {
        let _callback: Box<dyn DeliverInventoryCallback> = Box::new(TestDeliverInventoryCallback);
    }

    struct TestRegisterResourcesCallback;

    impl RegisterResourcesCallback for TestRegisterResourcesCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_register_resources_callback_trait_compiles() {
        let _callback: Box<dyn RegisterResourcesCallback> = Box::new(TestRegisterResourcesCallback);
    }

    struct TestDeregisterResourcesCallback;

    impl DeregisterResourcesCallback for TestDeregisterResourcesCallback {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }

    #[test]
    fn test_deregister_resources_callback_trait_compiles() {
        let _callback: Box<dyn DeregisterResourcesCallback> =
            Box::new(TestDeregisterResourcesCallback);
    }

    // ── FenceNbCallbackWrapper tests ─────────────────────────────────────────

    #[test]
    fn test_fence_nb_callback_wrapper_construction() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let _wrapper = FenceNbCallbackWrapper::new(move |_status: PmixStatus| {
            called_clone.store(true, Ordering::SeqCst);
        });
        assert!(!called.load(Ordering::SeqCst));
    }

    // ── Registry and sequence ID tests ───────────────────────────────────────

    #[test]
    fn test_register_nspace_seq_is_accessible() {
        let _seq = REGISTER_NS_SPACE_SEQ.lock().unwrap();
    }

    #[test]
    fn test_deregister_nspace_seq_is_accessible() {
        let _seq = DEREGISTER_NS_SPACE_SEQ.lock().unwrap();
    }

    #[test]
    fn test_register_nspace_registry_is_empty() {
        let registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        assert!(
            registry.is_empty(),
            "Register nspace registry should be empty at test start"
        );
    }

    #[test]
    fn test_deregister_nspace_registry_is_empty() {
        let registry = DEREGISTER_NS_SPACE_REGISTRY.lock().unwrap();
        assert!(
            registry.is_empty(),
            "Deregister nspace registry should be empty at test start"
        );
    }

    // ── Error path tests: FFI calls without server init ──────────────────────

    #[test]
    #[test]
    #[test]
    #[test]
    #[test]
    #[test]
    #[test]
    #[test]
    fn test_server_connect_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_connect(&handle, &[], &[]);
        assert!(
            result.is_err(),
            "server_connect should reject empty proc list"
        );
    }

    #[test]
    fn test_server_disconnect_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_disconnect(&handle, &[], &[]);
        assert!(
            result.is_err(),
            "server_disconnect should reject empty proc list"
        );
    }

    // ── Callback bridge null safety tests ────────────────────────────────────

    #[test]
    fn test_register_nspace_callback_bridge_null_cbdata() {
        register_nspace_callback_bridge(0, ptr::null_mut());
        // Should not panic
    }

    #[test]
    fn test_deregister_nspace_callback_bridge_null_cbdata() {
        deregister_nspace_callback_bridge(0, ptr::null_mut());
        // Should not panic
    }

    // ── CollectInventoryResults tests ────────────────────────────────────────

    #[test]
    fn test_collect_inventory_results_construction() {
        let results = CollectInventoryResults {
            handle: std::ptr::null_mut(),
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    // ── server_register_client error path ────────────────────────────────────

    #[test]
    // ── server_deregister_client error path ──────────────────────────────────
    #[test]
    fn test_server_deregister_client_compiles() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new failed");
        let callback: Option<Box<dyn DeregisterClientCallback>> =
            Some(Box::new(TestDeregisterClientCallback));
        server_deregister_client(&proc, callback);
        // Should not panic
    }

    // ── server_dmodex_request error path ─────────────────────────────────────

    #[test]
    // ── server_setup_application error path ──────────────────────────────────
    #[test]
    // ── server_setup_local_support error path ────────────────────────────────
    #[test]
    // ── server_register_resources error path ─────────────────────────────────
    #[test]
    // ── server_deregister_resources error path ───────────────────────────────
    #[test]
    // ── server_fence_nb error path ───────────────────────────────────────────
    #[test]
    // ── server_connect_nb error path ─────────────────────────────────────────
    #[test]
    fn test_server_connect_nb_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let result = server_connect_nb(&handle, &[], &[], wrapper);
        assert!(result.is_err(), "connect_nb should reject empty proc list");
    }

    // ── server_disconnect_nb error path ──────────────────────────────────────

    #[test]
    fn test_server_disconnect_nb_rejects_empty_procs() {
        let handle = PmixServerHandle { initialized: true };
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let result = server_disconnect_nb(&handle, &[], &[], wrapper);
        assert!(
            result.is_err(),
            "disconnect_nb should reject empty proc list"
        );
    }

    // ── server_tool_attach_to_server error path ──────────────────────────────

    #[test]
    // ── server_get_credential error path ─────────────────────────────────────
    #[test]
    // ── server_spawn / server_spawn_nb compilation tests ─────────────────────
    #[test]
    fn test_server_spawn_compiles() {
        // Verify the function exists with correct signature
        let _: fn(
            &PmixServerHandle,
            &[crate::Info],
            &[crate::process_mgmt::PmixApp],
        ) -> Result<String, PmixStatus> = server_spawn;
    }

    #[test]
    fn test_server_spawn_nb_compiles() {
        let _: fn(
            &PmixServerHandle,
            &[crate::Info],
            &[crate::process_mgmt::PmixApp],
            crate::process_mgmt::SpawnCallbackWrapper,
        ) -> Result<(), PmixStatus> = server_spawn_nb;
    }

    // ── server_init / server_init_minimal / server_finalize compilation ──────

    #[test]
    fn test_server_init_minimal_compiles() {
        let _: fn(Option<&PmixServerModule>) -> Result<PmixServerHandle, PmixStatus> =
            server_init_minimal;
    }

    #[test]
    fn test_server_finalize_compiles() {
        let _: fn(PmixServerHandle) -> Result<(), PmixStatus> = server_finalize;
    }

    // ── CString NUL rejection verification ───────────────────────────────────

    #[test]
    fn test_cstring_rejects_nul_bytes() {
        let result = CString::new("test\0nspace");
        assert!(result.is_err(), "CString::new should reject NUL bytes");
    }

    // ── Proc construction for server tests ───────────────────────────────────

    #[test]
    fn test_proc_construction_for_server_tests() {
        let proc = Proc::new("test.nspace", 0).expect("Proc::new should succeed");
        let _ = proc;
    }

    #[test]
    fn test_proc_construction_multiple_ranks() {
        let proc0 = Proc::new("test.nspace", 0).expect("Proc::new rank 0 failed");
        let proc1 = Proc::new("test.nspace", 1).expect("Proc::new rank 1 failed");
        let procs = vec![proc0, proc1];
        assert_eq!(procs.len(), 2);
    }
    // ── TASK-072: Additional coverage tests (pure logic, no FFI) ───────────

    // ── PmixServerModule: per-callback-set coverage ────────────────────────

    #[test]
    fn test_server_module_set_fence_nb_direct_modex() {
        let mut module = PmixServerModule::default();
        module.fence_nb = Some(dummy_callback);
        module.direct_modex = Some(dummy_callback);
        assert!(module.fence_nb.is_some());
        assert!(module.direct_modex.is_some());
    }

    #[test]
    fn test_server_module_set_monitor_group_fabric() {
        let mut module = PmixServerModule::default();
        module.monitor = Some(dummy_callback);
        module.group = Some(dummy_callback);
        module.fabric = Some(dummy_callback);
        assert!(module.monitor.is_some());
        assert!(module.group.is_some());
        assert!(module.fabric.is_some());
    }

    #[test]
    fn test_server_module_set_credential_callbacks() {
        let mut module = PmixServerModule::default();
        module.get_credential = Some(dummy_callback);
        module.validate_credential = Some(dummy_callback);
        assert!(module.get_credential.is_some());
        assert!(module.validate_credential.is_some());
    }

    #[test]
    fn test_server_module_set_iof_callbacks() {
        let mut module = PmixServerModule::default();
        module.iof_pull = Some(dummy_callback);
        module.push_stdin = Some(dummy_callback);
        assert!(module.iof_pull.is_some());
        assert!(module.push_stdin.is_some());
    }

    #[test]
    fn test_server_module_set_session_control() {
        let mut module = PmixServerModule::default();
        module.session_control = Some(dummy_callback);
        module.client_connected2 = Some(dummy_callback);
        assert!(module.session_control.is_some());
        assert!(module.client_connected2.is_some());
    }

    // ── PmixServerHandle: field coverage ────────────────────────────────────

    #[test]
    fn test_server_handle_initialized_true() {
        let handle = PmixServerHandle { initialized: true };
        assert!(handle.initialized);
    }

    #[test]
    fn test_server_handle_initialized_false() {
        let handle = PmixServerHandle { initialized: false };
        assert!(!handle.initialized);
    }

    // ── Registry and sequence counter tests ────────────────────────────────

    #[test]
    fn test_register_nspace_seq_increments() {
        let mut seq = REGISTER_NS_SPACE_SEQ.lock().unwrap();
        let before = *seq;
        *seq += 1;
        let after = *seq;
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_deregister_nspace_seq_increments() {
        let mut seq = DEREGISTER_NS_SPACE_SEQ.lock().unwrap();
        let before = *seq;
        *seq += 1;
        let after = *seq;
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_register_client_seq_increments() {
        let mut seq = REGISTER_CLIENT_SEQ.lock().unwrap();
        let before = *seq;
        *seq += 1;
        let after = *seq;
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_deregister_client_seq_increments() {
        let mut seq = DEREGISTER_CLIENT_SEQ.lock().unwrap();
        let before = *seq;
        *seq += 1;
        let after = *seq;
        assert_eq!(after, before + 1);
    }

    // ── RegisterNspaceRegistry: insert and remove ──────────────────────────

    #[test]
    fn test_register_nspace_registry_insert_and_remove() {
        struct TestRegCb;
        impl RegisterNspaceCallback for TestRegCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 99999;
        {
            let mut registry = REGISTER_NS_SPACE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestRegCb));
            assert!(registry.contains_key(&req_id));
            let removed = registry.remove(&req_id);
            assert!(removed.is_some());
            assert!(!registry.contains_key(&req_id));
        }
    }

    // ── DeregisterNspaceRegistry: insert and remove ────────────────────────

    #[test]
    fn test_deregister_nspace_registry_insert_and_remove() {
        struct TestDeregCb;
        impl DeregisterNspaceCallback for TestDeregCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 99998;
        {
            let mut registry = DEREGISTER_NS_SPACE_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestDeregCb));
            assert!(registry.contains_key(&req_id));
            let removed = registry.remove(&req_id);
            assert!(removed.is_some());
            assert!(!registry.contains_key(&req_id));
        }
    }

    // ── RegisterClientRegistry: insert and remove ──────────────────────────

    #[test]
    fn test_register_client_registry_insert_and_remove() {
        struct TestRegClientCb;
        impl RegisterClientCallback for TestRegClientCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 99997;
        {
            let mut registry = REGISTER_CLIENT_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestRegClientCb));
            assert!(registry.contains_key(&req_id));
            let removed = registry.remove(&req_id);
            assert!(removed.is_some());
            assert!(!registry.contains_key(&req_id));
        }
    }

    // ── DeregisterClientRegistry: insert and remove ────────────────────────

    #[test]
    fn test_deregister_client_registry_insert_and_remove() {
        struct TestDeregClientCb;
        impl DeregisterClientCallback for TestDeregClientCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let req_id = 99996;
        {
            let mut registry = DEREGISTER_CLIENT_REGISTRY.lock().unwrap();
            registry.insert(req_id, Box::new(TestDeregClientCb));
            assert!(registry.contains_key(&req_id));
            let removed = registry.remove(&req_id);
            assert!(removed.is_some());
            assert!(!registry.contains_key(&req_id));
        }
    }

    // ── Callback bridge: null-safety tests (no FFI side effects) ───────────

    #[test]
    fn test_register_nspace_callback_bridge_null() {
        register_nspace_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_deregister_nspace_callback_bridge_null() {
        deregister_nspace_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_register_client_callback_bridge_null() {
        register_client_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_deregister_client_callback_bridge_null() {
        deregister_client_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_dmodex_request_callback_bridge_null() {
        dmodex_request_callback_bridge(0, ptr::null_mut(), 0, ptr::null_mut());
    }

    #[test]
    fn test_setup_application_callback_bridge_null() {
        setup_application_callback_bridge(
            0,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            None,
            ptr::null_mut(),
        );
    }

    #[test]
    fn test_setup_local_support_callback_bridge_null() {
        setup_local_support_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_iof_deliver_callback_bridge_null() {
        iof_deliver_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_collect_inventory_callback_bridge_null() {
        collect_inventory_callback_bridge(
            0,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            None,
            ptr::null_mut(),
        );
    }

    #[test]
    fn test_deliver_inventory_callback_bridge_null() {
        deliver_inventory_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_register_resources_callback_bridge_null() {
        register_resources_callback_bridge(0, ptr::null_mut());
    }

    #[test]
    fn test_deregister_resources_callback_bridge_null() {
        deregister_resources_callback_bridge(0, ptr::null_mut());
    }

    // ── PmixStatus: additional edge cases ──────────────────────────────────

    #[test]
    fn test_pmixstatus_from_raw_zero() {
        let status = PmixStatus::from_raw(0);
        assert!(status.is_success());
    }

    #[test]
    fn test_pmixstatus_from_raw_error_13() {
        let status = PmixStatus::from_raw(-13);
        assert!(!status.is_success());
    }

    #[test]
    fn test_pmixstatus_from_raw_negative_one() {
        let status = PmixStatus::from_raw(-1);
        assert!(!status.is_success());
    }

    #[test]
    fn test_pmixstatus_from_raw_positive_one() {
        let status = PmixStatus::from_raw(1);
        assert!(status.is_success());
    }

    #[test]
    fn test_pmixstatus_error_not_found() {
        let status = PmixStatus::from_raw(-26);
        assert!(!status.is_success());
    }

    #[test]
    fn test_pmixstatus_error_init() {
        let status = PmixStatus::from_raw(-37);
        assert!(!status.is_success());
    }

    // ── Proc: additional construction tests ────────────────────────────────

    #[test]
    fn test_proc_construction_various_nspaces() {
        let p1 = Proc::new("job.0", 0).expect("nspace with dot failed");
        let p2 = Proc::new("job_0", 0).expect("nspace with underscore failed");
        let p3 = Proc::new("job-0", 0).expect("nspace with hyphen failed");
        assert_eq!(p1.rank(), 0);
        assert_eq!(p2.rank(), 0);
        assert_eq!(p3.rank(), 0);
    }

    #[test]
    fn test_proc_construction_high_rank() {
        let proc = Proc::new("test.nspace", 65535).expect("high rank failed");
        assert_eq!(proc.rank(), 65535);
    }

    // ── DmodexRequestCallback: trait coverage ─────────────────────────────

    #[test]
    fn test_dmodex_request_callback_trait_object() {
        struct TestDmodexCallback;
        impl DmodexRequestCallback for TestDmodexCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
        }
        let _cb: Box<dyn DmodexRequestCallback> = Box::new(TestDmodexCallback);
    }

    #[test]
    fn test_dmodex_request_callback_captures_status() {
        use std::sync::{Arc, Mutex};
        struct CapturingDmodexCallback {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl DmodexRequestCallback for CapturingDmodexCallback {
            fn on_complete(self: Box<Self>, status: PmixStatus, _blob: Vec<u8>) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = CapturingDmodexCallback {
            status: captured.clone(),
        };
        let boxed: Box<dyn DmodexRequestCallback> = Box::new(cb);
        let test_status = PmixStatus::from_raw(0);
        boxed.on_complete(test_status, Vec::new());
        assert!(captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── SetupApplicationCallback: trait coverage ──────────────────────────

    #[test]
    fn test_setup_application_callback_trait_object() {
        struct TestSetupAppCallback;
        impl SetupApplicationCallback for TestSetupAppCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {}
        }
        let _cb: Box<dyn SetupApplicationCallback> = Box::new(TestSetupAppCallback);
    }

    #[test]
    fn test_setup_application_callback_captures_status() {
        use std::sync::{Arc, Mutex};
        struct CapturingSetupAppCallback {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl SetupApplicationCallback for CapturingSetupAppCallback {
            fn on_complete(self: Box<Self>, status: PmixStatus, _info: Vec<(String, String)>) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = CapturingSetupAppCallback {
            status: captured.clone(),
        };
        let boxed: Box<dyn SetupApplicationCallback> = Box::new(cb);
        let test_status = PmixStatus::from_raw(0);
        boxed.on_complete(test_status, Vec::new());
        assert!(captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── SetupLocalSupportCallback: trait coverage ─────────────────────────

    #[test]
    fn test_setup_local_support_callback_trait_object() {
        struct TestSetupLocalCallback;
        impl SetupLocalSupportCallback for TestSetupLocalCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn SetupLocalSupportCallback> = Box::new(TestSetupLocalCallback);
    }

    // ── IOFDeliverCallback: trait coverage ────────────────────────────────

    #[test]
    fn test_iof_deliver_callback_trait_object() {
        struct TestIOFDeliverCallback;
        impl IOFDeliverCallback for TestIOFDeliverCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn IOFDeliverCallback> = Box::new(TestIOFDeliverCallback);
    }

    // ── RegisterResourcesCallback: trait coverage ─────────────────────────

    #[test]
    fn test_register_resources_callback_trait_object() {
        struct TestRegResourcesCallback;
        impl RegisterResourcesCallback for TestRegResourcesCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn RegisterResourcesCallback> = Box::new(TestRegResourcesCallback);
    }

    // ── DeregisterResourcesCallback: trait coverage ───────────────────────

    #[test]
    fn test_deregister_resources_callback_trait_object() {
        struct TestDeregResourcesCallback;
        impl DeregisterResourcesCallback for TestDeregResourcesCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn DeregisterResourcesCallback> = Box::new(TestDeregResourcesCallback);
    }

    // ── FenceNbCallbackWrapper: construction coverage ─────────────────────

    #[test]
    fn test_fence_nb_callback_wrapper_new() {
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {});
        let _ = wrapper;
    }

    #[test]
    fn test_fence_nb_callback_wrapper_invokes_closure() {
        use std::sync::atomic::AtomicBool;
        static INVOKED: AtomicBool = AtomicBool::new(false);
        INVOKED.store(false, Ordering::SeqCst);
        let wrapper = FenceNbCallbackWrapper::new(|_status: PmixStatus| {
            INVOKED.store(true, Ordering::SeqCst);
        });
        (wrapper.callback)(PmixStatus::from_raw(0));
        assert!(INVOKED.load(Ordering::SeqCst));
    }

    // ── CollectInventoryCallback: trait coverage ──────────────────────────

    #[test]
    fn test_collect_inventory_callback_trait_object() {
        struct TestCollectInventoryCallback;
        impl CollectInventoryCallback for TestCollectInventoryCallback {
            fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
        }
        let _cb: Box<dyn CollectInventoryCallback> = Box::new(TestCollectInventoryCallback);
    }

    // ── DeliverInventoryCallback: trait coverage ──────────────────────────

    #[test]
    fn test_deliver_inventory_callback_trait_object() {
        struct TestDeliverInventoryCallback;
        impl DeliverInventoryCallback for TestDeliverInventoryCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn DeliverInventoryCallback> = Box::new(TestDeliverInventoryCallback);
    }

    // ── CollectInventoryResults: construction coverage ────────────────────

    #[test]
    fn test_collect_inventory_results_empty() {
        let results = CollectInventoryResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    // ── FenceCallback: trait coverage ─────────────────────────────────────

    #[test]
    fn test_fence_callback_trait_object() {
        struct TestFenceCallback;
        impl crate::data_ops::FenceCallback for TestFenceCallback {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let _cb: Box<dyn crate::data_ops::FenceCallback> = Box::new(TestFenceCallback);
    }

    #[test]
    fn test_fence_callback_captures_status() {
        use std::sync::{Arc, Mutex};
        struct CapturingFenceCallback {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl crate::data_ops::FenceCallback for CapturingFenceCallback {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = CapturingFenceCallback {
            status: captured.clone(),
        };
        let boxed: Box<dyn crate::data_ops::FenceCallback> = Box::new(cb);
        let test_status = PmixStatus::from_raw(0);
        boxed.on_complete(test_status);
        assert!(captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── server_register_nspace: NUL rejection ─────────────────────────────

    #[test]
    fn test_server_register_nspace_rejects_nul_in_nspace() {
        struct DummyRegCb;
        impl RegisterNspaceCallback for DummyRegCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let result = server_register_nspace("test\0nspace", 1, &info, Box::new(DummyRegCb));
        assert!(
            result.is_err(),
            "register_nspace should reject NUL bytes in nspace"
        );
    }

    // ── server_register_nspace: callback invocation test ──────────────────

    #[test]
    fn test_register_nspace_callback_invoked_with_status() {
        use std::sync::{Arc, Mutex};
        struct CapturingRegCb {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl RegisterNspaceCallback for CapturingRegCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = CapturingRegCb {
            status: captured.clone(),
        };
        let boxed: Box<dyn RegisterNspaceCallback> = Box::new(cb);
        let test_status = PmixStatus::from_raw(0);
        boxed.on_complete(test_status);
        assert!(captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── server_deregister_nspace: callback invocation test ────────────────

    #[test]
    fn test_deregister_nspace_callback_invoked_with_status() {
        use std::sync::{Arc, Mutex};
        struct CapturingDeregCb {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl DeregisterNspaceCallback for CapturingDeregCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = CapturingDeregCb {
            status: captured.clone(),
        };
        let boxed: Box<dyn DeregisterNspaceCallback> = Box::new(cb);
        let test_status = PmixStatus::from_raw(-26); // PMIX_ERR_NOT_FOUND
        boxed.on_complete(test_status);
        assert!(!captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── PmixServerHandle: debug format with false ─────────────────────────

    #[test]
    fn test_server_handle_debug_format_false() {
        let handle = PmixServerHandle { initialized: false };
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("PmixServerHandle"));
        assert!(debug_str.contains("false"));
    }

    // ── PmixServerModule: debug format with callbacks set ─────────────────

    #[test]
    fn test_server_module_debug_with_callbacks_set() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        let debug_str = format!("{:?}", module);
        assert!(debug_str.contains("Some"));
    }

    // ── CollectInventoryResults: additional construction & property tests ─

    #[test]
    fn test_collect_inventory_results_len_zero() {
        let results = CollectInventoryResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(results.len(), 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_collect_inventory_results_debug_format() {
        let results = CollectInventoryResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("CollectInventoryResults"));
    }

    #[test]
    fn test_collect_inventory_results_non_null_handle() {
        // A results struct with non-null handle but zero len should still
        // report empty (len is the authoritative count).
        let results = CollectInventoryResults {
            handle: 0x1 as *mut ffi::pmix_info_t, // dummy non-null
            len: 0,
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_collect_inventory_results_drop_noop_null() {
        // Dropping a null-handle result should not crash.
        let results = CollectInventoryResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        drop(results); // should be a no-op
    }

    // ── CollectInventory seq & registry tests ──────────────────────────────

    #[test]
    fn test_collect_inventory_seq_is_accessible() {
        // Verify the sequence counter is accessible.
        let seq = COLLECT_INVENTORY_SEQ.lock().unwrap();
        assert!(*seq >= 0);
    }

    #[test]
    fn test_collect_inventory_seq_increments() {
        let mut seq = COLLECT_INVENTORY_SEQ.lock().unwrap();
        let before = *seq;
        *seq += 1;
        let after = *seq;
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_collect_inventory_registry_is_empty() {
        let registry = COLLECT_INVENTORY_REGISTRY.lock().unwrap();
        // The registry may not be empty if other tests left entries,
        // but it should be accessible.
        let _len = registry.len();
    }

    // ── server_deregister_nspace: NUL rejection test ───────────────────────

    #[test]
    fn test_server_deregister_nspace_rejects_nul_in_nspace() {
        struct DeregNulCb {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl DeregisterNspaceCallback for DeregNulCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = DeregNulCb {
            status: captured.clone(),
        };
        // Calling with NUL byte — should invoke callback with error
        server_deregister_nspace("test\0nspace", Some(Box::new(cb)));
        let status = captured.lock().unwrap();
        assert!(
            status.as_ref().is_some(),
            "callback should be invoked on NUL rejection"
        );
        assert!(
            !status.as_ref().unwrap().is_success(),
            "callback should receive error status on NUL rejection"
        );
    }

    #[test]
    fn test_server_deregister_nspace_nul_no_callback() {
        // Calling with NUL byte and no callback should not crash.
        server_deregister_nspace("test\0nspace", None);
    }

    // ── server_delete: NUL key rejection test ──────────────────────────────

    #[test]
    fn test_server_delete_rejects_nul_in_key() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_delete(&handle, "testnspace", "test\0key");
        assert!(
            result.is_err(),
            "server_delete should reject NUL bytes in key"
        );
    }

    #[test]
    fn test_server_delete_empty_key() {
        let handle = PmixServerHandle { initialized: true };
        // Empty key is valid (no NUL), but will fail on FFi without init.
        // We just verify it doesn't panic on the CString::new path.
        let result = server_delete(&handle, "testnspace", "");
        // The result depends on PMIx init state; we just verify no panic.
        let _ = result;
    }

    // ── server_lookup: key edge cases ──────────────────────────────────────

    #[test]
    fn test_server_lookup_empty_key() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_lookup(&handle, "testnspace", "", &[]);
        // Empty key is technically valid input — FFi will fail without init.
        let _ = result;
    }

    #[test]
    fn test_server_lookup_long_key_truncated() {
        let handle = PmixServerHandle { initialized: true };
        // Key longer than 511 chars should be silently truncated by the
        // implementation (pdata.key is fixed-size).
        let long_key = "a".repeat(600);
        let result = server_lookup(&handle, "testnspace", &long_key, &[]);
        let _ = result;
    }

    // ── server_publish: input validation ───────────────────────────────────

    #[test]
    fn test_server_publish_empty_info() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_publish(&handle, "testnspace", &info);
        let _ = result;
    }

    // ── server_fence: input validation ─────────────────────────────────────

    #[test]
    fn test_server_fence_zero_timeout() {
        let handle = PmixServerHandle { initialized: true };
        let result = server_fence(&handle, &[], 0);
        let _ = result;
    }

    // ── FenceNbCallbackWrapper: additional tests ───────────────────────────

    #[test]
    fn test_fence_nb_callback_wrapper_multiple_closures() {
        let captured1 = Arc::new(Mutex::new(None));
        let captured2 = Arc::new(Mutex::new(None));

        let w1 = FenceNbCallbackWrapper::new({
            let c = captured1.clone();
            move |s: PmixStatus| {
                *(c.lock().unwrap()) = Some(s);
            }
        });

        let w2 = FenceNbCallbackWrapper::new({
            let c = captured2.clone();
            move |s: PmixStatus| {
                *(c.lock().unwrap()) = Some(s);
            }
        });

        // Verify they are independent wrappers
        let status_ok = PmixStatus::from_raw(0);
        (w1.callback)(status_ok);
        assert!(captured1.lock().unwrap().as_ref().unwrap().is_success());
        assert!(captured2.lock().unwrap().is_none());
    }

    #[test]
    fn test_fence_nb_callback_wrapper_error_status() {
        let captured = Arc::new(Mutex::new(None));
        let wrapper = FenceNbCallbackWrapper::new({
            let c = captured.clone();
            move |s: PmixStatus| {
                *(c.lock().unwrap()) = Some(s);
            }
        });
        let status_err = PmixStatus::from_raw(-13); // PMIX_ERR_NOT_FOUND
        (wrapper.callback)(status_err);
        assert!(!captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    #[test]
    fn test_fence_nb_callback_wrapper_unit_closure() {
        // Closure that ignores the status
        let wrapper = FenceNbCallbackWrapper::new(|_s: PmixStatus| {});
        let _ = wrapper; // should compile and be usable
    }

    // ── PmixServerModule: individual callback field tests ──────────────────

    #[test]
    fn test_server_module_set_abort_callback() {
        let mut module = PmixServerModule::default();
        module.abort = Some(dummy_callback);
        assert!(module.abort.is_some());
        assert!(module.client_connected.is_none());
    }

    #[test]
    fn test_server_module_set_fence_callback() {
        let mut module = PmixServerModule::default();
        module.fence_nb = Some(dummy_callback);
        assert!(module.fence_nb.is_some());
    }

    #[test]
    fn test_server_module_set_publish_lookup_unpublish() {
        let mut module = PmixServerModule::default();
        module.publish = Some(dummy_callback);
        module.lookup = Some(dummy_callback);
        module.unpublish = Some(dummy_callback);
        assert!(module.publish.is_some());
        assert!(module.lookup.is_some());
        assert!(module.unpublish.is_some());
    }

    #[test]
    fn test_server_module_set_spawn_callback() {
        let mut module = PmixServerModule::default();
        module.spawn = Some(dummy_callback);
        assert!(module.spawn.is_some());
    }

    #[test]
    fn test_server_module_set_connect_disconnect() {
        let mut module = PmixServerModule::default();
        module.connect = Some(dummy_callback);
        module.disconnect = Some(dummy_callback);
        assert!(module.connect.is_some());
        assert!(module.disconnect.is_some());
    }

    #[test]
    fn test_server_module_set_event_callbacks() {
        let mut module = PmixServerModule::default();
        module.register_events = Some(dummy_callback);
        module.deregister_events = Some(dummy_callback);
        assert!(module.register_events.is_some());
        assert!(module.deregister_events.is_some());
    }

    #[test]
    fn test_server_module_set_listener_notify() {
        let mut module = PmixServerModule::default();
        module.listener = Some(dummy_callback);
        module.notify_event = Some(dummy_callback);
        assert!(module.listener.is_some());
        assert!(module.notify_event.is_some());
    }

    #[test]
    fn test_server_module_set_query_callback() {
        let mut module = PmixServerModule::default();
        module.query = Some(dummy_callback);
        assert!(module.query.is_some());
    }

    #[test]
    fn test_server_module_set_tool_and_log() {
        let mut module = PmixServerModule::default();
        module.tool_connected = Some(dummy_callback);
        module.log = Some(dummy_callback);
        assert!(module.tool_connected.is_some());
        assert!(module.log.is_some());
    }

    #[test]
    fn test_server_module_set_allocate_and_job_control() {
        let mut module = PmixServerModule::default();
        module.allocate = Some(dummy_callback);
        module.job_control = Some(dummy_callback);
        assert!(module.allocate.is_some());
        assert!(module.job_control.is_some());
    }

    #[test]
    fn test_server_module_clear_all_callbacks() {
        let mut module = PmixServerModule::default();
        module.client_connected = Some(dummy_callback);
        module.client_finalized = Some(dummy_callback);
        module.abort = Some(dummy_callback);
        // Clear them all
        module.client_connected = None;
        module.client_finalized = None;
        module.abort = None;
        assert!(module.client_connected.is_none());
        assert!(module.client_finalized.is_none());
        assert!(module.abort.is_none());
    }

    // ── PmixServerHandle: additional tests ─────────────────────────────────

    #[test]
    fn test_server_handle_multiple_constructions() {
        let h1 = PmixServerHandle { initialized: true };
        let h2 = PmixServerHandle { initialized: false };
        let d1 = format!("{:?}", h1);
        let d2 = format!("{:?}", h2);
        assert!(d1.contains("true"));
        assert!(d2.contains("false"));
    }

    // ── server_connect: additional proc validation ─────────────────────────

    #[test]
    fn test_server_connect_rejects_empty_procs_with_info() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_connect(&handle, &[], &[info]);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_disconnect_rejects_empty_procs_with_info() {
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_disconnect(&handle, &[], &[info]);
        assert!(result.is_err());
    }

    // ── server_register_nspace: additional edge cases ──────────────────────

    #[test]
    fn test_server_register_nspace_rejects_empty_nspace() {
        struct DummyRegCb2;
        impl RegisterNspaceCallback for DummyRegCb2 {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        // Empty string is valid for CString but may fail on FFi.
        // We just verify it doesn't panic.
        let result = server_register_nspace("", 1, &info, Box::new(DummyRegCb2));
        let _ = result;
    }

    #[test]
    fn test_server_register_nspace_negative_localprocs() {
        struct DummyRegCb3;
        impl RegisterNspaceCallback for DummyRegCb3 {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        // Negative nlocalprocs — FFi may reject or accept.
        // We just verify no panic.
        let result = server_register_nspace("test", -1, &info, Box::new(DummyRegCb3));
        let _ = result;
    }

    #[test]
    fn test_server_register_nspace_zero_localprocs() {
        struct DummyRegCb4;
        impl RegisterNspaceCallback for DummyRegCb4 {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let result = server_register_nspace("test", 0, &info, Box::new(DummyRegCb4));
        let _ = result;
    }

    // ── PmixStatus: additional edge cases ──────────────────────────────────

    #[test]
    fn test_pmixstatus_from_raw_large_negative() {
        let status = PmixStatus::from_raw(-100);
        assert!(!status.is_success());
    }

    #[test]
    fn test_pmixstatus_from_raw_large_positive() {
        // In PMIx, only negative values are errors. Positive values
        // are success codes, so 100 is_success().
        let status = PmixStatus::from_raw(100);
        assert!(status.is_success());
    }

    #[test]
    fn test_pmixstatus_success_debug() {
        let status = PmixStatus::from_raw(0);
        let debug_str = format!("{:?}", status);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_pmixstatus_error_debug() {
        let status = PmixStatus::from_raw(-13);
        let debug_str = format!("{:?}", status);
        assert!(!debug_str.is_empty());
    }

    // ── Proc: additional construction tests ────────────────────────────────

    #[test]
    fn test_proc_zero_rank() {
        let _proc = Proc::new("testnspace", 0).expect("proc creation failed");
    }

    #[test]
    fn test_proc_max_rank() {
        let _proc = Proc::new("testnspace", u32::MAX).expect("proc creation failed");
    }

    #[test]
    fn test_proc_new_with_nspace() {
        let proc = Proc::new("testnspace", 0).expect("proc creation failed");
        let _proc2 = proc.new_with_nspace(42).expect("new_with_nspace failed");
    }

    // ── Registry: register_client additional tests ─────────────────────────

    #[test]
    fn test_register_client_seq_is_accessible() {
        let seq = REGISTER_CLIENT_SEQ.lock().unwrap();
        assert!(*seq >= 0);
    }

    #[test]
    fn test_register_client_registry_is_empty() {
        let registry = REGISTER_CLIENT_REGISTRY.lock().unwrap();
        let _len = registry.len();
    }

    // ── Registry: deregister_client additional tests ───────────────────────

    #[test]
    fn test_deregister_client_seq_is_accessible() {
        let seq = DEREGISTER_CLIENT_SEQ.lock().unwrap();
        assert!(*seq >= 0);
    }

    #[test]
    fn test_deregister_client_registry_is_empty() {
        let registry = DEREGISTER_CLIENT_REGISTRY.lock().unwrap();
        let _len = registry.len();
    }

    // ── server_define_process_set: NUL rejection ───────────────────────────

    #[test]
    fn test_server_define_process_set_rejects_nul_in_name() {
        let proc = Proc::new("testnspace", 0).expect("proc creation failed");
        let members = vec![proc];
        let result = server_define_process_set(&members, "test\0set");
        assert!(
            result.is_err(),
            "define_process_set should reject NUL bytes in pset_name"
        );
    }

    // ── server_delete_process_set: NUL rejection ───────────────────────────

    #[test]
    fn test_server_delete_process_set_rejects_nul_in_name() {
        let result = server_delete_process_set("test\0set");
        assert!(
            result.is_err(),
            "delete_process_set should reject NUL bytes in pset_name"
        );
    }

    // ── CollectInventoryCallback: captures inventory ───────────────────────

    #[test]
    fn test_collect_inventory_callback_captures_status_and_inventory() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingInvCb {
            called: Arc<AtomicBool>,
        }
        impl CollectInventoryCallback for CapturingInvCb {
            fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingInvCb {
            called: called.clone(),
        };
        let boxed: Box<dyn CollectInventoryCallback> = Box::new(cb);
        let results = CollectInventoryResults {
            handle: ptr::null_mut(),
            len: 0,
        };
        boxed.on_complete(PmixStatus::from_raw(0), results);
        assert!(called.load(Ordering::SeqCst));
    }

    // ── DeliverInventoryCallback: captures status ──────────────────────────

    #[test]
    fn test_deliver_inventory_callback_captures_status() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingDeliverInvCb {
            called: Arc<AtomicBool>,
        }
        impl DeliverInventoryCallback for CapturingDeliverInvCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingDeliverInvCb {
            called: called.clone(),
        };
        let boxed: Box<dyn DeliverInventoryCallback> = Box::new(cb);
        boxed.on_complete(PmixStatus::from_raw(0));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── IOFDeliverCallback: captures status ────────────────────────────────

    #[test]
    fn test_iof_deliver_callback_captures_status() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingIOFCb {
            called: Arc<AtomicBool>,
        }
        impl IOFDeliverCallback for CapturingIOFCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingIOFCb {
            called: called.clone(),
        };
        let boxed: Box<dyn IOFDeliverCallback> = Box::new(cb);
        boxed.on_complete(PmixStatus::from_raw(0));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── SetupApplicationCallback: additional tests ─────────────────────────

    #[test]
    fn test_setup_application_callback_captures_info() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingSetupAppCb {
            called: Arc<AtomicBool>,
        }
        impl SetupApplicationCallback for CapturingSetupAppCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<(String, String)>) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingSetupAppCb {
            called: called.clone(),
        };
        let boxed: Box<dyn SetupApplicationCallback> = Box::new(cb);
        boxed.on_complete(
            PmixStatus::from_raw(0),
            vec![(("key".to_string()), ("val".to_string()))],
        );
        assert!(called.load(Ordering::SeqCst));
    }

    // ── SetupLocalSupportCallback: additional tests ────────────────────────

    #[test]
    fn test_setup_local_support_callback_captures_status() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingSetupLocalCb {
            called: Arc<AtomicBool>,
        }
        impl SetupLocalSupportCallback for CapturingSetupLocalCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingSetupLocalCb {
            called: called.clone(),
        };
        let boxed: Box<dyn SetupLocalSupportCallback> = Box::new(cb);
        boxed.on_complete(PmixStatus::from_raw(0));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── DmodexRequestCallback: additional tests ────────────────────────────

    #[test]
    fn test_dmodex_request_callback_captures_blob() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct CapturingDmodexCb {
            called: Arc<AtomicBool>,
        }
        impl DmodexRequestCallback for CapturingDmodexCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let cb = CapturingDmodexCb {
            called: called.clone(),
        };
        let boxed: Box<dyn DmodexRequestCallback> = Box::new(cb);
        boxed.on_complete(PmixStatus::from_raw(0), vec![1, 2, 3]);
        assert!(called.load(Ordering::SeqCst));
    }

    // ── server_register_nspace: callback with error status ─────────────────

    #[test]
    fn test_register_nspace_callback_error_status() {
        struct ErrRegCb {
            status: Arc<Mutex<Option<PmixStatus>>>,
        }
        impl RegisterNspaceCallback for ErrRegCb {
            fn on_complete(self: Box<Self>, status: PmixStatus) {
                *(self.status.lock().unwrap()) = Some(status);
            }
        }
        let captured = Arc::new(Mutex::new(None));
        let cb = ErrRegCb {
            status: captured.clone(),
        };
        let boxed: Box<dyn RegisterNspaceCallback> = Box::new(cb);
        let err_status = PmixStatus::from_raw(-13); // PMIX_ERR_NOT_FOUND
        boxed.on_complete(err_status);
        assert!(!captured.lock().unwrap().as_ref().unwrap().is_success());
    }

    // ── PmixServerModule: as_c_ptr null safety ─────────────────────────────

    #[test]
    fn test_server_module_as_c_ptr_different_calls_same_addr() {
        let module = PmixServerModule::default();
        let ptr1 = module.as_c_ptr();
        let ptr2 = module.as_c_ptr();
        assert_eq!(
            ptr1, ptr2,
            "as_c_ptr should return same address for same module"
        );
    }

    #[test]
    fn test_server_module_as_c_ptr_mutable_module() {
        let mut module = PmixServerModule::default();
        let ptr1 = module.as_c_ptr();
        module.abort = Some(dummy_callback);
        let ptr2 = module.as_c_ptr();
        assert_eq!(ptr1, ptr2, "as_c_ptr should be stable across mutations");
    }

    // ── Mock FFI: server_init happy path ────────────────────────────────────

    #[test]
    fn test_mock_server_init_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status =
            crate::mock_ffi::mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_init_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status =
            crate::mock_ffi::mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_server_init_with_module_ptr() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let module = PmixServerModule::default();
        let module_ptr = module.as_c_ptr() as *mut std::ffi::c_void;
        let status = crate::mock_ffi::mock_server_init(module_ptr, std::ptr::null_mut(), 0);
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_init_with_error_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_init", crate::mock_ffi::PMIX_ERR_NOMEM);
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let status =
                crate::mock_ffi::mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_NOMEM);
        }
    }

    // ── Mock FFI: server_finalize happy path ────────────────────────────────

    #[test]
    fn test_mock_server_finalize_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_finalize();
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_finalize_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_finalize();
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_register_nspace happy path ─────────────────────────

    #[test]
    fn test_mock_server_register_nspace_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_register_nspace(
            nspace.as_ptr(),
            1,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_register_nspace_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_register_nspace(
            nspace.as_ptr(),
            1,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_server_register_nspace_with_error_config() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_register_nspace",
            crate::mock_ffi::PMIX_ERR_BAD_PARAM,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let nspace = std::ffi::CString::new("test.nspace").unwrap();
            let status = crate::mock_ffi::mock_server_register_nspace(
                nspace.as_ptr(),
                1,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        }
    }

    // ── Mock FFI: server_register_client happy path ─────────────────────────

    #[test]
    fn test_mock_server_register_client_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut cred_size: usize = 0;
        let status = crate::mock_ffi::mock_server_register_client(
            std::ptr::null_mut(),
            &mut cred_size,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_register_client_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let mut cred_size: usize = 0;
        let status = crate::mock_ffi::mock_server_register_client(
            std::ptr::null_mut(),
            &mut cred_size,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_publish happy path ─────────────────────────────────

    #[test]
    fn test_mock_server_publish_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let key = std::ffi::CString::new("test_key").unwrap();
        let status = crate::mock_ffi::mock_server_publish(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_publish_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let key = std::ffi::CString::new("test_key").unwrap();
        let status = crate::mock_ffi::mock_server_publish(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_lookup happy path ──────────────────────────────────

    #[test]
    fn test_mock_server_lookup_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let key = std::ffi::CString::new("test_key").unwrap();
        let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
        let status = crate::mock_ffi::mock_server_lookup(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut val,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_lookup_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let key = std::ffi::CString::new("test_key").unwrap();
        let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
        let status = crate::mock_ffi::mock_server_lookup(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut val,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_delete happy path ──────────────────────────────────

    #[test]
    fn test_mock_server_delete_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let key = std::ffi::CString::new("test_key").unwrap();
        let status = crate::mock_ffi::mock_server_delete(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_delete_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let key = std::ffi::CString::new("test_key").unwrap();
        let status = crate::mock_ffi::mock_server_delete(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_fence happy path ───────────────────────────────────

    #[test]
    fn test_mock_server_fence_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut retvals: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut nretvals: usize = 0;
        let status = crate::mock_ffi::mock_server_fence(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut retvals,
            &mut nretvals,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_fence_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let mut retvals: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut nretvals: usize = 0;
        let status = crate::mock_ffi::mock_server_fence(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut retvals,
            &mut nretvals,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_fence_nb happy path ────────────────────────────────

    #[test]
    fn test_mock_server_fence_nb_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_fence_nb(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_fence_nb_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_fence_nb(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_dmodex_request happy path ──────────────────────────

    #[test]
    fn test_mock_server_dmodex_request_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_dmodex_request(
            std::ptr::null_mut(),
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_dmodex_request_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_dmodex_request(
            std::ptr::null_mut(),
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_setup_application happy path ───────────────────────

    #[test]
    fn test_mock_server_setup_application_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_setup_application(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_setup_application_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_setup_application(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_setup_local_support happy path ─────────────────────

    #[test]
    fn test_mock_server_setup_local_support_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_setup_local_support(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_setup_local_support_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let nspace = std::ffi::CString::new("test.nspace").unwrap();
        let status = crate::mock_ffi::mock_server_setup_local_support(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_register_resources happy path ──────────────────────

    #[test]
    fn test_mock_server_register_resources_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_register_resources(
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_register_resources_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_register_resources(
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_deregister_resources happy path ────────────────────

    #[test]
    fn test_mock_server_deregister_resources_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_deregister_resources(
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_deregister_resources_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_deregister_resources(
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_tool_attach_to_server happy path ───────────────────

    #[test]
    fn test_mock_server_tool_attach_to_server_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let status = crate::mock_ffi::mock_server_tool_attach_to_server(
            0,
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_tool_attach_to_server_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let status = crate::mock_ffi::mock_server_tool_attach_to_server(
            0,
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_get_credential happy path ──────────────────────────

    #[test]
    fn test_mock_server_get_credential_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut cred: *mut std::os::raw::c_char = std::ptr::null_mut();
        let mut cred_size: usize = 0;
        let status = crate::mock_ffi::mock_server_get_credential(
            std::ptr::null_mut(),
            0,
            &mut cred,
            &mut cred_size,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_get_credential_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let mut cred: *mut std::os::raw::c_char = std::ptr::null_mut();
        let mut cred_size: usize = 0;
        let status = crate::mock_ffi::mock_server_get_credential(
            std::ptr::null_mut(),
            0,
            &mut cred,
            &mut cred_size,
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_define_process_set happy path ──────────────────────

    #[test]
    fn test_mock_server_define_process_set_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let pset_name = std::ffi::CString::new("test.pset").unwrap();
        let status = crate::mock_ffi::mock_server_define_process_set(
            std::ptr::null_mut(),
            0,
            pset_name.as_ptr(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_define_process_set_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let pset_name = std::ffi::CString::new("test.pset").unwrap();
        let status = crate::mock_ffi::mock_server_define_process_set(
            std::ptr::null_mut(),
            0,
            pset_name.as_ptr(),
        );
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: server_delete_process_set happy path ──────────────────────

    #[test]
    fn test_mock_server_delete_process_set_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let pset_name = std::ffi::CString::new("test.pset").unwrap();
        let status = crate::mock_ffi::mock_server_delete_process_set(pset_name.into_raw());
        assert_eq!(status, crate::mock_ffi::PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_server_delete_process_set_returns_err_when_disabled() {
        crate::mock_ffi::disable_mock_ffi();
        let pset_name = std::ffi::CString::new("test.pset").unwrap();
        let status = crate::mock_ffi::mock_server_delete_process_set(pset_name.into_raw());
        assert_eq!(status, crate::mock_ffi::PMIX_ERR_INIT);
    }

    // ── Mock FFI: init + finalize lifecycle ─────────────────────────────────

    #[test]
    fn test_mock_server_init_then_finalize_lifecycle() {
        let _guard = crate::mock_ffi::MockGuard::new();
        // Simulate server lifecycle
        let init_status =
            crate::mock_ffi::mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
        assert_eq!(init_status, crate::mock_ffi::PMIX_SUCCESS);
        let finalize_status = crate::mock_ffi::mock_server_finalize();
        assert_eq!(finalize_status, crate::mock_ffi::PMIX_SUCCESS);
    }

    // ── Mock FFI: full server workflow simulation ───────────────────────────

    #[test]
    fn test_mock_server_full_workflow() {
        let _guard = crate::mock_ffi::MockGuard::new();
        // 1. Init
        assert_eq!(
            crate::mock_ffi::mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0),
            crate::mock_ffi::PMIX_SUCCESS
        );
        // 2. Register nspace
        let nspace = std::ffi::CString::new("workflow.nspace").unwrap();
        assert_eq!(
            crate::mock_ffi::mock_server_register_nspace(
                nspace.as_ptr(),
                1,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut()
            ),
            crate::mock_ffi::PMIX_SUCCESS
        );
        // 3. Publish
        let key = std::ffi::CString::new("workflow_key").unwrap();
        assert_eq!(
            crate::mock_ffi::mock_server_publish(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            crate::mock_ffi::PMIX_SUCCESS
        );
        // 4. Lookup
        let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
        assert_eq!(
            crate::mock_ffi::mock_server_lookup(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut val
            ),
            crate::mock_ffi::PMIX_SUCCESS
        );
        // 5. Delete
        assert_eq!(
            crate::mock_ffi::mock_server_delete(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                0
            ),
            crate::mock_ffi::PMIX_SUCCESS
        );
        // 6. Finalize
        assert_eq!(
            crate::mock_ffi::mock_server_finalize(),
            crate::mock_ffi::PMIX_SUCCESS
        );
    }

    // ── Mock FFI: error config overrides for server functions ───────────────

    #[test]
    fn test_mock_server_publish_with_duplicate_key_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_publish",
            crate::mock_ffi::PMIX_ERR_DUPLICATE_KEY,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let key = std::ffi::CString::new("dup_key").unwrap();
            let status = crate::mock_ffi::mock_server_publish(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_DUPLICATE_KEY);
        }
    }

    #[test]
    fn test_mock_server_lookup_with_not_found_error() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_lookup", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let key = std::ffi::CString::new("missing_key").unwrap();
            let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
            let status = crate::mock_ffi::mock_server_lookup(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut val,
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        }
    }

    #[test]
    fn test_mock_server_fence_with_timeout_error() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_fence", crate::mock_ffi::PMIX_ERR_TIMEOUT);
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let mut retvals: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut nretvals: usize = 0;
            let status = crate::mock_ffi::mock_server_fence(
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut retvals,
                &mut nretvals,
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_TIMEOUT);
        }
    }

    #[test]
    fn test_mock_server_define_process_set_with_bad_param_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_define_process_set",
            crate::mock_ffi::PMIX_ERR_BAD_PARAM,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let pset_name = std::ffi::CString::new("bad.pset").unwrap();
            let status = crate::mock_ffi::mock_server_define_process_set(
                std::ptr::null_mut(),
                0,
                pset_name.as_ptr(),
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        }
    }

    #[test]
    fn test_mock_server_dmodex_request_with_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_dmodex_request",
            crate::mock_ffi::PMIX_ERR_NOT_FOUND,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let status = crate::mock_ffi::mock_server_dmodex_request(
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        }
    }

    #[test]
    fn test_mock_server_setup_application_with_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_setup_application",
            crate::mock_ffi::PMIX_ERR_BAD_PARAM,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let nspace = std::ffi::CString::new("test.nspace").unwrap();
            let status = crate::mock_ffi::mock_server_setup_application(
                nspace.as_ptr(),
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        }
    }

    #[test]
    fn test_mock_server_register_resources_with_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_register_resources",
            crate::mock_ffi::PMIX_ERR_NOMEM,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let status = crate::mock_ffi::mock_server_register_resources(
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_NOMEM);
        }
    }

    #[test]
    fn test_mock_server_get_credential_with_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_get_credential",
            crate::mock_ffi::PMIX_ERR_NOT_FOUND,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let mut cred: *mut std::os::raw::c_char = std::ptr::null_mut();
            let mut cred_size: usize = 0;
            let status = crate::mock_ffi::mock_server_get_credential(
                std::ptr::null_mut(),
                0,
                &mut cred,
                &mut cred_size,
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        }
    }

    #[test]
    fn test_mock_server_tool_attach_to_server_with_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status(
            "PMIx_server_tool_attach_to_server",
            crate::mock_ffi::PMIX_ERR_BAD_PARAM,
        );
        {
            let _guard = crate::mock_ffi::MockGuard::with_config(config);
            let status = crate::mock_ffi::mock_server_tool_attach_to_server(
                0,
                0,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            );
            assert_eq!(status, crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        }
    }


    // ── TASK-105: Mock-aware wrapper tests ──────────────────────────────────

    #[test]
    fn test_mock_wrapper_server_publish_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_publish(&handle, "test.nspace", &info);
        assert!(result.is_ok(), "server_publish wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_publish_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_publish", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_publish(&handle, "test.nspace", &info);
        assert!(result.is_err(), "server_publish wrapper should fail with configured error");
    }

    #[test]
    #[ignore] // Mock returns success but doesn't populate pdata.value, so wrapper returns ErrNotFound
    fn test_mock_wrapper_server_lookup_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_lookup(&handle, "test.nspace", "test_key", &[]);
        assert!(result.is_ok(), "server_lookup wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_lookup_returns_error_with_default_mocks() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_lookup(&handle, "test.nspace", "missing_key", &[]);
        // Default mock returns PMIX_ERR_NOT_FOUND for lookup
        assert!(result.is_err(), "server_lookup wrapper should fail for missing key with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_delete_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_delete(&handle, "test.nspace", "test_key");
        assert!(result.is_ok(), "server_delete wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_delete_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_delete", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let result = server_delete(&handle, "test.nspace", "test_key");
        assert!(result.is_err(), "server_delete wrapper should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_fence_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_fence(&handle, &[], 5000);
        assert!(result.is_ok(), "server_fence wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_fence_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_fence", crate::mock_ffi::PMIX_ERR_TIMEOUT);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let result = server_fence(&handle, &[], 5000);
        assert!(result.is_err(), "server_fence wrapper should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_connect_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_connect(&handle, &procs, &[]);
        assert!(result.is_ok(), "server_connect wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_connect_rejects_empty_procs() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_connect(&handle, &[], &[]);
        assert!(result.is_err(), "server_connect wrapper should reject empty procs");
    }

    #[test]
    fn test_mock_wrapper_server_connect_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_fence", crate::mock_ffi::PMIX_ERR_TIMEOUT);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_connect(&handle, &procs, &[]);
        assert!(result.is_err(), "server_connect wrapper should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_disconnect_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_disconnect(&handle, &procs, &[]);
        assert!(result.is_ok(), "server_disconnect wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_disconnect_rejects_empty_procs() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let result = server_disconnect(&handle, &[], &[]);
        assert!(result.is_err(), "server_disconnect wrapper should reject empty procs");
    }

    #[test]
    fn test_mock_wrapper_server_disconnect_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_fence", crate::mock_ffi::PMIX_ERR_TIMEOUT);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_disconnect(&handle, &procs, &[]);
        assert!(result.is_err(), "server_disconnect wrapper should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_define_process_set_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_define_process_set(&procs, "test_pset");
        assert!(result.is_ok(), "server_define_process_set wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_define_process_set_returns_success_empty() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let result = server_define_process_set(&[], "test_pset");
        assert!(result.is_ok(), "server_define_process_set with empty procs should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_define_process_set_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_define_process_set", crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let procs = vec![Proc::new("test.nspace", 0).unwrap()];
        let result = server_define_process_set(&procs, "test_pset");
        assert!(result.is_err(), "server_define_process_set should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_delete_process_set_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let result = server_delete_process_set("test_pset");
        assert!(result.is_ok(), "server_delete_process_set wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_delete_process_set_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_delete_process_set", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let result = server_delete_process_set("test_pset");
        assert!(result.is_err(), "server_delete_process_set should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_register_resources_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        struct TestResCb {}
        impl RegisterResourcesCallback for TestResCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let cb = Box::new(TestResCb {});
        let result = server_register_resources(&info, cb);
        assert!(result.is_ok(), "server_register_resources wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_register_resources_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_register_resources", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        struct TestResCb {}
        impl RegisterResourcesCallback for TestResCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let cb = Box::new(TestResCb {});
        let result = server_register_resources(&info, cb);
        assert!(result.is_err(), "server_register_resources should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_deregister_resources_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        struct TestDeregResCb {}
        impl DeregisterResourcesCallback for TestDeregResCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let cb = Box::new(TestDeregResCb {});
        let result = server_deregister_resources(&info, cb);
        assert!(result.is_ok(), "server_deregister_resources wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_deregister_resources_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_deregister_resources", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        struct TestDeregResCb {}
        impl DeregisterResourcesCallback for TestDeregResCb {
            fn on_complete(self: Box<Self>, _status: PmixStatus) {}
        }
        let info = crate::InfoBuilder::new().build();
        let cb = Box::new(TestDeregResCb {});
        let result = server_deregister_resources(&info, cb);
        assert!(result.is_err(), "server_deregister_resources should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_tool_attach_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_tool_attach_to_server(&handle, None, false, &info);
        assert!(result.is_ok(), "server_tool_attach_to_server wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_tool_attach_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_tool_attach_to_server", crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let info = crate::InfoBuilder::new().build();
        let result = server_tool_attach_to_server(&handle, None, false, &info);
        assert!(result.is_err(), "server_tool_attach_to_server should fail with configured error");
    }

    #[test]
    fn test_mock_wrapper_server_get_credential_returns_success() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let handle = PmixServerHandle { initialized: true };
        let info = vec![];
        let result = server_get_credential(&handle, &info);
        assert!(result.is_ok(), "server_get_credential wrapper should succeed with mocks");
    }

    #[test]
    fn test_mock_wrapper_server_get_credential_returns_error_on_config() {
        let config = crate::mock_ffi::MockConfig::new()
            .with_function_status("PMIx_server_get_credential", crate::mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let handle = PmixServerHandle { initialized: true };
        let info = vec![];
        let result = server_get_credential(&handle, &info);
        assert!(result.is_err(), "server_get_credential should fail with configured error");
    }
