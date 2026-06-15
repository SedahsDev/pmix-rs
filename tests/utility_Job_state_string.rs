//! Integration tests for `PMIx_Job_state_string` via the safe `job_state_string()` wrapper.

use pmix::{PmixJobState, utility::job_state_string};

#[test]
fn job_state_string_all_defined() {
    let values = [
        PmixJobState::Undef,
        PmixJobState::AwaitingAlloc,
        PmixJobState::LaunchUnderway,
        PmixJobState::Running,
        PmixJobState::Suspended,
        PmixJobState::Connected,
        PmixJobState::Unterminated,
        PmixJobState::Terminated,
        PmixJobState::TerminatedWithError,
    ];
    for v in values {
        let result = job_state_string(v);
        assert!(
            result.is_ok(),
            "job_state_string({:?}) should return Ok, got {:?}",
            v,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "job_state_string({:?}) should not be empty",
            v
        );
    }
}

#[test]
fn job_state_string_distinct() {
    let running = job_state_string(PmixJobState::Running).unwrap();
    let term = job_state_string(PmixJobState::Terminated).unwrap();
    assert_ne!(running, term, "Running and Terminated must differ");
}

#[test]
fn job_state_string_unknown() {
    let result = job_state_string(PmixJobState::Unknown(99));
    assert!(
        result.is_ok(),
        "Unknown(99) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn job_state_string_return_type() {
    let _r: Result<String, pmix::PmixStatus> = job_state_string(PmixJobState::Running);
}
