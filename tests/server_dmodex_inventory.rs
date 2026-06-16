//! Batch 12 — server_dmodex_inventory tests.
//!
//! Tests 8 server dmodex/inventory functions with focus on compile-time type
//! checks, panic safety, callback traits, and signature verification:
//!
//! - server_dmodex_request
//! - server_collect_inventory
//! - server_deliver_inventory
//! - server_define_process_set
//! - server_delete_process_set
//! - server_generate_cpuset_string
//! - server_generate_locality_string
//! - server_iof_deliver
//!
//! IMPORTANT: Tests that call server_init_minimal corrupt C-level PMIx state.
//! All daemon-dependent tests are marked #[ignore].

use pmix::data_serialization::PmixByteObject;
use pmix::fabric::PmixCpuset;
use pmix::server::{
    CollectInventoryCallback, CollectInventoryResults, DeliverInventoryCallback,
    DmodexRequestCallback, IOFDeliverCallback, server_collect_inventory,
    server_deliver_inventory, server_delete_process_set, server_dmodex_request,
    server_define_process_set, server_generate_cpuset_string, server_generate_locality_string,
    server_iof_deliver,
};
use pmix::{Info, InfoBuilder, IOFChannelFlags, PmixError, PmixStatus, Proc};

// ============================================================================
// 1. server_dmodex_request — compile-time signature & callback trait checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn dmodex_request_signature() {
    let _: fn(&Proc, Box<dyn DmodexRequestCallback>) -> Result<(), PmixStatus> =
        server_dmodex_request;
}

/// DmodexRequestCallback trait: object-safe with Box<Self> receiver.
#[test]
fn dmodex_request_callback_box_self() {
    struct Cb;
    impl DmodexRequestCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus, _blob: Vec<u8>) {}
    }
    let _: Box<dyn DmodexRequestCallback> = Box::new(Cb);
}

/// DmodexRequestCallback on_complete receives (PmixStatus, Vec<u8>).
#[test]
fn dmodex_request_callback_param_types() {
    struct Cb;
    impl DmodexRequestCallback for Cb {
        fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
            let _: PmixStatus = status;
            let _: Vec<u8> = blob;
        }
    }
}

/// DmodexRequestCallback is Send (cross-thread safety).
#[test]
fn dmodex_request_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DmodexRequestCallback>>();
}

/// DmodexRequestCallback can be a zero-sized type.
#[test]
fn dmodex_request_callback_zst() {
    struct Empty;
    impl DmodexRequestCallback for Empty {
        fn on_complete(self: Box<Self>, _s: PmixStatus, _b: Vec<u8>) {}
    }
    let _: Box<dyn DmodexRequestCallback> = Box::new(Empty);
}

/// DmodexRequestCallback can capture state.
#[test]
fn dmodex_request_callback_with_state() {
    struct Stateful {
        job: String,
        rank: u32,
    }
    impl DmodexRequestCallback for Stateful {
        fn on_complete(self: Box<Self>, _s: PmixStatus, _b: Vec<u8>) {
            let _ = &self.job;
            let _ = self.rank;
        }
    }
    let _: Box<dyn DmodexRequestCallback> = Box::new(Stateful {
        job: "job.123".into(),
        rank: 42,
    });
}

/// server_dmodex_request does not panic — returns Result.
#[test]
fn dmodex_request_no_panic_returns_result() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let proc = Proc::new("test", 0).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        server_dmodex_request(&proc, Box::new(Nop))
    }));
    assert!(result.is_ok(), "should return Result, not panic");
}

/// server_dmodex_request before init returns PMIX_ERR_INIT.
#[test]
fn dmodex_request_before_init_err_init() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let proc = Proc::new("test", 0).unwrap();
    let result = server_dmodex_request(&proc, Box::new(Nop));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -31); // PMIX_ERR_INIT
}

/// Multiple procs all return PMIX_ERR_INIT consistently.
#[test]
fn dmodex_request_multiple_procs_consistent() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    for rank in [0u32, 1, 100, u32::MAX] {
        let proc = Proc::new("test", rank).unwrap();
        let result = server_dmodex_request(&proc, Box::new(Nop));
        assert_eq!(result.unwrap_err().to_raw(), -31);
    }
}

/// dmodex_request with wildcard proc (rank = PMIX_RANK_WILDCARD = u32::MAX).
#[test]
fn dmodex_request_wildcard_proc() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let proc = Proc::new("wildcard", u32::MAX).unwrap();
    let result = server_dmodex_request(&proc, Box::new(Nop));
    assert_eq!(result.unwrap_err().to_raw(), -31);
}

/// dmodex_request with various nspace formats.
#[test]
fn dmodex_request_various_nspaces() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let long_nspace = "x".repeat(255);
    for nspace in ["a", "job.12345", "myapp", &long_nspace] {
        let proc = Proc::new(nspace, 0).unwrap();
        let result = server_dmodex_request(&proc, Box::new(Nop));
        assert!(result.is_err());
    }
}

/// dmodex_request error is PmixError::ErrInit variant.
#[test]
fn dmodex_request_error_is_err_init_variant() {
    struct Nop;
    impl DmodexRequestCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let proc = Proc::new("test", 0).unwrap();
    let err = server_dmodex_request(&proc, Box::new(Nop)).unwrap_err();
    match err {
        PmixStatus::Known(e) => assert_eq!(e, PmixError::ErrInit),
        PmixStatus::Unknown(c) => assert_eq!(c, -31),
    }
}

/// dmodex_request with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn dmodex_request_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    struct Cb;
    impl DmodexRequestCallback for Cb {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let proc = Proc::new("test", 0).unwrap();
    let _ = server_dmodex_request(&proc, Box::new(Cb));
}

// ============================================================================
// 2. server_collect_inventory — compile-time signature & callback trait checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn collect_inventory_signature() {
    let _: fn(&Info, Box<dyn CollectInventoryCallback>) -> Result<(), PmixStatus> =
        server_collect_inventory;
}

/// CollectInventoryCallback trait: object-safe with &self receiver.
#[test]
fn collect_inventory_callback_ref_self() {
    struct Cb;
    impl CollectInventoryCallback for Cb {
        fn on_complete(&self, _status: PmixStatus, _inventory: CollectInventoryResults) {}
    }
    let _: Box<dyn CollectInventoryCallback> = Box::new(Cb);
}

/// CollectInventoryCallback on_complete receives (PmixStatus, CollectInventoryResults).
#[test]
fn collect_inventory_callback_param_types() {
    struct Cb;
    impl CollectInventoryCallback for Cb {
        fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
            let _: PmixStatus = status;
            let _: CollectInventoryResults = inventory;
        }
    }
}

/// CollectInventoryCallback is Send.
#[test]
fn collect_inventory_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn CollectInventoryCallback>>();
}

/// CollectInventoryResults is Debug.
#[test]
fn collect_inventory_results_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<CollectInventoryResults>();
}

/// CollectInventoryResults has Drop (memory safety).
#[test]
fn collect_inventory_results_has_drop() {
    fn assert_drop<T: Drop>() {}
    assert_drop::<CollectInventoryResults>();
}

/// CollectInventoryResults::len() returns usize.
#[test]
fn collect_inventory_results_len_returns_usize() {
    struct LenCheck;
    impl CollectInventoryCallback for LenCheck {
        fn on_complete(&self, _status: PmixStatus, inventory: CollectInventoryResults) {
            let _len: usize = inventory.len();
        }
    }
}

/// CollectInventoryResults::is_empty() returns bool.
#[test]
fn collect_inventory_results_is_empty_returns_bool() {
    struct EmptyCheck;
    impl CollectInventoryCallback for EmptyCheck {
        fn on_complete(&self, _status: PmixStatus, inventory: CollectInventoryResults) {
            let _empty: bool = inventory.is_empty();
        }
    }
}

/// server_collect_inventory before init returns PMIX_ERR_INIT.
#[test]
fn collect_inventory_before_init_err_init() {
    struct Nop;
    impl CollectInventoryCallback for Nop {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let directives = InfoBuilder::new().build();
    let result = server_collect_inventory(&directives, Box::new(Nop));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_raw(), -31);
}

/// server_collect_inventory with empty directives.
#[test]
fn collect_inventory_empty_directives() {
    struct Nop;
    impl CollectInventoryCallback for Nop {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let directives = InfoBuilder::new().build();
    let result = server_collect_inventory(&directives, Box::new(Nop));
    assert_eq!(result.unwrap_err(), PmixStatus::Known(PmixError::ErrInit));
}

/// Multiple CollectInventoryCallback implementations can coexist.
#[test]
fn collect_inventory_multiple_callback_types() {
    struct CbA;
    impl CollectInventoryCallback for CbA {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    struct CbB;
    impl CollectInventoryCallback for CbB {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let _v: Vec<Box<dyn CollectInventoryCallback>> = vec![Box::new(CbA), Box::new(CbB)];
}

/// CollectInventoryCallback with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn collect_inventory_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    struct Cb;
    impl CollectInventoryCallback for Cb {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let directives = InfoBuilder::new().build();
    let _ = server_collect_inventory(&directives, Box::new(Cb));
}

// ============================================================================
// 3. server_deliver_inventory — compile-time signature & callback trait checks
// ============================================================================

/// Compile-time check: function signature (callback is Option).
#[test]
fn deliver_inventory_signature() {
    let _: fn(&Info, &Info, Option<Box<dyn DeliverInventoryCallback>>) -> Result<(), PmixStatus> =
        server_deliver_inventory;
}

/// DeliverInventoryCallback trait: object-safe with Box<Self> receiver.
#[test]
fn deliver_inventory_callback_box_self() {
    struct Cb;
    impl DeliverInventoryCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _: Box<dyn DeliverInventoryCallback> = Box::new(Cb);
}

/// DeliverInventoryCallback on_complete receives PmixStatus.
#[test]
fn deliver_inventory_callback_param_type() {
    struct Cb;
    impl DeliverInventoryCallback for Cb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            let _: PmixStatus = status;
        }
    }
}

/// DeliverInventoryCallback is Send.
#[test]
fn deliver_inventory_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn DeliverInventoryCallback>>();
}

/// server_deliver_inventory accepts Some(callback).
#[test]
fn deliver_inventory_with_callback() {
    struct Nop;
    impl DeliverInventoryCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let inventory = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(Nop)));
}

/// server_deliver_inventory accepts None (blocking mode).
#[test]
fn deliver_inventory_blocking_mode() {
    let inventory = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let _result = server_deliver_inventory(&inventory, &directives, None);
}

/// server_deliver_inventory with empty inventory and directives.
#[test]
fn deliver_inventory_empty_params() {
    struct Nop;
    impl DeliverInventoryCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let inventory = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let _result = server_deliver_inventory(&inventory, &directives, Some(Box::new(Nop)));
}

/// DeliverInventoryCallback can capture state via Arc<Mutex<>>.
#[test]
fn deliver_inventory_callback_captures_state() {
    use std::sync::{Arc, Mutex};
    struct StateCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl DeliverInventoryCallback for StateCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StateCapture {
        status: Arc::clone(&status),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(status.lock().unwrap().is_some());
    assert!((*status.lock().unwrap()).as_ref().unwrap().is_success());
}

/// DeliverInventoryCallback success vs error status discrimination.
#[test]
fn deliver_inventory_callback_status_discrimination() {
    struct Discriminator {
        success: std::sync::Arc<std::sync::Mutex<bool>>,
    }
    impl DeliverInventoryCallback for Discriminator {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.success.lock().unwrap() = status.is_success();
        }
    }
    let success = std::sync::Arc::new(std::sync::Mutex::new(false));
    let cb = Box::new(Discriminator {
        success: std::sync::Arc::clone(&success),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(*success.lock().unwrap());

    let success2 = std::sync::Arc::new(std::sync::Mutex::new(true));
    let cb2 = Box::new(Discriminator {
        success: std::sync::Arc::clone(&success2),
    });
    cb2.on_complete(PmixStatus::from_raw(-1));
    assert!(!*success2.lock().unwrap());
}

/// DeliverInventoryCallback with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn deliver_inventory_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    struct Nop;
    impl DeliverInventoryCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let inventory = InfoBuilder::new().build();
    let directives = InfoBuilder::new().build();
    let _ = server_deliver_inventory(&inventory, &directives, Some(Box::new(Nop)));
}

// ============================================================================
// 4. server_define_process_set — compile-time signature & type checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn define_process_set_signature() {
    let _: fn(&[Proc], &str) -> Result<(), PmixStatus> = server_define_process_set;
}

/// server_define_process_set callable with empty slice.
#[test]
fn define_process_set_empty_slice() {
    let _result: Result<(), PmixStatus> = server_define_process_set(&[], "pset_empty");
}

/// server_define_process_set callable with single-member proc slice.
#[test]
fn define_process_set_single_member() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let _result: Result<(), PmixStatus> = server_define_process_set(&[proc], "pset1");
}

/// server_define_process_set callable with multi-member proc slice.
#[test]
fn define_process_set_multiple_members() {
    let members: Vec<Proc> = (0..5)
        .map(|r| Proc::new("test_ns", r).unwrap())
        .collect();
    let _result: Result<(), PmixStatus> = server_define_process_set(&members, "pset_multi");
}

/// server_define_process_set with various pset name formats.
#[test]
fn define_process_set_pset_name_formats() {
    let proc = Proc::new("test_ns", 0).unwrap();
    let _r1 = server_define_process_set(&[proc.clone()], "pset_1");
    let _r2 = server_define_process_set(&[proc.clone()], "pset-2");
    let _r3 = server_define_process_set(&[proc.clone()], "PSET3");
    let _r4 = server_define_process_set(&[proc], "a");
}

/// Proc construction for define_process_set with various ranks.
#[test]
fn define_process_set_proc_various_ranks() {
    for rank in [0u32, 1, 100, u32::MAX] {
        let proc = Proc::new("test_ns", rank).unwrap();
        assert_eq!(proc.get_rank(), rank);
    }
}

/// Proc::new rejects NUL bytes in nspace.
#[test]
fn define_process_set_nul_in_nspace_rejected() {
    let result: Result<Proc, std::ffi::NulError> = Proc::new("test\0ns", 0);
    assert!(result.is_err());
}

/// Proc::new_with_nspace creates copy with different rank.
#[test]
fn define_process_set_proc_new_with_nspace() {
    let proc1 = Proc::new("test_ns", 0).unwrap();
    let proc2 = proc1.new_with_nspace(42).unwrap();
    assert_eq!(proc2.get_rank(), 42);
}

/// Proc::set_rank updates rank in-place.
#[test]
fn define_process_set_proc_set_rank() {
    let mut proc = Proc::new("test_ns", 0).unwrap();
    proc.set_rank(99);
    assert_eq!(proc.get_rank(), 99);
}

/// server_define_process_set with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn define_process_set_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let proc = Proc::new("test_ns", 0).unwrap();
    let _ = server_define_process_set(&[proc], "pset1");
}

// ============================================================================
// 5. server_delete_process_set — compile-time signature & type checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn delete_process_set_signature() {
    let _: fn(&str) -> Result<(), PmixStatus> = server_delete_process_set;
}

/// server_delete_process_set callable with various string literals.
#[test]
fn delete_process_set_callable_with_literals() {
    let _r1 = server_delete_process_set("pset1");
    let _r2 = server_delete_process_set("pset_2");
    let _r3 = server_delete_process_set("a");
}

/// server_delete_process_set callable with String::as_str().
#[test]
fn delete_process_set_callable_with_string() {
    let name = String::from("dynamic_pset");
    let _result = server_delete_process_set(name.as_str());
}

/// PmixStatus round-trip for delete_process_set error codes.
#[test]
fn delete_process_set_status_roundtrip() {
    for code in [-46i32, -27, -1, 0, 1] {
        let status = PmixStatus::from_raw(code);
        assert_eq!(status.to_raw(), code);
    }
}

/// PmixStatus equality for delete_process_set error codes.
#[test]
fn delete_process_set_status_equality() {
    let s1 = PmixStatus::from_raw(-46);
    let s2 = PmixStatus::from_raw(-46);
    assert_eq!(s1, s2);
}

/// server_delete_process_set with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn delete_process_set_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let _ = server_delete_process_set("pset1");
}

// ============================================================================
// 6. server_generate_cpuset_string — compile-time signature & type checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn generate_cpuset_string_signature() {
    let _: fn(&mut PmixCpuset) -> Result<String, PmixStatus> = server_generate_cpuset_string;
}

/// PmixCpuset::new() constructs and destructs cleanly (RAII).
#[test]
fn generate_cpuset_string_cpuset_construction() {
    let cpuset = PmixCpuset::new();
    drop(cpuset);
}

/// Multiple PmixCpuset constructions/destructions (no leak).
#[test]
fn generate_cpuset_string_multiple_lifecycle() {
    for _ in 0..10 {
        let _cpuset = PmixCpuset::new();
    }
}

/// PmixCpuset is Debug.
#[test]
fn generate_cpuset_string_cpuset_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixCpuset>();
}

/// server_generate_cpuset_string returns Result<String, PmixStatus>.
#[test]
fn generate_cpuset_string_return_type() {
    fn assert_return_type(_: Result<String, PmixStatus>) {}
    let _ = assert_return_type;
}

/// PMIX_ERR_BAD_PARAM raw value for cpuset errors.
#[test]
fn generate_cpuset_string_err_bad_param_value() {
    let status = PmixStatus::from_raw(-27);
    assert!(!status.is_success());
}

/// PMIX_ERR_TAKE_NEXT_OPTION raw value for cpuset errors.
#[test]
fn generate_cpuset_string_err_take_next_option_value() {
    let status = PmixStatus::from_raw(-11);
    assert!(!status.is_success());
}

/// server_generate_cpuset_string with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn generate_cpuset_string_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let mut cpuset = PmixCpuset::new();
    let _ = server_generate_cpuset_string(&mut cpuset);
}

// ============================================================================
// 7. server_generate_locality_string — compile-time signature & type checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn generate_locality_string_signature() {
    let _: fn(&mut PmixCpuset) -> Result<String, PmixStatus> = server_generate_locality_string;
}

/// server_generate_locality_string returns Result<String, PmixStatus>.
#[test]
fn generate_locality_string_return_type() {
    fn assert_return_type(_: Result<String, PmixStatus>) {}
    let _ = assert_return_type;
}

/// PMIX_ERR_NOT_SUPPORTED raw value for locality errors.
#[test]
fn generate_locality_string_err_not_supported_value() {
    let status = PmixStatus::from_raw(-47);
    assert!(!status.is_success());
}

/// PMIX_ERR_NOT_FOUND raw value for locality errors.
#[test]
fn generate_locality_string_err_not_found_value() {
    let status = PmixStatus::from_raw(-46);
    assert!(!status.is_success());
}

/// server_generate_locality_string with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn generate_locality_string_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let mut cpuset = PmixCpuset::new();
    let _ = server_generate_locality_string(&mut cpuset);
}

// ============================================================================
// 8. server_iof_deliver — compile-time signature & callback trait checks
// ============================================================================

/// Compile-time check: function signature.
#[test]
fn iof_deliver_signature() {
    let _: fn(
        &Proc,
        IOFChannelFlags,
        &PmixByteObject,
        &Info,
        Box<dyn IOFDeliverCallback>,
    ) -> Result<(), PmixStatus> = server_iof_deliver;
}

/// IOFDeliverCallback trait: object-safe with Box<Self> receiver.
#[test]
fn iof_deliver_callback_box_self() {
    struct Cb;
    impl IOFDeliverCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let _: Box<dyn IOFDeliverCallback> = Box::new(Cb);
}

/// IOFDeliverCallback on_complete receives PmixStatus.
#[test]
fn iof_deliver_callback_param_type() {
    struct Cb;
    impl IOFDeliverCallback for Cb {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            let _: PmixStatus = status;
        }
    }
}

/// IOFDeliverCallback is Send.
#[test]
fn iof_deliver_callback_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Box<dyn IOFDeliverCallback>>();
}

/// IOFChannelFlags::STDOUT is usable.
#[test]
fn iof_deliver_channel_stdout() {
    let _channel: IOFChannelFlags = IOFChannelFlags::STDOUT;
}

/// IOFChannelFlags::STDERR is usable.
#[test]
fn iof_deliver_channel_stderr() {
    let _channel: IOFChannelFlags = IOFChannelFlags::STDERR;
}

/// IOFChannelFlags::STDIN is usable.
#[test]
fn iof_deliver_channel_stdin() {
    let _channel: IOFChannelFlags = IOFChannelFlags::STDIN;
}

/// PmixByteObject::new() creates empty byte object.
#[test]
fn iof_deliver_byte_object_new() {
    let bo = PmixByteObject::new();
    assert!(bo.is_empty());
    assert_eq!(bo.size(), 0);
}

/// PmixByteObject::from(Vec<u8>) creates byte object with data.
#[test]
fn iof_deliver_byte_object_from_vec() {
    let bo = PmixByteObject::from(b"hello".to_vec());
    assert!(!bo.is_empty());
    assert_eq!(bo.size(), 5);
}

/// PmixByteObject::as_slice() returns the data.
#[test]
fn iof_deliver_byte_object_as_slice() {
    let data = b"test data".to_vec();
    let bo = PmixByteObject::from(data.clone());
    let slice = bo.as_slice();
    assert_eq!(slice, data.as_slice());
}

/// PmixByteObject implements Debug.
#[test]
fn iof_deliver_byte_object_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixByteObject>();
}

/// server_iof_deliver with stdout channel and data.
#[test]
fn iof_deliver_stdout_with_data() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(b"hello".to_vec());
    let info = InfoBuilder::new().build();
    let _result = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver with stderr channel.
#[test]
fn iof_deliver_stderr_with_data() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(b"error".to_vec());
    let info = InfoBuilder::new().build();
    let _result = server_iof_deliver(&source, IOFChannelFlags::STDERR, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver with stdin channel.
#[test]
fn iof_deliver_stdin_with_data() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(b"input".to_vec());
    let info = InfoBuilder::new().build();
    let _result = server_iof_deliver(&source, IOFChannelFlags::STDIN, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver with empty byte object.
#[test]
fn iof_deliver_empty_byte_object() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(Vec::new());
    let info = InfoBuilder::new().build();
    let _result = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver with large byte object.
#[test]
fn iof_deliver_large_byte_object() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let large: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let bo = PmixByteObject::from(large);
    let info = InfoBuilder::new().build();
    let _result = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver with different source processes.
#[test]
fn iof_deliver_different_sources() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source1 = Proc::new("job1", 0).unwrap();
    let source2 = Proc::new("job2", 42).unwrap();
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = InfoBuilder::new().build();
    let _r1 = server_iof_deliver(&source1, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
    let _r2 = server_iof_deliver(&source2, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
}

/// server_iof_deliver consistent result across multiple calls.
#[test]
fn iof_deliver_consistent_result() {
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(b"data".to_vec());
    let info = InfoBuilder::new().build();
    let first = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop))
        .is_ok();
    for _ in 0..4 {
        let result = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop))
            .is_ok();
        assert_eq!(result, first, "results should be consistent");
    }
}

/// IOFDeliverCallback with Arc<Mutex<>> state capture.
#[test]
fn iof_deliver_callback_captures_state() {
    use std::sync::{Arc, Mutex};
    struct StateCapture {
        status: Arc<Mutex<Option<PmixStatus>>>,
    }
    impl IOFDeliverCallback for StateCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            *self.status.lock().unwrap() = Some(status);
        }
    }
    let status = Arc::new(Mutex::new(None));
    let cb = Box::new(StateCapture {
        status: Arc::clone(&status),
    });
    cb.on_complete(PmixStatus::from_raw(0));
    assert!(status.lock().unwrap().is_some());
}

/// IOFDeliverCallback with multiple error codes.
#[test]
fn iof_deliver_callback_error_codes() {
    use std::sync::{Arc, Mutex};
    struct CodeCapture {
        codes: Arc<Mutex<Vec<i32>>>,
    }
    impl IOFDeliverCallback for CodeCapture {
        fn on_complete(self: Box<Self>, status: PmixStatus) {
            self.codes.lock().unwrap().push(status.to_raw());
        }
    }
    let codes = Arc::new(Mutex::new(Vec::new()));
    let cb1 = Box::new(CodeCapture {
        codes: Arc::clone(&codes),
    });
    cb1.on_complete(PmixStatus::from_raw(0));
    let cb2 = Box::new(CodeCapture {
        codes: Arc::clone(&codes),
    });
    cb2.on_complete(PmixStatus::from_raw(-172)); // PMIX_ERR_IOF_FAILURE
    let captured = codes.lock().unwrap();
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0], 0);
    assert_eq!(captured[1], -172);
}

/// server_iof_deliver with initialized server (daemon-dependent).
#[test]
#[ignore = "requires PMIx_server_init and running PMIx daemon"]
fn iof_deliver_with_initialized_server() {
    use pmix::server::{server_init_minimal, PmixServerModule};
    struct Nop;
    impl IOFDeliverCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let module = PmixServerModule::default();
    let _handle = server_init_minimal(Some(&module)).expect("server_init");
    let source = Proc::new("test_ns", 0).unwrap();
    let bo = PmixByteObject::from(b"hello".to_vec());
    let info = InfoBuilder::new().build();
    let _ = server_iof_deliver(&source, IOFChannelFlags::STDOUT, &bo, &info, Box::new(Nop));
}

// ============================================================================
// Cross-function PmixStatus tests
// ============================================================================

/// PmixStatus::from_raw / to_raw round-trip for all relevant error codes.
#[test]
fn pmix_status_roundtrip_all_relevant_codes() {
    let codes = [
        0,    // PMIX_SUCCESS
        -1,   // PMIX_ERROR
        -27,  // PMIX_ERR_BAD_PARAM
        -31,  // PMIX_ERR_INIT
        -32,  // PMIX_ERR_NOMEM
        -46,  // PMIX_ERR_NOT_FOUND
        -47,  // PMIX_ERR_NOT_SUPPORTED
        -11,  // PMIX_ERR_EXISTS (TAKE_NEXT_OPTION in older versions)
        -172, // PMIX_ERR_IOF_FAILURE
    ];
    for code in codes {
        let status = PmixStatus::from_raw(code);
        assert_eq!(status.to_raw(), code, "round-trip failed for {}", code);
    }
}

/// PmixStatus::is_success for success and error codes.
#[test]
fn pmix_status_is_success_discrimination() {
    assert!(PmixStatus::from_raw(0).is_success());
    assert!(!PmixStatus::from_raw(-1).is_success());
    assert!(!PmixStatus::from_raw(-31).is_success());
    assert!(!PmixStatus::from_raw(-27).is_success());
}

/// PmixStatus::is_error is the negation of is_success.
#[test]
fn pmix_status_is_error_negation() {
    for code in [-31i32, -27, -1, 0, 1] {
        let status = PmixStatus::from_raw(code);
        assert_eq!(status.is_error(), !status.is_success());
    }
}

/// Proc::new with various valid nspaces.
#[test]
fn proc_construction_valid_nspaces() {
    let long_nspace = "x".repeat(255);
    for nspace in ["a", "job.12345", "myapp", &long_nspace] {
        let proc = Proc::new(nspace, 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }
}

/// Proc::new rejects NUL bytes.
#[test]
fn proc_construction_rejects_nul() {
    let result: Result<Proc, std::ffi::NulError> = Proc::new("test\0ns", 0);
    assert!(result.is_err());
}

/// InfoBuilder::new().build() produces a valid Info.
#[test]
fn info_builder_empty_builds() {
    let info = InfoBuilder::new().build();
    // Info is usable as a parameter — we verify by passing it to a function.
    struct Nop;
    impl DeliverInventoryCallback for Nop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let _ = server_deliver_inventory(&info, &info, None);
}

/// All 8 server functions have correct signatures (compile-time smoke test).
///
/// This test verifies the functions are callable and have the right types.
/// The cpuset/locality functions are excluded from the FFI call because they
/// segfault without PMIx_server_init (the C library dereferences internal
/// pointers that are null before init).
#[test]
fn all_eight_functions_callable_smoke() {
    // 1. server_dmodex_request
    struct DmodexNop;
    impl DmodexRequestCallback for DmodexNop {
        fn on_complete(self: Box<Self>, _: PmixStatus, _: Vec<u8>) {}
    }
    let proc = Proc::new("test", 0).unwrap();
    let _ = server_dmodex_request(&proc, Box::new(DmodexNop));

    // 2. server_collect_inventory
    struct CollectNop;
    impl CollectInventoryCallback for CollectNop {
        fn on_complete(&self, _: PmixStatus, _: CollectInventoryResults) {}
    }
    let info = InfoBuilder::new().build();
    let _ = server_collect_inventory(&info, Box::new(CollectNop));

    // 3. server_deliver_inventory
    struct DeliverNop;
    impl DeliverInventoryCallback for DeliverNop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let _ = server_deliver_inventory(&info, &info, None);

    // 4. server_define_process_set
    let _ = server_define_process_set(&[], "pset_smoke");

    // 5. server_delete_process_set
    let _ = server_delete_process_set("pset_smoke");

    // 6. server_generate_cpuset_string — only verify signature, not FFI call
    // (segfaults without PMIx_server_init)
    fn _check_cpuset_sig() {
        let _: fn(&mut PmixCpuset) -> Result<String, PmixStatus> = server_generate_cpuset_string;
    }
    let _ = _check_cpuset_sig;

    // 7. server_generate_locality_string — only verify signature, not FFI call
    // (segfaults without PMIx_server_init)
    fn _check_locality_sig() {
        let _: fn(&mut PmixCpuset) -> Result<String, PmixStatus> = server_generate_locality_string;
    }
    let _ = _check_locality_sig;

    // 8. server_iof_deliver
    struct IOFNop;
    impl IOFDeliverCallback for IOFNop {
        fn on_complete(self: Box<Self>, _: PmixStatus) {}
    }
    let bo = PmixByteObject::from(b"smoke".to_vec());
    let _ = server_iof_deliver(&proc, IOFChannelFlags::STDOUT, &bo, &info, Box::new(IOFNop));
}
