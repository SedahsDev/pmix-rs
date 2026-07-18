//! Structural unit tests for the groups module — TASK-043.
//!
//! Focus on code paths not yet exercised by existing tests.
//! All tests run WITHOUT PMIx_Init — they test the Rust wrapper layer only.

use pmix::groups::*;
use pmix::{InfoBuilder, PmixStatus, Proc};

fn test_proc(rank: u32) -> Proc {
    Proc::new("test_ns", rank).expect("Proc::new should succeed")
}

fn test_procs(count: usize) -> Vec<Proc> {
    (0..count).map(|i| test_proc(i as u32)).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct — extended parameter tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_with_special_group_ids() {
    let procs = test_procs(1);
    let special_ids = [
        "grp-with-dash",
        "grp_with_underscore",
        "grp.with.dots",
        "grp123",
    ];
    for id in &special_ids {
        let result = group_construct(id, &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                // BAD_PARAM is -27; verify we don't get it for valid IDs
                assert!(
                    e.to_raw() != -27,
                    "Group ID '{}' should not trigger BAD_PARAM",
                    id
                );
            }
        }
    }
}

#[test]
fn test_group_construct_with_directives() {
    let procs = test_procs(2);
    let directives = vec![InfoBuilder::new().build()];
    let result = group_construct("test_grp", &procs, &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_construct_with_large_proc_array() {
    let procs = test_procs(50);
    let result = group_construct("big_grp", &procs, &[]);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27, "50 procs should be valid");
        }
    }
}

#[test]
fn test_group_construct_both_empty() {
    let result = group_construct("", &[], &[]);
    assert!(result.is_err(), "Both empty should fail");
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert_eq!(err.to_raw(), -27);
}

// ─────────────────────────────────────────────────────────────────────────────
// group_construct_nb — extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_nb_with_special_group_ids() {
    let procs = test_procs(1);
    let cb = GroupConstructCallbackWrapper::new(|_, _| {});
    let result = group_construct_nb("grp-with-dash", &procs, &[], cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_construct_nb_with_info_directives() {
    let procs = test_procs(1);
    let info = vec![InfoBuilder::new().build()];
    let cb = GroupConstructCallbackWrapper::new(|_, _| {});
    let result = group_construct_nb("test_grp", &procs, &info, cb);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_invite — extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_invite_with_directives() {
    let procs = test_procs(1);
    let directives = vec![InfoBuilder::new().build()];
    let result = group_invite("invite_grp", &procs, &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_invite_with_multiple_procs() {
    let procs = test_procs(10);
    let result = group_invite("multi_invite", &procs, &[]);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_join — extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_join_with_directives() {
    let leader = test_proc(0);
    let directives = vec![InfoBuilder::new().build()];
    let result = group_join(
        "join_grp",
        &leader,
        pmix_group_opt_t::PMIX_GROUP_ACCEPT,
        &directives,
    );
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_join_with_different_leaders() {
    for rank in [0u32, 1, 42, u32::MAX] {
        let leader = test_proc(rank);
        let result = group_join(
            "join_grp",
            &leader,
            pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.to_raw() != -27);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_leave — extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_leave_with_directives() {
    let directives = vec![InfoBuilder::new().build()];
    let result = group_leave("leave_grp", &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_leave_with_multiple_directives() {
    let directives = vec![InfoBuilder::new().build(), InfoBuilder::new().build()];
    let result = group_leave("leave_grp", &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// group_destruct — extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_destruct_with_directives() {
    let directives = vec![InfoBuilder::new().build()];
    let result = group_destruct("destruct_grp", &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback wrapper Send trait verification (all wrappers)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_all_callback_wrappers_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<GroupConstructCallbackWrapper>();
    assert_send::<GroupInviteCallbackWrapper>();
    assert_send::<GroupJoinCallbackWrapper>();
    assert_send::<GroupLeaveCallbackWrapper>();
    assert_send::<GroupDestructCallbackWrapper>();
}

#[test]
fn test_all_callback_wrappers_in_arc() {
    let _a: std::sync::Arc<GroupConstructCallbackWrapper> =
        std::sync::Arc::new(GroupConstructCallbackWrapper::new(|_, _| {}));
    let _b: std::sync::Arc<GroupInviteCallbackWrapper> =
        std::sync::Arc::new(GroupInviteCallbackWrapper::new(|_, _| {}));
    let _c: std::sync::Arc<GroupJoinCallbackWrapper> =
        std::sync::Arc::new(GroupJoinCallbackWrapper::new(|_, _| {}));
    let _d: std::sync::Arc<GroupLeaveCallbackWrapper> =
        std::sync::Arc::new(GroupLeaveCallbackWrapper::new(|_| {}));
    let _e: std::sync::Arc<GroupDestructCallbackWrapper> =
        std::sync::Arc::new(GroupDestructCallbackWrapper::new(|_| {}));
}

// ─────────────────────────────────────────────────────────────────────────────
// Proc construction tests (used by group functions)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proc_new_with_various_namespaces() {
    let namespaces = ["test", "ns1", "ns2", "a", "very_long_namespace_name"];
    for ns in &namespaces {
        let proc = Proc::new(ns, 0);
        assert!(proc.is_ok(), "Proc::new with '{}' should succeed", ns);
    }
}

#[test]
fn test_proc_new_rejects_nul_in_namespace() {
    let result = Proc::new("test\0ns", 0);
    assert!(result.is_err());
}

#[test]
fn test_proc_set_and_get_rank() {
    let mut proc = test_proc(0);
    assert_eq!(proc.get_rank(), 0);
    proc.set_rank(42);
    assert_eq!(proc.get_rank(), 42);
    proc.set_rank(u32::MAX);
    assert_eq!(proc.get_rank(), u32::MAX);
}

#[test]
fn test_proc_new_with_nspace() {
    let proc0 = test_proc(0);
    let proc1 = proc0
        .new_with_nspace(1)
        .expect("new_with_nspace should succeed");
    assert_eq!(proc1.get_rank(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoBuilder integration with groups
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_with_infobuilder_directives() {
    let procs = test_procs(1);
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let directives = vec![builder.build()];
    let result = group_construct("test_grp", &procs, &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}

#[test]
fn test_group_invite_with_infobuilder_directives() {
    let procs = test_procs(1);
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let directives = vec![builder.build()];
    let result = group_invite("test_grp", &procs, &directives);
    match result {
        Ok(_) => {}
        Err(e) => {
            assert!(e.to_raw() != -27);
        }
    }
}
