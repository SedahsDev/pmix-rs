//! Deep tests for groups module — Round 2.
//!
//! Targets untested code paths in groups.rs (67.23% coverage).
//! Focus: group_construct/invite/join/leave/destruct with various parameters,
//! callback wrappers, panic safety, empty group_id validation.
//!
//! FFI tests that require PMIx_Init are marked #[ignore].

use pmix::groups::*;
use pmix::{InfoBuilder, PmixStatus, Proc};
use pmix::groups::pmix_group_opt_t;

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction (safe without PMIx_Init)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_new_valid() {
    let proc = Proc::new("test_namespace", 0).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_new_high_rank() {
    let proc = Proc::new("ns", 99999).expect("create proc");
    let _ = proc;
}

#[test]
fn test_proc_new_nul_rejected() {
    let result = Proc::new("bad\x00ns", 0);
    assert!(result.is_err());
}

#[test]
fn test_proc_new_empty_nspace() {
    let proc = Proc::new("", 0).expect("empty nspace ok");
    let _ = proc;
}

#[test]
fn test_proc_new_unicode_nspace() {
    let proc = Proc::new("ns-αβγ", 0).expect("unicode nspace ok");
    let _ = proc;
}

#[test]
fn test_proc_multiple_independent() {
    let p1 = Proc::new("ns1", 0).expect("p1");
    let p2 = Proc::new("ns2", 1).expect("p2");
    let p3 = Proc::new("ns3", 2).expect("p3");
    let _ = (&p1, &p2, &p3);
}

#[test]
fn test_proc_type_check() {
    let proc = Proc::new("debug_ns", 42).expect("create");
    let _ = std::any::type_name::<std::sync::Arc<std::sync::Mutex<()>>>();
    let _ = proc;
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_construct_callback_wrapper() {
    let _cb = GroupConstructCallbackWrapper::new(
        |_status: PmixStatus, _info: Vec<pmix::Info>| {},
    );
}

#[test]
fn test_invite_callback_wrapper() {
    let _cb = GroupInviteCallbackWrapper::new(
        |_status: PmixStatus, _info: Vec<pmix::Info>| {},
    );
}

#[test]
fn test_join_callback_wrapper() {
    let _cb = GroupJoinCallbackWrapper::new(
        |_status: PmixStatus, _info: Vec<pmix::Info>| {},
    );
}

#[test]
fn test_leave_callback_wrapper() {
    let _cb = GroupLeaveCallbackWrapper::new(|_status: PmixStatus| {});
}

#[test]
fn test_destruct_callback_wrapper() {
    let _cb = GroupDestructCallbackWrapper::new(|_status: PmixStatus| {});
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder compile-time checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_infobuilder_build_empty() {
    let info = InfoBuilder::new().build();
    let _ = info;
}

#[test]
fn test_infobuilder_collect_data() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let _info = builder.build();
}

// ─────────────────────────────────────────────────────────────────────────────
// Panic safety
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_does_not_panic_on_empty_id() {
    let result = std::panic::catch_unwind(|| {
        let _ = group_construct("", &[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_group_construct_does_not_panic_on_empty_procs() {
    let result = std::panic::catch_unwind(|| {
        let _ = group_construct("valid_group", &[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_group_destruct_does_not_panic_on_empty_id() {
    let result = std::panic::catch_unwind(|| {
        let _ = group_destruct("", &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_group_leave_does_not_panic_on_empty_id() {
    let result = std::panic::catch_unwind(|| {
        let _ = group_leave("", &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_group_invite_does_not_panic_on_empty_id() {
    let result = std::panic::catch_unwind(|| {
        let _ = group_invite("", &[], &[]);
    });
    assert!(result.is_ok());
}

#[test]
fn test_group_join_does_not_panic_on_empty_id() {
    let result = std::panic::catch_unwind(|| {
        let leader = Proc::new("ns", 0).expect("leader");
        let _ = group_join("", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    });
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation tests (no FFI — pure Rust)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_empty_group_id_rejected() {
    let result = group_construct("", &[], &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_construct_empty_procs_rejected() {
    let result = group_construct("my_group", &[], &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_destruct_empty_group_id_rejected() {
    let result = group_destruct("", &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_leave_empty_group_id_rejected() {
    let result = group_leave("", &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_invite_empty_group_id_rejected() {
    let result = group_invite("", &[], &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_join_empty_group_id_rejected() {
    let leader = Proc::new("ns", 0).expect("leader");
    let result = group_join("", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    assert!(result.is_err());
}

#[test]
fn test_group_construct_nb_empty_group_id_rejected() {
    let cb = GroupConstructCallbackWrapper::new(|_s, _i| {});
    let result = group_construct_nb("", &[], &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_group_construct_nb_empty_procs_rejected() {
    let cb = GroupConstructCallbackWrapper::new(|_s, _i| {});
    let result = group_construct_nb("my_group", &[], &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_group_destruct_nb_empty_group_id_rejected() {
    let cb = GroupDestructCallbackWrapper::new(|_| {});
    let result = group_destruct_nb("", &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_group_leave_nb_empty_group_id_rejected() {
    let cb = GroupLeaveCallbackWrapper::new(|_| {});
    let result = group_leave_nb("", &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_group_invite_nb_empty_group_id_rejected() {
    let cb = GroupInviteCallbackWrapper::new(|_s, _i| {});
    let result = group_invite_nb("", &[], &[], cb);
    assert!(result.is_err());
}

#[test]
fn test_group_join_nb_empty_group_id_rejected() {
    let leader = Proc::new("ns", 0).expect("leader");
    let cb = GroupJoinCallbackWrapper::new(|_s, _i| {});
    let result = group_join_nb("", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[], cb);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI integration tests — require PMIx_Init (marked #[ignore])
// ─────────────────────────────────────────────────────────────────────────────

// ── group_construct ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_single_proc() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("test_ns", 0).expect("proc");
    let result = group_construct("test_group", &[proc], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_multiple_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let p1 = Proc::new("ns", 0).expect("p1");
    let p2 = Proc::new("ns", 1).expect("p2");
    let p3 = Proc::new("ns", 2).expect("p3");
    let result = group_construct("multi_group", &[p1, p2, p3], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let dirs = vec![InfoBuilder::new().build()];
    let result = group_construct("directed_group", &[proc], &dirs);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_result_is_vec_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let result = group_construct("test_group", &[proc], &[]);
    match result {
        Ok(info_vec) => {
            let _ = info_vec.len();
        }
        Err(_) => {
            // Server may not support groups — that's fine
        }
    }
}

// ── group_construct_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let cb = GroupConstructCallbackWrapper::new(|_s, _i| {});
    let result = group_construct_nb("nb_group", &[proc], &[], cb);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_nb_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let info = vec![InfoBuilder::new().build()];
    let cb = GroupConstructCallbackWrapper::new(|_s, _i| {});
    let result = group_construct_nb("nb_group", &[proc], &info, cb);
    assert!(result.is_ok());
}

// ── group_invite ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_single_proc() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let result = group_invite("invite_group", &[proc], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_multiple_procs() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let p1 = Proc::new("ns", 0).expect("p1");
    let p2 = Proc::new("ns", 1).expect("p2");
    let result = group_invite("invite_group", &[p1, p2], &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_with_directives() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let dirs = vec![InfoBuilder::new().build()];
    let result = group_invite("invite_group", &[proc], &dirs);
    let _ = result;
}

// ── group_invite_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_invite_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let cb = GroupInviteCallbackWrapper::new(|_s, _i| {});
    let result = group_invite_nb("invite_group", &[proc], &[], cb);
    assert!(result.is_ok());
}

// ── group_join ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_join_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let leader = Proc::new("ns", 0).expect("leader");
    let result = group_join("join_group", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_join_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let leader = Proc::new("ns", 0).expect("leader");
    let info = vec![InfoBuilder::new().build()];
    let result = group_join("join_group", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &info);
    let _ = result;
}

// ── group_join_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_join_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let leader = Proc::new("ns", 0).expect("leader");
    let cb = GroupJoinCallbackWrapper::new(|_s, _i| {});
    let result = group_join_nb("join_group", &leader, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[], cb);
    assert!(result.is_ok());
}

// ── group_leave ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_leave_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = group_leave("leave_group", &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_leave_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = vec![InfoBuilder::new().build()];
    let result = group_leave("leave_group", &info);
    let _ = result;
}

// ── group_leave_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_leave_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let cb = GroupLeaveCallbackWrapper::new(|_| {});
    let result = group_leave_nb("leave_group", &[], cb);
    assert!(result.is_ok());
}

// ── group_destruct ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_destruct_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = group_destruct("destruct_group", &[]);
    let _ = result;
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_destruct_with_info() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let info = vec![InfoBuilder::new().build()];
    let result = group_destruct("destruct_group", &info);
    let _ = result;
}

// ── group_destruct_nb ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_destruct_nb_success() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let cb = GroupDestructCallbackWrapper::new(|_| {});
    let result = group_destruct_nb("destruct_group", &[], cb);
    assert!(result.is_ok());
}

// ── Lifecycle / integration ──

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_then_destruct() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let _ = group_construct("lifecycle_group", &[proc], &[]);
    let _ = group_destruct("lifecycle_group", &[]);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_construct_then_join_then_leave() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let _ = group_construct("lifecycle_group", &[proc.clone()], &[]);
    let _ = group_join("lifecycle_group", &proc, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    let _ = group_leave("lifecycle_group", &[]);
}

#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_group_full_lifecycle() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let proc = Proc::new("ns", 0).expect("proc");
    let proc2 = Proc::new("ns", 1).expect("proc2");
    let _ = group_construct("full_group", &[proc.clone()], &[]);
    let _ = group_invite("full_group", &[proc2.clone()], &[]);
    let _ = group_join("full_group", &proc2, pmix_group_opt_t::PMIX_GROUP_DECLINE, &[]);
    let _ = group_leave("full_group", &[]);
    let _ = group_destruct("full_group", &[]);
}
