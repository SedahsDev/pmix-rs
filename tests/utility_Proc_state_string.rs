//! Integration tests for `PMIx_Proc_state_string` and `PmixProcState`.
//!
//! These tests exercise the safe Rust wrapper around `PMIx_Proc_state_string`
//! and the `PmixProcState` enum's conversion methods. They call into the
//! real PMIx library — a running PMIx daemon is NOT required because
//! `PMIx_Proc_state_string` only performs a local lookup of a static string.

use pmix::{PmixProcState, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// proc_state_string — FFI wrapper tests
// ─────────────────────────────────────────────────────────────────────────────

/// `proc_state_string` returns a valid string for `Running` state.
#[test]
fn proc_state_string_running() {
    let desc = pmix::utility::proc_state_string(PmixProcState::Running)
        .expect("proc_state_string(Running) should succeed");
    assert!(!desc.is_empty(), "should not return empty string");
}

/// `proc_state_string` returns valid strings for all standard lifecycle states.
#[test]
fn proc_state_string_all_lifecycle_states() {
    let states = [
        PmixProcState::Undef,
        PmixProcState::Prepped,
        PmixProcState::LaunchUnderway,
        PmixProcState::Restart,
        PmixProcState::Terminate,
        PmixProcState::Running,
        PmixProcState::Connected,
        PmixProcState::Unterminated,
        PmixProcState::Terminated,
    ];
    for state in states {
        let result = pmix::utility::proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string({:?}) should succeed, got {:?}",
            state,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "proc_state_string({:?}) should not return empty",
            state
        );
    }
}

/// `proc_state_string` returns valid strings for all error-range states.
#[test]
fn proc_state_string_all_error_states() {
    let states = [
        PmixProcState::Error,
        PmixProcState::KilledByCmd,
        PmixProcState::Aborted,
        PmixProcState::FailedToStart,
        PmixProcState::AbortedBySig,
        PmixProcState::TermWoSync,
        PmixProcState::CommFailed,
        PmixProcState::SensorBoundExceeded,
        PmixProcState::CalledAbort,
        PmixProcState::HeartbeatFailed,
        PmixProcState::Migrating,
        PmixProcState::CannotRestart,
        PmixProcState::TermNonZero,
        PmixProcState::FailedToLaunch,
    ];
    for state in states {
        let result = pmix::utility::proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string({:?}) should succeed, got {:?}",
            state,
            result
        );
    }
}

/// `proc_state_string` handles unknown states gracefully (returns "UNKNOWN STATE").
#[test]
fn proc_state_string_unknown() {
    let state = PmixProcState::Unknown(99);
    let result = pmix::utility::proc_state_string(state);
    assert!(
        result.is_ok(),
        "proc_state_string(Unknown(99)) should succeed, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert!(
        !desc.is_empty(),
        "proc_state_string for unknown state should return non-empty string"
    );
}

/// `proc_state_string` is deterministic for the same input.
#[test]
fn proc_state_string_deterministic() {
    let state = PmixProcState::Terminated;
    let first = pmix::utility::proc_state_string(state).unwrap();
    let second = pmix::utility::proc_state_string(state).unwrap();
    assert_eq!(first, second, "proc_state_string must be deterministic");
}

/// `proc_state_string` returns distinct strings for distinct states.
#[test]
fn proc_state_string_distinct() {
    let running = pmix::utility::proc_state_string(PmixProcState::Running).unwrap();
    let terminated = pmix::utility::proc_state_string(PmixProcState::Terminated).unwrap();
    assert_ne!(running, terminated, "Running and Terminated must produce different strings");
}

/// `proc_state_string` for `Undef` returns the library's "UNDEFINED" string.
#[test]
fn proc_state_string_undef() {
    let desc = pmix::utility::proc_state_string(PmixProcState::Undef)
        .expect("proc_state_string(Undef) should succeed");
    assert_eq!(desc, "UNDEFINED", "Undef state should map to 'UNDEFINED'");
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcState — enum conversion tests
// ─────────────────────────────────────────────────────────────────────────────

/// `from_raw` / `to_raw` round-trip for all known states.
#[test]
fn proc_state_from_raw_to_raw_roundtrip() {
    let states: Vec<(u8, PmixProcState)> = vec![
        (0, PmixProcState::Undef),
        (1, PmixProcState::Prepped),
        (2, PmixProcState::LaunchUnderway),
        (3, PmixProcState::Restart),
        (4, PmixProcState::Terminate),
        (5, PmixProcState::Running),
        (6, PmixProcState::Connected),
        (15, PmixProcState::Unterminated),
        (20, PmixProcState::Terminated),
        (50, PmixProcState::Error),
        (51, PmixProcState::KilledByCmd),
        (52, PmixProcState::Aborted),
        (53, PmixProcState::FailedToStart),
        (54, PmixProcState::AbortedBySig),
        (55, PmixProcState::TermWoSync),
        (56, PmixProcState::CommFailed),
        (57, PmixProcState::SensorBoundExceeded),
        (58, PmixProcState::CalledAbort),
        (59, PmixProcState::HeartbeatFailed),
        (60, PmixProcState::Migrating),
        (61, PmixProcState::CannotRestart),
        (62, PmixProcState::TermNonZero),
        (63, PmixProcState::FailedToLaunch),
    ];
    for (raw, expected) in states {
        let state = PmixProcState::from_raw(raw);
        assert_eq!(state, expected, "from_raw({}) should yield {:?}", raw, expected);
        assert_eq!(state.to_raw(), raw, "to_raw({:?}) should yield {}", expected, raw);
    }
}

/// Unknown raw values are wrapped in `PmixProcState::Unknown`.
#[test]
fn proc_state_unknown_raw() {
    let state = PmixProcState::from_raw(99);
    assert!(matches!(state, PmixProcState::Unknown(99)));
    assert_eq!(state.to_raw(), 99);
}

/// `is_alive` correctly classifies active states.
#[test]
fn proc_state_is_alive() {
    assert!(PmixProcState::Running.is_alive());
    assert!(PmixProcState::Connected.is_alive());
    assert!(PmixProcState::Prepped.is_alive());
    assert!(PmixProcState::LaunchUnderway.is_alive());
    assert!(PmixProcState::Restart.is_alive());
    assert!(PmixProcState::Unterminated.is_alive());
    assert!(PmixProcState::Migrating.is_alive());
}

/// `is_alive` returns false for terminated and undefined states.
#[test]
fn proc_state_not_alive() {
    assert!(!PmixProcState::Undef.is_alive());
    assert!(!PmixProcState::Terminated.is_alive());
    assert!(!PmixProcState::Aborted.is_alive());
    assert!(!PmixProcState::Error.is_alive());
    assert!(!PmixProcState::Terminate.is_alive());
}

/// `is_terminated` correctly classifies terminated states.
#[test]
fn proc_state_is_terminated() {
    assert!(PmixProcState::Terminated.is_terminated());
    assert!(PmixProcState::Aborted.is_terminated());
    assert!(PmixProcState::KilledByCmd.is_terminated());
    assert!(PmixProcState::FailedToStart.is_terminated());
    assert!(PmixProcState::AbortedBySig.is_terminated());
    assert!(PmixProcState::TermWoSync.is_terminated());
    assert!(PmixProcState::CommFailed.is_terminated());
    assert!(PmixProcState::SensorBoundExceeded.is_terminated());
    assert!(PmixProcState::CalledAbort.is_terminated());
    assert!(PmixProcState::HeartbeatFailed.is_terminated());
    assert!(PmixProcState::CannotRestart.is_terminated());
    assert!(PmixProcState::TermNonZero.is_terminated());
    assert!(PmixProcState::FailedToLaunch.is_terminated());
}

/// `is_terminated` returns false for active and undefined states.
#[test]
fn proc_state_not_terminated() {
    assert!(!PmixProcState::Undef.is_terminated());
    assert!(!PmixProcState::Running.is_terminated());
    assert!(!PmixProcState::Connected.is_terminated());
    assert!(!PmixProcState::Prepped.is_terminated());
    assert!(!PmixProcState::Migrating.is_terminated());
}

// ─────────────────────────────────────────────────────────────────────────────
// Display tests
// ─────────────────────────────────────────────────────────────────────────────

/// `Display` for `PmixProcState` matches the C library's string output.
#[test]
fn proc_state_display_matches_c() {
    let states = [
        (PmixProcState::Undef, "UNDEFINED"),
        (PmixProcState::Running, "PROC EXECUTING"),
        (PmixProcState::Terminated, "PROC HAS TERMINATED"),
        (PmixProcState::Connected, "PROC HAS CONNECTED TO LOCAL PMIX SERVER"),
        (PmixProcState::Aborted, "PROC ABNORMALLY ABORTED"),
    ];
    for (state, expected) in states {
        let display = format!("{}", state);
        assert_eq!(
            display, expected,
            "Display for {:?} should be '{}', got '{}'",
            state, expected, display
        );
    }
}

/// `Display` for unknown states includes the raw value.
#[test]
fn proc_state_display_unknown() {
    let state = PmixProcState::Unknown(42);
    let display = format!("{}", state);
    assert!(
        display.contains("42"),
        "Display for Unknown(42) should contain '42', got '{}'",
        display
    );
}
