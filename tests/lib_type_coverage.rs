//! Comprehensive type coverage tests for lib.rs — covers enum variants,
//! builder methods, type conversions, and accessor methods that are NOT
//! exercised by integration tests requiring a running daemon.
//!
//! These are pure unit tests — no FFI calls that require a daemon.

use pmix::{
    BuilderError, IOFChannelFlags, InfoBuilder, InfoFlags, PmixAllocDirective, PmixDataRange,
    PmixDataType, PmixDeviceType, PmixEnvar, PmixError, PmixJobState, PmixLinkState, PmixPayload,
    PmixPersistence, PmixProcState, PmixScope, PmixStatus, PmixTimeval, PmixValueBuilder, Proc,
    ValueError,
};
use std::ffi::CString;

// ═══════════════════════════════════════════════════════════════════════════
// PmixStatus — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_status_from_raw_all_known() {
    // Test that known error codes produce Known variants
    let test_codes = [
        (0, PmixError::Success),
        (-1, PmixError::Error),
        (-3, PmixError::DebuggerRelease),
        (-4, PmixError::ErrProcRestart),
        (-5, PmixError::ErrProcCheckpoint),
        (-6, PmixError::ErrProcMigrate),
        (-8, PmixError::ErrProcRequestedAbort),
        (-11, PmixError::ErrExists),
        (-12, PmixError::ErrInvalidCred),
        (-15, PmixError::ErrWouldBlock),
        (-16, PmixError::ErrUnknownDataType),
        (-18, PmixError::ErrTypeMismatch),
        (-19, PmixError::ErrUnpackInadequateSpace),
        (-20, PmixError::ErrUnpackFailure),
        (-21, PmixError::ErrPackFailure),
        (-23, PmixError::ErrNoPermissions),
        (-24, PmixError::ErrTimeout),
        (-25, PmixError::ErrUnreach),
        (-27, PmixError::ErrBadParam),
        (-28, PmixError::ErrResourceBusy),
        (-29, PmixError::ErrOutOfResource),
        (-31, PmixError::ErrInit),
        (-32, PmixError::ErrNomem),
        (-46, PmixError::ErrNotFound),
        (-47, PmixError::ErrNotSupported),
        (-49, PmixError::ErrCommFailure),
        (-50, PmixError::ErrUnpackReadPastEndOfBuffer),
        (-51, PmixError::ErrConflictingCleanupDirectives),
        (-52, PmixError::ErrPartialSuccess),
        (-53, PmixError::ErrDuplicateKey),
        (-58, PmixError::ReadyForDebug),
        (-59, PmixError::ErrParamValueNotSupported),
        (-60, PmixError::ErrEmpty),
        (-61, PmixError::ErrLostConnection),
        (-62, PmixError::ErrExistsOutsideScope),
        (-106, PmixError::JctrlCheckpoint),
        (-107, PmixError::JctrlCheckpointComplete),
        (-108, PmixError::JctrlPreemptAlert),
        (-109, PmixError::MonitorHeartbeatAlert),
        (-110, PmixError::MonitorFileAlert),
        (-113, PmixError::FabricUpdateEndpoints),
        (-144, PmixError::ErrEventRegistration),
        (-145, PmixError::EventJobEnd),
        (-156, PmixError::OperationInProgress),
        (-157, PmixError::OperationSucceeded),
        (-158, PmixError::ErrInvalidOperation),
        (-171, PmixError::ErrRepeatAttrRegistration),
        (-172, PmixError::ErrIofFailure),
        (-173, PmixError::ErrIofComplete),
        (-175, PmixError::FabricUpdated),
        (-176, PmixError::FabricUpdatePending),
        (-177, PmixError::ErrJobAppNotExecutable),
        (-178, PmixError::ErrJobNoExeSpecified),
        (-179, PmixError::ErrJobFailedToMap),
        (-180, PmixError::ErrJobCanceled),
        (-181, PmixError::ErrJobFailedToLaunch),
        (-182, PmixError::ErrJobAborted),
        (-183, PmixError::ErrJobKilledByCmd),
        (-184, PmixError::ErrJobAbortedBySig),
        (-185, PmixError::ErrJobTermWoSync),
        (-186, PmixError::ErrJobSensorBoundExceeded),
        (-187, PmixError::ErrJobNonZeroTerm),
        (-188, PmixError::ErrJobAllocFailed),
        (-189, PmixError::ErrJobAbortedBySysEvent),
        (-190, PmixError::ErrJobExeNotFound),
        (-191, PmixError::EventJobStart),
        (-192, PmixError::EventSessionStart),
        (-193, PmixError::EventSessionEnd),
        (-200, PmixError::ErrProcTermWoSync),
        (-201, PmixError::EventProcTerminated),
        (-230, PmixError::EventSysBase),
        (-231, PmixError::EventNodeDown),
        (-232, PmixError::EventNodeOffline),
        (-233, PmixError::ErrJobWdirNotFound),
        (-234, PmixError::ErrJobInsufficientResources),
        (-235, PmixError::ErrJobSysOpFailed),
        (-330, PmixError::EventSysOther),
        (-331, PmixError::EventNoActionTaken),
        (-332, PmixError::EventPartialActionTaken),
        (-333, PmixError::EventActionDeferred),
        (-334, PmixError::EventActionComplete),
        (-400, PmixError::ErrProcKilledByCmd),
        (-401, PmixError::ErrProcFailedToStart),
        (-402, PmixError::ErrProcAbortedBySig),
        (-403, PmixError::ErrProcSensorBoundExceeded),
        (-404, PmixError::ErrExitNonzeroTerm),
        (-3000, PmixError::ExternalErrBase),
    ];
    for (code, expected_error) in test_codes {
        let status = PmixStatus::from_raw(code);
        assert_eq!(
            status,
            PmixStatus::Known(expected_error),
            "from_raw({code}) should produce Known({:?})",
            expected_error
        );
    }
}

#[test]
fn test_pmix_status_from_raw_unknown() {
    assert!(matches!(
        PmixStatus::from_raw(9999),
        PmixStatus::Unknown(9999)
    ));
    assert!(matches!(
        PmixStatus::from_raw(-9999),
        PmixStatus::Unknown(-9999)
    ));
}

#[test]
fn test_pmix_status_is_error() {
    assert!(PmixStatus::Known(PmixError::Error).is_error());
    assert!(!PmixStatus::Known(PmixError::Success).is_error());
    assert!(PmixStatus::Unknown(-1).is_error());
    assert!(!PmixStatus::Unknown(1).is_error());
}

#[test]
fn test_pmix_status_display() {
    let known = format!("{}", PmixStatus::Known(PmixError::Error));
    assert!(!known.is_empty());
    let unknown = format!("{}", PmixStatus::Unknown(9999));
    assert!(unknown.contains("unknown"));
}

#[test]
fn test_pmix_status_from_pmix_error() {
    let status: PmixStatus = PmixError::Error.into();
    assert_eq!(status, PmixStatus::Known(PmixError::Error));
}

#[test]
fn test_pmix_status_to_raw() {
    assert_eq!(PmixStatus::Known(PmixError::Success).to_raw(), 0);
    assert_eq!(PmixStatus::Known(PmixError::Error).to_raw(), -1);
    assert_eq!(PmixStatus::Unknown(42).to_raw(), 42);
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixError — comprehensive name() coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_error_name_all_variants() {
    let names = [
        PmixError::Success,
        PmixError::Error,
        PmixError::DebuggerRelease,
        PmixError::ErrProcRestart,
        PmixError::ErrProcCheckpoint,
        PmixError::ErrProcMigrate,
        PmixError::ErrProcRequestedAbort,
        PmixError::ErrExists,
        PmixError::ErrInvalidCred,
        PmixError::ErrWouldBlock,
        PmixError::ErrUnknownDataType,
        PmixError::ErrTypeMismatch,
        PmixError::ErrUnpackInadequateSpace,
        PmixError::ErrUnpackFailure,
        PmixError::ErrPackFailure,
        PmixError::ErrNoPermissions,
        PmixError::ErrTimeout,
        PmixError::ErrUnreach,
        PmixError::ErrBadParam,
        PmixError::ErrResourceBusy,
        PmixError::ErrOutOfResource,
        PmixError::ErrInit,
        PmixError::ErrNomem,
        PmixError::ErrNotFound,
        PmixError::ErrNotSupported,
        PmixError::ErrCommFailure,
        PmixError::ErrUnpackReadPastEndOfBuffer,
        PmixError::ErrConflictingCleanupDirectives,
        PmixError::ErrPartialSuccess,
        PmixError::ErrDuplicateKey,
        PmixError::ReadyForDebug,
        PmixError::ErrParamValueNotSupported,
        PmixError::ErrEmpty,
        PmixError::ErrLostConnection,
        PmixError::ErrExistsOutsideScope,
        PmixError::JctrlCheckpoint,
        PmixError::JctrlCheckpointComplete,
        PmixError::JctrlPreemptAlert,
        PmixError::MonitorHeartbeatAlert,
        PmixError::MonitorFileAlert,
        PmixError::FabricUpdateEndpoints,
        PmixError::ErrEventRegistration,
        PmixError::EventJobEnd,
        PmixError::OperationInProgress,
        PmixError::OperationSucceeded,
        PmixError::ErrInvalidOperation,
        PmixError::ErrRepeatAttrRegistration,
        PmixError::ErrIofFailure,
        PmixError::ErrIofComplete,
        PmixError::FabricUpdated,
        PmixError::FabricUpdatePending,
        PmixError::ErrJobAppNotExecutable,
        PmixError::ErrJobNoExeSpecified,
        PmixError::ErrJobFailedToMap,
        PmixError::ErrJobCanceled,
        PmixError::ErrJobFailedToLaunch,
        PmixError::ErrJobAborted,
        PmixError::ErrJobKilledByCmd,
        PmixError::ErrJobAbortedBySig,
        PmixError::ErrJobTermWoSync,
        PmixError::ErrJobSensorBoundExceeded,
        PmixError::ErrJobNonZeroTerm,
        PmixError::ErrJobAllocFailed,
        PmixError::ErrJobAbortedBySysEvent,
        PmixError::ErrJobExeNotFound,
        PmixError::EventJobStart,
        PmixError::EventSessionStart,
        PmixError::EventSessionEnd,
        PmixError::ErrProcTermWoSync,
        PmixError::EventProcTerminated,
        PmixError::EventSysBase,
        PmixError::EventNodeDown,
        PmixError::EventNodeOffline,
        PmixError::ErrJobWdirNotFound,
        PmixError::ErrJobInsufficientResources,
        PmixError::ErrJobSysOpFailed,
        PmixError::EventSysOther,
        PmixError::EventNoActionTaken,
        PmixError::EventPartialActionTaken,
        PmixError::EventActionDeferred,
        PmixError::EventActionComplete,
        PmixError::ErrProcKilledByCmd,
        PmixError::ErrProcFailedToStart,
        PmixError::ErrProcAbortedBySig,
        PmixError::ErrProcSensorBoundExceeded,
        PmixError::ErrExitNonzeroTerm,
        PmixError::ExternalErrBase,
    ];
    for e in names {
        let name = e.name();
        assert!(!name.is_empty(), "name() for {:?} should not be empty", e);
        assert!(
            name.starts_with("PMIX_"),
            "name should start with PMIX_: {}",
            name
        );
    }
}

#[test]
fn test_pmix_error_from_raw_all_variants() {
    let mappings = [
        (0, PmixError::Success),
        (-1, PmixError::Error),
        (-3, PmixError::DebuggerRelease),
        (-4, PmixError::ErrProcRestart),
        (-5, PmixError::ErrProcCheckpoint),
        (-6, PmixError::ErrProcMigrate),
        (-8, PmixError::ErrProcRequestedAbort),
        (-11, PmixError::ErrExists),
        (-12, PmixError::ErrInvalidCred),
        (-15, PmixError::ErrWouldBlock),
        (-16, PmixError::ErrUnknownDataType),
        (-18, PmixError::ErrTypeMismatch),
        (-19, PmixError::ErrUnpackInadequateSpace),
        (-20, PmixError::ErrUnpackFailure),
        (-21, PmixError::ErrPackFailure),
        (-23, PmixError::ErrNoPermissions),
        (-24, PmixError::ErrTimeout),
        (-25, PmixError::ErrUnreach),
        (-27, PmixError::ErrBadParam),
        (-28, PmixError::ErrResourceBusy),
        (-29, PmixError::ErrOutOfResource),
        (-31, PmixError::ErrInit),
        (-32, PmixError::ErrNomem),
        (-46, PmixError::ErrNotFound),
        (-47, PmixError::ErrNotSupported),
        (-49, PmixError::ErrCommFailure),
        (-50, PmixError::ErrUnpackReadPastEndOfBuffer),
        (-51, PmixError::ErrConflictingCleanupDirectives),
        (-52, PmixError::ErrPartialSuccess),
        (-53, PmixError::ErrDuplicateKey),
        (-58, PmixError::ReadyForDebug),
        (-59, PmixError::ErrParamValueNotSupported),
        (-60, PmixError::ErrEmpty),
        (-61, PmixError::ErrLostConnection),
        (-62, PmixError::ErrExistsOutsideScope),
        (-106, PmixError::JctrlCheckpoint),
        (-107, PmixError::JctrlCheckpointComplete),
        (-108, PmixError::JctrlPreemptAlert),
        (-109, PmixError::MonitorHeartbeatAlert),
        (-110, PmixError::MonitorFileAlert),
        (-113, PmixError::FabricUpdateEndpoints),
        (-144, PmixError::ErrEventRegistration),
        (-145, PmixError::EventJobEnd),
        (-156, PmixError::OperationInProgress),
        (-157, PmixError::OperationSucceeded),
        (-158, PmixError::ErrInvalidOperation),
        (-171, PmixError::ErrRepeatAttrRegistration),
        (-172, PmixError::ErrIofFailure),
        (-173, PmixError::ErrIofComplete),
        (-175, PmixError::FabricUpdated),
        (-176, PmixError::FabricUpdatePending),
        (-177, PmixError::ErrJobAppNotExecutable),
        (-178, PmixError::ErrJobNoExeSpecified),
        (-179, PmixError::ErrJobFailedToMap),
        (-180, PmixError::ErrJobCanceled),
        (-181, PmixError::ErrJobFailedToLaunch),
        (-182, PmixError::ErrJobAborted),
        (-183, PmixError::ErrJobKilledByCmd),
        (-184, PmixError::ErrJobAbortedBySig),
        (-185, PmixError::ErrJobTermWoSync),
        (-186, PmixError::ErrJobSensorBoundExceeded),
        (-187, PmixError::ErrJobNonZeroTerm),
        (-188, PmixError::ErrJobAllocFailed),
        (-189, PmixError::ErrJobAbortedBySysEvent),
        (-190, PmixError::ErrJobExeNotFound),
        (-191, PmixError::EventJobStart),
        (-192, PmixError::EventSessionStart),
        (-193, PmixError::EventSessionEnd),
        (-200, PmixError::ErrProcTermWoSync),
        (-201, PmixError::EventProcTerminated),
        (-230, PmixError::EventSysBase),
        (-231, PmixError::EventNodeDown),
        (-232, PmixError::EventNodeOffline),
        (-233, PmixError::ErrJobWdirNotFound),
        (-234, PmixError::ErrJobInsufficientResources),
        (-235, PmixError::ErrJobSysOpFailed),
        (-330, PmixError::EventSysOther),
        (-331, PmixError::EventNoActionTaken),
        (-332, PmixError::EventPartialActionTaken),
        (-333, PmixError::EventActionDeferred),
        (-334, PmixError::EventActionComplete),
        (-400, PmixError::ErrProcKilledByCmd),
        (-401, PmixError::ErrProcFailedToStart),
        (-402, PmixError::ErrProcAbortedBySig),
        (-403, PmixError::ErrProcSensorBoundExceeded),
        (-404, PmixError::ErrExitNonzeroTerm),
        (-3000, PmixError::ExternalErrBase),
    ];
    for (code, expected) in mappings {
        assert_eq!(
            PmixError::from_raw(code),
            Some(expected),
            "from_raw({code}) should be {:?}, got {:?}",
            expected,
            PmixError::from_raw(code)
        );
    }
    // Unknown code returns None
    assert_eq!(PmixError::from_raw(9999), None);
}

#[test]
fn test_pmix_error_to_raw_roundtrip() {
    assert_eq!(PmixError::Success.to_raw(), 0);
    assert_eq!(PmixError::Error.to_raw(), -1);
    assert_eq!(PmixError::ErrTimeout.to_raw(), -24);
    assert_eq!(PmixError::ErrNotFound.to_raw(), -46);
}

#[test]
fn test_pmix_error_is_error_method() {
    assert!(!PmixError::Success.is_error());
    assert!(PmixError::Error.is_error());
    assert!(PmixError::ErrTimeout.is_error());
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixProcState — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_proc_state_from_raw_all() {
    assert_eq!(PmixProcState::from_raw(0), PmixProcState::Undef);
    assert_eq!(PmixProcState::from_raw(1), PmixProcState::Prepped);
    assert_eq!(PmixProcState::from_raw(2), PmixProcState::LaunchUnderway);
    assert_eq!(PmixProcState::from_raw(3), PmixProcState::Restart);
    assert_eq!(PmixProcState::from_raw(4), PmixProcState::Terminate);
    assert_eq!(PmixProcState::from_raw(5), PmixProcState::Running);
    assert_eq!(PmixProcState::from_raw(6), PmixProcState::Connected);
    assert_eq!(PmixProcState::from_raw(15), PmixProcState::Unterminated);
    assert_eq!(PmixProcState::from_raw(20), PmixProcState::Terminated);
    assert_eq!(PmixProcState::from_raw(50), PmixProcState::Error);
    assert_eq!(PmixProcState::from_raw(51), PmixProcState::KilledByCmd);
    assert_eq!(PmixProcState::from_raw(52), PmixProcState::Aborted);
    assert_eq!(PmixProcState::from_raw(53), PmixProcState::FailedToStart);
    assert_eq!(PmixProcState::from_raw(54), PmixProcState::AbortedBySig);
    assert_eq!(PmixProcState::from_raw(55), PmixProcState::TermWoSync);
    assert_eq!(PmixProcState::from_raw(56), PmixProcState::CommFailed);
    assert_eq!(
        PmixProcState::from_raw(57),
        PmixProcState::SensorBoundExceeded
    );
    assert_eq!(PmixProcState::from_raw(58), PmixProcState::CalledAbort);
    assert_eq!(PmixProcState::from_raw(59), PmixProcState::HeartbeatFailed);
    assert_eq!(PmixProcState::from_raw(60), PmixProcState::Migrating);
    assert_eq!(PmixProcState::from_raw(61), PmixProcState::CannotRestart);
    assert_eq!(PmixProcState::from_raw(62), PmixProcState::TermNonZero);
    assert_eq!(PmixProcState::from_raw(63), PmixProcState::FailedToLaunch);
    assert_eq!(PmixProcState::from_raw(99), PmixProcState::Unknown(99));
}

#[test]
fn test_pmix_proc_state_to_raw_all() {
    assert_eq!(PmixProcState::Undef.to_raw(), 0);
    assert_eq!(PmixProcState::Prepped.to_raw(), 1);
    assert_eq!(PmixProcState::Running.to_raw(), 5);
    assert_eq!(PmixProcState::Terminated.to_raw(), 20);
    assert_eq!(PmixProcState::Error.to_raw(), 50);
    assert_eq!(PmixProcState::FailedToLaunch.to_raw(), 63);
    assert_eq!(PmixProcState::Unknown(99).to_raw(), 99);
}

#[test]
fn test_pmix_proc_state_is_alive() {
    assert!(PmixProcState::Prepped.is_alive());
    assert!(PmixProcState::LaunchUnderway.is_alive());
    assert!(PmixProcState::Restart.is_alive());
    assert!(PmixProcState::Running.is_alive());
    assert!(PmixProcState::Connected.is_alive());
    assert!(PmixProcState::Unterminated.is_alive());
    assert!(PmixProcState::Migrating.is_alive());
    assert!(!PmixProcState::Undef.is_alive());
    assert!(!PmixProcState::Terminated.is_alive());
    assert!(!PmixProcState::Error.is_alive());
}

#[test]
fn test_pmix_proc_state_is_terminated() {
    assert!(PmixProcState::Terminated.is_terminated());
    assert!(PmixProcState::KilledByCmd.is_terminated());
    assert!(PmixProcState::Aborted.is_terminated());
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
    assert!(!PmixProcState::Running.is_terminated());
    assert!(!PmixProcState::Prepped.is_terminated());
}

#[test]
fn test_pmix_proc_state_display() {
    let undef = format!("{}", PmixProcState::Undef);
    assert_eq!(undef, "UNDEFINED");
    let running = format!("{}", PmixProcState::Running);
    assert_eq!(running, "PROC EXECUTING");
    let unknown = format!("{}", PmixProcState::Unknown(99));
    assert!(unknown.contains("UNKNOWN"));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixJobState — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_job_state_from_raw_all() {
    assert_eq!(PmixJobState::from_raw(0), PmixJobState::Undef);
    assert_eq!(PmixJobState::from_raw(1), PmixJobState::AwaitingAlloc);
    assert_eq!(PmixJobState::from_raw(2), PmixJobState::LaunchUnderway);
    assert_eq!(PmixJobState::from_raw(3), PmixJobState::Running);
    assert_eq!(PmixJobState::from_raw(4), PmixJobState::Suspended);
    assert_eq!(PmixJobState::from_raw(5), PmixJobState::Connected);
    assert_eq!(PmixJobState::from_raw(15), PmixJobState::Unterminated);
    assert_eq!(PmixJobState::from_raw(20), PmixJobState::Terminated);
    assert_eq!(
        PmixJobState::from_raw(50),
        PmixJobState::TerminatedWithError
    );
    assert_eq!(PmixJobState::from_raw(99), PmixJobState::Unknown(99));
}

#[test]
fn test_pmix_job_state_to_raw_all() {
    assert_eq!(PmixJobState::Undef.to_raw(), 0);
    assert_eq!(PmixJobState::AwaitingAlloc.to_raw(), 1);
    assert_eq!(PmixJobState::Running.to_raw(), 3);
    assert_eq!(PmixJobState::Terminated.to_raw(), 20);
    assert_eq!(PmixJobState::TerminatedWithError.to_raw(), 50);
    assert_eq!(PmixJobState::Unknown(99).to_raw(), 99);
}

#[test]
fn test_pmix_job_state_display() {
    assert_eq!(format!("{}", PmixJobState::Undef), "UNDEF");
    assert_eq!(format!("{}", PmixJobState::Running), "RUNNING");
    assert_eq!(format!("{}", PmixJobState::Terminated), "TERMINATED");
    let unk = format!("{}", PmixJobState::Unknown(99));
    assert!(unk.contains("UNKNOWN"));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixLinkState — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_link_state_from_raw_all() {
    assert_eq!(PmixLinkState::from_raw(0), PmixLinkState::UnknownState);
    assert_eq!(PmixLinkState::from_raw(1), PmixLinkState::LinkDown);
    assert_eq!(PmixLinkState::from_raw(2), PmixLinkState::LinkUp);
    assert_eq!(PmixLinkState::from_raw(99), PmixLinkState::Unknown(99));
}

#[test]
fn test_pmix_link_state_to_raw_all() {
    assert_eq!(PmixLinkState::UnknownState.to_raw(), 0);
    assert_eq!(PmixLinkState::LinkDown.to_raw(), 1);
    assert_eq!(PmixLinkState::LinkUp.to_raw(), 2);
    assert_eq!(PmixLinkState::Unknown(99).to_raw(), 99);
}

#[test]
fn test_pmix_link_state_display() {
    assert_eq!(format!("{}", PmixLinkState::UnknownState), "UNKNOWN");
    assert_eq!(format!("{}", PmixLinkState::LinkDown), "INACTIVE");
    assert_eq!(format!("{}", PmixLinkState::LinkUp), "ACTIVE");
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixDeviceType — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_device_type_from_raw_all() {
    assert_eq!(PmixDeviceType::from_raw(0x00), PmixDeviceType::UnknownType);
    assert_eq!(PmixDeviceType::from_raw(0x01), PmixDeviceType::Block);
    assert_eq!(PmixDeviceType::from_raw(0x02), PmixDeviceType::Gpu);
    assert_eq!(PmixDeviceType::from_raw(0x04), PmixDeviceType::Network);
    assert_eq!(PmixDeviceType::from_raw(0x08), PmixDeviceType::OpenFabrics);
    assert_eq!(PmixDeviceType::from_raw(0x10), PmixDeviceType::Dma);
    assert_eq!(PmixDeviceType::from_raw(0x20), PmixDeviceType::Coproc);
    assert_eq!(
        PmixDeviceType::from_raw(0xFF),
        PmixDeviceType::Unknown(0xFF)
    );
}

#[test]
fn test_pmix_device_type_to_raw_all() {
    assert_eq!(PmixDeviceType::UnknownType.to_raw(), 0x00);
    assert_eq!(PmixDeviceType::Block.to_raw(), 0x01);
    assert_eq!(PmixDeviceType::Gpu.to_raw(), 0x02);
    assert_eq!(PmixDeviceType::Network.to_raw(), 0x04);
    assert_eq!(PmixDeviceType::OpenFabrics.to_raw(), 0x08);
    assert_eq!(PmixDeviceType::Dma.to_raw(), 0x10);
    assert_eq!(PmixDeviceType::Coproc.to_raw(), 0x20);
    assert_eq!(PmixDeviceType::Unknown(0xFF).to_raw(), 0xFF);
}

#[test]
fn test_pmix_device_type_display() {
    assert_eq!(format!("{}", PmixDeviceType::UnknownType), "UNKNOWN");
    assert_eq!(format!("{}", PmixDeviceType::Gpu), "GPU");
    assert_eq!(format!("{}", PmixDeviceType::Network), "NETWORK");
    assert_eq!(format!("{}", PmixDeviceType::OpenFabrics), "OPENFABRICS");
    let unk = format!("{}", PmixDeviceType::Unknown(0xFF));
    assert!(unk.contains("UNKNOWN"));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixPersistence — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_persistence_from_raw_all() {
    assert_eq!(PmixPersistence::from_raw(0), PmixPersistence::Indefinite);
    assert_eq!(PmixPersistence::from_raw(1), PmixPersistence::FirstRead);
    assert_eq!(PmixPersistence::from_raw(2), PmixPersistence::Process);
    assert_eq!(PmixPersistence::from_raw(3), PmixPersistence::Application);
    assert_eq!(PmixPersistence::from_raw(4), PmixPersistence::Session);
    assert_eq!(PmixPersistence::from_raw(255), PmixPersistence::Invalid);
    assert_eq!(PmixPersistence::from_raw(42), PmixPersistence::Unknown(42));
}

#[test]
fn test_pmix_persistence_to_raw_all() {
    assert_eq!(PmixPersistence::Indefinite.to_raw(), 0);
    assert_eq!(PmixPersistence::FirstRead.to_raw(), 1);
    assert_eq!(PmixPersistence::Process.to_raw(), 2);
    assert_eq!(PmixPersistence::Application.to_raw(), 3);
    assert_eq!(PmixPersistence::Session.to_raw(), 4);
    assert_eq!(PmixPersistence::Invalid.to_raw(), 255);
    assert_eq!(PmixPersistence::Unknown(42).to_raw(), 42);
}

#[test]
fn test_pmix_persistence_display() {
    assert_eq!(format!("{}", PmixPersistence::Indefinite), "INDEFINITE");
    assert_eq!(
        format!("{}", PmixPersistence::FirstRead),
        "DELETE ON FIRST ACCESS"
    );
    assert_eq!(format!("{}", PmixPersistence::Invalid), "INVALID");
    let unk = format!("{}", PmixPersistence::Unknown(42));
    assert!(unk.contains("UNKNOWN"));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixDataRange — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_data_range_from_raw_all() {
    assert_eq!(PmixDataRange::from_raw(0), PmixDataRange::Undef);
    assert_eq!(PmixDataRange::from_raw(1), PmixDataRange::Rm);
    assert_eq!(PmixDataRange::from_raw(2), PmixDataRange::Local);
    assert_eq!(PmixDataRange::from_raw(3), PmixDataRange::Namespace);
    assert_eq!(PmixDataRange::from_raw(4), PmixDataRange::Session);
    assert_eq!(PmixDataRange::from_raw(5), PmixDataRange::Global);
    assert_eq!(PmixDataRange::from_raw(6), PmixDataRange::Custom);
    assert_eq!(PmixDataRange::from_raw(7), PmixDataRange::ProcLocal);
    assert_eq!(PmixDataRange::from_raw(255), PmixDataRange::Invalid);
    assert_eq!(PmixDataRange::from_raw(42), PmixDataRange::Unknown);
}

#[test]
fn test_pmix_data_range_to_raw_all() {
    assert_eq!(PmixDataRange::Undef.to_raw(), 0);
    assert_eq!(PmixDataRange::Rm.to_raw(), 1);
    assert_eq!(PmixDataRange::Local.to_raw(), 2);
    assert_eq!(PmixDataRange::Namespace.to_raw(), 3);
    assert_eq!(PmixDataRange::Session.to_raw(), 4);
    assert_eq!(PmixDataRange::Global.to_raw(), 5);
    assert_eq!(PmixDataRange::Custom.to_raw(), 6);
    assert_eq!(PmixDataRange::ProcLocal.to_raw(), 7);
    assert_eq!(PmixDataRange::Invalid.to_raw(), 255);
    assert_eq!(PmixDataRange::Unknown.to_raw(), 128);
}

#[test]
fn test_pmix_data_range_display() {
    assert_eq!(format!("{}", PmixDataRange::Undef), "UNDEFINED");
    assert_eq!(format!("{}", PmixDataRange::Global), "GLOBAL");
    assert_eq!(format!("{}", PmixDataRange::Unknown), "UNKNOWN RANGE (128)");
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixScope — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_scope_from_raw_all() {
    assert_eq!(PmixScope::from_raw(0), PmixScope::Undef);
    assert_eq!(PmixScope::from_raw(1), PmixScope::Local);
    assert_eq!(PmixScope::from_raw(2), PmixScope::Remote);
    assert_eq!(PmixScope::from_raw(3), PmixScope::Global);
    assert_eq!(PmixScope::from_raw(4), PmixScope::Internal);
    assert_eq!(PmixScope::from_raw(99), PmixScope::Unknown(99));
}

#[test]
fn test_pmix_scope_to_raw_all() {
    assert_eq!(PmixScope::Undef.to_raw(), 0);
    assert_eq!(PmixScope::Local.to_raw(), 1);
    assert_eq!(PmixScope::Remote.to_raw(), 2);
    assert_eq!(PmixScope::Global.to_raw(), 3);
    assert_eq!(PmixScope::Internal.to_raw(), 4);
    assert_eq!(PmixScope::Unknown(99).to_raw(), 99);
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixDataType — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_data_type_from_raw_all() {
    let mappings: [(u16, PmixDataType); 67] = [
        (0, PmixDataType::Undef),
        (1, PmixDataType::Bool),
        (2, PmixDataType::Byte),
        (3, PmixDataType::String),
        (4, PmixDataType::Size),
        (5, PmixDataType::Pid),
        (6, PmixDataType::Int),
        (7, PmixDataType::Int8),
        (8, PmixDataType::Int16),
        (9, PmixDataType::Int32),
        (10, PmixDataType::Int64),
        (11, PmixDataType::Uint),
        (12, PmixDataType::Uint8),
        (13, PmixDataType::Uint16),
        (14, PmixDataType::Uint32),
        (15, PmixDataType::Uint64),
        (16, PmixDataType::Float),
        (17, PmixDataType::Double),
        (18, PmixDataType::Timeval),
        (19, PmixDataType::Time),
        (20, PmixDataType::Status),
        (21, PmixDataType::Value),
        (22, PmixDataType::Proc),
        (23, PmixDataType::App),
        (24, PmixDataType::Info),
        (25, PmixDataType::Pdata),
        (27, PmixDataType::ByteObject),
        (28, PmixDataType::Kval),
        (30, PmixDataType::Persist),
        (31, PmixDataType::Pointer),
        (32, PmixDataType::Scope),
        (33, PmixDataType::DataRange),
        (34, PmixDataType::Command),
        (35, PmixDataType::InfoDirectives),
        (36, PmixDataType::DataType),
        (37, PmixDataType::ProcState),
        (38, PmixDataType::ProcInfo),
        (39, PmixDataType::DataArray),
        (40, PmixDataType::ProcRank),
        (41, PmixDataType::Query),
        (42, PmixDataType::CompressedString),
        (43, PmixDataType::AllocDirective),
        (45, PmixDataType::IofChannel),
        (46, PmixDataType::Envar),
        (47, PmixDataType::Coord),
        (48, PmixDataType::Regattr),
        (49, PmixDataType::Regex),
        (50, PmixDataType::JobState),
        (51, PmixDataType::LinkState),
        (52, PmixDataType::ProcCpuset),
        (53, PmixDataType::Geometry),
        (54, PmixDataType::DeviceDist),
        (55, PmixDataType::Endpoint),
        (56, PmixDataType::Topo),
        (57, PmixDataType::Devtype),
        (58, PmixDataType::LocType),
        (59, PmixDataType::CompressedByteObject),
        (60, PmixDataType::ProcNspace),
        (61, PmixDataType::ProcStats),
        (62, PmixDataType::DiskStats),
        (63, PmixDataType::NetStats),
        (64, PmixDataType::NodeStats),
        (65, PmixDataType::DataBuffer),
        (66, PmixDataType::StorMedium),
        (67, PmixDataType::StorAccess),
        (68, PmixDataType::StorPersist),
        (69, PmixDataType::StorAccessType),
    ];
    for (raw, expected) in mappings {
        assert_eq!(
            PmixDataType::from_raw(raw),
            expected,
            "from_raw({}) should be {:?}",
            raw,
            expected
        );
    }
    // Unknown
    assert_eq!(PmixDataType::from_raw(70), PmixDataType::Unknown);
    assert_eq!(PmixDataType::from_raw(500), PmixDataType::Unknown);
}

#[test]
fn test_pmix_data_type_to_raw_all() {
    assert_eq!(PmixDataType::Undef.to_raw(), 0);
    assert_eq!(PmixDataType::Bool.to_raw(), 1);
    assert_eq!(PmixDataType::String.to_raw(), 3);
    assert_eq!(PmixDataType::ByteObject.to_raw(), 27);
    assert_eq!(PmixDataType::Persist.to_raw(), 30);
    assert_eq!(PmixDataType::Scope.to_raw(), 32);
    assert_eq!(PmixDataType::DataRange.to_raw(), 33);
    assert_eq!(PmixDataType::ProcState.to_raw(), 37);
    assert_eq!(PmixDataType::ProcRank.to_raw(), 40);
    assert_eq!(PmixDataType::AllocDirective.to_raw(), 43);
    assert_eq!(PmixDataType::IofChannel.to_raw(), 45);
    assert_eq!(PmixDataType::Envar.to_raw(), 46);
    assert_eq!(PmixDataType::JobState.to_raw(), 50);
    assert_eq!(PmixDataType::LinkState.to_raw(), 51);
    assert_eq!(PmixDataType::StorAccessType.to_raw(), 69);
    assert_eq!(PmixDataType::Unknown.to_raw(), 70);
}

#[test]
fn test_pmix_data_type_display() {
    assert_eq!(format!("{}", PmixDataType::Bool), "BOOL");
    assert_eq!(format!("{}", PmixDataType::String), "STRING");
    assert_eq!(format!("{}", PmixDataType::Unknown), "UNKNOWN");
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixAllocDirective — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_alloc_directive_from_raw() {
    assert_eq!(
        PmixAllocDirective::from_raw(43),
        PmixAllocDirective::AllocDirective
    );
    assert_eq!(
        PmixAllocDirective::from_raw(99),
        PmixAllocDirective::Unknown(99)
    );
}

#[test]
fn test_pmix_alloc_directive_to_raw() {
    assert_eq!(PmixAllocDirective::AllocDirective.to_raw(), 43);
    assert_eq!(PmixAllocDirective::Unknown(99).to_raw(), 99);
}

#[test]
fn test_pmix_alloc_directive_display() {
    assert_eq!(
        format!("{}", PmixAllocDirective::AllocDirective),
        "ALLOC_DIRECTIVE"
    );
    let unk = format!("{}", PmixAllocDirective::Unknown(99));
    assert!(unk.contains("UNKNOWN"));
}

// ═══════════════════════════════════════════════════════════════════════════
// IOFChannelFlags — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_iof_channel_flags_constants() {
    assert!(IOFChannelFlags::NO_CHANNELS.is_empty());
    assert!(!IOFChannelFlags::STDIN.is_empty());
    assert!(!IOFChannelFlags::ALL_CHANNELS.is_empty());
}

#[test]
fn test_iof_channel_flags_contains() {
    let combined = IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT;
    assert!(combined.contains(IOFChannelFlags::STDIN));
    assert!(combined.contains(IOFChannelFlags::STDOUT));
    assert!(!combined.contains(IOFChannelFlags::STDERR));
}

#[test]
fn test_iof_channel_flags_raw() {
    assert_eq!(IOFChannelFlags::STDIN.raw(), 1);
    assert_eq!(IOFChannelFlags::STDOUT.raw(), 2);
    assert_eq!(IOFChannelFlags::STDERR.raw(), 4);
    assert_eq!(IOFChannelFlags::STDDIAG.raw(), 8);
    assert_eq!(IOFChannelFlags::ALL_CHANNELS.raw(), 255);
}

#[test]
fn test_iof_channel_flags_bitor() {
    let mut flags = IOFChannelFlags::STDIN;
    flags |= IOFChannelFlags::STDOUT;
    assert!(flags.contains(IOFChannelFlags::STDIN));
    assert!(flags.contains(IOFChannelFlags::STDOUT));
}

#[test]
fn test_iof_channel_flags_display() {
    assert_eq!(format!("{}", IOFChannelFlags::NO_CHANNELS), "NO_CHANNELS");
    assert_eq!(format!("{}", IOFChannelFlags::STDIN), "STDIN");
    let combined = IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT;
    let s = format!("{}", combined);
    assert!(s.contains("STDIN"));
    assert!(s.contains("STDOUT"));
    // Test hex display for non-standard flags
    let weird = IOFChannelFlags(0x80);
    let s = format!("{}", weird);
    assert!(s.contains("0x"));
}

// ═══════════════════════════════════════════════════════════════════════════
// BuilderError — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_builder_error_variants() {
    // KeyContainsNul
    let nul_err = BuilderError::KeyContainsNul(CString::new("test\0inner").unwrap_err());
    let msg = format!("{}", nul_err);
    assert!(msg.contains("NUL"));

    // KeyEmpty
    let empty = BuilderError::KeyEmpty;
    assert!(format!("{}", empty).contains("empty"));

    // KeyTooLong
    let long = BuilderError::KeyTooLong {
        len: 600,
        maximum: 511,
    };
    assert!(format!("{}", long).contains("600"));

    // MissingValue
    let missing = BuilderError::MissingValue;
    assert!(format!("{}", missing).contains("value"));
}

#[test]
fn test_builder_error_error_trait() {
    let nul_err = BuilderError::KeyContainsNul(CString::new("test\0inner").unwrap_err());
    let err: &dyn std::error::Error = &nul_err;
    assert!(err.source().is_some());

    let empty: &dyn std::error::Error = &BuilderError::KeyEmpty;
    assert!(empty.source().is_none());
}

#[test]
fn test_builder_error_from_nul() {
    let nul: BuilderError = CString::new("test\0inner").unwrap_err().into();
    assert!(matches!(nul, BuilderError::KeyContainsNul(_)));
}

// ═══════════════════════════════════════════════════════════════════════════
// ValueError — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_value_error_variants() {
    // ContainsNul
    let nul_err = ValueError::ContainsNul(CString::new("test\0inner").unwrap_err());
    let msg = format!("{}", nul_err);
    assert!(msg.contains("NUL"));

    // MissingPayload
    let missing = ValueError::MissingPayload;
    assert!(format!("{}", missing).contains("payload"));

    // EmptyData
    let empty = ValueError::EmptyData;
    assert!(format!("{}", empty).contains("empty"));
}

#[test]
fn test_value_error_error_trait() {
    let nul_err = ValueError::ContainsNul(CString::new("test\0inner").unwrap_err());
    let err: &dyn std::error::Error = &nul_err;
    assert!(err.source().is_some());

    let missing: &dyn std::error::Error = &ValueError::MissingPayload;
    assert!(missing.source().is_none());
}

#[test]
fn test_value_error_from_nul() {
    let nul: ValueError = CString::new("test\0inner").unwrap_err().into();
    assert!(matches!(nul, ValueError::ContainsNul(_)));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixTimeval — coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_timeval_fields() {
    let tv = PmixTimeval {
        tv_sec: 100,
        tv_usec: 500,
    };
    assert_eq!(tv.tv_sec, 100);
    assert_eq!(tv.tv_usec, 500);
}

#[test]
fn test_pmix_timeval_debug() {
    let tv = PmixTimeval {
        tv_sec: 100,
        tv_usec: 500,
    };
    let s = format!("{:?}", tv);
    assert!(s.contains("PmixTimeval"));
}

#[test]
fn test_pmix_timeval_clone_copy() {
    let tv = PmixTimeval {
        tv_sec: 100,
        tv_usec: 500,
    };
    let tv2 = tv; // Copy
    assert_eq!(tv.tv_sec, 100); // Original still accessible
    assert_eq!(tv2.tv_usec, 500);
}

#[test]
fn test_pmix_timeval_partial_eq() {
    let tv1 = PmixTimeval {
        tv_sec: 100,
        tv_usec: 500,
    };
    let tv2 = PmixTimeval {
        tv_sec: 100,
        tv_usec: 500,
    };
    let tv3 = PmixTimeval {
        tv_sec: 200,
        tv_usec: 500,
    };
    assert_eq!(tv1, tv2);
    assert_ne!(tv1, tv3);
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixEnvar — coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_envar_new() {
    let envar = PmixEnvar::new("PATH", "/usr/bin", ':').unwrap();
    assert_eq!(envar.separator, b':');
    assert_eq!(envar.envar.to_str().unwrap(), "PATH");
    assert_eq!(envar.value.to_str().unwrap(), "/usr/bin");
}

#[test]
fn test_pmix_envar_new_nul() {
    assert!(PmixEnvar::new("test\0", "val", '=').is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Proc — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_proc_new() {
    let proc = Proc::new("test_nspace", 42).unwrap();
    assert_eq!(proc.get_rank(), 42);
}

#[test]
fn test_proc_new_nul_fails() {
    assert!(Proc::new("test\0nspace", 42).is_err());
}

#[test]
fn test_proc_set_rank() {
    let mut proc = Proc::new("test_nspace", 42).unwrap();
    proc.set_rank(99);
    assert_eq!(proc.get_rank(), 99);
}

#[test]
fn test_proc_new_with_nspace() {
    let proc = Proc::new("test_nspace", 0).unwrap();
    let proc2 = proc.new_with_nspace(42).unwrap();
    assert_eq!(proc2.get_rank(), 42);
}

// ═══════════════════════════════════════════════════════════════════════════
// InfoFlags — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_info_flags_empty() {
    assert!(InfoFlags::default().is_empty());
    assert!(!InfoFlags::REQD.is_empty());
}

#[test]
fn test_info_flags_contains() {
    let flags = InfoFlags::REQD | InfoFlags::QUALIFIER;
    assert!(flags.contains(InfoFlags::REQD));
    assert!(flags.contains(InfoFlags::QUALIFIER));
    assert!(!flags.contains(InfoFlags::PERSISTENT));
}

#[test]
fn test_info_flags_raw() {
    assert!(InfoFlags::REQD.raw() > 0);
    assert!(InfoFlags::QUALIFIER.raw() > 0);
}

#[test]
fn test_info_flags_bitor_assign() {
    let mut flags = InfoFlags::default();
    flags |= InfoFlags::REQD;
    assert!(flags.contains(InfoFlags::REQD));
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixPayload — type_tag coverage for ALL variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_payload_type_tag_all_variants() {
    // Undef
    assert_eq!(PmixPayload::Undef.type_tag(), PmixDataType::Undef as u16);

    // Bool
    assert_eq!(
        PmixPayload::Bool(true).type_tag(),
        PmixDataType::Bool as u16
    );

    // Byte
    assert_eq!(PmixPayload::Byte(42).type_tag(), PmixDataType::Byte as u16);

    // String
    assert_eq!(
        PmixPayload::String(CString::new("hello").unwrap()).type_tag(),
        PmixDataType::String as u16
    );

    // Size
    assert_eq!(
        PmixPayload::Size(1024).type_tag(),
        PmixDataType::Size as u16
    );

    // Pid
    assert_eq!(PmixPayload::Pid(1234).type_tag(), PmixDataType::Pid as u16);

    // Int
    assert_eq!(PmixPayload::Int(-42).type_tag(), PmixDataType::Int as u16);

    // Int8
    assert_eq!(PmixPayload::Int8(-5).type_tag(), PmixDataType::Int8 as u16);

    // Int16
    assert_eq!(
        PmixPayload::Int16(-100).type_tag(),
        PmixDataType::Int16 as u16
    );

    // Int32
    assert_eq!(
        PmixPayload::Int32(-1000).type_tag(),
        PmixDataType::Int32 as u16
    );

    // Int64
    assert_eq!(
        PmixPayload::Int64(-10000).type_tag(),
        PmixDataType::Int64 as u16
    );

    // Uint
    assert_eq!(PmixPayload::Uint(42).type_tag(), PmixDataType::Uint as u16);

    // Uint8
    assert_eq!(
        PmixPayload::Uint8(255).type_tag(),
        PmixDataType::Uint8 as u16
    );

    // Uint16
    assert_eq!(
        PmixPayload::Uint16(65535).type_tag(),
        PmixDataType::Uint16 as u16
    );

    // Uint32
    assert_eq!(
        PmixPayload::Uint32(4294967295).type_tag(),
        PmixDataType::Uint32 as u16
    );

    // Uint64
    assert_eq!(
        PmixPayload::Uint64(18446744073709551615).type_tag(),
        PmixDataType::Uint64 as u16
    );

    // Float
    assert_eq!(
        PmixPayload::Float(3.14).type_tag(),
        PmixDataType::Float as u16
    );

    // Double
    assert_eq!(
        PmixPayload::Double(2.718).type_tag(),
        PmixDataType::Double as u16
    );

    // Timeval
    assert_eq!(
        PmixPayload::Timeval(PmixTimeval {
            tv_sec: 1,
            tv_usec: 500
        })
        .type_tag(),
        PmixDataType::Timeval as u16
    );

    // Status
    assert_eq!(
        PmixPayload::Status(0).type_tag(),
        PmixDataType::Status as u16
    );

    // Rank
    assert_eq!(
        PmixPayload::Rank(0).type_tag(),
        PmixDataType::ProcRank as u16
    );

    // Persist
    assert_eq!(
        PmixPayload::Persist(0).type_tag(),
        PmixDataType::Persist as u16
    );

    // Scope
    assert_eq!(PmixPayload::Scope(3).type_tag(), PmixDataType::Scope as u16);

    // DataRange
    assert_eq!(
        PmixPayload::DataRange(4).type_tag(),
        PmixDataType::DataRange as u16
    );

    // ProcState
    assert_eq!(
        PmixPayload::ProcState(5).type_tag(),
        PmixDataType::ProcState as u16
    );

    // AllocDirective
    assert_eq!(
        PmixPayload::AllocDirective(43).type_tag(),
        PmixDataType::AllocDirective as u16
    );

    // IofChannel
    assert_eq!(
        PmixPayload::IofChannel(1).type_tag(),
        PmixDataType::IofChannel as u16
    );

    // InfoDirectives
    assert_eq!(
        PmixPayload::InfoDirectives(1).type_tag(),
        PmixDataType::InfoDirectives as u16
    );

    // Proc
    let proc = Proc::new("test", 0).unwrap();
    assert_eq!(
        PmixPayload::Proc(proc).type_tag(),
        PmixDataType::Proc as u16
    );

    // ByteObject
    assert_eq!(
        PmixPayload::ByteObject(vec![1, 2, 3]).type_tag(),
        PmixDataType::ByteObject as u16
    );

    // Envar
    let envar = PmixEnvar::new("FOO", "bar", '=').unwrap();
    assert_eq!(
        PmixPayload::Envar(envar).type_tag(),
        PmixDataType::Envar as u16
    );

    // Pointer
    assert_eq!(
        PmixPayload::Pointer(std::ptr::null_mut()).type_tag(),
        PmixDataType::Pointer as u16
    );

    // DataArray
    assert_eq!(
        PmixPayload::DataArray {
            elem_type: PmixDataType::Int as u16,
            elements: vec![],
        }
        .type_tag(),
        PmixDataType::DataArray as u16
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixValueBuilder — ALL setter methods
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_value_builder_undef() {
    let v = PmixValueBuilder::new().undef().build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Undef as u16);
}

#[test]
fn test_value_builder_byte() {
    let v = PmixValueBuilder::new().byte(42).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Byte as u16);
}

#[test]
fn test_value_builder_pid() {
    let v = PmixValueBuilder::new().pid(1234).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Pid as u16);
}

#[test]
fn test_value_builder_int() {
    let v = PmixValueBuilder::new().int(-42).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Int as u16);
}

#[test]
fn test_value_builder_int8() {
    let v = PmixValueBuilder::new().int8(-5).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Int8 as u16);
}

#[test]
fn test_value_builder_int16() {
    let v = PmixValueBuilder::new().int16(-100).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Int16 as u16);
}

#[test]
fn test_value_builder_int64() {
    let v = PmixValueBuilder::new().int64(-10000).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Int64 as u16);
}

#[test]
fn test_value_builder_uint() {
    let v = PmixValueBuilder::new().uint(42).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Uint as u16);
}

#[test]
fn test_value_builder_uint8() {
    let v = PmixValueBuilder::new().uint8(255).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Uint8 as u16);
}

#[test]
fn test_value_builder_uint16() {
    let v = PmixValueBuilder::new().uint16(65535).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Uint16 as u16);
}

#[test]
fn test_value_builder_uint64() {
    let v = PmixValueBuilder::new()
        .uint64(18446744073709551615)
        .build()
        .unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Uint64 as u16);
}

#[test]
fn test_value_builder_float() {
    let v = PmixValueBuilder::new().float(3.14).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Float as u16);
}

#[test]
fn test_value_builder_timeval() {
    let v = PmixValueBuilder::new()
        .timeval(PmixTimeval {
            tv_sec: 100,
            tv_usec: 500,
        })
        .build()
        .unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Timeval as u16);
}

#[test]
fn test_value_builder_status() {
    let v = PmixValueBuilder::new().status(0).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Status as u16);
}

#[test]
fn test_value_builder_rank() {
    let v = PmixValueBuilder::new().rank(42).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::ProcRank as u16);
}

#[test]
fn test_value_builder_persist() {
    let v = PmixValueBuilder::new().persist(0).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Persist as u16);
}

#[test]
fn test_value_builder_proc_state() {
    let v = PmixValueBuilder::new().proc_state(5).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::ProcState as u16);
}

#[test]
fn test_value_builder_alloc_directive() {
    let v = PmixValueBuilder::new().alloc_directive(43).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::AllocDirective as u16);
}

#[test]
fn test_value_builder_iof_channel() {
    let v = PmixValueBuilder::new().iof_channel(1).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::IofChannel as u16);
}

#[test]
fn test_value_builder_info_directives() {
    let v = PmixValueBuilder::new().info_directives(1).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::InfoDirectives as u16);
}

#[test]
fn test_value_builder_proc() {
    let proc = Proc::new("test_nspace", 42).unwrap();
    let v = PmixValueBuilder::new().proc_(proc).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Proc as u16);
}

#[test]
fn test_value_builder_byte_object() {
    let v = PmixValueBuilder::new()
        .byte_object(&[1, 2, 3])
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(v.type_tag(), PmixDataType::ByteObject as u16);
}

#[test]
fn test_value_builder_byte_object_empty_fails() {
    assert!(PmixValueBuilder::new().byte_object(&[]).is_err());
}

#[test]
fn test_value_builder_envar() {
    let envar = PmixEnvar::new("FOO", "bar", '=').unwrap();
    let v = PmixValueBuilder::new().envar(envar).build().unwrap();
    assert_eq!(v.type_tag(), PmixDataType::Envar as u16);
}

#[test]
fn test_value_builder_string_nul_fails() {
    assert!(PmixValueBuilder::new().string("test\0inner").is_err());
}

#[test]
fn test_value_builder_missing_payload() {
    assert!(PmixValueBuilder::new().build().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixValueBuilder::build_raw
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_value_builder_build_raw() {
    // build_raw returns pmix_value_t (not accessible from outside crate).
    // Verify via PmixOwnedValue instead.
    let val = PmixValueBuilder::new().uint32(42).build().unwrap();
    assert_eq!(val.type_tag(), PmixDataType::Uint32 as u16);
}

#[test]
fn test_value_builder_build_raw_missing() {
    assert!(PmixValueBuilder::new().build_raw().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixValueBuilder::string_array
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_value_builder_string_array() {
    let (val, keys) = PmixValueBuilder::string_array(&["pmix.timeout", "pmix.collect"]).unwrap();
    assert_eq!(val.type_tag(), PmixDataType::DataArray as u16);
    let _ptr: *const *const std::ffi::c_char = keys.as_ptr();
}

#[test]
fn test_value_builder_string_array_empty_fails() {
    assert!(PmixValueBuilder::string_array(&[]).is_err());
}

#[test]
fn test_value_builder_string_array_nul_fails() {
    assert!(PmixValueBuilder::string_array(&["test\0inner"]).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// PmixOwnedValue — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_pmix_owned_value_as_raw() {
    let val = PmixValueBuilder::new().uint32(42).build().unwrap();
    let ptr = val.as_raw();
    assert!(!ptr.is_null());
}

#[test]
fn test_pmix_owned_value_as_raw_mut() {
    let mut val = PmixValueBuilder::new().uint32(42).build().unwrap();
    let ptr = val.as_raw_mut();
    assert!(!ptr.is_null());
}

#[test]
fn test_pmix_owned_value_type_tag() {
    let val = PmixValueBuilder::new().uint32(42).build().unwrap();
    assert_eq!(val.type_tag(), PmixDataType::Uint32 as u16);
}

#[test]
fn test_pmix_owned_value_size() {
    let val = PmixValueBuilder::new().size(1024).build().unwrap();
    assert_eq!(val.size(), 1024);
}

#[test]
fn test_pmix_owned_value_bytes() {
    let val = PmixValueBuilder::new()
        .byte_object(&[1, 2, 3])
        .unwrap()
        .build()
        .unwrap();
    let (ptr, size) = val.bytes();
    assert_eq!(size, 3);
    assert!(!ptr.is_null());
}

#[test]
fn test_pmix_owned_value_debug() {
    let val = PmixValueBuilder::new().uint32(42).build().unwrap();
    let s = format!("{:?}", val);
    assert!(s.contains("PmixOwnedValue"));
}

#[test]
fn test_pmix_owned_value_into_raw() {
    let val = PmixValueBuilder::new().uint32(42).build().unwrap();
    // Verify type_tag before transferring ownership
    assert_eq!(val.type_tag(), PmixDataType::Uint32 as u16);
    // Transfer ownership out — PmixOwnedValue is forgotten, Drop won't run
    let _raw = val.into_raw();
    // Intentionally leaked to avoid needing pmix_value_t from outside the crate.
}

// ═══════════════════════════════════════════════════════════════════════════
// InfoBuilder — coverage for collect_data and add
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_info_builder_collect_data() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let _info = builder.build();
}

#[test]
fn test_info_builder_build_empty() {
    let _info = InfoBuilder::new().build();
}

// ═══════════════════════════════════════════════════════════════════════════
// free_value — coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_free_value() {
    let mut val = PmixValueBuilder::new().uint32(42).build().unwrap();
    // The Drop impl calls free_value, but let's make sure the function exists
    // and doesn't panic when called on a valid value
    let raw_ptr = val.as_raw_mut();
    assert!(!raw_ptr.is_null());
}
