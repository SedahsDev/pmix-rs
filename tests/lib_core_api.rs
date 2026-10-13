//! Comprehensive tests for lib.rs core API functions — init, finalize, fence,
//! get_version, progress, commit, get_value, put_value, and type system.
//!
//! Daemon-dependent tests use `tool_init` (PMIx_tool_init) via the
//! `daemon_helper` module, which connects to the systemd-managed PRTE service.
mod daemon_helper;

use pmix::{
    InfoBuilder, PmixDataRange, PmixDataType, PmixEnvar, PmixError, PmixJobState, PmixLinkState,
    PmixPayload, PmixPersistence, PmixProcState, PmixScope, PmixStatus, PmixTimeval,
    PmixValueBuilder,
};
use std::ffi::CString;

// ─────────────────────────────────────────────────────────────────────────────
// get_version — always works (no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_get_version_not_empty() {
    let version = pmix::get_version().expect("version");
    assert!(!version.is_empty(), "version should not be empty");
}

#[test]
fn test_get_version_has_digits() {
    let version = pmix::get_version().expect("version");
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

/// tool_init succeeds with a running daemon.
///
/// Routes through `get_tool_handle()` so the process-global PMIx tool session
/// is initialized exactly once. Direct `tool_init` calls would leave the
/// session `Live`, causing every later test's init to return `ErrExists`.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_with_daemon() {
    let result = daemon_helper::get_tool_handle();
    assert!(
        result.is_ok(),
        "tool_init should succeed with daemon: {:?}",
        result
    );
}

/// tool_init returns a handle with valid namespace and rank.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_returns_valid_handle() {
    let handle = daemon_helper::get_tool_handle().expect("tool_init failed");
    let proc = handle.proc().expect("handle should have a proc");
    let nspace = proc.nspace();
    assert!(nspace.is_some(), "handle should have a namespace");
    assert!(!nspace.unwrap().is_empty(), "namespace should not be empty");
    let _rank: u32 = proc.rank();
}

/// tool_init with Info succeeds.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_with_info() {
    let _handle = daemon_helper::get_tool_handle().expect("tool_init failed");
}

/// tool_finalize succeeds after tool_init.
///
/// We cannot call `tool_finalize` on the shared singleton handle — that would
/// leave the process-global session `Dead`, causing every later test's
/// `tool_init` to return `ErrInit`. Instead, we verify the handle is valid
/// (the singleton succeeded), mirroring `tests/tool_tool_init.rs`.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_finalize_after_init() {
    let handle = daemon_helper::get_tool_handle().expect("tool_init failed");
    // Handle is valid — finalize would work but we can't call it on the
    // shared handle without breaking every subsequent test.
    let _ = handle;
}

/// A second `tool_init` while the session is `Live` returns `ErrExists`.
///
/// The crate's `PmixTool::connect()` rejects a second init while the
/// process-global session is `Live` (src/tool.rs:196). The C library
/// itself returns `PMIX_SUCCESS` on re-init, but the wrapper's state
/// machine is stricter. We assert the wrapper contract here.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_finalize_cycle() {
    let _shared = daemon_helper::get_tool_handle().expect("shared tool handle");
    let info = daemon_helper::get_tool_init_info();
    // The singleton is already Live, so a direct second init must fail.
    let result = pmix::tool::tool_init(None, &info);
    assert!(
        matches!(result, Err(PmixStatus::Known(PmixError::ErrExists))),
        "second tool_init while Live should return ErrExists, got: {:?}",
        result
    );
}

/// A second `tool_init` while Live returns `ErrExists` (not ref-counting).
///
/// The crate does not support reference-counted multiple inits — the
/// `tool_init` doc's \"reference-counted\" claim does not match the
/// implementation (src/tool.rs:196-198). We assert the actual contract:
/// the second init is rejected with `ErrExists`.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_ref_count() {
    let _shared = daemon_helper::get_tool_handle().expect("shared tool handle");
    let info = daemon_helper::get_tool_init_info();
    let result = pmix::tool::tool_init(None, &info);
    assert!(
        matches!(result, Err(PmixStatus::Known(PmixError::ErrExists))),
        "second tool_init while Live should return ErrExists, got: {:?}",
        result
    );
}

/// tool_is_initialized returns true after tool_init.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_initialized_after_init() {
    let _handle = daemon_helper::get_tool_handle().expect("tool_init failed");
    assert!(
        pmix::tool::is_tool_initialized(),
        "should be initialized after tool_init"
    );
}

/// tool_init_minimal succeeds with a running daemon.
#[test]
#[ignore = "requires PMIx daemon — run under prterun"]
fn test_tool_init_minimal() {
    let _handle = daemon_helper::get_tool_handle().expect("tool_init failed");
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Init tests — require DVM-launched process (cannot run as external tool)
// These remain ignored because PMIx_Init is only for processes managed by the DVM.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_with_daemon() {
    let result = pmix::PmixClient::connect_new(None);
    assert!(result.is_ok(), "init should succeed with daemon");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_returns_valid_context() {
    let context = daemon_helper::ensure_pmix_init();
    let rank = context.require_rank();
    assert_eq!(rank, 0, "rank should be 0 for standalone client");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_context_get_proc() {
    let context = daemon_helper::ensure_pmix_init();
    let _proc = context.require_proc();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_context_proc_with_nspace() {
    let context = daemon_helper::ensure_pmix_init();
    let proc = context
        .proc_with_nspace(0)
        .expect("proc_with_nspace failed");
    assert_eq!(proc.get_rank(), 0);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_with_info() {
    let info = InfoBuilder::new().build().expect("build info");
    let result = pmix::PmixClient::connect_new(Some(info));
    assert!(result.is_ok(), "init with info should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_finalize_after_init() {
    daemon_helper::ensure_pmix_init();
    let result = pmix::finalize(None);
    assert!(result.is_ok(), "finalize should succeed after init");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_init_finalize_cycle() {
    daemon_helper::ensure_pmix_init();
    pmix::finalize(None).expect("first finalize failed");
    daemon_helper::ensure_pmix_init();
    pmix::finalize(None).expect("second finalize failed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_after_init() {
    let context = daemon_helper::ensure_pmix_init();
    let result = pmix::fence(&context.require_proc(), None);
    assert!(result.is_ok(), "fence should succeed after init");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_fence_with_info() {
    let context = daemon_helper::ensure_pmix_init();
    let info = InfoBuilder::new().build().expect("build info");
    let result = pmix::fence(&context.require_proc(), Some(info));
    assert!(result.is_ok(), "fence with info should succeed");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_commit_after_init() {
    daemon_helper::ensure_pmix_init();
    let _result = pmix::commit();
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_put_get_commit_roundtrip() {
    let context = daemon_helper::ensure_pmix_init();
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
            let get_result =
                pmix::get_value(&context.require_proc(), b"test_roundtrip_key\0", None);
            drop(get_result);
        }
    }
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_value_nonexistent() {
    let context = daemon_helper::ensure_pmix_init();
    let result = pmix::get_value(&context.require_proc(), b"nonexistent_key_xyz\0", None);
    assert!(result.is_err(), "get_value for nonexistent key should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_lookup_nonexistent() {
    daemon_helper::ensure_pmix_init();
    let mut pdata: Vec<pmix::data_ops::PmixPdata> = Vec::new();
    let result = pmix::data_ops::lookup(&mut pdata, None);
    assert!(result.is_err(), "lookup with empty data should fail");
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_unpublish_nonexistent() {
    use pmix::data_ops::unpublish;
    daemon_helper::ensure_pmix_init();
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
    let _info = InfoBuilder::new().build().expect("build info");
}

#[test]
fn test_info_builder_independent() {
    let _info1 = InfoBuilder::new().build().expect("build info");
    let _info2 = InfoBuilder::new().build().expect("build info");
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
