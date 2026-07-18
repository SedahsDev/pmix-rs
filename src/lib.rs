#![allow(unused_imports)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::ptr_offset_with_cast)]

use std::fmt::Debug;
pub mod allocation;
pub mod cpu_locality;
pub mod data_ops;
pub mod data_serialization;
pub mod events;
pub mod fabric;
#[allow(clippy::upper_case_acronyms, clippy::enum_variant_names)]
mod ffi;
pub mod groups;
pub mod info;
pub mod mock_ffi;
pub mod monitoring;
pub mod process_mgmt;
pub mod query_log;
pub mod security;
pub mod server;
pub mod tool;
pub mod utility;

use crate::ffi::*;
use cstring_array::CStringArray;
use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::{c_char, c_void};
use std::ptr::{null, null_mut};
use std::{fmt, mem, ptr};

pub const GLOBAL: u8 = PMIX_GLOBAL as u8;
pub const NUM_NODES: &[u8; 15] = PMIX_NUM_NODES;
pub const JOB_SIZE: &[u8; 14] = PMIX_JOB_SIZE;
pub const RANK_WILDCARD: u32 = PMIX_RANK_WILDCARD;

// ─────────────────────────────────────────────────────────────────────────────
// PmixError enum
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_status_t` (PMIx 5.0).
///
/// `pmix_status_t` is an `int32_t` where:
/// * `0`            → [`Success`][PmixError::Success]
/// * positive       → informational / event codes
/// * negative       → error codes, grouped by subsystem
/// * `< −9999`      → user/implementation-defined
///
/// All values defined in `pmix_common.h §4.3` are represented.  Unknown raw
/// values produced by future library versions (or by user extensions) are
/// captured in the [`Unknown`][PmixError::Unknown] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum PmixError {
    // ── ❶  Success & informational (0 and positive) ──────────────────────────
    /// `PMIX_SUCCESS` (0)
    Success = 0,

    // ── ❷  Widely-used base codes ────────────────────────────────────────────
    /// `PMIX_ERROR` (−1) — generic unspecified error.
    Error = -1,

    /// `PMIX_DEBUGGER_RELEASE` (−3) — replaces the deprecated
    /// `PMIX_ERR_DEBUGGER_RELEASE`; the debugger has released a stopped process.
    DebuggerRelease = -3,

    /// `PMIX_ERR_PROC_RESTART` (−4)
    ErrProcRestart = -4,

    /// `PMIX_ERR_PROC_CHECKPOINT` (−5)
    ErrProcCheckpoint = -5,

    /// `PMIX_ERR_PROC_MIGRATE` (−6)
    ErrProcMigrate = -6,

    /// `PMIX_ERR_PROC_REQUESTED_ABORT` (−8) — a process called `PMIx_Abort`.
    ErrProcRequestedAbort = -8,

    /// `PMIX_ERR_EXISTS` (−11) — the key or object already exists.
    ErrExists = -11,

    /// `PMIX_ERR_INVALID_CRED` (−12) — invalid or unverifiable security credential.
    ErrInvalidCred = -12,

    /// `PMIX_ERR_WOULD_BLOCK` (−15) — call would block; returned only when
    /// non-blocking behaviour was requested.
    ErrWouldBlock = -15,

    /// `PMIX_ERR_UNKNOWN_DATA_TYPE` (−16) — `pmix_data_type_t` discriminant
    /// is not recognised.
    ErrUnknownDataType = -16,

    /// `PMIX_ERR_TYPE_MISMATCH` (−18) — stored and requested types differ.
    ErrTypeMismatch = -18,

    /// `PMIX_ERR_UNPACK_INADEQUATE_SPACE` (−19)
    ErrUnpackInadequateSpace = -19,

    /// `PMIX_ERR_UNPACK_FAILURE` (−20)
    ErrUnpackFailure = -20,

    /// `PMIX_ERR_PACK_FAILURE` (−21)
    ErrPackFailure = -21,

    /// `PMIX_ERR_NO_PERMISSIONS` (−23) — caller lacks required credentials.
    ErrNoPermissions = -23,

    /// `PMIX_ERR_TIMEOUT` (−24) — operation exceeded `PMIX_TIMEOUT`.
    ErrTimeout = -24,

    /// `PMIX_ERR_UNREACH` (−25) — target process or server is unreachable.
    ErrUnreach = -25,

    /// `PMIX_ERR_BAD_PARAM` (−27) — parameter out of range or inconsistent.
    ErrBadParam = -27,

    /// `PMIX_ERR_RESOURCE_BUSY` (−28) — requested resource is in use.
    ErrResourceBusy = -28,

    /// `PMIX_ERR_OUT_OF_RESOURCE` (−29) — a system resource was exhausted.
    ErrOutOfResource = -29,

    /// `PMIX_ERR_INIT` (−31) — PMIx was not initialised, or init failed.
    ErrInit = -31,

    /// `PMIX_ERR_NOMEM` (−32) — memory allocation failed.
    ErrNomem = -32,

    // ── ❸  Data / lookup errors ──────────────────────────────────────────────
    /// `PMIX_ERR_NOT_FOUND` (−46) — the requested data item does not exist.
    ErrNotFound = -46,

    /// `PMIX_ERR_NOT_SUPPORTED` (−47) — API or attribute not supported here.
    ErrNotSupported = -47,

    /// `PMIX_ERR_COMM_FAILURE` (−49) — general communication failure.
    ErrCommFailure = -49,

    /// `PMIX_ERR_UNPACK_READ_PAST_END_OF_BUFFER` (−50)
    ErrUnpackReadPastEndOfBuffer = -50,

    /// `PMIX_ERR_CONFLICTING_CLEANUP_DIRECTIVES` (−51) — two cleanup
    /// directives for the same path conflict.
    ErrConflictingCleanupDirectives = -51,

    /// `PMIX_ERR_PARTIAL_SUCCESS` (−52) — succeeded for some but not all targets.
    ErrPartialSuccess = -52,

    /// `PMIX_ERR_DUPLICATE_KEY` (−53) — key already exists in scope.
    ErrDuplicateKey = -53,

    /// `PMIX_READY_FOR_DEBUG` (−58) — process reached the breakpoint and
    /// is waiting for a debugger (accompanied by `PMIX_BREAKPOINT`).
    ReadyForDebug = -58,

    /// `PMIX_ERR_PARAM_VALUE_NOT_SUPPORTED` (−59) — parameter value not
    /// supported by this implementation.
    ErrParamValueNotSupported = -59,

    /// `PMIX_ERR_EMPTY` (−60) — container or collection is empty.
    ErrEmpty = -60,

    /// `PMIX_ERR_LOST_CONNECTION` (−61) — established connection was lost.
    ErrLostConnection = -61,

    /// `PMIX_ERR_EXISTS_OUTSIDE_SCOPE` (−62) — key exists but was published
    /// outside the caller's accessible scope.
    ErrExistsOutsideScope = -62,

    // ── ❹  Job-control event codes ───────────────────────────────────────────
    /// `PMIX_JCTRL_CHECKPOINT` (−106) — trigger a checkpoint.
    JctrlCheckpoint = -106,

    /// `PMIX_JCTRL_CHECKPOINT_COMPLETE` (−107) — checkpoint finished.
    JctrlCheckpointComplete = -107,

    /// `PMIX_JCTRL_PREEMPT_ALERT` (−108) — scheduler will preempt this job.
    JctrlPreemptAlert = -108,

    // ── ❺  Monitoring alert codes ────────────────────────────────────────────
    /// `PMIX_MONITOR_HEARTBEAT_ALERT` (−109) — heartbeat missed.
    MonitorHeartbeatAlert = -109,

    /// `PMIX_MONITOR_FILE_ALERT` (−110) — watched file changed unexpectedly.
    MonitorFileAlert = -110,

    // ── ❻  Fabric / network event codes ─────────────────────────────────────
    /// `PMIX_FABRIC_UPDATE_ENDPOINTS` (−113) — fabric endpoint info changed.
    FabricUpdateEndpoints = -113,

    // ── ❼  Internal / registration errors ───────────────────────────────────
    /// `PMIX_ERR_EVENT_REGISTRATION` (−144) — event handler registration failed.
    ErrEventRegistration = -144,

    // ── ❽  Job lifecycle event codes ─────────────────────────────────────────
    /// `PMIX_EVENT_JOB_END` (−145) — the job has ended.
    EventJobEnd = -145,

    // ── ❾  Operational-state codes ──────────────────────────────────────────
    //
    // These are NEGATIVE in the real header.
    /// `PMIX_OPERATION_IN_PROGRESS` (−156) — operation launched; result
    /// delivered via callback.
    OperationInProgress = -156,

    /// `PMIX_OPERATION_SUCCEEDED` (−157) — event handler signals the event
    /// was fully handled.
    OperationSucceeded = -157,

    /// `PMIX_ERR_INVALID_OPERATION` (−158) — operation is not valid in the
    /// current state.
    ErrInvalidOperation = -158,

    // ── ❿  Attribute / registration errors ──────────────────────────────────
    /// `PMIX_ERR_REPEAT_ATTR_REGISTRATION` (−171) — attribute registered more
    /// than once with conflicting parameters.
    ErrRepeatAttrRegistration = -171,

    // ── ⓫  I/O-forwarding codes ──────────────────────────────────────────────
    /// `PMIX_ERR_IOF_FAILURE` (−172) — general I/O-forwarding error.
    ErrIofFailure = -172,

    /// `PMIX_ERR_IOF_COMPLETE` (−173) — I/O-forwarding stream closed gracefully.
    ErrIofComplete = -173,

    // ── ⓬  Fabric status codes ───────────────────────────────────────────────
    /// `PMIX_FABRIC_UPDATED` (−175) — fabric topology has been updated.
    FabricUpdated = -175,

    /// `PMIX_FABRIC_UPDATE_PENDING` (−176) — fabric update is in progress.
    FabricUpdatePending = -176,

    // ── ⓭  Job-level error codes ─────────────────────────────────────────────
    /// `PMIX_ERR_JOB_APP_NOT_EXECUTABLE` (−177) — binary is not executable.
    ErrJobAppNotExecutable = -177,

    /// `PMIX_ERR_JOB_NO_EXE_SPECIFIED` (−178) — no executable in spawn request.
    ErrJobNoExeSpecified = -178,

    /// `PMIX_ERR_JOB_FAILED_TO_MAP` (−179) — RM could not map processes to nodes.
    ErrJobFailedToMap = -179,

    /// `PMIX_ERR_JOB_CANCELED` (−180) — job was cancelled.
    ErrJobCanceled = -180,

    /// `PMIX_ERR_JOB_FAILED_TO_LAUNCH` (−181) — spawn rejected before any
    /// process started.
    ErrJobFailedToLaunch = -181,

    /// `PMIX_ERR_JOB_ABORTED` (−182) — job aborted due to an error.
    ErrJobAborted = -182,

    /// `PMIX_ERR_JOB_KILLED_BY_CMD` (−183) — job killed by control command.
    ErrJobKilledByCmd = -183,

    /// `PMIX_ERR_JOB_ABORTED_BY_SIG` (−184) — job killed by unhandled signal.
    ErrJobAbortedBySig = -184,

    /// `PMIX_ERR_JOB_TERM_WO_SYNC` (−185) — job terminated without completing
    /// a required barrier / fence.
    ErrJobTermWoSync = -185,

    /// `PMIX_ERR_JOB_SENSOR_BOUND_EXCEEDED` (−186) — sensor threshold exceeded.
    ErrJobSensorBoundExceeded = -186,

    /// `PMIX_ERR_JOB_NON_ZERO_TERM` (−187) — job exited with non-zero code.
    ErrJobNonZeroTerm = -187,

    /// `PMIX_ERR_JOB_ALLOC_FAILED` (−188) — resource allocation for the job
    /// failed.
    ErrJobAllocFailed = -188,

    /// `PMIX_ERR_JOB_ABORTED_BY_SYS_EVENT` (−189) — job aborted due to an
    /// unrecoverable system event (e.g. node failure).
    ErrJobAbortedBySysEvent = -189,

    /// `PMIX_ERR_JOB_EXE_NOT_FOUND` (−190) — executable not found on exec node.
    ErrJobExeNotFound = -190,

    // ── ⓮  Job-lifecycle event codes ────────────────────────────────────────
    /// `PMIX_EVENT_JOB_START` (−191) — job has started.
    EventJobStart = -191,

    /// `PMIX_EVENT_SESSION_START` (−192) — new session has started.
    EventSessionStart = -192,

    /// `PMIX_EVENT_SESSION_END` (−193) — session has ended.
    EventSessionEnd = -193,

    // ── ⓯  Process-level error codes ────────────────────────────────────────
    /// `PMIX_ERR_PROC_TERM_WO_SYNC` (−200) — process exited without completing
    /// a required collective operation.
    ErrProcTermWoSync = -200,

    /// `PMIX_EVENT_PROC_TERMINATED` (−201) — a process has terminated.
    EventProcTerminated = -201,

    // ── ⓰  System-event codes ────────────────────────────────────────────────
    /// `PMIX_EVENT_SYS_BASE` (−230) — base sentinel for system events.
    EventSysBase = -230,

    /// `PMIX_EVENT_NODE_DOWN` (−231) — a node has gone down.
    EventNodeDown = -231,

    /// `PMIX_EVENT_NODE_OFFLINE` (−232) — a node has gone offline.
    EventNodeOffline = -232,

    // ── ⓱  Additional job-level errors ──────────────────────────────────────
    /// `PMIX_ERR_JOB_WDIR_NOT_FOUND` (−233) — working directory not found on
    /// exec node.
    ErrJobWdirNotFound = -233,

    /// `PMIX_ERR_JOB_INSUFFICIENT_RESOURCES` (−234) — not enough resources
    /// for the spawn request.
    ErrJobInsufficientResources = -234,

    /// `PMIX_ERR_JOB_SYS_OP_FAILED` (−235) — internal system operation needed
    /// for launch failed.
    ErrJobSysOpFailed = -235,

    // ── ⓲  System-event "other" range ───────────────────────────────────────
    /// `PMIX_EVENT_SYS_OTHER` (−330) — catch-all for undefined system events.
    EventSysOther = -330,

    // ── ⓳  Event-handler return codes ───────────────────────────────────────
    /// `PMIX_EVENT_NO_ACTION_TAKEN` (−331) — handler ran but took no action.
    EventNoActionTaken = -331,

    /// `PMIX_EVENT_PARTIAL_ACTION_TAKEN` (−332) — handler took partial action.
    EventPartialActionTaken = -332,

    /// `PMIX_EVENT_ACTION_DEFERRED` (−333) — handler queued actions for later.
    EventActionDeferred = -333,

    /// `PMIX_EVENT_ACTION_COMPLETE` (−334) — handler fully resolved the event.
    EventActionComplete = -334,

    // ── ⓴  Per-process error codes ──────────────────────────────────────────
    /// `PMIX_ERR_PROC_KILLED_BY_CMD` (−400) — process killed by control command.
    ErrProcKilledByCmd = -400,

    /// `PMIX_ERR_PROC_FAILED_TO_START` (−401) — spawned process never called
    /// `PMIx_Init`.
    ErrProcFailedToStart = -401,

    /// `PMIX_ERR_PROC_ABORTED_BY_SIG` (−402) — process killed by unhandled signal.
    ErrProcAbortedBySig = -402,

    /// `PMIX_ERR_PROC_SENSOR_BOUND_EXCEEDED` (−403) — per-process sensor
    /// threshold exceeded.
    ErrProcSensorBoundExceeded = -403,

    /// `PMIX_ERR_EXIT_NONZERO_TERM` (−404) — process exited with non-zero code.
    ErrExitNonzeroTerm = -404,

    // ── ㉑  External / user-defined boundary ────────────────────────────────
    /// `PMIX_EXTERNAL_ERR_BASE` (−3000) — all values **more negative** than
    /// this are reserved for user / implementation defined codes.
    ExternalErrBase = -3000,
    // ── ⓯  Unknown / user-defined fall-through ──────────────────────────────
    // (Not a repr(i32) discriminant — stored out-of-band via the newtype trick
    //  in `from_raw`. See implementation notes below.)
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus — the public-facing type that wraps PmixError + Unknown(i32)
// ─────────────────────────────────────────────────────────────────────────────

/// The complete value space of `pmix_status_t`.
///
/// `PmixError` covers every constant defined in the PMIx 5.0 standard.
/// `PmixStatus::Unknown` captures any raw value that does not correspond to a
/// known constant — typically a user/implementation-defined code below
/// `PMIX_EXTERNAL_ERR_BASE` (−9999).
///
/// Use [`PmixStatus::from_raw`] to convert a `pmix_status_t` received from C
/// and [`PmixStatus::to_raw`] to convert back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PmixStatus {
    /// A known, standard PMIx status code.
    Known(PmixError),
    /// An unrecognised or user-defined status code (value < −9999 or any
    /// future standard extension not yet reflected in this crate).
    Unknown(i32),
}

impl PmixStatus {
    /// Convert a raw `pmix_status_t` (`i32`) into a `PmixStatus`.
    ///
    /// ```
    /// use pmix::{PmixError, PmixStatus};
    ///
    /// assert_eq!(PmixStatus::from_raw(0),   PmixStatus::Known(PmixError::Success));
    /// assert_eq!(PmixStatus::from_raw(-1),  PmixStatus::Known(PmixError::Error));
    /// assert!(matches!(PmixStatus::from_raw(-99999), PmixStatus::Unknown(_)));
    /// ```
    pub fn from_raw(code: i32) -> Self {
        match PmixError::from_raw(code) {
            Some(e) => Self::Known(e),
            None => Self::Unknown(code),
        }
    }

    /// Return the raw `i32` value.
    pub fn to_raw(self) -> i32 {
        match self {
            Self::Known(e) => e as i32,
            Self::Unknown(v) => v,
        }
    }

    /// `true` for `PMIX_SUCCESS` and any positive informational code.
    pub fn is_success(self) -> bool {
        match self {
            Self::Known(e) => e.is_success(),
            Self::Unknown(v) => v > 0,
        }
    }

    /// `true` for all negative codes.
    pub fn is_error(self) -> bool {
        !self.is_success()
    }

    /// Return the inner `PmixError` if the code is known.
    pub fn known(self) -> Option<PmixError> {
        match self {
            Self::Known(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for PmixStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(e) => e.fmt(f),
            Self::Unknown(v) => write!(f, "pmix_status_t({v}) [unknown/user-defined]"),
        }
    }
}

impl std::error::Error for PmixStatus {}

impl From<PmixError> for PmixStatus {
    fn from(e: PmixError) -> Self {
        Self::Known(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixError implementation
// ─────────────────────────────────────────────────────────────────────────────

impl PmixError {
    /// Convert a raw `pmix_status_t` to a `PmixError`, or `None` if the value
    /// is not a known standard code.
    ///
    /// This is `O(1)` — it matches against a table of all known discriminants.
    pub fn from_raw(code: i32) -> Option<Self> {
        // SAFETY: We explicitly match against every discriminant so that no
        // unknown value ever passes through the transmute path.  The function
        // returns None for all values not present in the table.
        Some(match code {
            // ── Success / informational ──────────────────────────────────
            0 => Self::Success,
            -1 => Self::Error,
            -3 => Self::DebuggerRelease,
            -4 => Self::ErrProcRestart,
            -5 => Self::ErrProcCheckpoint,
            -6 => Self::ErrProcMigrate,
            -8 => Self::ErrProcRequestedAbort,
            -11 => Self::ErrExists,
            -12 => Self::ErrInvalidCred,
            -15 => Self::ErrWouldBlock,
            -16 => Self::ErrUnknownDataType,
            -18 => Self::ErrTypeMismatch,
            -19 => Self::ErrUnpackInadequateSpace,
            -20 => Self::ErrUnpackFailure,
            -21 => Self::ErrPackFailure,
            -23 => Self::ErrNoPermissions,
            -24 => Self::ErrTimeout,
            -25 => Self::ErrUnreach,
            -27 => Self::ErrBadParam,
            -28 => Self::ErrResourceBusy,
            -29 => Self::ErrOutOfResource,
            -31 => Self::ErrInit,
            -32 => Self::ErrNomem,
            -46 => Self::ErrNotFound,
            -47 => Self::ErrNotSupported,
            -49 => Self::ErrCommFailure,
            -50 => Self::ErrUnpackReadPastEndOfBuffer,
            -51 => Self::ErrConflictingCleanupDirectives,
            -52 => Self::ErrPartialSuccess,
            -53 => Self::ErrDuplicateKey,
            -58 => Self::ReadyForDebug,
            -59 => Self::ErrParamValueNotSupported,
            -60 => Self::ErrEmpty,
            -61 => Self::ErrLostConnection,
            -62 => Self::ErrExistsOutsideScope,
            -106 => Self::JctrlCheckpoint,
            -107 => Self::JctrlCheckpointComplete,
            -108 => Self::JctrlPreemptAlert,
            -109 => Self::MonitorHeartbeatAlert,
            -110 => Self::MonitorFileAlert,
            -113 => Self::FabricUpdateEndpoints,
            -144 => Self::ErrEventRegistration,
            -145 => Self::EventJobEnd,
            -156 => Self::OperationInProgress,
            -157 => Self::OperationSucceeded,
            -158 => Self::ErrInvalidOperation,
            -171 => Self::ErrRepeatAttrRegistration,
            -172 => Self::ErrIofFailure,
            -173 => Self::ErrIofComplete,
            -175 => Self::FabricUpdated,
            -176 => Self::FabricUpdatePending,
            -177 => Self::ErrJobAppNotExecutable,
            -178 => Self::ErrJobNoExeSpecified,
            -179 => Self::ErrJobFailedToMap,
            -180 => Self::ErrJobCanceled,
            -181 => Self::ErrJobFailedToLaunch,
            -182 => Self::ErrJobAborted,
            -183 => Self::ErrJobKilledByCmd,
            -184 => Self::ErrJobAbortedBySig,
            -185 => Self::ErrJobTermWoSync,
            -186 => Self::ErrJobSensorBoundExceeded,
            -187 => Self::ErrJobNonZeroTerm,
            -188 => Self::ErrJobAllocFailed,
            -189 => Self::ErrJobAbortedBySysEvent,
            -190 => Self::ErrJobExeNotFound,
            -191 => Self::EventJobStart,
            -192 => Self::EventSessionStart,
            -193 => Self::EventSessionEnd,
            -200 => Self::ErrProcTermWoSync,
            -201 => Self::EventProcTerminated,
            -230 => Self::EventSysBase,
            -231 => Self::EventNodeDown,
            -232 => Self::EventNodeOffline,
            -233 => Self::ErrJobWdirNotFound,
            -234 => Self::ErrJobInsufficientResources,
            -235 => Self::ErrJobSysOpFailed,
            -330 => Self::EventSysOther,
            -331 => Self::EventNoActionTaken,
            -332 => Self::EventPartialActionTaken,
            -333 => Self::EventActionDeferred,
            -334 => Self::EventActionComplete,
            -400 => Self::ErrProcKilledByCmd,
            -401 => Self::ErrProcFailedToStart,
            -402 => Self::ErrProcAbortedBySig,
            -403 => Self::ErrProcSensorBoundExceeded,
            -404 => Self::ErrExitNonzeroTerm,
            -3000 => Self::ExternalErrBase,
            _ => return None,
        })
    }

    /// Return the raw `i32` discriminant (`pmix_status_t` value).
    #[inline]
    pub fn to_raw(self) -> i32 {
        self as i32
    }

    /// `true` for `PMIX_SUCCESS` (0) and positive informational codes.
    ///
    /// Positive codes are used by event handlers to signal varying degrees
    /// of success rather than failure.
    #[inline]
    pub fn is_success(self) -> bool {
        (self as i32) >= 0
    }

    /// `true` for any negative error code.
    #[inline]
    pub fn is_error(self) -> bool {
        !self.is_success()
    }

    /// The standard short-name string (e.g. `"PMIX_ERR_NOMEM"`).
    ///
    /// Mirrors the output of `PMIx_Error_string()` from the C library.
    pub fn name(self) -> &'static str {
        match self {
            Self::Success => "PMIX_SUCCESS",
            Self::Error => "PMIX_ERROR",
            Self::DebuggerRelease => "PMIX_DEBUGGER_RELEASE",
            Self::ErrProcRestart => "PMIX_ERR_PROC_RESTART",
            Self::ErrProcCheckpoint => "PMIX_ERR_PROC_CHECKPOINT",
            Self::ErrProcMigrate => "PMIX_ERR_PROC_MIGRATE",
            Self::ErrProcRequestedAbort => "PMIX_ERR_PROC_REQUESTED_ABORT",
            Self::ErrExists => "PMIX_ERR_EXISTS",
            Self::ErrInvalidCred => "PMIX_ERR_INVALID_CRED",
            Self::ErrWouldBlock => "PMIX_ERR_WOULD_BLOCK",
            Self::ErrUnknownDataType => "PMIX_ERR_UNKNOWN_DATA_TYPE",
            Self::ErrTypeMismatch => "PMIX_ERR_TYPE_MISMATCH",
            Self::ErrUnpackInadequateSpace => "PMIX_ERR_UNPACK_INADEQUATE_SPACE",
            Self::ErrUnpackFailure => "PMIX_ERR_UNPACK_FAILURE",
            Self::ErrPackFailure => "PMIX_ERR_PACK_FAILURE",
            Self::ErrNoPermissions => "PMIX_ERR_NO_PERMISSIONS",
            Self::ErrTimeout => "PMIX_ERR_TIMEOUT",
            Self::ErrUnreach => "PMIX_ERR_UNREACH",
            Self::ErrBadParam => "PMIX_ERR_BAD_PARAM",
            Self::ErrResourceBusy => "PMIX_ERR_RESOURCE_BUSY",
            Self::ErrOutOfResource => "PMIX_ERR_OUT_OF_RESOURCE",
            Self::ErrInit => "PMIX_ERR_INIT",
            Self::ErrNomem => "PMIX_ERR_NOMEM",
            Self::ErrNotFound => "PMIX_ERR_NOT_FOUND",
            Self::ErrNotSupported => "PMIX_ERR_NOT_SUPPORTED",
            Self::ErrCommFailure => "PMIX_ERR_COMM_FAILURE",
            Self::ErrUnpackReadPastEndOfBuffer => "PMIX_ERR_UNPACK_READ_PAST_END_OF_BUFFER",
            Self::ErrConflictingCleanupDirectives => "PMIX_ERR_CONFLICTING_CLEANUP_DIRECTIVES",
            Self::ErrPartialSuccess => "PMIX_ERR_PARTIAL_SUCCESS",
            Self::ErrDuplicateKey => "PMIX_ERR_DUPLICATE_KEY",
            Self::ReadyForDebug => "PMIX_READY_FOR_DEBUG",
            Self::ErrParamValueNotSupported => "PMIX_ERR_PARAM_VALUE_NOT_SUPPORTED",
            Self::ErrEmpty => "PMIX_ERR_EMPTY",
            Self::ErrLostConnection => "PMIX_ERR_LOST_CONNECTION",
            Self::ErrExistsOutsideScope => "PMIX_ERR_EXISTS_OUTSIDE_SCOPE",
            Self::JctrlCheckpoint => "PMIX_JCTRL_CHECKPOINT",
            Self::JctrlCheckpointComplete => "PMIX_JCTRL_CHECKPOINT_COMPLETE",
            Self::JctrlPreemptAlert => "PMIX_JCTRL_PREEMPT_ALERT",
            Self::MonitorHeartbeatAlert => "PMIX_MONITOR_HEARTBEAT_ALERT",
            Self::MonitorFileAlert => "PMIX_MONITOR_FILE_ALERT",
            Self::FabricUpdateEndpoints => "PMIX_FABRIC_UPDATE_ENDPOINTS",
            Self::ErrEventRegistration => "PMIX_ERR_EVENT_REGISTRATION",
            Self::EventJobEnd => "PMIX_EVENT_JOB_END",
            Self::OperationInProgress => "PMIX_OPERATION_IN_PROGRESS",
            Self::OperationSucceeded => "PMIX_OPERATION_SUCCEEDED",
            Self::ErrInvalidOperation => "PMIX_ERR_INVALID_OPERATION",
            Self::ErrRepeatAttrRegistration => "PMIX_ERR_REPEAT_ATTR_REGISTRATION",
            Self::ErrIofFailure => "PMIX_ERR_IOF_FAILURE",
            Self::ErrIofComplete => "PMIX_ERR_IOF_COMPLETE",
            Self::FabricUpdated => "PMIX_FABRIC_UPDATED",
            Self::FabricUpdatePending => "PMIX_FABRIC_UPDATE_PENDING",
            Self::ErrJobAppNotExecutable => "PMIX_ERR_JOB_APP_NOT_EXECUTABLE",
            Self::ErrJobNoExeSpecified => "PMIX_ERR_JOB_NO_EXE_SPECIFIED",
            Self::ErrJobFailedToMap => "PMIX_ERR_JOB_FAILED_TO_MAP",
            Self::ErrJobCanceled => "PMIX_ERR_JOB_CANCELED",
            Self::ErrJobFailedToLaunch => "PMIX_ERR_JOB_FAILED_TO_LAUNCH",
            Self::ErrJobAborted => "PMIX_ERR_JOB_ABORTED",
            Self::ErrJobKilledByCmd => "PMIX_ERR_JOB_KILLED_BY_CMD",
            Self::ErrJobAbortedBySig => "PMIX_ERR_JOB_ABORTED_BY_SIG",
            Self::ErrJobTermWoSync => "PMIX_ERR_JOB_TERM_WO_SYNC",
            Self::ErrJobSensorBoundExceeded => "PMIX_ERR_JOB_SENSOR_BOUND_EXCEEDED",
            Self::ErrJobNonZeroTerm => "PMIX_ERR_JOB_NON_ZERO_TERM",
            Self::ErrJobAllocFailed => "PMIX_ERR_JOB_ALLOC_FAILED",
            Self::ErrJobAbortedBySysEvent => "PMIX_ERR_JOB_ABORTED_BY_SYS_EVENT",
            Self::ErrJobExeNotFound => "PMIX_ERR_JOB_EXE_NOT_FOUND",
            Self::EventJobStart => "PMIX_EVENT_JOB_START",
            Self::EventSessionStart => "PMIX_EVENT_SESSION_START",
            Self::EventSessionEnd => "PMIX_EVENT_SESSION_END",
            Self::ErrProcTermWoSync => "PMIX_ERR_PROC_TERM_WO_SYNC",
            Self::EventProcTerminated => "PMIX_EVENT_PROC_TERMINATED",
            Self::EventSysBase => "PMIX_EVENT_SYS_BASE",
            Self::EventNodeDown => "PMIX_EVENT_NODE_DOWN",
            Self::EventNodeOffline => "PMIX_EVENT_NODE_OFFLINE",
            Self::ErrJobWdirNotFound => "PMIX_ERR_JOB_WDIR_NOT_FOUND",
            Self::ErrJobInsufficientResources => "PMIX_ERR_JOB_INSUFFICIENT_RESOURCES",
            Self::ErrJobSysOpFailed => "PMIX_ERR_JOB_SYS_OP_FAILED",
            Self::EventSysOther => "PMIX_EVENT_SYS_OTHER",
            Self::EventNoActionTaken => "PMIX_EVENT_NO_ACTION_TAKEN",
            Self::EventPartialActionTaken => "PMIX_EVENT_PARTIAL_ACTION_TAKEN",
            Self::EventActionDeferred => "PMIX_EVENT_ACTION_DEFERRED",
            Self::EventActionComplete => "PMIX_EVENT_ACTION_COMPLETE",
            Self::ErrProcKilledByCmd => "PMIX_ERR_PROC_KILLED_BY_CMD",
            Self::ErrProcFailedToStart => "PMIX_ERR_PROC_FAILED_TO_START",
            Self::ErrProcAbortedBySig => "PMIX_ERR_PROC_ABORTED_BY_SIG",
            Self::ErrProcSensorBoundExceeded => "PMIX_ERR_PROC_SENSOR_BOUND_EXCEEDED",
            Self::ErrExitNonzeroTerm => "PMIX_ERR_EXIT_NONZERO_TERM",
            Self::ExternalErrBase => "PMIX_EXTERNAL_ERR_BASE",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcState
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_proc_state_t` (PMIx v4.0+).
///
/// `pmix_proc_state_t` is a `uint8_t` that encodes the lifecycle state
/// of a process managed by the PMIx resource manager.  Values are grouped
/// into logical ranges:
///
/// * `0`        — undefined
/// * `1–6`      — pre-launch and active states
/// * `15`       — unterminated (still alive)
/// * `20`       — cleanly terminated
/// * `50+`      — error / abnormal termination states
///
/// All values defined in `pmix_common.h §Process State Definitions` are
/// represented.  Unknown raw values from future library versions are
/// captured in the [`Unknown`][PmixProcState::Unknown] variant.
///
/// # C API
/// `typedef uint8_t pmix_proc_state_t;`
///
/// See also [`crate::utility::proc_state_string`] for the human-readable
/// string representation of each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixProcState {
    /// `PMIX_PROC_STATE_UNDEF` (0) — undefined process state.
    Undef = 0,

    /// `PMIX_PROC_STATE_PREPPED` (1) — process is ready to be launched.
    Prepped = 1,

    /// `PMIX_PROC_STATE_LAUNCH_UNDERWAY` (2) — launch process underway.
    LaunchUnderway = 2,

    /// `PMIX_PROC_STATE_RESTART` (3) — the proc is ready for restart.
    Restart = 3,

    /// `PMIX_PROC_STATE_TERMINATE` (4) — process is marked for termination.
    Terminate = 4,

    /// `PMIX_PROC_STATE_RUNNING` (5) — daemon has locally forked process.
    Running = 5,

    /// `PMIX_PROC_STATE_CONNECTED` (6) — proc connected to PMIx server.
    Connected = 6,

    /// `PMIX_PROC_STATE_UNTERMINATED` (15) — process has not yet terminated.
    Unterminated = 15,

    /// `PMIX_PROC_STATE_TERMINATED` (20) — process has terminated cleanly.
    Terminated = 20,

    /// `PMIX_PROC_STATE_ERROR` (50) — generic process error.
    Error = 50,

    /// `PMIX_PROC_STATE_KILLED_BY_CMD` (51) — process was killed by command.
    KilledByCmd = 51,

    /// `PMIX_PROC_STATE_ABORTED` (52) — process aborted abnormally.
    Aborted = 52,

    /// `PMIX_PROC_STATE_FAILED_TO_START` (53) — process failed to start.
    FailedToStart = 53,

    /// `PMIX_PROC_STATE_ABORTED_BY_SIG` (54) — process aborted by signal.
    AbortedBySig = 54,

    /// `PMIX_PROC_STATE_TERM_WO_SYNC` (55) — process exited without calling
    /// `PMIx_Finalize`.
    TermWoSync = 55,

    /// `PMIX_PROC_STATE_COMM_FAILED` (56) — process communication has failed.
    CommFailed = 56,

    /// `PMIX_PROC_STATE_SENSOR_BOUND_EXCEEDED` (57) — process exceeded a
    /// sensor limit.
    SensorBoundExceeded = 57,

    /// `PMIX_PROC_STATE_CALLED_ABORT` (58) — process called `PMIx_Abort`.
    CalledAbort = 58,

    /// `PMIX_PROC_STATE_HEARTBEAT_FAILED` (59) — process failed to send
    /// heartbeat within time limit.
    HeartbeatFailed = 59,

    /// `PMIX_PROC_STATE_MIGRATING` (60) — process failed and is waiting for
    /// resources before restarting.
    Migrating = 60,

    /// `PMIX_PROC_STATE_CANNOT_RESTART` (61) — process failed and cannot be
    /// restarted.
    CannotRestart = 61,

    /// `PMIX_PROC_STATE_TERM_NON_ZERO` (62) — process exited with a
    /// non-zero status, indicating abnormal termination.
    TermNonZero = 62,

    /// `PMIX_PROC_STATE_FAILED_TO_LAUNCH` (63) — unable to launch process.
    FailedToLaunch = 63,

    /// An unrecognised or future process state value.
    Unknown(u8),
}

impl PmixProcState {
    /// Convert a raw `pmix_proc_state_t` (`u8`) into a `PmixProcState`.
    pub fn from_raw(state: u8) -> Self {
        match state {
            0 => Self::Undef,
            1 => Self::Prepped,
            2 => Self::LaunchUnderway,
            3 => Self::Restart,
            4 => Self::Terminate,
            5 => Self::Running,
            6 => Self::Connected,
            15 => Self::Unterminated,
            20 => Self::Terminated,
            50 => Self::Error,
            51 => Self::KilledByCmd,
            52 => Self::Aborted,
            53 => Self::FailedToStart,
            54 => Self::AbortedBySig,
            55 => Self::TermWoSync,
            56 => Self::CommFailed,
            57 => Self::SensorBoundExceeded,
            58 => Self::CalledAbort,
            59 => Self::HeartbeatFailed,
            60 => Self::Migrating,
            61 => Self::CannotRestart,
            62 => Self::TermNonZero,
            63 => Self::FailedToLaunch,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::Undef => 0,
            Self::Prepped => 1,
            Self::LaunchUnderway => 2,
            Self::Restart => 3,
            Self::Terminate => 4,
            Self::Running => 5,
            Self::Connected => 6,
            Self::Unterminated => 15,
            Self::Terminated => 20,
            Self::Error => 50,
            Self::KilledByCmd => 51,
            Self::Aborted => 52,
            Self::FailedToStart => 53,
            Self::AbortedBySig => 54,
            Self::TermWoSync => 55,
            Self::CommFailed => 56,
            Self::SensorBoundExceeded => 57,
            Self::CalledAbort => 58,
            Self::HeartbeatFailed => 59,
            Self::Migrating => 60,
            Self::CannotRestart => 61,
            Self::TermNonZero => 62,
            Self::FailedToLaunch => 63,
        }
    }

    /// `true` if the state indicates the process is still alive (running,
    /// connected, or in a transitional pre-launch state).
    pub fn is_alive(self) -> bool {
        matches!(
            self,
            Self::Prepped
                | Self::LaunchUnderway
                | Self::Restart
                | Self::Running
                | Self::Connected
                | Self::Unterminated
                | Self::Migrating
        )
    }

    /// `true` if the state indicates the process has terminated (cleanly or
    /// not).
    pub fn is_terminated(self) -> bool {
        matches!(
            self,
            Self::Terminated
                | Self::KilledByCmd
                | Self::Aborted
                | Self::FailedToStart
                | Self::AbortedBySig
                | Self::TermWoSync
                | Self::CommFailed
                | Self::SensorBoundExceeded
                | Self::CalledAbort
                | Self::HeartbeatFailed
                | Self::CannotRestart
                | Self::TermNonZero
                | Self::FailedToLaunch
        )
    }
}

impl std::fmt::Display for PmixProcState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undef => write!(f, "UNDEFINED"),
            Self::Prepped => write!(f, "PREPPED FOR LAUNCH"),
            Self::LaunchUnderway => write!(f, "LAUNCH UNDERWAY"),
            Self::Restart => write!(f, "PROC READY FOR RESTART"),
            Self::Terminate => write!(f, "PROC MARKED FOR TERMINATION"),
            Self::Running => write!(f, "PROC EXECUTING"),
            Self::Connected => write!(f, "PROC HAS CONNECTED TO LOCAL PMIX SERVER"),
            Self::Unterminated => write!(f, "PROC HAS NOT TERMINATED"),
            Self::Terminated => write!(f, "PROC HAS TERMINATED"),
            Self::Error => write!(f, "PROC ERROR"),
            Self::KilledByCmd => write!(f, "PROC KILLED BY CMD"),
            Self::Aborted => write!(f, "PROC ABNORMALLY ABORTED"),
            Self::FailedToStart => write!(f, "PROC FAILED TO START"),
            Self::AbortedBySig => write!(f, "PROC ABORTED BY SIGNAL"),
            Self::TermWoSync => write!(f, "PROC TERMINATED WITHOUT CALLING PMIx_Finalize"),
            Self::CommFailed => write!(f, "PROC LOST COMMUNICATION"),
            Self::SensorBoundExceeded => write!(f, "PROC SENSOR BOUND EXCEEDED"),
            Self::CalledAbort => write!(f, "PROC CALLED PMIx_Abort"),
            Self::HeartbeatFailed => write!(f, "PROC FAILED TO REPORT HEARTBEAT"),
            Self::Migrating => write!(f, "PROC WAITING TO MIGRATE"),
            Self::CannotRestart => write!(f, "PROC CANNOT BE RESTARTED"),
            Self::TermNonZero => write!(f, "PROC TERMINATED WITH NON-ZERO STATUS"),
            Self::FailedToLaunch => write!(f, "PROC FAILED TO LAUNCH"),
            Self::Unknown(v) => write!(f, "UNKNOWN STATE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixScope — safe Rust wrapper around `pmix_scope_t`
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_scope_t` (PMIx 5.0).
///
/// `pmix_scope_t` defines the visibility scope for data stored via `PMIx_Put`.
/// It is a `uint8_t` with the following values defined in `pmix_common.h`:
///
/// * `0` — Undefined scope
/// * `1` — Local (same node only)
/// * `2` — Remote (remote nodes only)
/// * `3` — Global (all nodes)
/// * `4` — Internal (library-internal storage)
///
/// Use [`PmixScope::from_raw`] to convert a `pmix_scope_t` received from C
/// and [`PmixScope::to_raw`] to convert back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixScope {
    /// `PMIX_SCOPE_UNDEF` (0) — undefined scope.
    Undef = 0,

    /// `PMIX_LOCAL` (1) — share with processes also on this node.
    Local = 1,

    /// `PMIX_REMOTE` (2) — share with processes not on this node.
    Remote = 2,

    /// `PMIX_GLOBAL` (3) — share with all processes (local + remote).
    Global = 3,

    /// `PMIX_INTERNAL` (4) — store data in the internal tables only.
    Internal = 4,

    /// An unrecognised or future scope value.
    Unknown(u8),
}

impl PmixScope {
    /// Convert a raw `pmix_scope_t` (`u8`) into a `PmixScope`.
    pub fn from_raw(scope: u8) -> Self {
        match scope {
            0 => Self::Undef,
            1 => Self::Local,
            2 => Self::Remote,
            3 => Self::Global,
            4 => Self::Internal,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::Undef => 0,
            Self::Local => 1,
            Self::Remote => 2,
            Self::Global => 3,
            Self::Internal => 4,
        }
    }
}

impl std::fmt::Display for PmixScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undef => write!(f, "UNDEFINED"),
            Self::Local => write!(f, "LOCAL"),
            Self::Remote => write!(f, "REMOTE"),
            Self::Global => write!(f, "GLOBAL"),
            Self::Internal => write!(f, "INTERNAL"),
            Self::Unknown(v) => write!(f, "UNKNOWN SCOPE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixJobState — pmix_job_state_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_job_state_t` (PMIx v4.0+).
///
/// `pmix_job_state_t` is a `uint8_t` that encodes the lifecycle state
/// of a job managed by the PMIx resource manager.  Values are grouped
/// into logical ranges:
///
/// * `0`        — undefined
/// * `1–5`      — pre-launch and active states
/// * `15`       — unterminated (still alive boundary)
/// * `20`       — cleanly terminated
/// * `50+`      — error / abnormal termination states
///
/// All values defined in `pmix_common.h §Job State Definitions` are
/// represented.  Unknown raw values from future library versions are
/// captured in the [`Unknown`][PmixJobState::Unknown] variant.
///
/// # C API
/// `typedef uint8_t pmix_job_state_t;`
///
/// See also [`crate::utility::job_state_string`] for the human-readable
/// string representation of each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixJobState {
    /// `PMIX_JOB_STATE_UNDEF` (0) — undefined job state.
    Undef = 0,

    /// `PMIX_JOB_STATE_AWAITING_ALLOC` (1) — job is waiting for resources
    /// to be allocated to it.
    AwaitingAlloc = 1,

    /// `PMIX_JOB_STATE_LAUNCH_UNDERWAY` (2) — job launch is underway.
    LaunchUnderway = 2,

    /// `PMIX_JOB_STATE_RUNNING` (3) — all processes have been spawned.
    Running = 3,

    /// `PMIX_JOB_STATE_SUSPENDED` (4) — job has been suspended.
    Suspended = 4,

    /// `PMIX_JOB_STATE_CONNECTED` (5) — all processes have connected to
    /// their PMIx server.
    Connected = 5,

    /// `PMIX_JOB_STATE_UNTERMINATED` (15) — boundary value; any state less
    /// than this means the job has not terminated.
    Unterminated = 15,

    /// `PMIX_JOB_STATE_TERMINATED` (20) — job has terminated and is no
    /// longer running, typically accompanied by the job exit status.
    Terminated = 20,

    /// `PMIX_JOB_STATE_TERMINATED_WITH_ERROR` (50) — job has terminated
    /// and is no longer running, typically accompanied by a job-related
    /// error code.
    TerminatedWithError = 50,

    /// An unrecognised or future job state value.
    Unknown(u8),
}

impl PmixJobState {
    /// Convert a raw `pmix_job_state_t` (`u8`) into a `PmixJobState`.
    pub fn from_raw(state: u8) -> Self {
        match state {
            0 => Self::Undef,
            1 => Self::AwaitingAlloc,
            2 => Self::LaunchUnderway,
            3 => Self::Running,
            4 => Self::Suspended,
            5 => Self::Connected,
            15 => Self::Unterminated,
            20 => Self::Terminated,
            50 => Self::TerminatedWithError,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::Undef => 0,
            Self::AwaitingAlloc => 1,
            Self::LaunchUnderway => 2,
            Self::Running => 3,
            Self::Suspended => 4,
            Self::Connected => 5,
            Self::Unterminated => 15,
            Self::Terminated => 20,
            Self::TerminatedWithError => 50,
        }
    }
}

impl std::fmt::Display for PmixJobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undef => write!(f, "UNDEF"),
            Self::AwaitingAlloc => write!(f, "AWAITING_ALLOC"),
            Self::LaunchUnderway => write!(f, "LAUNCH_UNDERWAY"),
            Self::Running => write!(f, "RUNNING"),
            Self::Suspended => write!(f, "SUSPENDED"),
            Self::Connected => write!(f, "CONNECTED"),
            Self::Unterminated => write!(f, "UNTERMINATED"),
            Self::Terminated => write!(f, "TERMINATED"),
            Self::TerminatedWithError => write!(f, "TERMINATED_WITH_ERROR"),
            Self::Unknown(v) => write!(f, "UNKNOWN JOB STATE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixLinkState — pmix_link_state_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_link_state_t` (PMIx v4.1+).
///
/// `pmix_link_state_t` is a `uint8_t` that encodes the physical link state
/// of a fabric device port. Used by the fabric device API to report port
/// status:
///
/// * `0` — `UNKNOWN` — port state is unknown or not applicable.
/// * `1` — `LINK_DOWN` — port is inactive.
/// * `2` — `LINK_UP` — port is active.
///
/// All values defined in `pmix_common.h §Link State Definitions` are
/// represented.  Unknown raw values from future library versions are
/// captured in the [`Unknown`][PmixLinkState::Unknown] variant.
///
/// # C API
/// `typedef uint8_t pmix_link_state_t;`
///
/// See also [`crate::utility::link_state_string`] for the human-readable
/// string representation of each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixLinkState {
    /// `PMIX_LINK_STATE_UNKNOWN` (0) — the port state is unknown or not
    /// applicable.
    UnknownState = 0,

    /// `PMIX_LINK_DOWN` (1) — the port is inactive.
    LinkDown = 1,

    /// `PMIX_LINK_UP` (2) — the port is active.
    LinkUp = 2,

    /// An unrecognised or future link state value.
    Unknown(u8),
}

impl PmixLinkState {
    /// Convert a raw `pmix_link_state_t` (`u8`) into a `PmixLinkState`.
    pub fn from_raw(state: u8) -> Self {
        match state {
            0 => Self::UnknownState,
            1 => Self::LinkDown,
            2 => Self::LinkUp,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::UnknownState => 0,
            Self::LinkDown => 1,
            Self::LinkUp => 2,
        }
    }
}

impl std::fmt::Display for PmixLinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownState => write!(f, "UNKNOWN"),
            Self::LinkDown => write!(f, "INACTIVE"),
            Self::LinkUp => write!(f, "ACTIVE"),
            Self::Unknown(v) => write!(f, "UNKNOWN LINK STATE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDeviceType — pmix_device_type_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_device_type_t` (PMIx v4.1+).
///
/// `pmix_device_type_t` is a `uint64_t` bitmask that encodes hardware device
/// types on a node. Used by the fabric / hardware API to classify devices:
///
/// * `0x00` — `UNKNOWN` — device type is unknown or not applicable.
/// * `0x01` — `BLOCK` — block storage device (disk, NVMe, etc.).
/// * `0x02` — `GPU` — graphics processing unit.
/// * `0x04` — `NETWORK` — network interface card.
/// * `0x08` — `OPENFABRICS` — InfiniBand / RoCE / iWARP fabric adapter.
/// * `0x10` — `DMA` — direct memory access engine.
/// * `0x20` — `COPROC` — coprocessor (FPGA, accelerator, etc.).
///
/// All values defined in `pmix_common.h §Device Type Definitions` are
/// represented.  Unknown raw values from future library versions are
/// captured in the [`Unknown`][PmixDeviceType::Unknown] variant.
///
/// # C API
/// `typedef uint64_t pmix_device_type_t;`
///
/// See also [`crate::utility::device_type_string`] for the human-readable
/// string representation of each type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
#[non_exhaustive]
pub enum PmixDeviceType {
    /// `PMIX_DEVTYPE_UNKNOWN` (0x00) — device type is unknown or not
    /// applicable.
    UnknownType = 0x00,

    /// `PMIX_DEVTYPE_BLOCK` (0x01) — block storage device.
    Block = 0x01,

    /// `PMIX_DEVTYPE_GPU` (0x02) — graphics processing unit.
    Gpu = 0x02,

    /// `PMIX_DEVTYPE_NETWORK` (0x04) — network interface card.
    Network = 0x04,

    /// `PMIX_DEVTYPE_OPENFABRICS` (0x08) — InfiniBand / RoCE / iWARP
    /// fabric adapter.
    OpenFabrics = 0x08,

    /// `PMIX_DEVTYPE_DMA` (0x10) — direct memory access engine.
    Dma = 0x10,

    /// `PMIX_DEVTYPE_COPROC` (0x20) — coprocessor (FPGA, accelerator,
    /// etc.).
    Coproc = 0x20,

    /// An unrecognised or future device type value.
    Unknown(u64),
}

impl PmixDeviceType {
    /// Convert a raw `pmix_device_type_t` (`u64`) into a `PmixDeviceType`.
    pub fn from_raw(ty: u64) -> Self {
        match ty {
            0x00 => Self::UnknownType,
            0x01 => Self::Block,
            0x02 => Self::Gpu,
            0x04 => Self::Network,
            0x08 => Self::OpenFabrics,
            0x10 => Self::Dma,
            0x20 => Self::Coproc,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u64` value suitable for passing to the C API.
    pub fn to_raw(self) -> u64 {
        match self {
            Self::Unknown(v) => v,
            Self::UnknownType => 0x00,
            Self::Block => 0x01,
            Self::Gpu => 0x02,
            Self::Network => 0x04,
            Self::OpenFabrics => 0x08,
            Self::Dma => 0x10,
            Self::Coproc => 0x20,
        }
    }
}

impl std::fmt::Display for PmixDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType => write!(f, "UNKNOWN"),
            Self::Block => write!(f, "BLOCK"),
            Self::Gpu => write!(f, "GPU"),
            Self::Network => write!(f, "NETWORK"),
            Self::OpenFabrics => write!(f, "OPENFABRICS"),
            Self::Dma => write!(f, "DMA"),
            Self::Coproc => write!(f, "COPROCESSOR"),
            Self::Unknown(v) => write!(f, "UNKNOWN DEVICE TYPE ({v:X})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPersistence — pmix_persistence_t
// ─────────────────────────────────────────────────────────────────────────────

/// Persistence of a published data item — how long the server should retain
/// the value before automatically discarding it.
///
/// Maps to `pmix_persistence_t` (`uint8_t`) in the C API.
///
/// # C API
/// `typedef uint8_t pmix_persistence_t`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
#[non_exhaustive]
pub enum PmixPersistence {
    /// `PMIX_PERSIST_INDEF` (0) — retain until specifically deleted.
    Indefinite = 0,

    /// `PMIX_PERSIST_FIRST_READ` (1) — delete upon first access.
    FirstRead = 1,

    /// `PMIX_PERSIST_PROC` (2) — retain until publishing process terminates.
    Process = 2,

    /// `PMIX_PERSIST_APP` (3) — retain until application terminates.
    Application = 3,

    /// `PMIX_PERSIST_SESSION` (4) — retain until session/allocation terminates.
    Session = 4,

    /// `PMIX_PERSIST_INVALID` (255) — invalid persistence value.
    Invalid = 255,

    /// An unrecognised or future persistence value.
    Unknown(u8),
}

impl PmixPersistence {
    /// Convert a raw `pmix_persistence_t` (`u8`) into a `PmixPersistence`.
    pub fn from_raw(persist: u8) -> Self {
        match persist {
            0 => Self::Indefinite,
            1 => Self::FirstRead,
            2 => Self::Process,
            3 => Self::Application,
            4 => Self::Session,
            255 => Self::Invalid,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::Indefinite => 0,
            Self::FirstRead => 1,
            Self::Process => 2,
            Self::Application => 3,
            Self::Session => 4,
            Self::Invalid => 255,
        }
    }
}

impl std::fmt::Display for PmixPersistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Indefinite => write!(f, "INDEFINITE"),
            Self::FirstRead => write!(f, "DELETE ON FIRST ACCESS"),
            Self::Process => write!(f, "RETAIN UNTIL PUBLISHING PROCESS TERMINATES"),
            Self::Application => write!(
                f,
                "RETAIN UNTIL APPLICATION OF PUBLISHING PROCESS TERMINATES"
            ),
            Self::Session => write!(
                f,
                "RETAIN UNTIL ALLOCATION OF PUBLISHING PROCESS TERMINATES"
            ),
            Self::Invalid => write!(f, "INVALID"),
            Self::Unknown(v) => write!(f, "UNKNOWN PERSISTENCE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataRange — pmix_data_range_t
// ─────────────────────────────────────────────────────────────────────────────

/// Range for data published by PMIx — where the data is visible.
///
/// Maps to `pmix_data_range_t` (`uint8_t`) in the C API. Defines the
/// range across which published data is accessible, used by
/// `PMIx_Publish`, `PMIx_Lookup`, `PMIx_Unpublish`, and notification
/// event delivery.
///
/// # C API
/// `typedef uint8_t pmix_data_range_t`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixDataRange {
    /// `PMIX_RANGE_UNDEF` (0) — undefined range.
    Undef = 0,

    /// `PMIX_RANGE_RM` (1) — data is intended for the host resource manager.
    Rm = 1,

    /// `PMIX_RANGE_LOCAL` (2) — available on local node only.
    Local = 2,

    /// `PMIX_RANGE_NAMESPACE` (3) — data is available to procs in the same
    /// namespace only.
    Namespace = 3,

    /// `PMIX_RANGE_SESSION` (4) — data available to all procs in the session.
    Session = 4,

    /// `PMIX_RANGE_GLOBAL` (5) — data available to all procs.
    Global = 5,

    /// `PMIX_RANGE_CUSTOM` (6) — range is specified in a `pmix_info_t`.
    Custom = 6,

    /// `PMIX_RANGE_PROC_LOCAL` (7) — restrict range to the local process.
    ProcLocal = 7,

    /// `PMIX_RANGE_INVALID` (255) — invalid range value.
    Invalid = 255,

    /// An unrecognised or future range value.
    Unknown = 128,
}

impl PmixDataRange {
    /// Convert a raw `pmix_data_range_t` (`u8`) into a `PmixDataRange`.
    pub fn from_raw(range: u8) -> Self {
        match range {
            0 => Self::Undef,
            1 => Self::Rm,
            2 => Self::Local,
            3 => Self::Namespace,
            4 => Self::Session,
            5 => Self::Global,
            6 => Self::Custom,
            7 => Self::ProcLocal,
            255 => Self::Invalid,
            _other => Self::Unknown,
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::Unknown => 128,
            Self::Undef => 0,
            Self::Rm => 1,
            Self::Local => 2,
            Self::Namespace => 3,
            Self::Session => 4,
            Self::Global => 5,
            Self::Custom => 6,
            Self::ProcLocal => 7,
            Self::Invalid => 255,
        }
    }
}

impl std::fmt::Display for PmixDataRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undef => write!(f, "UNDEFINED"),
            Self::Rm => write!(f, "RM"),
            Self::Local => write!(f, "LOCAL"),
            Self::Namespace => write!(f, "NAMESPACE"),
            Self::Session => write!(f, "SESSION"),
            Self::Global => write!(f, "GLOBAL"),
            Self::Custom => write!(f, "CUSTOM"),
            Self::ProcLocal => write!(f, "PROC LOCAL"),
            Self::Invalid => write!(f, "INVALID"),
            Self::Unknown => write!(f, "UNKNOWN RANGE (128)"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataType — pmix_data_type_t
// ─────────────────────────────────────────────────────────────────────────────

/// PMIx data type identifier — maps to `pmix_data_type_t` (`uint16_t`).
///
/// Used by the serialization / deserialization APIs (`PMIx_Data_pack`,
/// `PMIx_Data_unpack`, `PMIx_Data_load`, `PMIx_Data_unload`, etc.) to
/// describe the type of a value being transferred.
///
/// # C API
/// `typedef uint16_t pmix_data_type_t`
///
/// Values 0–69 are defined by the PMIx v4.1 standard. Values 70–499 are
/// reserved for implementation extensions. Values ≥ 500 (`PMIX_DATA_TYPE_MAX`)
/// are invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
#[non_exhaustive]
pub enum PmixDataType {
    /// `PMIX_UNDEF` (0) — undefined type.
    Undef = 0,

    /// `PMIX_BOOL` (1) — boolean, packed as `uint8_t`.
    Bool = 1,

    /// `PMIX_BYTE` (2) — a single byte of data.
    Byte = 2,

    /// `PMIX_STRING` (3) — NULL-terminated string.
    String = 3,

    /// `PMIX_SIZE` (4) — `size_t`.
    Size = 4,

    /// `PMIX_PID` (5) — OS process ID.
    Pid = 5,

    /// `PMIX_INT` (6) — C `int`.
    Int = 6,

    /// `PMIX_INT8` (7) — signed 8-bit integer.
    Int8 = 7,

    /// `PMIX_INT16` (8) — signed 16-bit integer.
    Int16 = 8,

    /// `PMIX_INT32` (9) — signed 32-bit integer.
    Int32 = 9,

    /// `PMIX_INT64` (10) — signed 64-bit integer.
    Int64 = 10,

    /// `PMIX_UINT` (11) — C `unsigned int`.
    Uint = 11,

    /// `PMIX_UINT8` (12) — unsigned 8-bit integer.
    Uint8 = 12,

    /// `PMIX_UINT16` (13) — unsigned 16-bit integer.
    Uint16 = 13,

    /// `PMIX_UINT32` (14) — unsigned 32-bit integer.
    Uint32 = 14,

    /// `PMIX_UINT64` (15) — unsigned 64-bit integer.
    Uint64 = 15,

    /// `PMIX_FLOAT` (16) — 32-bit floating point.
    Float = 16,

    /// `PMIX_DOUBLE` (17) — 64-bit floating point.
    Double = 17,

    /// `PMIX_TIMEVAL` (18) — `struct timeval` (seconds + microseconds).
    Timeval = 18,

    /// `PMIX_TIME` (19) — `time_t`.
    Time = 19,

    /// `PMIX_STATUS` (20) — PMIx status code.
    Status = 20,

    /// `PMIX_VALUE` (21) — `pmix_value_t` container.
    Value = 21,

    /// `PMIX_PROC` (22) — `pmix_proc_t` (namespace + rank).
    Proc = 22,

    /// `PMIX_APP` (23) — `pmix_app_t` application descriptor.
    App = 23,

    /// `PMIX_INFO` (24) — `pmix_info_t` key-value triple.
    Info = 24,

    /// `PMIX_PDATA` (25) — `pmix_pdata_t` (proc + key + value).
    Pdata = 25,

    /// `PMIX_BYTE_OBJECT` (27) — opaque byte array with length.
    ByteObject = 27,

    /// `PMIX_KVAL` (28) — key-value list container.
    Kval = 28,

    /// `PMIX_PERSIST` (30) — persistence enum value.
    Persist = 30,

    /// `PMIX_POINTER` (31) — raw pointer (not portable across processes).
    Pointer = 31,

    /// `PMIX_SCOPE` (32) — scope enum value.
    Scope = 32,

    /// `PMIX_DATA_RANGE` (33) — data range enum value.
    DataRange = 33,

    /// `PMIX_COMMAND` (34) — command string identifier.
    Command = 34,

    /// `PMIX_INFO_DIRECTIVES` (35) — info directives bitmask.
    InfoDirectives = 35,

    /// `PMIX_DATA_TYPE` (36) — recursive data type identifier.
    DataType = 36,

    /// `PMIX_PROC_STATE` (37) — process state enum value.
    ProcState = 37,

    /// `PMIX_PROC_INFO` (38) — `pmix_proc_info_t` process info.
    ProcInfo = 38,

    /// `PMIX_DATA_ARRAY` (39) — `pmix_data_array_t` typed array.
    DataArray = 39,

    /// `PMIX_PROC_RANK` (40) — process rank within a namespace.
    ProcRank = 40,

    /// `PMIX_QUERY` (41) — `pmix_query_t` query descriptor.
    Query = 41,

    /// `PMIX_COMPRESSED_STRING` (42) — zlib-compressed string.
    CompressedString = 42,

    /// `PMIX_ALLOC_DIRECTIVE` (43) — allocation directive enum.
    AllocDirective = 43,

    /// `PMIX_IOF_CHANNEL` (45) — I/O forwarding channel enum.
    IofChannel = 45,

    /// `PMIX_ENVAR` (46) — environment variable (name + value + separator).
    Envar = 46,

    /// `PMIX_COORD` (47) — fabric coordinates.
    Coord = 47,

    /// `PMIX_REGATTR` (48) — registered attribute descriptor.
    Regattr = 48,

    /// `PMIX_REGEX` (49) — regex object.
    Regex = 49,

    /// `PMIX_JOB_STATE` (50) — job state enum value.
    JobState = 50,

    /// `PMIX_LINK_STATE` (51) — link state enum value.
    LinkState = 51,

    /// `PMIX_PROC_CPUSET` (52) — process CPU set.
    ProcCpuset = 52,

    /// `PMIX_GEOMETRY` (53) — fabric geometry descriptor.
    Geometry = 53,

    /// `PMIX_DEVICE_DIST` (54) — device distance matrix entry.
    DeviceDist = 54,

    /// `PMIX_ENDPOINT` (55) — fabric endpoint descriptor.
    Endpoint = 55,

    /// `PMIX_TOPO` (56) — topology descriptor.
    Topo = 56,

    /// `PMIX_DEVTYPE` (57) — device type identifier.
    Devtype = 57,

    /// `PMIX_LOCTYPE` (58) — locality type identifier.
    LocType = 58,

    /// `PMIX_COMPRESSED_BYTE_OBJECT` (59) — zlib-compressed byte object.
    CompressedByteObject = 59,

    /// `PMIX_PROC_NSPACE` (60) — process namespace string.
    ProcNspace = 60,

    /// `PMIX_PROC_STATS` (61) — process statistics.
    ProcStats = 61,

    /// `PMIX_DISK_STATS` (62) — disk I/O statistics.
    DiskStats = 62,

    /// `PMIX_NET_STATS` (63) — network I/O statistics.
    NetStats = 63,

    /// `PMIX_NODE_STATS` (64) — node-level aggregate statistics.
    NodeStats = 64,

    /// `PMIX_DATA_BUFFER` (65) — `pmix_data_buffer_t` buffer object.
    DataBuffer = 65,

    /// `PMIX_STOR_MEDIUM` (66) — storage medium type.
    StorMedium = 66,

    /// `PMIX_STOR_ACCESS` (67) — storage access descriptor.
    StorAccess = 67,

    /// `PMIX_STOR_PERSIST` (68) — storage persistence type.
    StorPersist = 68,

    /// `PMIX_STOR_ACCESS_TYPE` (69) — storage access type.
    StorAccessType = 69,

    /// An unrecognised or future data type value (70–499).
    Unknown,
}

impl PmixDataType {
    /// Convert a raw `pmix_data_type_t` (`u16`) into a `PmixDataType`.
    pub fn from_raw(ty: u16) -> Self {
        match ty {
            0 => Self::Undef,
            1 => Self::Bool,
            2 => Self::Byte,
            3 => Self::String,
            4 => Self::Size,
            5 => Self::Pid,
            6 => Self::Int,
            7 => Self::Int8,
            8 => Self::Int16,
            9 => Self::Int32,
            10 => Self::Int64,
            11 => Self::Uint,
            12 => Self::Uint8,
            13 => Self::Uint16,
            14 => Self::Uint32,
            15 => Self::Uint64,
            16 => Self::Float,
            17 => Self::Double,
            18 => Self::Timeval,
            19 => Self::Time,
            20 => Self::Status,
            21 => Self::Value,
            22 => Self::Proc,
            23 => Self::App,
            24 => Self::Info,
            25 => Self::Pdata,
            27 => Self::ByteObject,
            28 => Self::Kval,
            30 => Self::Persist,
            31 => Self::Pointer,
            32 => Self::Scope,
            33 => Self::DataRange,
            34 => Self::Command,
            35 => Self::InfoDirectives,
            36 => Self::DataType,
            37 => Self::ProcState,
            38 => Self::ProcInfo,
            39 => Self::DataArray,
            40 => Self::ProcRank,
            41 => Self::Query,
            42 => Self::CompressedString,
            43 => Self::AllocDirective,
            45 => Self::IofChannel,
            46 => Self::Envar,
            47 => Self::Coord,
            48 => Self::Regattr,
            49 => Self::Regex,
            50 => Self::JobState,
            51 => Self::LinkState,
            52 => Self::ProcCpuset,
            53 => Self::Geometry,
            54 => Self::DeviceDist,
            55 => Self::Endpoint,
            56 => Self::Topo,
            57 => Self::Devtype,
            58 => Self::LocType,
            59 => Self::CompressedByteObject,
            60 => Self::ProcNspace,
            61 => Self::ProcStats,
            62 => Self::DiskStats,
            63 => Self::NetStats,
            64 => Self::NodeStats,
            65 => Self::DataBuffer,
            66 => Self::StorMedium,
            67 => Self::StorAccess,
            68 => Self::StorPersist,
            69 => Self::StorAccessType,
            _ => Self::Unknown,
        }
    }

    /// Return the raw `u16` value suitable for passing to the C API.
    pub fn to_raw(self) -> u16 {
        match self {
            Self::Undef => 0,
            Self::Bool => 1,
            Self::Byte => 2,
            Self::String => 3,
            Self::Size => 4,
            Self::Pid => 5,
            Self::Int => 6,
            Self::Int8 => 7,
            Self::Int16 => 8,
            Self::Int32 => 9,
            Self::Int64 => 10,
            Self::Uint => 11,
            Self::Uint8 => 12,
            Self::Uint16 => 13,
            Self::Uint32 => 14,
            Self::Uint64 => 15,
            Self::Float => 16,
            Self::Double => 17,
            Self::Timeval => 18,
            Self::Time => 19,
            Self::Status => 20,
            Self::Value => 21,
            Self::Proc => 22,
            Self::App => 23,
            Self::Info => 24,
            Self::Pdata => 25,
            Self::ByteObject => 27,
            Self::Kval => 28,
            Self::Persist => 30,
            Self::Pointer => 31,
            Self::Scope => 32,
            Self::DataRange => 33,
            Self::Command => 34,
            Self::InfoDirectives => 35,
            Self::DataType => 36,
            Self::ProcState => 37,
            Self::ProcInfo => 38,
            Self::DataArray => 39,
            Self::ProcRank => 40,
            Self::Query => 41,
            Self::CompressedString => 42,
            Self::AllocDirective => 43,
            Self::IofChannel => 45,
            Self::Envar => 46,
            Self::Coord => 47,
            Self::Regattr => 48,
            Self::Regex => 49,
            Self::JobState => 50,
            Self::LinkState => 51,
            Self::ProcCpuset => 52,
            Self::Geometry => 53,
            Self::DeviceDist => 54,
            Self::Endpoint => 55,
            Self::Topo => 56,
            Self::Devtype => 57,
            Self::LocType => 58,
            Self::CompressedByteObject => 59,
            Self::ProcNspace => 60,
            Self::ProcStats => 61,
            Self::DiskStats => 62,
            Self::NetStats => 63,
            Self::NodeStats => 64,
            Self::DataBuffer => 65,
            Self::StorMedium => 66,
            Self::StorAccess => 67,
            Self::StorPersist => 68,
            Self::StorAccessType => 69,
            Self::Unknown => 70,
        }
    }
}

impl std::fmt::Display for PmixDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undef => write!(f, "UNDEF"),
            Self::Bool => write!(f, "BOOL"),
            Self::Byte => write!(f, "BYTE"),
            Self::String => write!(f, "STRING"),
            Self::Size => write!(f, "SIZE"),
            Self::Pid => write!(f, "PID"),
            Self::Int => write!(f, "INT"),
            Self::Int8 => write!(f, "INT8"),
            Self::Int16 => write!(f, "INT16"),
            Self::Int32 => write!(f, "INT32"),
            Self::Int64 => write!(f, "INT64"),
            Self::Uint => write!(f, "UINT"),
            Self::Uint8 => write!(f, "UINT8"),
            Self::Uint16 => write!(f, "UINT16"),
            Self::Uint32 => write!(f, "UINT32"),
            Self::Uint64 => write!(f, "UINT64"),
            Self::Float => write!(f, "FLOAT"),
            Self::Double => write!(f, "DOUBLE"),
            Self::Timeval => write!(f, "TIMEVAL"),
            Self::Time => write!(f, "TIME"),
            Self::Status => write!(f, "STATUS"),
            Self::Value => write!(f, "VALUE"),
            Self::Proc => write!(f, "PROC"),
            Self::App => write!(f, "APP"),
            Self::Info => write!(f, "INFO"),
            Self::Pdata => write!(f, "PDATA"),
            Self::ByteObject => write!(f, "BYTE_OBJECT"),
            Self::Kval => write!(f, "KVAL"),
            Self::Persist => write!(f, "PERSIST"),
            Self::Pointer => write!(f, "POINTER"),
            Self::Scope => write!(f, "SCOPE"),
            Self::DataRange => write!(f, "DATA_RANGE"),
            Self::Command => write!(f, "COMMAND"),
            Self::InfoDirectives => write!(f, "INFO_DIRECTIVES"),
            Self::DataType => write!(f, "DATA_TYPE"),
            Self::ProcState => write!(f, "PROC_STATE"),
            Self::ProcInfo => write!(f, "PROC_INFO"),
            Self::DataArray => write!(f, "DATA_ARRAY"),
            Self::ProcRank => write!(f, "PROC_RANK"),
            Self::Query => write!(f, "QUERY"),
            Self::CompressedString => write!(f, "COMPRESSED_STRING"),
            Self::AllocDirective => write!(f, "ALLOC_DIRECTIVE"),
            Self::IofChannel => write!(f, "IOF_CHANNEL"),
            Self::Envar => write!(f, "ENVAR"),
            Self::Coord => write!(f, "COORD"),
            Self::Regattr => write!(f, "REGATTR"),
            Self::Regex => write!(f, "REGEX"),
            Self::JobState => write!(f, "JOB_STATE"),
            Self::LinkState => write!(f, "LINK_STATE"),
            Self::ProcCpuset => write!(f, "PROC_CPUSET"),
            Self::Geometry => write!(f, "GEOMETRY"),
            Self::DeviceDist => write!(f, "DEVICE_DIST"),
            Self::Endpoint => write!(f, "ENDPOINT"),
            Self::Topo => write!(f, "TOPO"),
            Self::Devtype => write!(f, "DEVTYPE"),
            Self::LocType => write!(f, "LOCTYPE"),
            Self::CompressedByteObject => write!(f, "COMPRESSED_BYTE_OBJECT"),
            Self::ProcNspace => write!(f, "PROC_NSPACE"),
            Self::ProcStats => write!(f, "PROC_STATS"),
            Self::DiskStats => write!(f, "DISK_STATS"),
            Self::NetStats => write!(f, "NET_STATS"),
            Self::NodeStats => write!(f, "NODE_STATS"),
            Self::DataBuffer => write!(f, "DATA_BUFFER"),
            Self::StorMedium => write!(f, "STOR_MEDIUM"),
            Self::StorAccess => write!(f, "STOR_ACCESS"),
            Self::StorPersist => write!(f, "STOR_PERSIST"),
            Self::StorAccessType => write!(f, "STOR_ACCESS_TYPE"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixAllocDirective — pmix_alloc_directive_t
// ─────────────────────────────────────────────────────────────────────────────

/// Allocation directive — controls the behavior of `PMIx_Allocation_request`.
///
/// Maps to `pmix_alloc_directive_t` (`uint8_t`) in the C API. Currently only
/// one value is defined by the standard: `PMIX_ALLOC_DIRECTIVE` (43), which
/// indicates a hard allocation request. Future versions may add soft requests
/// or other variants.
///
/// # C API
/// `typedef uint8_t pmix_alloc_directive_t`
/// `#define PMIX_ALLOC_DIRECTIVE 43`
///
/// Use [`PmixAllocDirective::from_raw`] to convert a `pmix_alloc_directive_t`
/// received from C and [`PmixAllocDirective::to_raw`] to convert back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PmixAllocDirective {
    /// `PMIX_ALLOC_DIRECTIVE` (43) — hard allocation request.
    AllocDirective = 43,

    /// An unrecognised or future directive value.
    Unknown(u8),
}

impl PmixAllocDirective {
    /// Convert a raw `pmix_alloc_directive_t` (`u8`) into a `PmixAllocDirective`.
    pub fn from_raw(directive: u8) -> Self {
        match directive {
            43 => Self::AllocDirective,
            other => Self::Unknown(other),
        }
    }

    /// Return the raw `u8` value suitable for passing to the C API.
    pub fn to_raw(self) -> u8 {
        match self {
            Self::AllocDirective => 43,
            Self::Unknown(v) => v,
        }
    }
}

impl std::fmt::Display for PmixAllocDirective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllocDirective => write!(f, "ALLOC_DIRECTIVE"),
            Self::Unknown(v) => write!(f, "UNKNOWN DIRECTIVE ({v})"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IOFChannelFlags — type-safe bitmask over pmix_iof_channel_t
// ─────────────────────────────────────────────────────────────────────────────

/// Newtype over the raw `pmix_iof_channel_t` (u16 bitmask).
///
/// The `pmix_iof_channel_t` defines bit-mask flags for specifying IO
/// forwarding channels. These can be bitwise OR'd together to reference
/// multiple channels.
///
/// * `PMIX_FWD_NO_CHANNELS`   (0x0000) — forward no channels.
/// * `PMIX_FWD_STDIN_CHANNEL` (0x0001) — forward stdin.
/// * `PMIX_FWD_STDOUT_CHANNEL`(0x0002) — forward stdout.
/// * `PMIX_FWD_STDERR_CHANNEL`(0x0004) — forward stderr.
/// * `PMIX_FWD_STDDIAG_CHANNEL`(0x0008) — forward stddiag, if available.
/// * `PMIX_FWD_ALL_CHANNELS`  (0x00FF) — forward all available channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IOFChannelFlags(pub pmix_iof_channel_t);

impl IOFChannelFlags {
    /// `PMIX_FWD_NO_CHANNELS` (0x0000) — forward no channels.
    pub const NO_CHANNELS: Self = Self(PMIX_FWD_NO_CHANNELS as pmix_iof_channel_t);
    /// `PMIX_FWD_STDIN_CHANNEL` (0x0001) — forward stdin.
    pub const STDIN: Self = Self(PMIX_FWD_STDIN_CHANNEL as pmix_iof_channel_t);
    /// `PMIX_FWD_STDOUT_CHANNEL` (0x0002) — forward stdout.
    pub const STDOUT: Self = Self(PMIX_FWD_STDOUT_CHANNEL as pmix_iof_channel_t);
    /// `PMIX_FWD_STDERR_CHANNEL` (0x0004) — forward stderr.
    pub const STDERR: Self = Self(PMIX_FWD_STDERR_CHANNEL as pmix_iof_channel_t);
    /// `PMIX_FWD_STDDIAG_CHANNEL` (0x0008) — forward stddiag, if available.
    pub const STDDIAG: Self = Self(PMIX_FWD_STDDIAG_CHANNEL as pmix_iof_channel_t);
    /// `PMIX_FWD_ALL_CHANNELS` (0x00FF) — forward all available channels.
    pub const ALL_CHANNELS: Self = Self(PMIX_FWD_ALL_CHANNELS as pmix_iof_channel_t);

    /// Check if no channels are set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// Check if a specific channel flag is set.
    pub fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }
    /// Return the raw `pmix_iof_channel_t` value.
    pub fn raw(self) -> pmix_iof_channel_t {
        self.0
    }
}

impl std::ops::BitOr for IOFChannelFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for IOFChannelFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::fmt::Display for IOFChannelFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::STDIN) {
            parts.push("STDIN");
        }
        if self.contains(Self::STDOUT) {
            parts.push("STDOUT");
        }
        if self.contains(Self::STDERR) {
            parts.push("STDERR");
        }
        if self.contains(Self::STDDIAG) {
            parts.push("STDDIAG");
        }
        if parts.is_empty() {
            if self.is_empty() {
                write!(f, "NO_CHANNELS")
            } else {
                write!(f, "0x{:04X}", self.0)
            }
        } else {
            write!(f, "{}", parts.join("|"))
        }
    }
}

/// All errors the builder can produce.
#[derive(Debug, PartialEq, Eq)]
pub enum BuilderError {
    /// The key string contained an interior NUL byte.
    KeyContainsNul(NulError),
    /// The key is empty (PMIx requires a non-empty key).
    KeyEmpty,
    /// The key (in bytes, *excluding* the NUL terminator) exceeds
    /// `PMIX_MAX_KEYLEN` (511).
    KeyTooLong { len: usize, maximum: usize },
    /// No value was supplied before `build()` / `build_raw()` was called.
    MissingValue,
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyContainsNul(e) => write!(f, "key contains interior NUL: {e}"),
            Self::KeyEmpty => write!(f, "key must not be empty"),
            Self::KeyTooLong { len, maximum } => {
                write!(f, "key length {len} exceeds PMIX_MAX_KEYLEN ({maximum})")
            }
            Self::MissingValue => write!(f, "no value supplied to builder"),
        }
    }
}

impl std::error::Error for BuilderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::KeyContainsNul(e) => Some(e),
            _ => None,
        }
    }
}

impl From<NulError> for BuilderError {
    fn from(e: NulError) -> Self {
        Self::KeyContainsNul(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// All errors the builder can produce.
#[derive(Debug, PartialEq, Eq)]
pub enum ValueError {
    /// A string argument contained an interior NUL byte.
    ContainsNul(NulError),
    /// `build()` / `build_raw()` was called before any payload setter.
    MissingPayload,
    /// A byte-object or data-array was given a length of zero when one or
    /// more bytes/elements were required.
    EmptyData,
}

impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContainsNul(e) => write!(f, "string contains interior NUL: {e}"),
            Self::MissingPayload => write!(f, "no payload set on PmixValueBuilder"),
            Self::EmptyData => write!(f, "data slice must not be empty"),
        }
    }
}

impl std::error::Error for ValueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContainsNul(e) => Some(e),
            _ => None,
        }
    }
}

impl From<NulError> for ValueError {
    fn from(e: NulError) -> Self {
        Self::ContainsNul(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTimeval – Rust mirror of pmix_timeval_t
// ─────────────────────────────────────────────────────────────────────────────

/// Newtype so callers don't need to import `sys::pmix_timeval_t` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmixTimeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

impl From<PmixTimeval> for timeval {
    fn from(v: PmixTimeval) -> Self {
        Self {
            tv_sec: v.tv_sec,
            tv_usec: v.tv_usec,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixEnvar – Rust mirror of pmix_envar_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_envar_t`.
///
/// Both `envar` and `value` are stored as `CString` so interior-NUL checking
/// happens at construction time, not at `build()` time.
#[derive(Debug, Clone)]
pub struct PmixEnvar {
    pub envar: CString,
    pub value: CString,
    pub separator: u8,
}

impl PmixEnvar {
    /// Create from `&str` arguments; returns `NulError` if either string has
    /// an interior NUL.
    pub fn new(envar: &str, value: &str, separator: char) -> Result<Self, NulError> {
        Ok(Self {
            envar: CString::new(envar)?,
            value: CString::new(value)?,
            separator: separator as u8,
        })
    }
}

impl Proc {
    pub fn new(nspace: &str, rank: u32) -> Result<Self, NulError> {
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        let c_name = CString::new(nspace)?;
        unsafe {
            PMIx_Load_nspace(handle.nspace.as_mut_ptr(), c_name.as_ptr());
        }
        Ok(Proc { handle, len: 1 })
    }

    pub fn new_with_nspace(&self, rank: u32) -> Result<Self, NulError> {
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        unsafe {
            let src_handle = self.handle;
            PMIx_Load_nspace(handle.nspace.as_mut_ptr(), src_handle.nspace.as_ptr());
        }
        Ok(Proc { handle, len: 1 })
    }

    pub fn get_rank(&self) -> u32 {
        self.handle.rank
    }

    pub fn set_rank(&mut self, rank: u32) {
        self.handle.rank = rank;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoFlags — type-safe bitmask over pmix_info_directives_t
// ─────────────────────────────────────────────────────────────────────────────

/// Newtype over the raw `pmix_info_directives_t` (u32 bitmask).
///
/// Predefined constants mirror the `PMIX_INFO_*` C macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InfoFlags(pub pmix_info_directives_t);

impl InfoFlags {
    /// `PMIX_INFO_REQD` — fail if the attribute is unsupported.
    pub const REQD: Self = Self(PMIX_INFO_REQD);
    /// `PMIX_INFO_QUALIFIER` — qualifies a peer key.
    pub const QUALIFIER: Self = Self(PMIX_INFO_QUALIFIER);
    /// `PMIX_INFO_PERSISTENT` — do not release after processing.
    pub const PERSISTENT: Self = Self(PMIX_INFO_PERSISTENT);
    /// `PMIX_INFO_REQD_PROCESSED` — set by the library upon processing.
    pub const REQD_PROCESSED: Self = Self(PMIX_INFO_REQD_PROCESSED);

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn contains(self, f: Self) -> bool {
        (self.0 & f.0) == f.0
    }
    /// Return the raw `pmix_info_directives_t` value.
    pub fn raw(self) -> pmix_info_directives_t {
        self.0
    }
}

impl std::ops::BitOr for InfoFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for InfoFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
// ─────────────────────────────────────────────────────────────────────────────
// PmixPayload – the Rust-side discriminated union
// ─────────────────────────────────────────────────────────────────────────────

/// Every discriminant of `pmix_val_data`, expressed as a safe Rust enum.
///
/// Variants that wrap heap-allocated C data (`String`, `Proc`, `ByteObject`,
/// `Envar`, `DataArray`) own their data here; `write_into` transfers ownership
/// to the raw `pmix_value_t`.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum PmixPayload {
    Undef,
    // ── Scalar types ──────────────────────────────────────────────────────
    Bool(bool),
    Byte(u8),
    /// NUL-terminated heap string (`PMIX_STRING`).
    String(CString),
    Size(usize),
    Pid(u32),
    Int(i32),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Uint(u32),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Float(f32),
    Double(f64),
    /// Elapsed / absolute time (`PMIX_TIMEVAL`).
    Timeval(PmixTimeval),
    Status(pmix_status_t),
    Rank(pmix_rank_t),
    // ── Enum-like scalar types (thin newtypes over integers) ──────────────
    Persist(pmix_persistence_t),
    Scope(pmix_scope_t),
    DataRange(pmix_data_range_t),
    ProcState(pmix_proc_state_t),
    AllocDirective(pmix_alloc_directive_t),
    IofChannel(pmix_iof_channel_t),
    InfoDirectives(pmix_info_directives_t),
    // ── Composite heap types ──────────────────────────────────────────────
    /// A `pmix_proc_t` (namespace + rank) stored as `Box<pmix_proc_t>`.
    Proc(Proc),
    /// Raw byte buffer (`PMIX_BYTE_OBJECT`).
    ByteObject(Vec<u8>),
    /// An environment variable modification (`PMIX_ENVAR`).
    Envar(PmixEnvar),
    /// Opaque pointer (`PMIX_POINTER`). Caller is fully responsible for
    /// the lifetime of the pointed-to data.
    Pointer(*mut std::ffi::c_void),
    /// Heterogeneous or homogeneous array of `pmix_value_t` structs
    /// (`PMIX_DATA_ARRAY`). Element type is recorded separately.
    DataArray {
        elem_type: pmix_data_type_t,
        elements: Vec<pmix_value_t>,
    },
}

impl PmixPayload {
    /// Return the `pmix_data_type_t` constant that matches this variant.
    pub fn type_tag(&self) -> pmix_data_type_t {
        (match self {
            Self::Undef => PMIX_UNDEF,
            Self::Bool(_) => PMIX_BOOL,
            Self::Byte(_) => PMIX_BYTE,
            Self::String(_) => PMIX_STRING,
            Self::Size(_) => PMIX_SIZE,
            Self::Pid(_) => PMIX_PID,
            Self::Int(_) => PMIX_INT,
            Self::Int8(_) => PMIX_INT8,
            Self::Int16(_) => PMIX_INT16,
            Self::Int32(_) => PMIX_INT32,
            Self::Int64(_) => PMIX_INT64,
            Self::Uint(_) => PMIX_UINT,
            Self::Uint8(_) => PMIX_UINT8,
            Self::Uint16(_) => PMIX_UINT16,
            Self::Uint32(_) => PMIX_UINT32,
            Self::Uint64(_) => PMIX_UINT64,
            Self::Float(_) => PMIX_FLOAT,
            Self::Double(_) => PMIX_DOUBLE,
            Self::Timeval(_) => PMIX_TIMEVAL,
            Self::Status(_) => PMIX_STATUS,
            Self::Rank(_) => PMIX_PROC_RANK,
            Self::Persist(_) => PMIX_PERSIST,
            Self::Scope(_) => PMIX_SCOPE,
            Self::DataRange(_) => PMIX_DATA_RANGE,
            Self::ProcState(_) => PMIX_PROC_STATE,
            Self::AllocDirective(_) => PMIX_ALLOC_DIRECTIVE,
            Self::IofChannel(_) => PMIX_IOF_CHANNEL,
            Self::InfoDirectives(_) => PMIX_INFO_DIRECTIVES,
            Self::Proc(_) => PMIX_PROC,
            Self::ByteObject(_) => PMIX_BYTE_OBJECT,
            Self::Envar(_) => PMIX_ENVAR,
            Self::Pointer(_) => PMIX_POINTER,
            Self::DataArray { .. } => PMIX_DATA_ARRAY,
        }) as pmix_data_type_t
    }
}
// ─────────────────────────────────────────────────────────────────────────────
// PmixValueBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Fluent builder for `pmix_value_t`.
///
/// Call one of the typed setter methods, then call [`build`][Self::build] for
/// a [`PmixOwnedValue`] (RAII) or [`build_raw`][Self::build_raw] for a raw
/// `pmix_value_t` whose heap data the caller must free manually.
///
/// # Example
/// ```
/// use pmix::{PmixValueBuilder, ValueError};
///
/// let owned = PmixValueBuilder::new().uint32(42).build().expect("build");
/// drop(owned);
/// ```
#[derive(Default)]
pub struct PmixValueBuilder {
    payload: Option<PmixPayload>,
}

impl PmixValueBuilder {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new() -> Self {
        Self::default()
    }

    // ── Generic payload setter ────────────────────────────────────────────

    /// Set the payload to any [`PmixPayload`] variant.
    pub fn payload(mut self, p: PmixPayload) -> Self {
        self.payload = Some(p);
        self
    }

    // ── Typed scalar setters ──────────────────────────────────────────────

    pub fn undef(self) -> Self {
        self.payload(PmixPayload::Undef)
    }
    pub fn bool(self, v: bool) -> Self {
        self.payload(PmixPayload::Bool(v))
    }
    pub fn byte(self, v: u8) -> Self {
        self.payload(PmixPayload::Byte(v))
    }
    pub fn size(self, v: usize) -> Self {
        self.payload(PmixPayload::Size(v))
    }
    pub fn pid(self, v: u32) -> Self {
        self.payload(PmixPayload::Pid(v))
    }
    pub fn int(self, v: i32) -> Self {
        self.payload(PmixPayload::Int(v))
    }
    pub fn int8(self, v: i8) -> Self {
        self.payload(PmixPayload::Int8(v))
    }
    pub fn int16(self, v: i16) -> Self {
        self.payload(PmixPayload::Int16(v))
    }
    pub fn int32(self, v: i32) -> Self {
        self.payload(PmixPayload::Int32(v))
    }
    pub fn int64(self, v: i64) -> Self {
        self.payload(PmixPayload::Int64(v))
    }
    pub fn uint(self, v: u32) -> Self {
        self.payload(PmixPayload::Uint(v))
    }
    pub fn uint8(self, v: u8) -> Self {
        self.payload(PmixPayload::Uint8(v))
    }
    pub fn uint16(self, v: u16) -> Self {
        self.payload(PmixPayload::Uint16(v))
    }
    pub fn uint32(self, v: u32) -> Self {
        self.payload(PmixPayload::Uint32(v))
    }
    pub fn uint64(self, v: u64) -> Self {
        self.payload(PmixPayload::Uint64(v))
    }
    pub fn float(self, v: f32) -> Self {
        self.payload(PmixPayload::Float(v))
    }
    pub fn double(self, v: f64) -> Self {
        self.payload(PmixPayload::Double(v))
    }
    pub fn timeval(self, v: PmixTimeval) -> Self {
        self.payload(PmixPayload::Timeval(v))
    }
    pub fn status(self, v: pmix_status_t) -> Self {
        self.payload(PmixPayload::Status(v))
    }
    pub fn rank(self, v: pmix_rank_t) -> Self {
        self.payload(PmixPayload::Rank(v))
    }
    pub fn persist(self, v: pmix_persistence_t) -> Self {
        self.payload(PmixPayload::Persist(v))
    }
    pub fn scope(self, v: pmix_scope_t) -> Self {
        self.payload(PmixPayload::Scope(v))
    }
    pub fn data_range(self, v: pmix_data_range_t) -> Self {
        self.payload(PmixPayload::DataRange(v))
    }
    pub fn proc_state(self, v: pmix_proc_state_t) -> Self {
        self.payload(PmixPayload::ProcState(v))
    }
    pub fn alloc_directive(self, v: pmix_alloc_directive_t) -> Self {
        self.payload(PmixPayload::AllocDirective(v))
    }
    pub fn iof_channel(self, v: pmix_iof_channel_t) -> Self {
        self.payload(PmixPayload::IofChannel(v))
    }
    pub fn info_directives(self, v: pmix_info_directives_t) -> Self {
        self.payload(PmixPayload::InfoDirectives(v))
    }

    // ── Typed heap setters ────────────────────────────────────────────────

    /// Set a NUL-terminated string payload (`PMIX_STRING`).
    ///
    /// Returns `Err(ValueError::ContainsNul)` if `s` contains an interior NUL.
    pub fn string(self, s: &str) -> Result<Self, ValueError> {
        Ok(self.payload(PmixPayload::String(CString::new(s)?)))
    }

    /// Set a `pmix_proc_t` payload (`PMIX_PROC`).
    pub fn proc_(self, p: Proc) -> Self {
        self.payload(PmixPayload::Proc(p))
    }

    /// Set a raw byte-buffer payload (`PMIX_BYTE_OBJECT`).
    ///
    /// Returns `Err(ValueError::EmptyData)` for a zero-length slice.
    pub fn byte_object(self, bytes: &[u8]) -> Result<Self, ValueError> {
        if bytes.is_empty() {
            return Err(ValueError::EmptyData);
        }
        Ok(self.payload(PmixPayload::ByteObject(bytes.to_vec())))
    }

    /// Set an environment-variable modification payload (`PMIX_ENVAR`).
    pub fn envar(self, e: PmixEnvar) -> Self {
        self.payload(PmixPayload::Envar(e))
    }

    /// Set an opaque pointer payload (`PMIX_POINTER`).
    ///
    /// # Safety
    /// The caller must ensure the pointed-to data outlives the resulting
    /// `pmix_value_t` and is freed independently.
    pub unsafe fn pointer(self, p: *mut std::ffi::c_void) -> Self {
        self.payload(PmixPayload::Pointer(p))
    }

    /// Set a heterogeneous `pmix_data_array_t` payload (`PMIX_DATA_ARRAY`).
    ///
    /// `elem_type` records the declared element type (use `PMIX_VALUE` /
    /// `PMIX_UNDEF` for a mixed array).  Every element must already be a
    /// fully-initialised `pmix_value_t`; use other builders to produce them.
    ///
    /// Returns `Err(ValueError::EmptyData)` for an empty slice.
    pub fn data_array(
        self,
        elem_type: pmix_data_type_t,
        elements: Vec<pmix_value_t>,
    ) -> Result<Self, ValueError> {
        if elements.is_empty() {
            return Err(ValueError::EmptyData);
        }
        Ok(self.payload(PmixPayload::DataArray {
            elem_type,
            elements,
        }))
    }

    // ── Static constructor: string_array ─────────────────────────────────
    //
    // This is the primary place where CStringArray is used: when a caller
    // has a list of string values (e.g. query keys, regex patterns) it wants
    // to pack into a PMIX_DATA_ARRAY of PMIX_STRING elements AND simultaneously
    // hold a char** view for PMIx list APIs.

    /// Build a `PMIX_DATA_ARRAY` of `PMIX_STRING` elements from a slice of
    /// `&str` values.
    ///
    /// Returns:
    /// - A [`PmixOwnedValue`] wrapping the resulting `pmix_value_t`.
    /// - A [`CStringArray`] whose `*const *const c_char` pointer covers the
    ///   same strings in the same order, for passing to PMIx list APIs that
    ///   accept `char**`.
    ///
    /// Both objects independently own their strings; the `CStringArray` is
    /// a separate copy intended for the C API call's lifetime only.
    ///
    /// ```
    /// use pmix::PmixValueBuilder;
    ///
    /// let (val, keys) = PmixValueBuilder::string_array(&["pmix.timeout", "pmix.collect"]).expect("build");
    /// let _pp: *const *const std::ffi::c_char = keys.as_ptr();
    /// ```
    pub fn string_array(strings: &[&str]) -> Result<(PmixOwnedValue, CStringArray), ValueError> {
        if strings.is_empty() {
            return Err(ValueError::EmptyData);
        }

        // Build the pmix_value_t elements (each a PMIX_STRING).
        let elements: Result<Vec<pmix_value_t>, ValueError> = strings
            .iter()
            .map(|s| PmixValueBuilder::new().string(s)?.build_raw())
            .collect();

        let owned = PmixValueBuilder::new()
            .data_array(PMIX_STRING as pmix_data_type_t, elements?)?
            .build()?;

        // Build CStringArray from the same strings. CStringArray owns its own
        // CString copies, so the two lifetimes are fully independent.
        let cstrings: Result<Vec<CString>, NulError> =
            strings.iter().map(|s| CString::new(*s)).collect();
        let key_array =
            CStringArray::from_cstrings(cstrings?).expect("strings are already validated CStrings");

        Ok((owned, key_array))
    }

    // ── Build ─────────────────────────────────────────────────────────────

    /// Validate and return a [`PmixOwnedValue`] (RAII — heap data freed on
    /// `Drop`).
    pub fn build(self) -> Result<PmixOwnedValue, ValueError> {
        Ok(PmixOwnedValue {
            inner: self.build_raw()?,
        })
    }

    /// Validate and write directly into a `pmix_value_t`.
    ///
    /// **Ownership note:** any heap allocations (string, proc, envar,
    /// byte-object, data-array) are now owned by the returned struct.  The
    /// caller must call [`free_value`] or use [`build`][Self::build] instead.
    pub fn build_raw(self) -> Result<pmix_value_t, ValueError> {
        let payload = self.payload.ok_or(ValueError::MissingPayload)?;
        // SAFETY: zeroed gives a valid starting state for every scalar field
        // and every pointer field (NULL).
        let mut v: pmix_value_t = unsafe { std::mem::zeroed() };
        v.type_ = payload.type_tag();
        write_payload(&mut v, payload);
        Ok(v)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// write_payload – transfer PmixPayload into a pmix_val_data union
// ─────────────────────────────────────────────────────────────────────────────

/// Write `payload` into `dst`.
///
/// For heap variants (String, Proc, ByteObject, Envar, DataArray) ownership
/// is transferred; the caller is responsible for freeing via [`free_value`].
fn write_payload(dst: &mut pmix_value, payload: PmixPayload) {
    //let &mut dst = &mut dst_in.data;
    // SAFETY: caller has already set v.type_; we write exactly the matching
    // union arm. All other arms remain as zeroed bytes from `mem::zeroed`.
    unsafe {
        match payload {
            PmixPayload::Undef => { /* data stays zeroed */ }
            PmixPayload::Bool(v) => dst.data.flag = v,
            PmixPayload::Byte(v) => dst.data.byte = v,
            PmixPayload::Size(v) => dst.data.size = v,
            PmixPayload::Pid(v) => dst.data.pid = v as pid_t,
            PmixPayload::Int(v) => dst.data.integer = v,
            PmixPayload::Int8(v) => dst.data.int8 = v,
            PmixPayload::Int16(v) => dst.data.int16 = v,
            PmixPayload::Int32(v) => dst.data.int32 = v,
            PmixPayload::Int64(v) => dst.data.int64 = v,
            PmixPayload::Uint(v) => dst.data.uint = v,
            PmixPayload::Uint8(v) => dst.data.uint8 = v,
            PmixPayload::Uint16(v) => dst.data.uint16 = v,
            PmixPayload::Uint32(v) => dst.data.uint32 = v,
            PmixPayload::Uint64(v) => dst.data.uint64 = v,
            PmixPayload::Float(v) => dst.data.fval = v,
            PmixPayload::Double(v) => dst.data.dval = v,
            PmixPayload::Timeval(v) => dst.data.tv = v.into(),
            PmixPayload::Status(v) => dst.data.status = v,
            PmixPayload::Rank(v) => dst.data.rank = v,
            PmixPayload::Persist(v) => dst.data.persist = v,
            PmixPayload::Scope(v) => dst.data.scope = v,
            PmixPayload::DataRange(v) => dst.data.range = v,
            PmixPayload::ProcState(v) => dst.data.state = v,
            PmixPayload::AllocDirective(v) => dst.data.adir = v,
            PmixPayload::IofChannel(v) => dst.data.uint16 = v,
            PmixPayload::InfoDirectives(v) => dst.data.uint32 = v,

            // Transfer CString → *mut c_char (null-termination included).
            PmixPayload::String(cs) => {
                dst.data.string = cs.into_raw();
            }

            // Heap-allocate pmix_proc_t, fill nspace (fixed char array), set rank.
            PmixPayload::Proc(p) => {
                let mut raw_proc: pmix_proc_t = std::mem::zeroed();
                let copy_len = p.handle.nspace.len().min(raw_proc.nspace.len());
                ptr::copy_nonoverlapping(
                    p.handle.nspace.as_ptr() as *const c_void,
                    raw_proc.nspace.as_mut_ptr() as *mut c_void,
                    copy_len,
                );
                raw_proc.rank = p.handle.rank;
                dst.data.proc_ = Box::into_raw(Box::new(raw_proc));
            }

            // Leak Vec<u8> into pmix_byte_object_t.
            PmixPayload::ByteObject(mut bytes) => {
                bytes.shrink_to_fit();
                let len = bytes.len();
                let ptr = bytes.as_mut_ptr() as *mut i8;
                std::mem::forget(bytes);
                dst.data.bo = pmix_byte_object_t {
                    bytes: ptr,
                    size: len,
                };
            }

            // Heap-allocate pmix_envar_t; transfer both CStrings.
            PmixPayload::Envar(e) => {
                let raw = Box::new(pmix_envar_t {
                    envar: e.envar.into_raw(),
                    value: e.value.into_raw(),
                    separator: e.separator as i8,
                });
                dst.data.envar = *Box::into_raw(raw);
            }

            // Opaque pointer – no allocation here, caller owns data.
            PmixPayload::Pointer(p) => {
                dst.data.ptr = p;
            }

            // Leak Vec<pmix_value_t> into a heap-allocated pmix_data_array_t.
            PmixPayload::DataArray {
                elem_type,
                mut elements,
            } => {
                elements.shrink_to_fit();
                let len = elements.len();
                let arr = elements.as_mut_ptr() as *mut std::ffi::c_void;
                std::mem::forget(elements);
                let darray = Box::new(pmix_data_array_t {
                    type_: elem_type,
                    size: len,
                    array: arr,
                });
                dst.data.darray = Box::into_raw(darray);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// free_value – release heap data inside a pmix_value_t
// ─────────────────────────────────────────────────────────────────────────────

/// Free any heap-allocated data inside `v` that was created by this builder.
///
/// After this call `v` is zeroed and safe to drop or reuse.
///
/// # Safety
/// `v` must have been produced by this crate's builder (or have equivalent
/// allocation discipline).  Calling this on a `pmix_value_t` produced by the C
/// library (and therefore managed by `PMIX_VALUE_RELEASE`) is a double-free.
pub fn free_value(v: &mut pmix_value_t) {
    // SAFETY: type_ was set by write_payload; we access only the matching arm.
    unsafe {
        match v.type_ as u32 {
            t if t == PMIX_STRING => {
                if !v.data.string.is_null() {
                    drop(CString::from_raw(v.data.string));
                    v.data.string = ptr::null_mut();
                }
            }
            t if t == PMIX_PROC => {
                if !v.data.proc_.is_null() {
                    drop(Box::from_raw(v.data.proc_));
                    v.data.proc_ = ptr::null_mut();
                }
            }
            t if t == PMIX_BYTE_OBJECT => {
                if !v.data.bo.bytes.is_null() && v.data.bo.size > 0 {
                    let _ = Vec::from_raw_parts(
                        v.data.bo.bytes as *mut u8,
                        v.data.bo.size,
                        v.data.bo.size,
                    );
                    v.data.bo.bytes = ptr::null_mut();
                    v.data.bo.size = 0;
                }
            }
            t if t == PMIX_ENVAR => {
                // envar is embedded in the union (not heap-allocated).
                // Only free the CString pointers inside it.
                if !v.data.envar.envar.is_null() {
                    drop(CString::from_raw(v.data.envar.envar));
                    v.data.envar.envar = ptr::null_mut();
                }
                if !v.data.envar.value.is_null() {
                    drop(CString::from_raw(v.data.envar.value));
                    v.data.envar.value = ptr::null_mut();
                }
            }
            t if t == PMIX_DATA_ARRAY && !v.data.darray.is_null() => {
                let darray = Box::from_raw(v.data.darray);
                if !darray.array.is_null() && darray.size > 0 {
                    // Reconstruct the Vec to let each element's Drop run.
                    let elements = Vec::from_raw_parts(
                        darray.array as *mut pmix_value_t,
                        darray.size,
                        darray.size,
                    );
                    // Each element may itself own heap data; free recursively.
                    for mut elem in elements {
                        free_value(&mut elem);
                    }
                }
                v.data.darray = ptr::null_mut();
            }
            // PMIX_POINTER – caller owns; we never free it.
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixOwnedValue – RAII wrapper with automatic cleanup
// ─────────────────────────────────────────────────────────────────────────────

/// Owns a `pmix_value_t` and calls [`free_value`] when dropped.
///
/// Use [`as_raw`][Self::as_raw] / [`as_raw_mut`][Self::as_raw_mut] to borrow
/// the struct for FFI calls, or [`into_raw`][Self::into_raw] to transfer full
/// ownership to a C API that calls `PMIX_VALUE_RELEASE` itself.
pub struct PmixOwnedValue {
    inner: pmix_value_t,
}

impl PmixOwnedValue {
    /// Immutable raw pointer.
    pub fn as_raw(&self) -> *const pmix_value_t {
        &self.inner
    }

    /// Mutable raw pointer.
    pub fn as_raw_mut(&mut self) -> *mut pmix_value_t {
        &mut self.inner
    }

    /// Return the `pmix_data_type_t` tag.
    pub fn type_tag(&self) -> pmix_data_type_t {
        self.inner.type_
    }

    /// Transfer ownership out of RAII; caller must free via `PMIX_VALUE_RELEASE`
    /// or [`free_value`].
    pub fn into_raw(self) -> pmix_value_t {
        let inner = self.inner;
        std::mem::forget(self);
        inner
    }

    pub fn bytes(&self) -> (*const c_void, usize) {
        let bytes = unsafe { self.inner.data.bo }; //.bytes.cast_const() as *const c_void
        (bytes.bytes.cast_const() as *const c_void, bytes.size)
    }

    pub fn size(&self) -> usize {
        unsafe { self.inner.data.size }
    }

    /// Read the value as a u32.
    pub fn uint32(&self) -> u32 {
        unsafe { self.inner.data.uint32 }
    }

    /// Read the value as a u64.
    pub fn uint64(&self) -> u64 {
        unsafe { self.inner.data.uint64 }
    }

    /// Read the byte object as a Vec<u8> copy.
    pub fn bytes_copy(&self) -> Vec<u8> {
        let (ptr, len) = self.bytes();
        if ptr.is_null() || len == 0 {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len).to_vec() }
    }
}

impl Drop for PmixOwnedValue {
    fn drop(&mut self) {
        free_value(&mut self.inner);
    }
}

impl std::fmt::Debug for PmixOwnedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixOwnedValue")
            .field("type_", &self.inner.type_)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Proc {
    pub(crate) handle: pmix_proc_t,
    len: usize,
}

pub struct Info {
    handle: *mut pmix_info_t,
    len: usize,
}

impl Info {
    /// Number of entries in this info array.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw pointer for FFI (valid for `len()` elements).
    pub fn as_ptr(&self) -> *mut pmix_info_t {
        self.handle
    }
}

struct InfoEntry {
    key: &'static [u8; 13],
    value: *const std::ffi::c_void,
    data_type: pmix_data_type_t,
}

#[derive(Default)]
pub struct InfoBuilder {
    infos: Vec<InfoEntry>,
}

impl InfoBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(
        &mut self,
        key: &'static [u8; 13],
        value: *const std::ffi::c_void,
        data_type: pmix_data_type_t,
    ) {
        assert_ne!(key.as_ptr(), std::ptr::null());
        self.infos.push(InfoEntry {
            key,
            value,
            data_type,
        })
    }
    pub fn collect_data(&mut self) -> &mut InfoBuilder {
        let collect = true;
        self.add(
            PMIX_COLLECT_DATA,
            &collect as *const bool as *const c_void,
            PMIX_BOOL as pmix_data_type_t,
        );
        self
    }
    pub fn build(self) -> Info {
        let info_ptr: *mut pmix_info_t;
        let mut idx: usize = 0;
        unsafe { info_ptr = PMIx_Info_create(self.infos.len()) }
        for info in &self.infos {
            let status = unsafe {
                PMIx_Info_load(
                    info_ptr.add(idx),
                    info.key.as_ptr().cast(),
                    info.value,
                    info.data_type,
                )
            };
            if status != PMIX_SUCCESS as i32 {
                panic!("Error loading info: {}", status);
            }
            idx += 1;
        }
        Info {
            handle: info_ptr,
            len: idx,
        }
    }
}

/// Create an `Info` array with a single string key/value pair, bypassing
/// the 13-byte key limit of `InfoBuilder::add()`.
///
/// This is useful for keys like `"pmix.srvr.uri"` (14 bytes) which don't
/// fit in `InfoBuilder::add(key: &'static [u8; 13])`.
pub fn info_with_string_key(key: &str, value: &str) -> Info {
    let info_ptr = unsafe { PMIx_Info_create(1) };
    let key_cstr = CString::new(key).expect("key must not contain null bytes");
    let value_cstr = CString::new(value).expect("value must not contain null bytes");
    unsafe {
        let status = PMIx_Info_load(
            info_ptr,
            key_cstr.as_ptr(),
            value_cstr.as_ptr() as *const c_void,
            PMIX_STRING as pmix_data_type_t,
        );
        if status != PMIX_SUCCESS as i32 {
            panic!("PMIx_Info_load failed for key {}: {}", key, status);
        }
    }
    // Leak the CString allocations — the PMIx library copies the data
    // internally and the Info handle is managed by the library.
    std::mem::forget(key_cstr);
    std::mem::forget(value_cstr);
    Info {
        handle: info_ptr,
        len: 1,
    }
}

pub struct Context {
    pub(crate) proc: Proc,
}

impl Context {
    pub fn proc_with_nspace(&self, rank: u32) -> Result<Proc, NulError> {
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        unsafe {
            PMIx_Load_nspace(handle.nspace.as_mut_ptr(), self.proc.handle.nspace.as_ptr());
        }
        Ok(Proc { handle, len: 1 })
    }

    pub fn get_rank(&self) -> u32 {
        self.proc.handle.rank
    }
    pub fn get_proc(&self) -> &Proc {
        &self.proc
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // Always log finalize failures (including release builds). This runs at
        // end-of-scope / process teardown where diagnostics matter more than
        // avoiding a single eprintln. Never panic in Drop (double-panic → abort).
        if let Err(status) = finalize(None) {
            eprintln!("pmix: finalize in Context::Drop failed: status={status}");
        }
    }
}

pub fn init(info: Option<Info>) -> Result<Context, PmixError> {
    let proc: pmix_proc_t;
    let mut uninit_proc = mem::MaybeUninit::<pmix_proc_t>::uninit();
    let status = match info {
        Some(info) => unsafe { PMIx_Init(uninit_proc.as_mut_ptr(), info.handle, info.len) },
        None => unsafe { PMIx_Init(uninit_proc.as_mut_ptr(), ptr::null_mut(), 0) },
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        unsafe {
            proc = uninit_proc.assume_init();
        }
        Ok(Context {
            proc: Proc {
                handle: proc,
                len: 1,
            },
        })
    } else {
        if let Some(known) = pmix_status.known() {
            Err(known)
        } else {
            Err(PmixError::Error)
        }
    }
}

pub fn get_value(proc: &Proc, key: &[u8], info: Option<Info>) -> Result<PmixOwnedValue, PmixError> {
    let status: PmixStatus;
    let mut value: *mut pmix_value_t = null_mut();
    let info_handle: *const pmix_info_t;
    let ninfos: usize;

    match info {
        Some(info) => {
            info_handle = info.handle;
            if info_handle.is_null() {
                ninfos = 0;
            } else {
                ninfos = info.len;
            }
        }
        None => {
            info_handle = null();
            ninfos = 0;
        }
    }

    unsafe {
        status = PmixStatus::from_raw(PMIx_Get(
            &proc.handle,
            CStr::from_bytes_with_nul(key).unwrap().as_ptr(),
            info_handle,
            ninfos,
            &mut value,
        ));
    }

    if status.is_success() {
        Ok(PmixOwnedValue {
            inner: unsafe { *value },
        })
    } else {
        if let Some(known) = status.known() {
            Err(known)
        } else {
            Err(PmixError::Error)
        }
    }
}

pub fn put_value(
    scope: pmix_scope_t,
    key: &CStr,
    value: &mut PmixOwnedValue,
) -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe {
        status = PMIx_Put(scope, key.as_ptr(), &mut value.inner);
    }
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn commit() -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe {
        status = PMIx_Commit();
    }
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn fence(proc: &Proc, info: Option<Info>) -> Result<(), pmix_status_t> {
    let proc_handle: *const pmix_proc_t = &proc.handle;
    let nprocs = if proc_handle.is_null() { 0 } else { proc.len };
    let (info_handle, ninfos) = match info {
        Some(info) => {
            let ih = info.handle;
            let ni = if proc_handle.is_null() { 0 } else { info.len };
            (ih, ni)
        }
        None => (ptr::null_mut(), 0),
    };

    let status = unsafe { PMIx_Fence(proc_handle, nprocs, info_handle, ninfos) };
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn get_version() -> &'static str {
    let version: &CStr;
    unsafe {
        version = CStr::from_ptr(PMIx_Get_version());
    }
    version.to_str().unwrap()
}
pub fn progress() {
    unsafe {
        PMIx_Progress();
    }
}

pub fn finalize(info: Option<Info>) -> Result<(), pmix_status_t> {
    let status = match info {
        Some(x) => unsafe { PMIx_Finalize(x.handle, x.len) },
        None => unsafe { PMIx_Finalize(ptr::null_mut(), 0) },
    };
    if status as u32 == PMIX_SUCCESS {
        Result::Ok(())
    } else {
        Result::Err(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────────────────────────────
    // get_version
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_get_version() {
        let ver = super::get_version();
        assert!(!ver.is_empty(), "PMIx version string should not be empty");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixError — from_raw / to_raw roundtrip and properties
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_error_from_raw_known_values() {
        assert_eq!(PmixError::from_raw(0), Some(PmixError::Success));
        assert_eq!(PmixError::from_raw(-1), Some(PmixError::Error));
        assert_eq!(PmixError::from_raw(-31), Some(PmixError::ErrInit));
        assert_eq!(PmixError::from_raw(-32), Some(PmixError::ErrNomem));
        assert_eq!(PmixError::from_raw(-46), Some(PmixError::ErrNotFound));
        assert_eq!(PmixError::from_raw(-47), Some(PmixError::ErrNotSupported));
        assert_eq!(PmixError::from_raw(-24), Some(PmixError::ErrTimeout));
        assert_eq!(PmixError::from_raw(-25), Some(PmixError::ErrUnreach));
        assert_eq!(PmixError::from_raw(-27), Some(PmixError::ErrBadParam));
        assert_eq!(
            PmixError::from_raw(-171),
            Some(PmixError::ErrRepeatAttrRegistration)
        );
        assert_eq!(PmixError::from_raw(-172), Some(PmixError::ErrIofFailure));
        assert_eq!(PmixError::from_raw(-180), Some(PmixError::ErrJobCanceled));
        assert_eq!(
            PmixError::from_raw(-401),
            Some(PmixError::ErrProcFailedToStart)
        );
        assert_eq!(PmixError::from_raw(-3000), Some(PmixError::ExternalErrBase));
    }

    #[test]
    fn test_pmix_error_from_raw_unknown_returns_none() {
        assert_eq!(PmixError::from_raw(-9999), None);
        assert_eq!(PmixError::from_raw(-99999), None);
        assert_eq!(PmixError::from_raw(42), None);
        assert_eq!(PmixError::from_raw(1), None);
    }

    #[test]
    fn test_pmix_error_to_raw_roundtrip() {
        let errors: &[PmixError] = &[
            PmixError::Success,
            PmixError::Error,
            PmixError::ErrInit,
            PmixError::ErrNomem,
            PmixError::ErrNotFound,
            PmixError::ErrNotSupported,
            PmixError::ErrTimeout,
            PmixError::ErrUnreach,
            PmixError::ErrBadParam,
            PmixError::ErrJobCanceled,
            PmixError::ErrProcFailedToStart,
            PmixError::ExternalErrBase,
        ];
        for err in errors {
            let raw = err.to_raw();
            assert_eq!(
                PmixError::from_raw(raw),
                Some(*err),
                "roundtrip failed for {:?}",
                err
            );
        }
    }

    #[test]
    fn test_pmix_error_is_success() {
        assert!(PmixError::Success.is_success());
        assert!(!PmixError::Error.is_success());
        assert!(!PmixError::ErrInit.is_success());
        assert!(!PmixError::ErrNotFound.is_success());
    }

    #[test]
    fn test_pmix_error_is_error() {
        assert!(!PmixError::Success.is_error());
        assert!(PmixError::Error.is_error());
        assert!(PmixError::ErrInit.is_error());
        assert!(PmixError::ErrNotFound.is_error());
    }

    #[test]
    fn test_pmix_error_name_returns_valid_strings() {
        assert_eq!(PmixError::Success.name(), "PMIX_SUCCESS");
        assert_eq!(PmixError::Error.name(), "PMIX_ERROR");
        assert_eq!(PmixError::ErrInit.name(), "PMIX_ERR_INIT");
        assert_eq!(PmixError::ErrNomem.name(), "PMIX_ERR_NOMEM");
        assert_eq!(PmixError::ErrNotFound.name(), "PMIX_ERR_NOT_FOUND");
        assert_eq!(PmixError::ErrJobCanceled.name(), "PMIX_ERR_JOB_CANCELED");
        assert_eq!(
            PmixError::ErrProcFailedToStart.name(),
            "PMIX_ERR_PROC_FAILED_TO_START"
        );
        assert_eq!(PmixError::ExternalErrBase.name(), "PMIX_EXTERNAL_ERR_BASE");
        assert_eq!(PmixError::ErrTimeout.name(), "PMIX_ERR_TIMEOUT");
        assert_eq!(PmixError::ErrUnreach.name(), "PMIX_ERR_UNREACH");
    }

    #[test]
    fn test_pmix_error_derives() {
        let a = PmixError::Success;
        let b = PmixError::Success;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixError> =
            [a, PmixError::Error].iter().cloned().collect();
        let _debug = format!("{:?}", a);
        assert!(!_debug.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixStatus — from_raw / to_raw / is_success / is_error / known
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_status_from_raw_known() {
        assert_eq!(
            PmixStatus::from_raw(0),
            PmixStatus::Known(PmixError::Success)
        );
        assert_eq!(
            PmixStatus::from_raw(-1),
            PmixStatus::Known(PmixError::Error)
        );
        assert_eq!(
            PmixStatus::from_raw(-31),
            PmixStatus::Known(PmixError::ErrInit)
        );
        assert_eq!(
            PmixStatus::from_raw(-46),
            PmixStatus::Known(PmixError::ErrNotFound)
        );
    }

    #[test]
    fn test_pmix_status_from_raw_unknown() {
        let status = PmixStatus::from_raw(-99999);
        assert!(matches!(status, PmixStatus::Unknown(_)));
        if let PmixStatus::Unknown(v) = status {
            assert_eq!(v, -99999);
        }
    }

    #[test]
    fn test_pmix_status_to_raw() {
        assert_eq!(PmixStatus::Known(PmixError::Success).to_raw(), 0);
        assert_eq!(PmixStatus::Known(PmixError::Error).to_raw(), -1);
        assert_eq!(PmixStatus::Known(PmixError::ErrInit).to_raw(), -31);
        assert_eq!(PmixStatus::Unknown(-99999).to_raw(), -99999);
    }

    #[test]
    fn test_pmix_status_is_success() {
        assert!(PmixStatus::Known(PmixError::Success).is_success());
        assert!(!PmixStatus::Known(PmixError::Error).is_success());
        assert!(!PmixStatus::Known(PmixError::ErrInit).is_success());
        assert!(PmixStatus::Unknown(42).is_success());
        assert!(!PmixStatus::Unknown(-42).is_success());
    }

    #[test]
    fn test_pmix_status_is_error() {
        assert!(!PmixStatus::Known(PmixError::Success).is_error());
        assert!(PmixStatus::Known(PmixError::Error).is_error());
        assert!(PmixStatus::Known(PmixError::ErrInit).is_error());
        assert!(!PmixStatus::Unknown(42).is_error());
        assert!(PmixStatus::Unknown(-42).is_error());
    }

    #[test]
    fn test_pmix_status_known() {
        assert!(PmixStatus::Known(PmixError::Success).known().is_some());
        assert_eq!(
            PmixStatus::Known(PmixError::Success).known(),
            Some(PmixError::Success)
        );
        assert!(PmixStatus::Unknown(-99999).known().is_none());
    }

    #[test]
    fn test_pmix_status_display() {
        let s = format!("{}", PmixStatus::Known(PmixError::Success));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Unknown(-99999));
        assert!(s.contains("unknown"));
    }

    #[test]
    fn test_pmix_status_from_pmix_error() {
        let status: PmixStatus = PmixError::ErrInit.into();
        assert_eq!(status, PmixStatus::Known(PmixError::ErrInit));
    }

    #[test]
    fn test_pmix_status_derives() {
        let a = PmixStatus::Known(PmixError::Success);
        let b = PmixStatus::Known(PmixError::Success);
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _debug = format!("{:?}", a);
        assert!(!_debug.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixProcState — from_raw / to_raw / is_alive / is_terminated
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_proc_state_from_raw_known() {
        assert_eq!(PmixProcState::from_raw(0), PmixProcState::Undef);
        assert_eq!(PmixProcState::from_raw(5), PmixProcState::Running);
        assert_eq!(PmixProcState::from_raw(15), PmixProcState::Unterminated);
        assert_eq!(PmixProcState::from_raw(20), PmixProcState::Terminated);
        assert_eq!(PmixProcState::from_raw(50), PmixProcState::Error);
        assert_eq!(PmixProcState::from_raw(51), PmixProcState::KilledByCmd);
        assert_eq!(PmixProcState::from_raw(52), PmixProcState::Aborted);
        assert_eq!(PmixProcState::from_raw(53), PmixProcState::FailedToStart);
    }

    #[test]
    fn test_pmix_proc_state_from_raw_unknown() {
        assert!(matches!(
            PmixProcState::from_raw(99),
            PmixProcState::Unknown(99)
        ));
    }

    #[test]
    fn test_pmix_proc_state_to_raw_roundtrip() {
        let states: &[PmixProcState] = &[
            PmixProcState::Undef,
            PmixProcState::Running,
            PmixProcState::Unterminated,
            PmixProcState::Terminated,
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
            let raw = state.to_raw();
            assert_eq!(
                PmixProcState::from_raw(raw),
                *state,
                "roundtrip failed for {:?}",
                state
            );
        }
    }

    #[test]
    fn test_pmix_proc_state_is_alive() {
        assert!(PmixProcState::Running.is_alive());
        assert!(PmixProcState::Unterminated.is_alive());
        assert!(PmixProcState::Connected.is_alive());
        assert!(!PmixProcState::Terminated.is_alive());
        assert!(!PmixProcState::Error.is_alive());
        assert!(!PmixProcState::Aborted.is_alive());
        assert!(!PmixProcState::FailedToStart.is_alive());
    }

    #[test]
    fn test_pmix_proc_state_is_terminated() {
        assert!(PmixProcState::Terminated.is_terminated());
        assert!(PmixProcState::Aborted.is_terminated());
        assert!(PmixProcState::FailedToStart.is_terminated());
        assert!(PmixProcState::KilledByCmd.is_terminated());
        assert!(PmixProcState::AbortedBySig.is_terminated());
        assert!(PmixProcState::TermWoSync.is_terminated());
        assert!(PmixProcState::CommFailed.is_terminated());
        assert!(PmixProcState::SensorBoundExceeded.is_terminated());
        assert!(PmixProcState::CalledAbort.is_terminated());
        assert!(PmixProcState::HeartbeatFailed.is_terminated());
        assert!(PmixProcState::CannotRestart.is_terminated());
        assert!(PmixProcState::TermNonZero.is_terminated());
        assert!(PmixProcState::FailedToLaunch.is_terminated());
        assert!(!PmixProcState::Error.is_terminated());
        assert!(!PmixProcState::Running.is_terminated());
        assert!(!PmixProcState::Unterminated.is_terminated());
        assert!(!PmixProcState::Undef.is_terminated());
        assert!(!PmixProcState::Migrating.is_terminated());
    }

    #[test]
    fn test_pmix_proc_state_display() {
        let s = format!("{}", PmixProcState::Running);
        assert!(!s.is_empty());
        let s = format!("{}", PmixProcState::Terminated);
        assert!(!s.is_empty());
        let s = format!("{}", PmixProcState::Unknown(99));
        assert!(s.contains("99"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixScope — from_raw / to_raw / display (Local/Remote/Global/Internal)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_scope_from_raw_known() {
        assert_eq!(PmixScope::from_raw(0), PmixScope::Undef);
        assert_eq!(PmixScope::from_raw(1), PmixScope::Local);
        assert_eq!(PmixScope::from_raw(2), PmixScope::Remote);
        assert_eq!(PmixScope::from_raw(3), PmixScope::Global);
        assert_eq!(PmixScope::from_raw(4), PmixScope::Internal);
    }

    #[test]
    fn test_pmix_scope_from_raw_unknown() {
        assert!(matches!(PmixScope::from_raw(99), PmixScope::Unknown(99)));
    }

    #[test]
    fn test_pmix_scope_to_raw_roundtrip() {
        let scopes: &[PmixScope] = &[
            PmixScope::Undef,
            PmixScope::Local,
            PmixScope::Remote,
            PmixScope::Global,
            PmixScope::Internal,
        ];
        for scope in scopes {
            let raw = scope.to_raw();
            assert_eq!(
                PmixScope::from_raw(raw),
                *scope,
                "roundtrip failed for {:?}",
                scope
            );
        }
    }

    #[test]
    fn test_pmix_scope_display() {
        assert!(!format!("{}", PmixScope::Global).is_empty());
        assert!(!format!("{}", PmixScope::Local).is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixJobState — from_raw / to_raw / display
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_job_state_from_raw_known() {
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
    }

    #[test]
    fn test_pmix_job_state_from_raw_unknown() {
        assert!(matches!(
            PmixJobState::from_raw(99),
            PmixJobState::Unknown(99)
        ));
    }

    #[test]
    fn test_pmix_job_state_to_raw_roundtrip() {
        let states: &[PmixJobState] = &[
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
        for state in states {
            let raw = state.to_raw();
            assert_eq!(
                PmixJobState::from_raw(raw),
                *state,
                "roundtrip failed for {:?}",
                state
            );
        }
    }

    #[test]
    fn test_pmix_job_state_display() {
        assert!(!format!("{}", PmixJobState::Running).is_empty());
        assert!(!format!("{}", PmixJobState::Terminated).is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixLinkState — from_raw / to_raw / display
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_link_state_from_raw_known() {
        assert_eq!(PmixLinkState::from_raw(0), PmixLinkState::UnknownState);
        assert_eq!(PmixLinkState::from_raw(1), PmixLinkState::LinkDown);
        assert_eq!(PmixLinkState::from_raw(2), PmixLinkState::LinkUp);
    }

    #[test]
    fn test_pmix_link_state_from_raw_unknown() {
        assert!(matches!(
            PmixLinkState::from_raw(99),
            PmixLinkState::Unknown(99)
        ));
    }

    #[test]
    fn test_pmix_link_state_to_raw_roundtrip() {
        let states: &[PmixLinkState] = &[
            PmixLinkState::UnknownState,
            PmixLinkState::LinkDown,
            PmixLinkState::LinkUp,
        ];
        for state in states {
            let raw = state.to_raw();
            assert_eq!(
                PmixLinkState::from_raw(raw),
                *state,
                "roundtrip failed for {:?}",
                state
            );
        }
    }

    #[test]
    fn test_pmix_link_state_display() {
        assert!(!format!("{}", PmixLinkState::LinkUp).is_empty());
        assert!(!format!("{}", PmixLinkState::LinkDown).is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixDataRange — from_raw / to_raw / display
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_data_range_from_raw_known() {
        assert_eq!(PmixDataRange::from_raw(0), PmixDataRange::Undef);
        assert_eq!(PmixDataRange::from_raw(1), PmixDataRange::Rm);
        assert_eq!(PmixDataRange::from_raw(2), PmixDataRange::Local);
        assert_eq!(PmixDataRange::from_raw(3), PmixDataRange::Namespace);
        assert_eq!(PmixDataRange::from_raw(4), PmixDataRange::Session);
        assert_eq!(PmixDataRange::from_raw(5), PmixDataRange::Global);
        assert_eq!(PmixDataRange::from_raw(6), PmixDataRange::Custom);
        assert_eq!(PmixDataRange::from_raw(7), PmixDataRange::ProcLocal);
        assert_eq!(PmixDataRange::from_raw(255), PmixDataRange::Invalid);
    }

    #[test]
    fn test_pmix_data_range_from_raw_unknown() {
        assert_eq!(PmixDataRange::from_raw(99), PmixDataRange::Unknown);
    }

    #[test]
    fn test_pmix_data_range_to_raw_roundtrip() {
        let ranges: &[PmixDataRange] = &[
            PmixDataRange::Undef,
            PmixDataRange::Rm,
            PmixDataRange::Local,
            PmixDataRange::Namespace,
            PmixDataRange::Session,
            PmixDataRange::Global,
            PmixDataRange::Custom,
            PmixDataRange::ProcLocal,
            PmixDataRange::Invalid,
            PmixDataRange::Unknown,
        ];
        for range in ranges {
            let raw = range.to_raw();
            assert_eq!(
                PmixDataRange::from_raw(raw),
                *range,
                "roundtrip failed for {:?}",
                range
            );
        }
    }

    #[test]
    fn test_pmix_data_range_display() {
        assert!(!format!("{}", PmixDataRange::Global).is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixAllocDirective — from_raw / to_raw / display
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_alloc_directive_from_raw_known() {
        assert_eq!(
            PmixAllocDirective::from_raw(43),
            PmixAllocDirective::AllocDirective
        );
        assert!(matches!(
            PmixAllocDirective::from_raw(0),
            PmixAllocDirective::Unknown(_)
        ));
    }

    #[test]
    fn test_pmix_alloc_directive_to_raw_roundtrip() {
        let d = PmixAllocDirective::AllocDirective;
        let raw = d.to_raw();
        assert_eq!(PmixAllocDirective::from_raw(raw), d);
    }

    #[test]
    fn test_pmix_alloc_directive_display() {
        assert!(!format!("{}", PmixAllocDirective::AllocDirective).is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOFChannelFlags — bitmask operations
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_iof_channel_flags_is_empty() {
        assert!(IOFChannelFlags::NO_CHANNELS.is_empty());
        assert!(!IOFChannelFlags::STDOUT.is_empty());
        assert!(!IOFChannelFlags::ALL_CHANNELS.is_empty());
    }

    #[test]
    fn test_iof_channel_flags_contains() {
        assert!(IOFChannelFlags::ALL_CHANNELS.contains(IOFChannelFlags::STDOUT));
        assert!(IOFChannelFlags::ALL_CHANNELS.contains(IOFChannelFlags::STDERR));
        assert!(IOFChannelFlags::ALL_CHANNELS.contains(IOFChannelFlags::STDIN));
        assert!(!IOFChannelFlags::STDOUT.contains(IOFChannelFlags::STDERR));
        assert!(IOFChannelFlags::STDOUT.contains(IOFChannelFlags::STDOUT));
    }

    #[test]
    fn test_iof_channel_flags_display() {
        let s = format!("{}", IOFChannelFlags::STDOUT);
        assert!(!s.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // InfoFlags — bitmask operations
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_info_flags_is_empty() {
        assert!(InfoFlags::default().is_empty());
        assert!(!InfoFlags::REQD.is_empty());
    }

    #[test]
    fn test_info_flags_contains() {
        let flags = InfoFlags::REQD | InfoFlags::PERSISTENT;
        assert!(flags.contains(InfoFlags::REQD));
        assert!(flags.contains(InfoFlags::PERSISTENT));
        assert!(!flags.contains(InfoFlags::QUALIFIER));
    }

    #[test]
    fn test_info_flags_bitor() {
        let a = InfoFlags::REQD;
        let b = InfoFlags::PERSISTENT;
        let c = a | b;
        assert!(c.contains(InfoFlags::REQD));
        assert!(c.contains(InfoFlags::PERSISTENT));
    }

    #[test]
    fn test_info_flags_bitor_assign() {
        let mut flags = InfoFlags::REQD;
        flags |= InfoFlags::PERSISTENT;
        assert!(flags.contains(InfoFlags::REQD));
        assert!(flags.contains(InfoFlags::PERSISTENT));
    }

    #[test]
    fn test_info_flags_raw() {
        let flags = InfoFlags::REQD | InfoFlags::PERSISTENT;
        assert_ne!(flags.raw(), 0);
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixPayload — type_tag correctness (cast to u16 for comparison)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_payload_type_tag_scalars() {
        assert_eq!(PmixPayload::Undef.type_tag(), PMIX_UNDEF as u16);
        assert_eq!(PmixPayload::Bool(true).type_tag(), PMIX_BOOL as u16);
        assert_eq!(PmixPayload::Byte(42).type_tag(), PMIX_BYTE as u16);
        assert_eq!(PmixPayload::Size(100).type_tag(), PMIX_SIZE as u16);
        assert_eq!(PmixPayload::Pid(1234).type_tag(), PMIX_PID as u16);
        assert_eq!(PmixPayload::Int(42).type_tag(), PMIX_INT as u16);
        assert_eq!(PmixPayload::Int8(8).type_tag(), PMIX_INT8 as u16);
        assert_eq!(PmixPayload::Int16(16).type_tag(), PMIX_INT16 as u16);
        assert_eq!(PmixPayload::Int32(32).type_tag(), PMIX_INT32 as u16);
        assert_eq!(PmixPayload::Int64(64).type_tag(), PMIX_INT64 as u16);
        assert_eq!(PmixPayload::Uint(42).type_tag(), PMIX_UINT as u16);
        assert_eq!(PmixPayload::Uint8(8).type_tag(), PMIX_UINT8 as u16);
        assert_eq!(PmixPayload::Uint16(16).type_tag(), PMIX_UINT16 as u16);
        assert_eq!(PmixPayload::Uint32(32).type_tag(), PMIX_UINT32 as u16);
        assert_eq!(PmixPayload::Uint64(64).type_tag(), PMIX_UINT64 as u16);
        assert_eq!(PmixPayload::Float(1.0).type_tag(), PMIX_FLOAT as u16);
        assert_eq!(PmixPayload::Double(1.0).type_tag(), PMIX_DOUBLE as u16);
    }

    #[test]
    fn test_pmix_payload_type_tag_composite() {
        assert_eq!(PmixPayload::Status(0).type_tag(), PMIX_STATUS as u16);
        assert_eq!(PmixPayload::Rank(0).type_tag(), PMIX_PROC_RANK as u16);
        assert_eq!(
            PmixPayload::ByteObject(vec![1, 2, 3]).type_tag(),
            PMIX_BYTE_OBJECT as u16
        );
        assert_eq!(
            PmixPayload::Pointer(std::ptr::null_mut()).type_tag(),
            PMIX_POINTER as u16
        );
    }

    #[test]
    fn test_pmix_payload_type_tag_string() {
        let payload = PmixPayload::String(CString::new("hello").unwrap());
        assert_eq!(payload.type_tag(), PMIX_STRING as u16);
    }

    #[test]
    fn test_pmix_payload_type_tag_data_array() {
        let payload = PmixPayload::DataArray {
            elem_type: PMIX_INT as u16,
            elements: vec![],
        };
        assert_eq!(payload.type_tag(), PMIX_DATA_ARRAY as u16);
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixValueBuilder — build / build_raw
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_value_builder_build_uint32() {
        let owned = PmixValueBuilder::new().uint32(42).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_int32() {
        let owned = PmixValueBuilder::new().int32(-42).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_bool() {
        let owned = PmixValueBuilder::new().bool(true).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_string() {
        let owned = PmixValueBuilder::new()
            .string("hello")
            .unwrap()
            .build()
            .unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_size() {
        let owned = PmixValueBuilder::new().size(1024).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_unbuilt_returns_error() {
        let result = PmixValueBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_value_builder_build_raw_uint32() {
        let raw_val = PmixValueBuilder::new().uint32(42).build_raw().unwrap();
        unsafe {
            assert_eq!(raw_val.data.uint32, 42);
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixEnvar — constructor
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_envar_new() {
        let envar = PmixEnvar::new("PATH", "/usr/bin", ':').unwrap();
        assert_eq!(envar.separator, b':');
    }

    #[test]
    fn test_pmix_envar_new_nul_error() {
        let result = PmixEnvar::new("has\0null", "value", ':');
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // Proc — constructor and accessors
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_proc_new() {
        let proc = Proc::new("test_namespace", 42).unwrap();
        assert_eq!(proc.get_rank(), 42);
    }

    #[test]
    fn test_proc_set_rank() {
        let mut proc = Proc::new("test_namespace", 0).unwrap();
        proc.set_rank(99);
        assert_eq!(proc.get_rank(), 99);
    }

    #[test]
    fn test_proc_new_nul_error() {
        let result = Proc::new("has\0null", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_proc_new_with_nspace() {
        let proc1 = Proc::new("test_ns", 0).unwrap();
        let proc2 = proc1.new_with_nspace(5).unwrap();
        assert_eq!(proc2.get_rank(), 5);
    }

    // ──────────────────────────────────────────────────────────────────────
    // Constants
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_constants() {
        assert_eq!(GLOBAL, PMIX_GLOBAL as u8);
        assert!(!NUM_NODES.is_empty());
        assert!(!JOB_SIZE.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixDeviceType — from_raw / to_raw / display (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_device_type_from_raw_known() {
        assert_eq!(PmixDeviceType::from_raw(0x00), PmixDeviceType::UnknownType);
        assert_eq!(PmixDeviceType::from_raw(0x01), PmixDeviceType::Block);
        assert_eq!(PmixDeviceType::from_raw(0x02), PmixDeviceType::Gpu);
        assert_eq!(PmixDeviceType::from_raw(0x04), PmixDeviceType::Network);
        assert_eq!(PmixDeviceType::from_raw(0x08), PmixDeviceType::OpenFabrics);
        assert_eq!(PmixDeviceType::from_raw(0x10), PmixDeviceType::Dma);
        assert_eq!(PmixDeviceType::from_raw(0x20), PmixDeviceType::Coproc);
    }

    #[test]
    fn test_pmix_device_type_from_raw_unknown() {
        assert!(matches!(
            PmixDeviceType::from_raw(0xFF),
            PmixDeviceType::Unknown(0xFF)
        ));
        assert!(matches!(
            PmixDeviceType::from_raw(0x1234),
            PmixDeviceType::Unknown(0x1234)
        ));
    }

    #[test]
    fn test_pmix_device_type_to_raw_roundtrip() {
        let types: &[PmixDeviceType] = &[
            PmixDeviceType::UnknownType,
            PmixDeviceType::Block,
            PmixDeviceType::Gpu,
            PmixDeviceType::Network,
            PmixDeviceType::OpenFabrics,
            PmixDeviceType::Dma,
            PmixDeviceType::Coproc,
        ];
        for ty in types {
            let raw = ty.to_raw();
            assert_eq!(
                PmixDeviceType::from_raw(raw),
                *ty,
                "roundtrip failed for {:?}",
                ty
            );
        }
    }

    #[test]
    fn test_pmix_device_type_display() {
        assert_eq!(format!("{}", PmixDeviceType::UnknownType), "UNKNOWN");
        assert_eq!(format!("{}", PmixDeviceType::Block), "BLOCK");
        assert_eq!(format!("{}", PmixDeviceType::Gpu), "GPU");
        assert_eq!(format!("{}", PmixDeviceType::Network), "NETWORK");
        assert_eq!(format!("{}", PmixDeviceType::OpenFabrics), "OPENFABRICS");
        assert_eq!(format!("{}", PmixDeviceType::Dma), "DMA");
        assert_eq!(format!("{}", PmixDeviceType::Coproc), "COPROCESSOR");
        let s = format!("{}", PmixDeviceType::Unknown(0xFF));
        assert!(s.contains("UNKNOWN"));
        assert!(s.contains("FF"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixPersistence — from_raw / to_raw / display (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_persistence_from_raw_known() {
        assert_eq!(PmixPersistence::from_raw(0), PmixPersistence::Indefinite);
        assert_eq!(PmixPersistence::from_raw(1), PmixPersistence::FirstRead);
        assert_eq!(PmixPersistence::from_raw(2), PmixPersistence::Process);
        assert_eq!(PmixPersistence::from_raw(3), PmixPersistence::Application);
        assert_eq!(PmixPersistence::from_raw(4), PmixPersistence::Session);
        assert_eq!(PmixPersistence::from_raw(255), PmixPersistence::Invalid);
    }

    #[test]
    fn test_pmix_persistence_from_raw_unknown() {
        assert!(matches!(
            PmixPersistence::from_raw(42),
            PmixPersistence::Unknown(42)
        ));
    }

    #[test]
    fn test_pmix_persistence_to_raw_roundtrip() {
        let persistences: &[PmixPersistence] = &[
            PmixPersistence::Indefinite,
            PmixPersistence::FirstRead,
            PmixPersistence::Process,
            PmixPersistence::Application,
            PmixPersistence::Session,
            PmixPersistence::Invalid,
        ];
        for p in persistences {
            let raw = p.to_raw();
            assert_eq!(
                PmixPersistence::from_raw(raw),
                *p,
                "roundtrip failed for {:?}",
                p
            );
        }
    }

    #[test]
    fn test_pmix_persistence_display() {
        assert_eq!(format!("{}", PmixPersistence::Indefinite), "INDEFINITE");
        assert_eq!(format!("{}", PmixPersistence::FirstRead), "DELETE ON FIRST ACCESS");
        assert_eq!(format!("{}", PmixPersistence::Process), "RETAIN UNTIL PUBLISHING PROCESS TERMINATES");
        assert_eq!(format!("{}", PmixPersistence::Application), "RETAIN UNTIL APPLICATION OF PUBLISHING PROCESS TERMINATES");
        assert_eq!(format!("{}", PmixPersistence::Session), "RETAIN UNTIL ALLOCATION OF PUBLISHING PROCESS TERMINATES");
        assert_eq!(format!("{}", PmixPersistence::Invalid), "INVALID");
        let s = format!("{}", PmixPersistence::Unknown(42));
        assert!(s.contains("UNKNOWN"));
        assert!(s.contains("42"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixError — exhaustive from_raw for all known variants (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_error_from_raw_all_known() {
        // Base codes
        assert_eq!(PmixError::from_raw(-3), Some(PmixError::DebuggerRelease));
        assert_eq!(PmixError::from_raw(-4), Some(PmixError::ErrProcRestart));
        assert_eq!(PmixError::from_raw(-5), Some(PmixError::ErrProcCheckpoint));
        assert_eq!(PmixError::from_raw(-6), Some(PmixError::ErrProcMigrate));
        assert_eq!(PmixError::from_raw(-8), Some(PmixError::ErrProcRequestedAbort));
        assert_eq!(PmixError::from_raw(-11), Some(PmixError::ErrExists));
        assert_eq!(PmixError::from_raw(-12), Some(PmixError::ErrInvalidCred));
        assert_eq!(PmixError::from_raw(-15), Some(PmixError::ErrWouldBlock));
        assert_eq!(PmixError::from_raw(-16), Some(PmixError::ErrUnknownDataType));
        assert_eq!(PmixError::from_raw(-18), Some(PmixError::ErrTypeMismatch));
        assert_eq!(PmixError::from_raw(-19), Some(PmixError::ErrUnpackInadequateSpace));
        assert_eq!(PmixError::from_raw(-20), Some(PmixError::ErrUnpackFailure));
        assert_eq!(PmixError::from_raw(-21), Some(PmixError::ErrPackFailure));
        assert_eq!(PmixError::from_raw(-23), Some(PmixError::ErrNoPermissions));
        assert_eq!(PmixError::from_raw(-28), Some(PmixError::ErrResourceBusy));
        assert_eq!(PmixError::from_raw(-29), Some(PmixError::ErrOutOfResource));
        // Data/lookup
        assert_eq!(PmixError::from_raw(-49), Some(PmixError::ErrCommFailure));
        assert_eq!(PmixError::from_raw(-50), Some(PmixError::ErrUnpackReadPastEndOfBuffer));
        assert_eq!(PmixError::from_raw(-51), Some(PmixError::ErrConflictingCleanupDirectives));
        assert_eq!(PmixError::from_raw(-52), Some(PmixError::ErrPartialSuccess));
        assert_eq!(PmixError::from_raw(-53), Some(PmixError::ErrDuplicateKey));
        assert_eq!(PmixError::from_raw(-58), Some(PmixError::ReadyForDebug));
        assert_eq!(PmixError::from_raw(-59), Some(PmixError::ErrParamValueNotSupported));
        assert_eq!(PmixError::from_raw(-60), Some(PmixError::ErrEmpty));
        assert_eq!(PmixError::from_raw(-61), Some(PmixError::ErrLostConnection));
        assert_eq!(PmixError::from_raw(-62), Some(PmixError::ErrExistsOutsideScope));
        // Job control
        assert_eq!(PmixError::from_raw(-106), Some(PmixError::JctrlCheckpoint));
        assert_eq!(PmixError::from_raw(-107), Some(PmixError::JctrlCheckpointComplete));
        assert_eq!(PmixError::from_raw(-108), Some(PmixError::JctrlPreemptAlert));
        // Monitoring
        assert_eq!(PmixError::from_raw(-109), Some(PmixError::MonitorHeartbeatAlert));
        assert_eq!(PmixError::from_raw(-110), Some(PmixError::MonitorFileAlert));
        // Fabric
        assert_eq!(PmixError::from_raw(-113), Some(PmixError::FabricUpdateEndpoints));
        // Internal
        assert_eq!(PmixError::from_raw(-144), Some(PmixError::ErrEventRegistration));
        assert_eq!(PmixError::from_raw(-145), Some(PmixError::EventJobEnd));
        // Operational
        assert_eq!(PmixError::from_raw(-156), Some(PmixError::OperationInProgress));
        assert_eq!(PmixError::from_raw(-157), Some(PmixError::OperationSucceeded));
        assert_eq!(PmixError::from_raw(-158), Some(PmixError::ErrInvalidOperation));
        // Attribute
        assert_eq!(PmixError::from_raw(-171), Some(PmixError::ErrRepeatAttrRegistration));
        // I/O forwarding
        assert_eq!(PmixError::from_raw(-172), Some(PmixError::ErrIofFailure));
        assert_eq!(PmixError::from_raw(-173), Some(PmixError::ErrIofComplete));
        // Fabric status
        assert_eq!(PmixError::from_raw(-175), Some(PmixError::FabricUpdated));
        assert_eq!(PmixError::from_raw(-176), Some(PmixError::FabricUpdatePending));
        // Job errors
        assert_eq!(PmixError::from_raw(-177), Some(PmixError::ErrJobAppNotExecutable));
        assert_eq!(PmixError::from_raw(-178), Some(PmixError::ErrJobNoExeSpecified));
        assert_eq!(PmixError::from_raw(-179), Some(PmixError::ErrJobFailedToMap));
        assert_eq!(PmixError::from_raw(-181), Some(PmixError::ErrJobFailedToLaunch));
        assert_eq!(PmixError::from_raw(-182), Some(PmixError::ErrJobAborted));
        assert_eq!(PmixError::from_raw(-183), Some(PmixError::ErrJobKilledByCmd));
        assert_eq!(PmixError::from_raw(-184), Some(PmixError::ErrJobAbortedBySig));
        assert_eq!(PmixError::from_raw(-185), Some(PmixError::ErrJobTermWoSync));
        assert_eq!(PmixError::from_raw(-186), Some(PmixError::ErrJobSensorBoundExceeded));
        assert_eq!(PmixError::from_raw(-187), Some(PmixError::ErrJobNonZeroTerm));
        assert_eq!(PmixError::from_raw(-188), Some(PmixError::ErrJobAllocFailed));
        assert_eq!(PmixError::from_raw(-189), Some(PmixError::ErrJobAbortedBySysEvent));
        assert_eq!(PmixError::from_raw(-190), Some(PmixError::ErrJobExeNotFound));
        // Job lifecycle events
        assert_eq!(PmixError::from_raw(-191), Some(PmixError::EventJobStart));
        assert_eq!(PmixError::from_raw(-192), Some(PmixError::EventSessionStart));
        assert_eq!(PmixError::from_raw(-193), Some(PmixError::EventSessionEnd));
        // Process errors
        assert_eq!(PmixError::from_raw(-200), Some(PmixError::ErrProcTermWoSync));
        assert_eq!(PmixError::from_raw(-201), Some(PmixError::EventProcTerminated));
        // System events
        assert_eq!(PmixError::from_raw(-230), Some(PmixError::EventSysBase));
        assert_eq!(PmixError::from_raw(-231), Some(PmixError::EventNodeDown));
        assert_eq!(PmixError::from_raw(-232), Some(PmixError::EventNodeOffline));
        // Additional job errors
        assert_eq!(PmixError::from_raw(-233), Some(PmixError::ErrJobWdirNotFound));
        assert_eq!(PmixError::from_raw(-234), Some(PmixError::ErrJobInsufficientResources));
        assert_eq!(PmixError::from_raw(-235), Some(PmixError::ErrJobSysOpFailed));
        // System event other
        assert_eq!(PmixError::from_raw(-330), Some(PmixError::EventSysOther));
        // Event handler return codes
        assert_eq!(PmixError::from_raw(-331), Some(PmixError::EventNoActionTaken));
        assert_eq!(PmixError::from_raw(-332), Some(PmixError::EventPartialActionTaken));
        assert_eq!(PmixError::from_raw(-333), Some(PmixError::EventActionDeferred));
        assert_eq!(PmixError::from_raw(-334), Some(PmixError::EventActionComplete));
        // Per-process errors
        assert_eq!(PmixError::from_raw(-400), Some(PmixError::ErrProcKilledByCmd));
        assert_eq!(PmixError::from_raw(-402), Some(PmixError::ErrProcAbortedBySig));
        assert_eq!(PmixError::from_raw(-403), Some(PmixError::ErrProcSensorBoundExceeded));
        assert_eq!(PmixError::from_raw(-404), Some(PmixError::ErrExitNonzeroTerm));
    }

    #[test]
    fn test_pmix_error_to_raw_all_known() {
        // Verify to_raw returns correct discriminant for all categories
        assert_eq!(PmixError::DebuggerRelease.to_raw(), -3);
        assert_eq!(PmixError::ErrProcRestart.to_raw(), -4);
        assert_eq!(PmixError::ErrExists.to_raw(), -11);
        assert_eq!(PmixError::ErrWouldBlock.to_raw(), -15);
        assert_eq!(PmixError::ErrTypeMismatch.to_raw(), -18);
        assert_eq!(PmixError::ErrNoPermissions.to_raw(), -23);
        assert_eq!(PmixError::ErrCommFailure.to_raw(), -49);
        assert_eq!(PmixError::ErrPartialSuccess.to_raw(), -52);
        assert_eq!(PmixError::ReadyForDebug.to_raw(), -58);
        assert_eq!(PmixError::ErrLostConnection.to_raw(), -61);
        assert_eq!(PmixError::JctrlCheckpoint.to_raw(), -106);
        assert_eq!(PmixError::MonitorHeartbeatAlert.to_raw(), -109);
        assert_eq!(PmixError::FabricUpdateEndpoints.to_raw(), -113);
        assert_eq!(PmixError::ErrEventRegistration.to_raw(), -144);
        assert_eq!(PmixError::EventJobEnd.to_raw(), -145);
        assert_eq!(PmixError::OperationInProgress.to_raw(), -156);
        assert_eq!(PmixError::ErrRepeatAttrRegistration.to_raw(), -171);
        assert_eq!(PmixError::ErrIofFailure.to_raw(), -172);
        assert_eq!(PmixError::FabricUpdated.to_raw(), -175);
        assert_eq!(PmixError::ErrJobAppNotExecutable.to_raw(), -177);
        assert_eq!(PmixError::ErrJobCanceled.to_raw(), -180);
        assert_eq!(PmixError::ErrJobAborted.to_raw(), -182);
        assert_eq!(PmixError::ErrJobTermWoSync.to_raw(), -185);
        assert_eq!(PmixError::ErrJobAllocFailed.to_raw(), -188);
        assert_eq!(PmixError::EventJobStart.to_raw(), -191);
        assert_eq!(PmixError::EventSessionEnd.to_raw(), -193);
        assert_eq!(PmixError::ErrProcTermWoSync.to_raw(), -200);
        assert_eq!(PmixError::EventProcTerminated.to_raw(), -201);
        assert_eq!(PmixError::EventSysBase.to_raw(), -230);
        assert_eq!(PmixError::EventNodeDown.to_raw(), -231);
        assert_eq!(PmixError::ErrJobWdirNotFound.to_raw(), -233);
        assert_eq!(PmixError::EventSysOther.to_raw(), -330);
        assert_eq!(PmixError::EventNoActionTaken.to_raw(), -331);
        assert_eq!(PmixError::EventActionComplete.to_raw(), -334);
        assert_eq!(PmixError::ErrProcKilledByCmd.to_raw(), -400);
        assert_eq!(PmixError::ErrExitNonzeroTerm.to_raw(), -404);
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixValueBuilder — additional build types (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_value_builder_build_float() {
        let owned = PmixValueBuilder::new().float(3.14).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_double() {
        let owned = PmixValueBuilder::new().double(2.718).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_i64() {
        let owned = PmixValueBuilder::new().int64(-9223372036854775808i64).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_u64() {
        let owned = PmixValueBuilder::new().uint64(18446744073709551615u64).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_byte() {
        let owned = PmixValueBuilder::new().byte(0xAB).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_pid() {
        let owned = PmixValueBuilder::new().pid(12345).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_byte_object() {
        let owned = PmixValueBuilder::new()
            .byte_object(&[1, 2, 3, 4])
            .unwrap()
            .build()
            .unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_pointer() {
        let owned = unsafe {
            PmixValueBuilder::new()
                .pointer(std::ptr::null_mut())
                .build()
                .unwrap()
        };
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_data_array() {
        // data_array requires Vec<pmix_value_t> — use build_raw to construct elements
        let e1 = PmixValueBuilder::new().int32(1).build_raw().unwrap();
        let e2 = PmixValueBuilder::new().int32(2).build_raw().unwrap();
        let e3 = PmixValueBuilder::new().int32(3).build_raw().unwrap();
        let owned = PmixValueBuilder::new()
            .data_array(PMIX_INT as u16, vec![e1, e2, e3])
            .unwrap()
            .build()
            .unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_rank() {
        let owned = PmixValueBuilder::new().rank(42).build().unwrap();
        drop(owned);
    }

    #[test]
    fn test_pmix_value_builder_build_status() {
        let owned = PmixValueBuilder::new().status(0).build().unwrap();
        drop(owned);
    }

    // ──────────────────────────────────────────────────────────────────────
    // BuilderError — Display (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_builder_error_display_key_empty() {
        let e = BuilderError::KeyEmpty;
        let s = format!("{}", e);
        assert!(s.contains("empty"));
    }

    #[test]
    fn test_builder_error_display_key_too_long() {
        let e = BuilderError::KeyTooLong { len: 600, maximum: 511 };
        let s = format!("{}", e);
        assert!(s.contains("600"));
        assert!(s.contains("511"));
    }

    #[test]
    fn test_builder_error_display_missing_value() {
        let e = BuilderError::MissingValue;
        let s = format!("{}", e);
        assert!(s.contains("no value"));
    }

    #[test]
    fn test_builder_error_display_key_contains_nul() {
        // Create a real NulError from a CString failure
        let nul_err = CString::new("has\0null").unwrap_err();
        let e = BuilderError::KeyContainsNul(nul_err);
        let s = format!("{}", e);
        assert!(s.contains("NUL"));
    }

    #[test]
    fn test_builder_error_is_std_error() {
        let nul_err = CString::new("has\0null").unwrap_err();
        let e = BuilderError::KeyContainsNul(nul_err);
        let _: &dyn std::error::Error = &e;
    }

    // ──────────────────────────────────────────────────────────────────────
    // ValueError — Display (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_value_error_display_missing_payload() {
        let e = ValueError::MissingPayload;
        let s = format!("{}", e);
        assert!(s.contains("no payload"));
    }

    #[test]
    fn test_value_error_display_empty_data() {
        let e = ValueError::EmptyData;
        let s = format!("{}", e);
        assert!(s.contains("empty"));
    }

    #[test]
    fn test_value_error_display_contains_nul() {
        // Create a real NulError from a CString failure
        let nul_err = CString::new("has\0null").unwrap_err();
        let e = ValueError::ContainsNul(nul_err);
        let s = format!("{}", e);
        assert!(s.contains("NUL"));
    }

    #[test]
    fn test_value_error_is_std_error() {
        let e = ValueError::MissingPayload;
        let _: &dyn std::error::Error = &e;
    }

    // ──────────────────────────────────────────────────────────────────────
    // Enum derives — Clone, Copy, PartialEq, Eq, Hash, Debug (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_proc_state_derives() {
        let a = PmixProcState::Running;
        let b = PmixProcState::Running;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixProcState> =
            [a, PmixProcState::Terminated].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_scope_derives() {
        let a = PmixScope::Global;
        let b = PmixScope::Global;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixScope> =
            [a, PmixScope::Local].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_job_state_derives() {
        let a = PmixJobState::Running;
        let b = PmixJobState::Running;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixJobState> =
            [a, PmixJobState::Terminated].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_link_state_derives() {
        let a = PmixLinkState::LinkUp;
        let b = PmixLinkState::LinkUp;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixLinkState> =
            [a, PmixLinkState::LinkDown].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_data_range_derives() {
        let a = PmixDataRange::Global;
        let b = PmixDataRange::Global;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixDataRange> =
            [a, PmixDataRange::Local].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_device_type_derives() {
        let a = PmixDeviceType::Gpu;
        let b = PmixDeviceType::Gpu;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixDeviceType> =
            [a, PmixDeviceType::Block].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    #[test]
    fn test_pmix_persistence_derives() {
        let a = PmixPersistence::Indefinite;
        let b = PmixPersistence::Indefinite;
        assert_eq!(a, b);
        assert_eq!(a.clone(), b);
        let _hash: std::collections::HashSet<PmixPersistence> =
            [a, PmixPersistence::FirstRead].iter().cloned().collect();
        let debug = format!("{:?}", a);
        assert!(!debug.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixError — Display via PmixStatus (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_error_display_via_status() {
        // PmixStatus delegates to PmixError for Display
        let s = format!("{}", PmixStatus::Known(PmixError::Success));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Known(PmixError::ErrInit));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Known(PmixError::ErrNomem));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Known(PmixError::ErrNotFound));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Known(PmixError::ErrTimeout));
        assert!(!s.is_empty());
        let s = format!("{}", PmixStatus::Known(PmixError::ErrJobCanceled));
        assert!(!s.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixStatus — Error trait (TASK-107)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_status_is_std_error() {
        let status = PmixStatus::Known(PmixError::ErrInit);
        let _: &dyn std::error::Error = &status;
    }
}
