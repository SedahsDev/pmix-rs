//! Comprehensive tests for lib.rs core API functions — init, finalize, fence,
//! get_version, progress, commit, get_value, put_value, and type system.
//!
//! Daemon-dependent tests use `tool_init` (PMIx_tool_init) via the
//! `daemon_helper` module, which connects to the systemd-managed PRTE service.

mod daemon_helper;

use pmix::{
    InfoBuilder, PmixDataRange, PmixDataType, PmixEnvar, PmixError, PmixJobState, PmixLinkState,
    PmixPayload, PmixPersistence, PmixProcState, PmixScope, PmixStatus, PmixTimeval,
    PmixValueBuilder, info_with_string_key,
};
use std::ffi::CString;

// ─────────────────────────────────────────────────────────────────────────────
// get_version — always works (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_version_not_empty() {
    let version = pmix::get_version();
    assert!(!version.is_empty(), "version should not be empty");
}

#[test]
fn test_get_version_has_digits() {
    let version = pmix::get_version();
    assert!(
        version.chars().any(|c| c.is_ascii_digit()),
        "version should contain digits: {}",
        version
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// progress — no-op, should not panic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_progress_no_panic() {
    pmix::progress();
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init via daemon — live daemon tests (require PMIx daemon)
//
// We use tool_init (PMIx_tool_init) instead of PMIx_Init because we are an
// external tool connecting to the DVM, not a process launched by it.
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init succeeds with a running daemon (replaces test_init_with_daemon).
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_with_daemon() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let result = pmix::tool::tool_init(None, &info);
    assert!(result.is_ok(), "tool_init should succeed with daemon");
}

/// tool_init returns a handle with valid namespace and rank.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_returns_valid_handle() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let handle = pmix::tool::tool_init(None, &info).expect("tool_init failed");
    let nspace = handle.proc().nspace();
    assert!(nspace.is_some(), "handle should have a namespace");
    assert!(!nspace.unwrap().is_empty(), "namespace should not be empty");
    let _rank: u32 = handle.proc().rank();
}

/// tool_init with Info succeeds.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_with_info() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let result = pmix::tool::tool_init(None, &info);
    assert!(result.is_ok(), "tool_init with info should succeed");
}

/// tool_finalize succeeds after tool_init.
#[test]
fn test_tool_finalize_after_init() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let uri = daemon_helper::read_uri().expect("PMIx daemon not available");
    let info = info_with_string_key("pmix.srvr.uri", &uri);
    let handle = pmix::tool::tool_init(None, &info).expect("tool_init failed");
    let result = pmix::tool::tool_finalize(handle);
    assert!(result.is_ok(), "tool_finalize should succeed after init");
}

/// tool_init -> tool_finalize cycle is idempotent.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_finalize_cycle() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let h1 = pmix::tool::tool_init(None, &info).expect("first init failed");
    pmix::tool::tool_finalize(h1).expect("first finalize failed");
    let h2 = pmix::tool::tool_init(None, &info).expect("second init failed");
    pmix::tool::tool_finalize(h2).expect("second finalize failed");
}

/// tool_init ref counting — two inits need two finalizes.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_ref_count() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let h1 = pmix::tool::tool_init(None, &info).expect("first init failed");
    let h2 = pmix::tool::tool_init(None, &info).expect("second init failed");
    pmix::tool::tool_finalize(h1).expect("first finalize failed");
    pmix::tool::tool_finalize(h2).expect("second finalize failed");
}

/// tool_is_initialized returns true after tool_init.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_initialized_after_init() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let info = InfoBuilder::new().build();
    let _handle = pmix::tool::tool_init(None, &info).expect("tool_init failed");
    assert!(
        pmix::tool::is_tool_initialized(),
        "should be initialized after tool_init"
    );
}

/// tool_init_minimal succeeds with a running daemon.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_minimal() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");
    let result = pmix::tool::tool_init_minimal();
    assert!(result.is_ok(), "tool_init_minimal should succeed");
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Init tests — require DVM-launched process (cannot run as external tool)
// These remain ignored because PMIx_Init is only for processes managed by the DVM.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_with_daemon() {
    let result = pmix::init(None);
    assert!(result.is_ok(), "init should succeed with daemon");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_returns_valid_context() {
    let context = pmix::init(None).expect("init failed");
    let rank = context.get_rank();
    assert_eq!(rank, 0, "rank should be 0 for standalone client");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_context_get_proc() {
    let context = pmix::init(None).expect("init failed");
    let _proc = context.get_proc();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_context_proc_with_nspace() {
    let context = pmix::init(None).expect("init failed");
    let proc = context
        .proc_with_nspace(0)
        .expect("proc_with_nspace failed");
    assert_eq!(proc.get_rank(), 0);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_with_info() {
    let info = InfoBuilder::new().build();
    let result = pmix::init(Some(info));
    assert!(result.is_ok(), "init with info should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_finalize_after_init() {
    let _context = pmix::init(None).expect("init failed");
    let result = pmix::finalize(None);
    assert!(result.is_ok(), "finalize should succeed after init");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_finalize_cycle() {
    let _c1 = pmix::init(None).expect("first init failed");
    pmix::finalize(None).expect("first finalize failed");
    let _c2 = pmix::init(None).expect("second init failed");
    pmix::finalize(None).expect("second finalize failed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_after_init() {
    let context = pmix::init(None).expect("init failed");
    let result = pmix::fence(context.get_proc(), None);
    assert!(result.is_ok(), "fence should succeed after init");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_with_info() {
    let context = pmix::init(None).expect("init failed");
    let info = InfoBuilder::new().build();
    let result = pmix::fence(context.get_proc(), Some(info));
    assert!(result.is_ok(), "fence with info should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_commit_after_init() {
    let _context = pmix::init(None).expect("init failed");
    let _result = pmix::commit();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_put_get_commit_roundtrip() {
    let context = pmix::init(None).expect("init failed");
    let key = CString::new("test_roundtrip_key").unwrap();
    let mut value = PmixValueBuilder::new()
        .string("roundtrip_value")
        .unwrap()
        .build()
        .unwrap();
    let put_result = pmix::put_value(PmixScope::Global.to_raw(), &key, &mut value);
    if put_result.is_ok() {
        let commit_result = pmix::commit();
        if commit_result.is_ok() {
            let get_result = pmix::get_value(context.get_proc(), b"test_roundtrip_key\0", None);
            drop(get_result);
        }
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_value_nonexistent() {
    let context = pmix::init(None).expect("init failed");
    let result = pmix::get_value(context.get_proc(), b"nonexistent_key_xyz\0", None);
    assert!(result.is_err(), "get_value for nonexistent key should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nonexistent() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let mut pdata: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let result = pmix::data_ops::lookup(&mut pdata, None);
    assert!(result.is_err(), "lookup with empty data should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_nonexistent() {
    use pmix::data_ops::unpublish;
    let _context = pmix::init(None).expect("init failed");
    let result = unpublish(Some(&["nonexistent_unpub_key_xyz"]), None);
    assert!(result.is_err(), "unpublish for nonexistent key should fail");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus — comprehensive tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_status_success() {
    let success = PmixStatus::Known(PmixError::Success);
    assert!(success.is_success());
    assert_eq!(success.to_raw(), 0);
}

#[test]
fn test_pmix_status_from_raw_zero() {
    assert_eq!(
        PmixStatus::from_raw(0),
        PmixStatus::Known(PmixError::Success)
    );
}

#[test]
fn test_pmix_status_from_raw_known() {
    assert_eq!(
        PmixStatus::from_raw(-1),
        PmixStatus::Known(PmixError::Error)
    );
}

#[test]
fn test_pmix_status_known_not_success() {
    assert!(!PmixStatus::Known(PmixError::Error).is_success());
}

#[test]
fn test_pmix_status_unknown_not_success() {
    assert!(!PmixStatus::Unknown(-9999).is_success());
}

#[test]
fn test_pmix_status_unknown_positive_is_success() {
    assert!(PmixStatus::Unknown(1).is_success());
}

#[test]
fn test_pmix_status_debug() {
    let _ = format!("{:?}", PmixStatus::Known(PmixError::Success));
    let _ = format!("{:?}", PmixStatus::Known(PmixError::Error));
    let _ = format!("{:?}", PmixStatus::Unknown(-9999));
}

#[test]
fn test_pmix_status_traits() {
    fn assert_traits<T: Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixStatus>();
}

#[test]
fn test_pmix_status_known_method() {
    let s = PmixStatus::Known(PmixError::Error);
    assert!(s.known().is_some());
    let u = PmixStatus::Unknown(-9999);
    assert!(u.known().is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixError — comprehensive tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_error_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixError>();
}

#[test]
fn test_pmix_error_from_raw() {
    assert_eq!(PmixError::from_raw(0), Some(PmixError::Success));
    assert_eq!(PmixError::from_raw(-1), Some(PmixError::Error));
    assert_eq!(PmixError::from_raw(-9999), None);
}

#[test]
fn test_pmix_error_success() {
    assert!(PmixError::Success.is_success());
}

#[test]
fn test_pmix_error_error() {
    assert!(!PmixError::Error.is_success());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPayload — comprehensive tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_payload_variants() {
    let payloads = vec![
        PmixPayload::Undef,
        PmixPayload::Bool(true),
        PmixPayload::Byte(42),
        PmixPayload::String(CString::new("hello").unwrap()),
        PmixPayload::Size(1024),
        PmixPayload::Pid(1234),
        PmixPayload::Int(-42),
        PmixPayload::Int8(-5),
        PmixPayload::Int16(-100),
        PmixPayload::Int32(-1000),
        PmixPayload::Int64(-10000),
        PmixPayload::Uint(42),
        PmixPayload::Uint8(255),
        PmixPayload::Uint16(65535),
        PmixPayload::Uint32(4294967295),
        PmixPayload::Uint64(18446744073709551615),
        PmixPayload::Float(3.14),
        PmixPayload::Double(2.718),
        PmixPayload::Timeval(PmixTimeval {
            tv_sec: 1,
            tv_usec: 500,
        }),
        PmixPayload::Status(0),
        PmixPayload::Rank(0),
        PmixPayload::ByteObject(vec![1, 2, 3]),
        PmixPayload::Envar(PmixEnvar::new("FOO", "bar", '=').unwrap()),
    ];
    for p in payloads {
        let _ = p.type_tag();
    }
}

#[test]
fn test_pmix_payload_type_tag() {
    assert_eq!(PmixPayload::Bool(true).type_tag(), 1);
    assert_eq!(PmixPayload::Int(42).type_tag(), 6);
    assert_eq!(PmixPayload::Size(1024).type_tag(), 4);
}

// ─────────────────────────────────────────────────────────────────────────────
// Enum trait tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pmix_scope_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixScope>();
}

#[test]
fn test_pmix_data_range_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixDataRange>();
}

#[test]
fn test_pmix_persistence_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixPersistence>();
}

#[test]
fn test_pmix_proc_state_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixProcState>();
}

#[test]
fn test_pmix_job_state_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixJobState>();
}

#[test]
fn test_pmix_link_state_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixLinkState>();
}

#[test]
fn test_pmix_data_type_traits() {
    fn assert_traits<T: std::fmt::Debug + Clone + Copy + PartialEq + Eq + std::hash::Hash>() {}
    assert_traits::<PmixDataType>();
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder — tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_info_builder_build() {
    let _info = InfoBuilder::new().build();
}

#[test]
fn test_info_builder_independent() {
    let _info1 = InfoBuilder::new().build();
    let _info2 = InfoBuilder::new().build();
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixValueBuilder — tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_value_builder_new() {
    let _builder = PmixValueBuilder::new();
}

#[test]
fn test_value_builder_bool() {
    let value = PmixValueBuilder::new().bool(true).build().unwrap();
    assert_eq!(value.type_tag(), 1);
}

#[test]
fn test_value_builder_string() {
    let value = PmixValueBuilder::new()
        .string("hello")
        .unwrap()
        .build()
        .unwrap();
    // PMIX_STRING
    assert_eq!(value.type_tag(), 3);
}

#[test]
fn test_value_builder_u32() {
    let value = PmixValueBuilder::new().uint32(42).build().unwrap();
    // PMIX_UINT32
    assert_eq!(value.type_tag(), 14);
}

#[test]
fn test_value_builder_i32() {
    let value = PmixValueBuilder::new().int32(-42).build().unwrap();
    // PMIX_INT32
    assert_eq!(value.type_tag(), 9);
}

#[test]
fn test_value_builder_f64() {
    let value = PmixValueBuilder::new().double(3.14).build().unwrap();
    // PMIX_DOUBLE
    assert_eq!(value.type_tag(), 17);
}

#[test]
fn test_value_builder_size() {
    let value = PmixValueBuilder::new().size(1024).build().unwrap();
    // PMIX_SIZE
    assert_eq!(value.type_tag(), 4);
}

#[test]
fn test_value_builder_scope() {
    // scope() sets the payload to PmixPayload::Scope
    let value = PmixValueBuilder::new()
        .scope(PmixScope::Global.to_raw())
        .build()
        .unwrap();
    // PMIX_SCOPE
    assert_eq!(value.type_tag(), 32);
}

#[test]
fn test_value_builder_data_range() {
    // data_range() sets the payload to PmixPayload::DataRange
    let value = PmixValueBuilder::new()
        .data_range(PmixDataRange::Session.to_raw())
        .build()
        .unwrap();
    // PMIX_DATA_RANGE
    assert_eq!(value.type_tag(), 33);
}

#[test]
fn test_value_builder_persistence() {
    // persist() sets the payload to PmixPayload::Persist
    let value = PmixValueBuilder::new()
        .persist(PmixPersistence::Indefinite.to_raw())
        .build()
        .unwrap();
    // PMIX_PERSISTENCE
    assert_eq!(value.type_tag(), 30);
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants — tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_global_constant() {
    let _global: u8 = pmix::GLOBAL;
}

#[test]
fn test_num_nodes_constant() {
    let _num_nodes: &[u8] = pmix::NUM_NODES;
    assert!(!_num_nodes.is_empty());
}

#[test]
fn test_job_size_constant() {
    let _job_size: &[u8] = pmix::JOB_SIZE;
    assert!(!_job_size.is_empty());
}

#[test]
fn test_rank_wildcard_constant() {
    let _wildcard: u32 = pmix::RANK_WILDCARD;
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc — construction tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_new() {
    let proc = pmix::Proc::new("test-nspace", 42).expect("proc new failed");
    assert_eq!(proc.get_rank(), 42);
}

#[test]
fn test_proc_new_nul_fails() {
    assert!(pmix::Proc::new("test\0nspace", 42).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixEnvar — construction tests
// ─────────────────────────────────────────────────────────────────────────────
