//! Round 8 — P2: groups.rs module via prte-beast daemon.
//!
//! Uses server_init for groups testing. Single consolidated test to avoid PMIx
//! global state corruption. Uses daemon_lock for serialization.
//!
//! Run:
//!   cargo test --test daemon_groups_via_daemon -- --include-ignored --test-threads=1

mod daemon_helper;

use pmix::groups::{
    group_construct, group_construct_nb, group_destruct, group_destruct_nb, group_invite,
    group_invite_nb, group_join, group_join_nb, group_leave, group_leave_nb,
    GroupConstructCallbackWrapper, GroupDestructCallbackWrapper, GroupInviteCallbackWrapper,
    GroupJoinCallbackWrapper, GroupLeaveCallbackWrapper, pmix_group_opt_t,
};
use pmix::server::{server_finalize, server_init, PmixServerModule};
use pmix::{InfoBuilder, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Standalone type-check tests (always run, no daemon needed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_group_construct_type() {
    let _f: fn(&str, &[Proc], &[pmix::Info]) -> Result<Vec<pmix::Info>, PmixStatus> =
        group_construct;
}

#[test]
fn test_group_construct_nb_type() {
    let _f: fn(
        &str,
        &[Proc],
        &[pmix::Info],
        GroupConstructCallbackWrapper,
    ) -> Result<(), PmixStatus> = group_construct_nb;
}

#[test]
fn test_group_invite_type() {
    let _f: fn(&str, &[Proc], &[pmix::Info]) -> Result<Vec<pmix::Info>, PmixStatus> = group_invite;
}

#[test]
fn test_group_invite_nb_type() {
    let _f: fn(&str, &[Proc], &[pmix::Info], GroupInviteCallbackWrapper) -> Result<(), PmixStatus> =
        group_invite_nb;
}

#[test]
fn test_group_join_type() {
    let _f: fn(
        &str,
        &Proc,
        pmix_group_opt_t,
        &[pmix::Info],
    ) -> Result<Vec<pmix::Info>, PmixStatus> = group_join;
}

#[test]
fn test_group_join_nb_type() {
    let _f: fn(
        &str,
        &Proc,
        pmix_group_opt_t,
        &[pmix::Info],
        GroupJoinCallbackWrapper,
    ) -> Result<(), PmixStatus> = group_join_nb;
}

#[test]
fn test_group_leave_type() {
    let _f: fn(&str, &[pmix::Info]) -> Result<(), PmixStatus> = group_leave;
}

#[test]
fn test_group_leave_nb_type() {
    let _f: fn(&str, &[pmix::Info], GroupLeaveCallbackWrapper) -> Result<(), PmixStatus> =
        group_leave_nb;
}

#[test]
fn test_group_destruct_type() {
    let _f: fn(&str, &[pmix::Info]) -> Result<(), PmixStatus> = group_destruct;
}

#[test]
fn test_group_destruct_nb_type() {
    let _f: fn(&str, &[pmix::Info], GroupDestructCallbackWrapper) -> Result<(), PmixStatus> =
        group_destruct_nb;
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon tests — single consolidated test using server_init/server_finalize.
// Groups API requires server role, so we cannot use the shared tool handle.
//
// NOTE: Do NOT call get_tool_handle() before server_init — the new PRTE version
// is stricter about mixing tool and server roles and can hang.
// ─────────────────────────────────────────────────────────────────────────────

/// Full groups workflow: construct → invite → join → leave → destruct (all nb variants too)
#[test]
#[ignore = "daemon isolation"]
fn test_groups_all_ffi_operations() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");

    // Initialize as server — do NOT use shared tool handle first
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");

    let member = Proc::new("test-nspace", 0).expect("proc");
    let members = vec![member];
    let directives: &[pmix::Info] = &[];
    let group_id = "test-group-001";

    // ── Blocking group_construct ──
    let _ = group_construct(group_id, &members, directives);

    // ── Blocking group_construct_nb ──
    let cb = GroupConstructCallbackWrapper::new(|_status, _info| {});
    let _ = group_construct_nb(group_id, &members, directives, cb);

    // ── Blocking group_invite ──
    let _ = group_invite(group_id, &members, directives);

    // ── Blocking group_invite_nb ──
    let cb = GroupInviteCallbackWrapper::new(|_status, _info| {});
    let _ = group_invite_nb(group_id, &members, directives, cb);

    // ── Blocking group_join ──
    let leader = Proc::new("test-nspace", 0).expect("proc");
    let _ = group_join(
        group_id,
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        directives,
    );

    // ── Blocking group_join_nb ──
    let cb = GroupJoinCallbackWrapper::new(|_status, _info| {});
    let _ = group_join_nb(
        group_id,
        &leader,
        pmix_group_opt_t::PMIX_GROUP_DECLINE,
        directives,
        cb,
    );

    // ── Blocking group_leave ──
    let _ = group_leave(group_id, directives);

    // ── Blocking group_leave_nb ──
    let cb = GroupLeaveCallbackWrapper::new(|_status| {});
    let _ = group_leave_nb(group_id, directives, cb);

    // ── Blocking group_destruct ──
    let _ = group_destruct(group_id, directives);

    // ── Blocking group_destruct_nb ──
    let cb = GroupDestructCallbackWrapper::new(|_status| {});
    let _ = group_destruct_nb(group_id, directives, cb);

    // ── Edge cases: empty group_id returns ErrBadParam ──
    assert!(group_construct("", &members, directives).is_err());
    assert!(group_invite("", &members, directives).is_err());
    assert!(group_leave("", directives).is_err());
    assert!(group_destruct("", directives).is_err());

    // Cleanup
    let _ = server_finalize(handle);
}
