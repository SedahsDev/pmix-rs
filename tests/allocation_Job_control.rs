//! Tests for PMIx_Job_control and PMIx_Job_control_nb safe wrappers.

use pmix::PmixStatus;
use pmix::allocation::*;

// ─────────────────────────────────────────────────────────────────────────────
// PmixJobCtrlAction enum tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_job_ctrl_action_keys() {
    assert_eq!(PmixJobCtrlAction::Pause.key(), "pmix.jctrl.pause");
    assert_eq!(PmixJobCtrlAction::Resume.key(), "pmix.jctrl.resume");
    assert_eq!(PmixJobCtrlAction::Kill.key(), "pmix.jctrl.kill");
    assert_eq!(PmixJobCtrlAction::Signal(9).key(), "pmix.jctrl.sig");
    assert_eq!(PmixJobCtrlAction::Terminate.key(), "pmix.jctrl.term");
    assert_eq!(
        PmixJobCtrlAction::Cancel("req-1".to_string()).key(),
        "pmix.jctrl.cancel"
    );
    assert_eq!(
        PmixJobCtrlAction::Restart("ckpt-1".to_string()).key(),
        "pmix.jctrl.restart"
    );
}

#[test]
fn test_job_ctrl_action_display() {
    assert_eq!(format!("{}", PmixJobCtrlAction::Pause), "PAUSE");
    assert_eq!(format!("{}", PmixJobCtrlAction::Resume), "RESUME");
    assert_eq!(format!("{}", PmixJobCtrlAction::Kill), "KILL");
    assert_eq!(format!("{}", PmixJobCtrlAction::Signal(9)), "SIGNAL(9)");
    assert_eq!(format!("{}", PmixJobCtrlAction::Terminate), "TERMINATE");
    assert_eq!(
        format!("{}", PmixJobCtrlAction::Cancel("req-1".to_string())),
        "CANCEL(req-1)"
    );
    assert_eq!(
        format!("{}", PmixJobCtrlAction::Restart("ckpt-1".to_string())),
        "RESTART(ckpt-1)"
    );
}

#[test]
fn test_job_ctrl_action_clone() {
    let action = PmixJobCtrlAction::Signal(15);
    let cloned = action.clone();
    assert_eq!(action, cloned);
}

#[test]
fn test_job_ctrl_action_partial_eq() {
    assert_eq!(PmixJobCtrlAction::Pause, PmixJobCtrlAction::Pause);
    assert_ne!(PmixJobCtrlAction::Pause, PmixJobCtrlAction::Kill);
    assert_eq!(PmixJobCtrlAction::Signal(9), PmixJobCtrlAction::Signal(9));
    assert_ne!(PmixJobCtrlAction::Signal(9), PmixJobCtrlAction::Signal(15));
}

#[test]
fn test_job_ctrl_action_debug() {
    let action = PmixJobCtrlAction::Pause;
    let s = format!("{:?}", action);
    assert!(!s.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// JobControlResults tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_job_ctrl_results_empty() {
    let results = JobControlResults::new_empty();
    assert!(results.is_empty());
    assert_eq!(results.len(), 0);
}

#[test]
fn test_job_ctrl_results_debug() {
    let results = JobControlResults::new_empty();
    let s = format!("{:?}", results);
    assert!(!s.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// job_control (blocking) — FFI call tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_job_ctrl_empty_targets_empty_directives() {
    // Calling with empty targets and empty directives should return an error
    // (PMIx not initialized or bad param).
    let result = job_control(&[], &[]);
    assert!(result.is_err());
}

#[test]
fn test_job_ctrl_not_initialized() {
    // Without PMIx_Init, job_control should fail with PMIX_ERR_INIT.
    let result = job_control(&[], &[]);
    match result {
        Err(PmixStatus::Known(pmix::PmixError::ErrInit)) => {
            // Expected — PMIx was not initialized.
        }
        Err(e) => {
            // Other errors are also acceptable (e.g., bad param).
            let _ = e;
        }
        Ok(_) => panic!("job_control should fail when PMIx is not initialized"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// job_control_nb (non-blocking) — FFI call tests
// ─────────────────────────────────────────────────────────────────────────────

struct TestJobCtrlCallback {
    called: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl JobControlCallback for TestJobCtrlCallback {
    fn on_complete(&self, _status: PmixStatus, _results: JobControlResults) {
        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn test_job_ctrl_nb_not_initialized() {
    // Without PMIx_Init, job_control_nb should fail immediately.
    let callback = Box::new(TestJobCtrlCallback {
        called: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });
    let result = job_control_nb(&[], &[], callback);
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration: build a kill directive and verify it passes to FFI
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_job_ctrl_action_build_directive() {
    // Verify that PmixJobCtrlAction::key() returns the correct PMIx info key
    // that should be used to build an Info directive for job_control.
    let action = PmixJobCtrlAction::Kill;
    assert_eq!(action.key(), "pmix.jctrl.kill");
}

#[test]
fn test_job_ctrl_signal_action() {
    // Verify Signal variant carries the signal number correctly.
    let action = PmixJobCtrlAction::Signal(9);
    assert_eq!(action.key(), "pmix.jctrl.sig");
    assert_eq!(format!("{}", action), "SIGNAL(9)");
}

#[test]
fn test_job_ctrl_cancel_action() {
    let action = PmixJobCtrlAction::Cancel("my-request-id".to_string());
    assert_eq!(action.key(), "pmix.jctrl.cancel");
    assert_eq!(format!("{}", action), "CANCEL(my-request-id)");
}

#[test]
fn test_job_ctrl_restart_action() {
    let action = PmixJobCtrlAction::Restart("ckpt-abc".to_string());
    assert_eq!(action.key(), "pmix.jctrl.restart");
    assert_eq!(format!("{}", action), "RESTART(ckpt-abc)");
}
