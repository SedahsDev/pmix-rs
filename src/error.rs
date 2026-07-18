//! PMIx status and error types (`pmix_status_t`).

use crate::ffi::*;
use std::fmt;

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
            Self::Known(e) => write!(f, "{e:?}"),
            Self::Unknown(v) => write!(f, "pmix_status_t({v}) [unknown/user-defined]"),
        }
    }
}

impl std::fmt::Display for PmixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
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

