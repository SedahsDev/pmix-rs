#![allow(unused_imports)]

use std::fmt::Debug;
mod ffi;
mod info;

use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::{c_char, c_void};
use std::{fmt, mem, ptr};
use std::ptr::{null, null_mut};
use crate::ffi::*;
use cstring_array::CStringArray;

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
    Success                             =    0,

    // ── ❷  Widely-used base codes ────────────────────────────────────────────

    /// `PMIX_ERROR` (−1) — generic unspecified error.
    Error                               =   -1,

    /// `PMIX_DEBUGGER_RELEASE` (−3) — replaces the deprecated
    /// `PMIX_ERR_DEBUGGER_RELEASE`; the debugger has released a stopped process.
    DebuggerRelease                     =   -3,

    /// `PMIX_ERR_PROC_RESTART` (−4)
    ErrProcRestart                      =   -4,

    /// `PMIX_ERR_PROC_CHECKPOINT` (−5)
    ErrProcCheckpoint                   =   -5,

    /// `PMIX_ERR_PROC_MIGRATE` (−6)
    ErrProcMigrate                      =   -6,

    /// `PMIX_ERR_PROC_REQUESTED_ABORT` (−8) — a process called `PMIx_Abort`.
    ErrProcRequestedAbort               =   -8,

    /// `PMIX_ERR_EXISTS` (−11) — the key or object already exists.
    ErrExists                           =  -11,

    /// `PMIX_ERR_INVALID_CRED` (−12) — invalid or unverifiable security credential.
    ErrInvalidCred                      =  -12,

    /// `PMIX_ERR_WOULD_BLOCK` (−15) — call would block; returned only when
    /// non-blocking behaviour was requested.
    ErrWouldBlock                       =  -15,

    /// `PMIX_ERR_UNKNOWN_DATA_TYPE` (−16) — `pmix_data_type_t` discriminant
    /// is not recognised.
    ErrUnknownDataType                  =  -16,

    /// `PMIX_ERR_TYPE_MISMATCH` (−18) — stored and requested types differ.
    ErrTypeMismatch                     =  -18,

    /// `PMIX_ERR_UNPACK_INADEQUATE_SPACE` (−19)
    ErrUnpackInadequateSpace            =  -19,

    /// `PMIX_ERR_UNPACK_FAILURE` (−20)
    ErrUnpackFailure                    =  -20,

    /// `PMIX_ERR_PACK_FAILURE` (−21)
    ErrPackFailure                      =  -21,

    /// `PMIX_ERR_NO_PERMISSIONS` (−23) — caller lacks required credentials.
    ErrNoPermissions                    =  -23,

    /// `PMIX_ERR_TIMEOUT` (−24) — operation exceeded `PMIX_TIMEOUT`.
    ErrTimeout                          =  -24,

    /// `PMIX_ERR_UNREACH` (−25) — target process or server is unreachable.
    ErrUnreach                          =  -25,

    /// `PMIX_ERR_BAD_PARAM` (−27) — parameter out of range or inconsistent.
    ErrBadParam                         =  -27,

    /// `PMIX_ERR_RESOURCE_BUSY` (−28) — requested resource is in use.
    ErrResourceBusy                     =  -28,

    /// `PMIX_ERR_OUT_OF_RESOURCE` (−29) — a system resource was exhausted.
    ErrOutOfResource                    =  -29,

    /// `PMIX_ERR_INIT` (−31) — PMIx was not initialised, or init failed.
    ErrInit                             =  -31,

    /// `PMIX_ERR_NOMEM` (−32) — memory allocation failed.
    ErrNomem                            =  -32,

    // ── ❸  Data / lookup errors ──────────────────────────────────────────────

    /// `PMIX_ERR_NOT_FOUND` (−46) — the requested data item does not exist.
    ErrNotFound                         =  -46,

    /// `PMIX_ERR_NOT_SUPPORTED` (−47) — API or attribute not supported here.
    ErrNotSupported                     =  -47,

    /// `PMIX_ERR_COMM_FAILURE` (−49) — general communication failure.
    ErrCommFailure                      =  -49,

    /// `PMIX_ERR_UNPACK_READ_PAST_END_OF_BUFFER` (−50)
    ErrUnpackReadPastEndOfBuffer        =  -50,

    /// `PMIX_ERR_CONFLICTING_CLEANUP_DIRECTIVES` (−51) — two cleanup
    /// directives for the same path conflict.
    ErrConflictingCleanupDirectives     =  -51,

    /// `PMIX_ERR_PARTIAL_SUCCESS` (−52) — succeeded for some but not all targets.
    ErrPartialSuccess                   =  -52,

    /// `PMIX_ERR_DUPLICATE_KEY` (−53) — key already exists in scope.
    ErrDuplicateKey                     =  -53,

    /// `PMIX_READY_FOR_DEBUG` (−58) — process reached the breakpoint and
    /// is waiting for a debugger (accompanied by `PMIX_BREAKPOINT`).
    ReadyForDebug                       =  -58,

    /// `PMIX_ERR_PARAM_VALUE_NOT_SUPPORTED` (−59) — parameter value not
    /// supported by this implementation.
    ErrParamValueNotSupported           =  -59,

    /// `PMIX_ERR_EMPTY` (−60) — container or collection is empty.
    ErrEmpty                            =  -60,

    /// `PMIX_ERR_LOST_CONNECTION` (−61) — established connection was lost.
    ErrLostConnection                   =  -61,

    /// `PMIX_ERR_EXISTS_OUTSIDE_SCOPE` (−62) — key exists but was published
    /// outside the caller's accessible scope.
    ErrExistsOutsideScope               =  -62,

    // ── ❹  Job-control event codes ───────────────────────────────────────────

    /// `PMIX_JCTRL_CHECKPOINT` (−106) — trigger a checkpoint.
    JctrlCheckpoint                     = -106,

    /// `PMIX_JCTRL_CHECKPOINT_COMPLETE` (−107) — checkpoint finished.
    JctrlCheckpointComplete             = -107,

    /// `PMIX_JCTRL_PREEMPT_ALERT` (−108) — scheduler will preempt this job.
    JctrlPreemptAlert                   = -108,

    // ── ❺  Monitoring alert codes ────────────────────────────────────────────

    /// `PMIX_MONITOR_HEARTBEAT_ALERT` (−109) — heartbeat missed.
    MonitorHeartbeatAlert               = -109,

    /// `PMIX_MONITOR_FILE_ALERT` (−110) — watched file changed unexpectedly.
    MonitorFileAlert                    = -110,

    // ── ❻  Fabric / network event codes ─────────────────────────────────────

    /// `PMIX_FABRIC_UPDATE_ENDPOINTS` (−113) — fabric endpoint info changed.
    FabricUpdateEndpoints               = -113,

    // ── ❼  Internal / registration errors ───────────────────────────────────

    /// `PMIX_ERR_EVENT_REGISTRATION` (−144) — event handler registration failed.
    ErrEventRegistration                = -144,

    // ── ❽  Job lifecycle event codes ─────────────────────────────────────────

    /// `PMIX_EVENT_JOB_END` (−145) — the job has ended.
    EventJobEnd                         = -145,

    // ── ❾  Operational-state codes ──────────────────────────────────────────
    //
    // These are NEGATIVE in the real header.

    /// `PMIX_OPERATION_IN_PROGRESS` (−156) — operation launched; result
    /// delivered via callback.
    OperationInProgress                 = -156,

    /// `PMIX_OPERATION_SUCCEEDED` (−157) — event handler signals the event
    /// was fully handled.
    OperationSucceeded                  = -157,

    /// `PMIX_ERR_INVALID_OPERATION` (−158) — operation is not valid in the
    /// current state.
    ErrInvalidOperation                 = -158,

    // ── ❿  Attribute / registration errors ──────────────────────────────────

    /// `PMIX_ERR_REPEAT_ATTR_REGISTRATION` (−171) — attribute registered more
    /// than once with conflicting parameters.
    ErrRepeatAttrRegistration           = -171,

    // ── ⓫  I/O-forwarding codes ──────────────────────────────────────────────

    /// `PMIX_ERR_IOF_FAILURE` (−172) — general I/O-forwarding error.
    ErrIofFailure                       = -172,

    /// `PMIX_ERR_IOF_COMPLETE` (−173) — I/O-forwarding stream closed gracefully.
    ErrIofComplete                      = -173,

    // ── ⓬  Fabric status codes ───────────────────────────────────────────────

    /// `PMIX_FABRIC_UPDATED` (−175) — fabric topology has been updated.
    FabricUpdated                       = -175,

    /// `PMIX_FABRIC_UPDATE_PENDING` (−176) — fabric update is in progress.
    FabricUpdatePending                 = -176,

    // ── ⓭  Job-level error codes ─────────────────────────────────────────────

    /// `PMIX_ERR_JOB_APP_NOT_EXECUTABLE` (−177) — binary is not executable.
    ErrJobAppNotExecutable              = -177,

    /// `PMIX_ERR_JOB_NO_EXE_SPECIFIED` (−178) — no executable in spawn request.
    ErrJobNoExeSpecified                = -178,

    /// `PMIX_ERR_JOB_FAILED_TO_MAP` (−179) — RM could not map processes to nodes.
    ErrJobFailedToMap                   = -179,

    /// `PMIX_ERR_JOB_CANCELED` (−180) — job was cancelled.
    ErrJobCanceled                      = -180,

    /// `PMIX_ERR_JOB_FAILED_TO_LAUNCH` (−181) — spawn rejected before any
    /// process started.
    ErrJobFailedToLaunch                = -181,

    /// `PMIX_ERR_JOB_ABORTED` (−182) — job aborted due to an error.
    ErrJobAborted                       = -182,

    /// `PMIX_ERR_JOB_KILLED_BY_CMD` (−183) — job killed by control command.
    ErrJobKilledByCmd                   = -183,

    /// `PMIX_ERR_JOB_ABORTED_BY_SIG` (−184) — job killed by unhandled signal.
    ErrJobAbortedBySig                  = -184,

    /// `PMIX_ERR_JOB_TERM_WO_SYNC` (−185) — job terminated without completing
    /// a required barrier / fence.
    ErrJobTermWoSync                    = -185,

    /// `PMIX_ERR_JOB_SENSOR_BOUND_EXCEEDED` (−186) — sensor threshold exceeded.
    ErrJobSensorBoundExceeded           = -186,

    /// `PMIX_ERR_JOB_NON_ZERO_TERM` (−187) — job exited with non-zero code.
    ErrJobNonZeroTerm                   = -187,

    /// `PMIX_ERR_JOB_ALLOC_FAILED` (−188) — resource allocation for the job
    /// failed.
    ErrJobAllocFailed                   = -188,

    /// `PMIX_ERR_JOB_ABORTED_BY_SYS_EVENT` (−189) — job aborted due to an
    /// unrecoverable system event (e.g. node failure).
    ErrJobAbortedBySysEvent             = -189,

    /// `PMIX_ERR_JOB_EXE_NOT_FOUND` (−190) — executable not found on exec node.
    ErrJobExeNotFound                   = -190,

    // ── ⓮  Job-lifecycle event codes ────────────────────────────────────────

    /// `PMIX_EVENT_JOB_START` (−191) — job has started.
    EventJobStart                       = -191,

    /// `PMIX_EVENT_SESSION_START` (−192) — new session has started.
    EventSessionStart                   = -192,

    /// `PMIX_EVENT_SESSION_END` (−193) — session has ended.
    EventSessionEnd                     = -193,

    // ── ⓯  Process-level error codes ────────────────────────────────────────

    /// `PMIX_ERR_PROC_TERM_WO_SYNC` (−200) — process exited without completing
    /// a required collective operation.
    ErrProcTermWoSync                   = -200,

    /// `PMIX_EVENT_PROC_TERMINATED` (−201) — a process has terminated.
    EventProcTerminated                 = -201,

    // ── ⓰  System-event codes ────────────────────────────────────────────────

    /// `PMIX_EVENT_SYS_BASE` (−230) — base sentinel for system events.
    EventSysBase                        = -230,

    /// `PMIX_EVENT_NODE_DOWN` (−231) — a node has gone down.
    EventNodeDown                       = -231,

    /// `PMIX_EVENT_NODE_OFFLINE` (−232) — a node has gone offline.
    EventNodeOffline                    = -232,

    // ── ⓱  Additional job-level errors ──────────────────────────────────────

    /// `PMIX_ERR_JOB_WDIR_NOT_FOUND` (−233) — working directory not found on
    /// exec node.
    ErrJobWdirNotFound                  = -233,

    /// `PMIX_ERR_JOB_INSUFFICIENT_RESOURCES` (−234) — not enough resources
    /// for the spawn request.
    ErrJobInsufficientResources         = -234,

    /// `PMIX_ERR_JOB_SYS_OP_FAILED` (−235) — internal system operation needed
    /// for launch failed.
    ErrJobSysOpFailed                   = -235,

    // ── ⓲  System-event "other" range ───────────────────────────────────────

    /// `PMIX_EVENT_SYS_OTHER` (−330) — catch-all for undefined system events.
    EventSysOther                       = -330,

    // ── ⓳  Event-handler return codes ───────────────────────────────────────

    /// `PMIX_EVENT_NO_ACTION_TAKEN` (−331) — handler ran but took no action.
    EventNoActionTaken                  = -331,

    /// `PMIX_EVENT_PARTIAL_ACTION_TAKEN` (−332) — handler took partial action.
    EventPartialActionTaken             = -332,

    /// `PMIX_EVENT_ACTION_DEFERRED` (−333) — handler queued actions for later.
    EventActionDeferred                 = -333,

    /// `PMIX_EVENT_ACTION_COMPLETE` (−334) — handler fully resolved the event.
    EventActionComplete                 = -334,

    // ── ⓴  Per-process error codes ──────────────────────────────────────────

    /// `PMIX_ERR_PROC_KILLED_BY_CMD` (−400) — process killed by control command.
    ErrProcKilledByCmd                  = -400,

    /// `PMIX_ERR_PROC_FAILED_TO_START` (−401) — spawned process never called
    /// `PMIx_Init`.
    ErrProcFailedToStart                = -401,

    /// `PMIX_ERR_PROC_ABORTED_BY_SIG` (−402) — process killed by unhandled signal.
    ErrProcAbortedBySig                 = -402,

    /// `PMIX_ERR_PROC_SENSOR_BOUND_EXCEEDED` (−403) — per-process sensor
    /// threshold exceeded.
    ErrProcSensorBoundExceeded          = -403,

    /// `PMIX_ERR_EXIT_NONZERO_TERM` (−404) — process exited with non-zero code.
    ErrExitNonzeroTerm                  = -404,

    // ── ㉑  External / user-defined boundary ────────────────────────────────

    /// `PMIX_EXTERNAL_ERR_BASE` (−3000) — all values **more negative** than
    /// this are reserved for user / implementation defined codes.
    ExternalErrBase                     = -3000,

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
    /// ```rust
    /// use pmix_error::{PmixError, PmixStatus};
    ///
    /// assert_eq!(PmixStatus::from_raw(0),   PmixStatus::Known(PmixError::Success));
    /// assert_eq!(PmixStatus::from_raw(-1),  PmixStatus::Known(PmixError::Error));
    /// assert!(matches!(PmixStatus::from_raw(-99999), PmixStatus::Unknown(_)));
    /// ```
    pub fn from_raw(code: i32) -> Self {
        match PmixError::from_raw(code) {
            Some(e) => Self::Known(e),
            None    => Self::Unknown(code),
        }
    }

    /// Return the raw `i32` value.
    pub fn to_raw(self) -> i32 {
        match self {
            Self::Known(e)   => e as i32,
            Self::Unknown(v) => v,
        }
    }

    /// `true` for `PMIX_SUCCESS` and any positive informational code.
    pub fn is_success(self) -> bool {
        match self {
            Self::Known(e)   => e.is_success(),
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
            _              => None,
        }
    }
}

impl std::fmt::Display for PmixStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(e)   => e.fmt(f),
            Self::Unknown(v) => write!(f, "pmix_status_t({v}) [unknown/user-defined]"),
        }
    }
}

impl std::error::Error for PmixStatus {}

impl From<PmixError> for PmixStatus {
    fn from(e: PmixError) -> Self { Self::Known(e) }
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
            _    => return None,
        })
    }

    /// Return the raw `i32` discriminant (`pmix_status_t` value).
    #[inline]
    pub fn to_raw(self) -> i32 { self as i32 }

    /// `true` for `PMIX_SUCCESS` (0) and positive informational codes.
    ///
    /// Positive codes are used by event handlers to signal varying degrees
    /// of success rather than failure.
    #[inline]
    pub fn is_success(self) -> bool { (self as i32) >= 0 }

    /// `true` for any negative error code.
    #[inline]
    pub fn is_error(self) -> bool { !self.is_success() }

    /// The standard short-name string (e.g. `"PMIX_ERR_NOMEM"`).
    ///
    /// Mirrors the output of `PMIx_Error_string()` from the C library.
    pub fn name(self) -> &'static str {
        match self {
            Self::Success                           => "PMIX_SUCCESS",
            Self::Error                             => "PMIX_ERROR",
            Self::DebuggerRelease                   => "PMIX_DEBUGGER_RELEASE",
            Self::ErrProcRestart                    => "PMIX_ERR_PROC_RESTART",
            Self::ErrProcCheckpoint                 => "PMIX_ERR_PROC_CHECKPOINT",
            Self::ErrProcMigrate                    => "PMIX_ERR_PROC_MIGRATE",
            Self::ErrProcRequestedAbort             => "PMIX_ERR_PROC_REQUESTED_ABORT",
            Self::ErrExists                         => "PMIX_ERR_EXISTS",
            Self::ErrInvalidCred                    => "PMIX_ERR_INVALID_CRED",
            Self::ErrWouldBlock                     => "PMIX_ERR_WOULD_BLOCK",
            Self::ErrUnknownDataType                => "PMIX_ERR_UNKNOWN_DATA_TYPE",
            Self::ErrTypeMismatch                   => "PMIX_ERR_TYPE_MISMATCH",
            Self::ErrUnpackInadequateSpace          => "PMIX_ERR_UNPACK_INADEQUATE_SPACE",
            Self::ErrUnpackFailure                  => "PMIX_ERR_UNPACK_FAILURE",
            Self::ErrPackFailure                    => "PMIX_ERR_PACK_FAILURE",
            Self::ErrNoPermissions                  => "PMIX_ERR_NO_PERMISSIONS",
            Self::ErrTimeout                        => "PMIX_ERR_TIMEOUT",
            Self::ErrUnreach                        => "PMIX_ERR_UNREACH",
            Self::ErrBadParam                       => "PMIX_ERR_BAD_PARAM",
            Self::ErrResourceBusy                   => "PMIX_ERR_RESOURCE_BUSY",
            Self::ErrOutOfResource                  => "PMIX_ERR_OUT_OF_RESOURCE",
            Self::ErrInit                           => "PMIX_ERR_INIT",
            Self::ErrNomem                          => "PMIX_ERR_NOMEM",
            Self::ErrNotFound                       => "PMIX_ERR_NOT_FOUND",
            Self::ErrNotSupported                   => "PMIX_ERR_NOT_SUPPORTED",
            Self::ErrCommFailure                    => "PMIX_ERR_COMM_FAILURE",
            Self::ErrUnpackReadPastEndOfBuffer       => "PMIX_ERR_UNPACK_READ_PAST_END_OF_BUFFER",
            Self::ErrConflictingCleanupDirectives   => "PMIX_ERR_CONFLICTING_CLEANUP_DIRECTIVES",
            Self::ErrPartialSuccess                 => "PMIX_ERR_PARTIAL_SUCCESS",
            Self::ErrDuplicateKey                   => "PMIX_ERR_DUPLICATE_KEY",
            Self::ReadyForDebug                     => "PMIX_READY_FOR_DEBUG",
            Self::ErrParamValueNotSupported         => "PMIX_ERR_PARAM_VALUE_NOT_SUPPORTED",
            Self::ErrEmpty                          => "PMIX_ERR_EMPTY",
            Self::ErrLostConnection                 => "PMIX_ERR_LOST_CONNECTION",
            Self::ErrExistsOutsideScope             => "PMIX_ERR_EXISTS_OUTSIDE_SCOPE",
            Self::JctrlCheckpoint                   => "PMIX_JCTRL_CHECKPOINT",
            Self::JctrlCheckpointComplete           => "PMIX_JCTRL_CHECKPOINT_COMPLETE",
            Self::JctrlPreemptAlert                 => "PMIX_JCTRL_PREEMPT_ALERT",
            Self::MonitorHeartbeatAlert             => "PMIX_MONITOR_HEARTBEAT_ALERT",
            Self::MonitorFileAlert                  => "PMIX_MONITOR_FILE_ALERT",
            Self::FabricUpdateEndpoints             => "PMIX_FABRIC_UPDATE_ENDPOINTS",
            Self::ErrEventRegistration              => "PMIX_ERR_EVENT_REGISTRATION",
            Self::EventJobEnd                       => "PMIX_EVENT_JOB_END",
            Self::OperationInProgress               => "PMIX_OPERATION_IN_PROGRESS",
            Self::OperationSucceeded                => "PMIX_OPERATION_SUCCEEDED",
            Self::ErrInvalidOperation               => "PMIX_ERR_INVALID_OPERATION",
            Self::ErrRepeatAttrRegistration         => "PMIX_ERR_REPEAT_ATTR_REGISTRATION",
            Self::ErrIofFailure                     => "PMIX_ERR_IOF_FAILURE",
            Self::ErrIofComplete                    => "PMIX_ERR_IOF_COMPLETE",
            Self::FabricUpdated                     => "PMIX_FABRIC_UPDATED",
            Self::FabricUpdatePending               => "PMIX_FABRIC_UPDATE_PENDING",
            Self::ErrJobAppNotExecutable            => "PMIX_ERR_JOB_APP_NOT_EXECUTABLE",
            Self::ErrJobNoExeSpecified              => "PMIX_ERR_JOB_NO_EXE_SPECIFIED",
            Self::ErrJobFailedToMap                 => "PMIX_ERR_JOB_FAILED_TO_MAP",
            Self::ErrJobCanceled                    => "PMIX_ERR_JOB_CANCELED",
            Self::ErrJobFailedToLaunch              => "PMIX_ERR_JOB_FAILED_TO_LAUNCH",
            Self::ErrJobAborted                     => "PMIX_ERR_JOB_ABORTED",
            Self::ErrJobKilledByCmd                 => "PMIX_ERR_JOB_KILLED_BY_CMD",
            Self::ErrJobAbortedBySig                => "PMIX_ERR_JOB_ABORTED_BY_SIG",
            Self::ErrJobTermWoSync                  => "PMIX_ERR_JOB_TERM_WO_SYNC",
            Self::ErrJobSensorBoundExceeded         => "PMIX_ERR_JOB_SENSOR_BOUND_EXCEEDED",
            Self::ErrJobNonZeroTerm                 => "PMIX_ERR_JOB_NON_ZERO_TERM",
            Self::ErrJobAllocFailed                 => "PMIX_ERR_JOB_ALLOC_FAILED",
            Self::ErrJobAbortedBySysEvent           => "PMIX_ERR_JOB_ABORTED_BY_SYS_EVENT",
            Self::ErrJobExeNotFound                 => "PMIX_ERR_JOB_EXE_NOT_FOUND",
            Self::EventJobStart                     => "PMIX_EVENT_JOB_START",
            Self::EventSessionStart                 => "PMIX_EVENT_SESSION_START",
            Self::EventSessionEnd                   => "PMIX_EVENT_SESSION_END",
            Self::ErrProcTermWoSync                 => "PMIX_ERR_PROC_TERM_WO_SYNC",
            Self::EventProcTerminated               => "PMIX_EVENT_PROC_TERMINATED",
            Self::EventSysBase                      => "PMIX_EVENT_SYS_BASE",
            Self::EventNodeDown                     => "PMIX_EVENT_NODE_DOWN",
            Self::EventNodeOffline                  => "PMIX_EVENT_NODE_OFFLINE",
            Self::ErrJobWdirNotFound                => "PMIX_ERR_JOB_WDIR_NOT_FOUND",
            Self::ErrJobInsufficientResources       => "PMIX_ERR_JOB_INSUFFICIENT_RESOURCES",
            Self::ErrJobSysOpFailed                 => "PMIX_ERR_JOB_SYS_OP_FAILED",
            Self::EventSysOther                     => "PMIX_EVENT_SYS_OTHER",
            Self::EventNoActionTaken                => "PMIX_EVENT_NO_ACTION_TAKEN",
            Self::EventPartialActionTaken           => "PMIX_EVENT_PARTIAL_ACTION_TAKEN",
            Self::EventActionDeferred               => "PMIX_EVENT_ACTION_DEFERRED",
            Self::EventActionComplete               => "PMIX_EVENT_ACTION_COMPLETE",
            Self::ErrProcKilledByCmd                => "PMIX_ERR_PROC_KILLED_BY_CMD",
            Self::ErrProcFailedToStart              => "PMIX_ERR_PROC_FAILED_TO_START",
            Self::ErrProcAbortedBySig               => "PMIX_ERR_PROC_ABORTED_BY_SIG",
            Self::ErrProcSensorBoundExceeded        => "PMIX_ERR_PROC_SENSOR_BOUND_EXCEEDED",
            Self::ErrExitNonzeroTerm                => "PMIX_ERR_EXIT_NONZERO_TERM",
            Self::ExternalErrBase                   => "PMIX_EXTERNAL_ERR_BASE",
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
    KeyTooLong {
        len:     usize,
        maximum: usize,
    },
    /// No value was supplied before `build()` / `build_raw()` was called.
    MissingValue,
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyContainsNul(e)           => write!(f, "key contains interior NUL: {e}"),
            Self::KeyEmpty                     => write!(f, "key must not be empty"),
            Self::KeyTooLong { len, maximum }  =>
                write!(f, "key length {len} exceeds PMIX_MAX_KEYLEN ({maximum})"),
            Self::MissingValue                 => write!(f, "no value supplied to builder"),
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
            Self::ContainsNul(e)  => write!(f, "string contains interior NUL: {e}"),
            Self::MissingPayload  => write!(f, "no payload set on PmixValueBuilder"),
            Self::EmptyData       => write!(f, "data slice must not be empty"),
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
    fn from(e: NulError) -> Self { Self::ContainsNul(e) }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTimeval – Rust mirror of pmix_timeval_t
// ─────────────────────────────────────────────────────────────────────────────

/// Newtype so callers don't need to import `sys::pmix_timeval_t` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmixTimeval {
    pub tv_sec:  i64,
    pub tv_usec: i64,
}

impl From<PmixTimeval> for timeval {
    fn from(v: PmixTimeval) -> Self {
        Self { tv_sec: v.tv_sec, tv_usec: v.tv_usec }
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
    pub envar:     CString,
    pub value:     CString,
    pub separator: u8,
}

impl PmixEnvar {
    /// Create from `&str` arguments; returns `NulError` if either string has
    /// an interior NUL.
    pub fn new(envar: &str, value: &str, separator: char) -> Result<Self, NulError> {
        Ok(Self {
            envar:     CString::new(envar)?,
            value:     CString::new(value)?,
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
        unsafe {PMIx_Load_nspace(handle.nspace.as_mut_ptr(), c_name.as_ptr());}
        Ok(Proc{handle: handle, len:1})
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
        Ok(Proc{handle: handle, len:1})
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

    pub fn is_empty(self)        -> bool { self.0 == 0 }
    pub fn contains(self, f: Self) -> bool { (self.0 & f.0) == f.0 }
    /// Return the raw `pmix_info_directives_t` value.
    pub fn raw(self) -> pmix_info_directives_t { self.0 }
}

impl std::ops::BitOr for InfoFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}
impl std::ops::BitOrAssign for InfoFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
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
        elements:  Vec<pmix_value_t>,
    },
}

impl PmixPayload {
    /// Return the `pmix_data_type_t` constant that matches this variant.
    pub fn type_tag(&self) -> pmix_data_type_t {
        (match self {
            Self::Undef             => PMIX_UNDEF,
            Self::Bool(_)           => PMIX_BOOL,
            Self::Byte(_)           => PMIX_BYTE,
            Self::String(_)         => PMIX_STRING,
            Self::Size(_)           => PMIX_SIZE,
            Self::Pid(_)            => PMIX_PID,
            Self::Int(_)            => PMIX_INT,
            Self::Int8(_)           => PMIX_INT8,
            Self::Int16(_)          => PMIX_INT16,
            Self::Int32(_)          => PMIX_INT32,
            Self::Int64(_)          => PMIX_INT64,
            Self::Uint(_)           => PMIX_UINT,
            Self::Uint8(_)          => PMIX_UINT8,
            Self::Uint16(_)         => PMIX_UINT16,
            Self::Uint32(_)         => PMIX_UINT32,
            Self::Uint64(_)         => PMIX_UINT64,
            Self::Float(_)          => PMIX_FLOAT,
            Self::Double(_)         => PMIX_DOUBLE,
            Self::Timeval(_)        => PMIX_TIMEVAL,
            Self::Status(_)         => PMIX_STATUS,
            Self::Rank(_)           => PMIX_PROC_RANK,
            Self::Persist(_)        => PMIX_PERSIST,
            Self::Scope(_)          => PMIX_SCOPE,
            Self::DataRange(_)      => PMIX_DATA_RANGE,
            Self::ProcState(_)      => PMIX_PROC_STATE,
            Self::AllocDirective(_) => PMIX_ALLOC_DIRECTIVE,
            Self::IofChannel(_)     => PMIX_IOF_CHANNEL,
            Self::InfoDirectives(_) => PMIX_INFO_DIRECTIVES,
            Self::Proc(_)           => PMIX_PROC,
            Self::ByteObject(_)     => PMIX_BYTE_OBJECT,
            Self::Envar(_)          => PMIX_ENVAR,
            Self::Pointer(_)        => PMIX_POINTER,
            Self::DataArray { .. }  => PMIX_DATA_ARRAY,
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
/// ```rust
/// # use pmix_value_builder::PmixValueBuilder;
/// let owned = PmixValueBuilder::new().uint32(42).build()?;
/// # Ok::<(), pmix_value_builder::ValueError>(())
/// ```
#[derive(Default)]
pub struct PmixValueBuilder {
    payload: Option<PmixPayload>,
}

impl PmixValueBuilder {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new() -> Self { Self::default() }

    // ── Generic payload setter ────────────────────────────────────────────

    /// Set the payload to any [`PmixPayload`] variant.
    pub fn payload(mut self, p: PmixPayload) -> Self {
        self.payload = Some(p);
        self
    }

    // ── Typed scalar setters ──────────────────────────────────────────────

    pub fn undef(self)                              -> Self { self.payload(PmixPayload::Undef) }
    pub fn bool(self, v: bool)                      -> Self { self.payload(PmixPayload::Bool(v)) }
    pub fn byte(self, v: u8)                        -> Self { self.payload(PmixPayload::Byte(v)) }
    pub fn size(self, v: usize)                     -> Self { self.payload(PmixPayload::Size(v)) }
    pub fn pid(self, v: u32)                        -> Self { self.payload(PmixPayload::Pid(v)) }
    pub fn int(self, v: i32)                        -> Self { self.payload(PmixPayload::Int(v)) }
    pub fn int8(self, v: i8)                        -> Self { self.payload(PmixPayload::Int8(v)) }
    pub fn int16(self, v: i16)                      -> Self { self.payload(PmixPayload::Int16(v)) }
    pub fn int32(self, v: i32)                      -> Self { self.payload(PmixPayload::Int32(v)) }
    pub fn int64(self, v: i64)                      -> Self { self.payload(PmixPayload::Int64(v)) }
    pub fn uint(self, v: u32)                       -> Self { self.payload(PmixPayload::Uint(v)) }
    pub fn uint8(self, v: u8)                       -> Self { self.payload(PmixPayload::Uint8(v)) }
    pub fn uint16(self, v: u16)                     -> Self { self.payload(PmixPayload::Uint16(v)) }
    pub fn uint32(self, v: u32)                     -> Self { self.payload(PmixPayload::Uint32(v)) }
    pub fn uint64(self, v: u64)                     -> Self { self.payload(PmixPayload::Uint64(v)) }
    pub fn float(self, v: f32)                      -> Self { self.payload(PmixPayload::Float(v)) }
    pub fn double(self, v: f64)                     -> Self { self.payload(PmixPayload::Double(v)) }
    pub fn timeval(self, v: PmixTimeval)            -> Self { self.payload(PmixPayload::Timeval(v)) }
    pub fn status(self, v: pmix_status_t)           -> Self { self.payload(PmixPayload::Status(v)) }
    pub fn rank(self, v: pmix_rank_t)               -> Self { self.payload(PmixPayload::Rank(v)) }
    pub fn persist(self, v: pmix_persistence_t)     -> Self { self.payload(PmixPayload::Persist(v)) }
    pub fn scope(self, v: pmix_scope_t)             -> Self { self.payload(PmixPayload::Scope(v)) }
    pub fn data_range(self, v: pmix_data_range_t)   -> Self { self.payload(PmixPayload::DataRange(v)) }
    pub fn proc_state(self, v: pmix_proc_state_t)   -> Self { self.payload(PmixPayload::ProcState(v)) }
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
        if bytes.is_empty() { return Err(ValueError::EmptyData); }
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
        elements:  Vec<pmix_value_t>,
    ) -> Result<Self, ValueError> {
        if elements.is_empty() { return Err(ValueError::EmptyData); }
        Ok(self.payload(PmixPayload::DataArray { elem_type, elements }))
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
    /// ```rust
    /// # use pmix_value_builder::PmixValueBuilder;
    /// let (val, keys) = PmixValueBuilder::string_array(&["pmix.timeout", "pmix.collect"])?;
    /// let _pp: *const *const std::ffi::c_char = keys.as_ptr();
    /// # Ok::<(), pmix_value_builder::ValueError>(())
    /// ```
    pub fn string_array(
        strings: &[&str],
    ) -> Result<(PmixOwnedValue, CStringArray), ValueError> {
        if strings.is_empty() { return Err(ValueError::EmptyData); }

        // Build the pmix_value_t elements (each a PMIX_STRING).
        let elements: Result<Vec<pmix_value_t>, ValueError> = strings
            .iter()
            .map(|s| {
                PmixValueBuilder::new().string(s)?.build_raw()
            })
            .collect();

        let owned = PmixValueBuilder::new()
            .data_array(PMIX_STRING as pmix_data_type_t, elements?)?
            .build()?;

        // Build CStringArray from the same strings. CStringArray owns its own
        // CString copies, so the two lifetimes are fully independent.
        let cstrings: Result<Vec<CString>, NulError> =
            strings.iter().map(|s| CString::new(*s)).collect();
        let key_array = CStringArray::from_cstrings(cstrings?)
            .expect("strings are already validated CStrings");

        Ok((owned, key_array))
    }

    // ── Build ─────────────────────────────────────────────────────────────

    /// Validate and return a [`PmixOwnedValue`] (RAII — heap data freed on
    /// `Drop`).
    pub fn build(self) -> Result<PmixOwnedValue, ValueError> {
        Ok(PmixOwnedValue { inner: self.build_raw()? })
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
            PmixPayload::Undef              => { /* data stays zeroed */ }
            PmixPayload::Bool(v)            => dst.data.flag        = v,
            PmixPayload::Byte(v)            => dst.data.byte        = v,
            PmixPayload::Size(v)            => dst.data.size        = v,
            PmixPayload::Pid(v)             => dst.data.pid         = v as pid_t,
            PmixPayload::Int(v)             => dst.data.integer     = v,
            PmixPayload::Int8(v)            => dst.data.int8        = v,
            PmixPayload::Int16(v)           => dst.data.int16       = v,
            PmixPayload::Int32(v)           => dst.data.int32       = v,
            PmixPayload::Int64(v)           => dst.data.int64       = v,
            PmixPayload::Uint(v)            => dst.data.uint        = v,
            PmixPayload::Uint8(v)           => dst.data.uint8       = v,
            PmixPayload::Uint16(v)          => dst.data.uint16      = v,
            PmixPayload::Uint32(v)          => dst.data.uint32      = v,
            PmixPayload::Uint64(v)          => dst.data.uint64      = v,
            PmixPayload::Float(v)           => dst.data.fval        = v,
            PmixPayload::Double(v)          => dst.data.dval        = v,
            PmixPayload::Timeval(v)         => dst.data.tv          = v.into(),
            PmixPayload::Status(v)          => dst.data.status      = v,
            PmixPayload::Rank(v)            => dst.data.rank        = v,
            PmixPayload::Persist(v)         => dst.data.persist     = v,
            PmixPayload::Scope(v)           => dst.data.scope       = v,
            PmixPayload::DataRange(v)       => dst.data.range       = v,
            PmixPayload::ProcState(v)       => dst.data.state      = v,
            PmixPayload::AllocDirective(v)  => dst.data.adir        = v,
            PmixPayload::IofChannel(v)      => dst.data.uint16    = v,
            PmixPayload::InfoDirectives(v)  => dst.data.uint32  = v,

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
                dst.data.bo = pmix_byte_object_t { bytes: ptr, size: len };
            }

            // Heap-allocate pmix_envar_t; transfer both CStrings.
            PmixPayload::Envar(e) => {
                let raw = Box::new(pmix_envar_t {
                    envar:     e.envar.into_raw(),
                    value:     e.value.into_raw(),
                    separator: e.separator as i8,
                });
                dst.data.envar = *Box::into_raw(raw);
            }

            // Opaque pointer – no allocation here, caller owns data.
            PmixPayload::Pointer(p) => {
                dst.data.ptr = p;
            }

            // Leak Vec<pmix_value_t> into a heap-allocated pmix_data_array_t.
            PmixPayload::DataArray { elem_type, mut elements } => {
                elements.shrink_to_fit();
                let len = elements.len();
                let arr = elements.as_mut_ptr() as *mut std::ffi::c_void;
                std::mem::forget(elements);
                let darray = Box::new(pmix_data_array_t {
                    type_: elem_type,
                    size:  len,
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
            t if t == PMIX_STRING as u32 => {
                if !v.data.string.is_null() {
                    drop(CString::from_raw(v.data.string));
                    v.data.string = ptr::null_mut();
                }
            }
            t if t == PMIX_PROC as u32 => {
                if !v.data.proc_.is_null() {
                    drop(Box::from_raw(v.data.proc_));
                    v.data.proc_ = ptr::null_mut();
                }
            }
            t if t == PMIX_BYTE_OBJECT as u32 => {
                if !v.data.bo.bytes.is_null() && v.data.bo.size > 0 {
                    let _ = Vec::from_raw_parts(
                        v.data.bo.bytes as *mut u8,
                        v.data.bo.size,
                        v.data.bo.size,
                    );
                    v.data.bo.bytes = ptr::null_mut();
                    v.data.bo.size  = 0;
                }
            }
            t if t == PMIX_ENVAR => {
                    let e = Box::from_raw(&mut v.data.envar);
                    if !e.envar.is_null() { drop(CString::from_raw(e.envar)); }
                    if !e.value.is_null() { drop(CString::from_raw(e.value)); }
                    // Box<pmix_envar_t> is dropped here; strings already freed above.
            }
            t if t == PMIX_DATA_ARRAY => {
                if !v.data.darray.is_null() {
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
    pub fn as_raw(&self) -> *const pmix_value_t { &self.inner }

    /// Mutable raw pointer.
    pub fn as_raw_mut(&mut self) -> *mut pmix_value_t { &mut self.inner }

    /// Return the `pmix_data_type_t` tag.
    pub fn type_tag(&self) -> pmix_data_type_t { self.inner.type_ }

    /// Transfer ownership out of RAII; caller must free via `PMIX_VALUE_RELEASE`
    /// or [`free_value`].
    pub fn into_raw(self) -> pmix_value_t {
        let inner = self.inner;
        std::mem::forget(self);
        inner
    }

    pub fn bytes(&self) -> (*const c_void, usize) {
        let bytes = unsafe {self.inner.data.bo}; //.bytes.cast_const() as *const c_void
        (bytes.bytes.cast_const() as *const c_void, bytes.size)
    }

    pub fn size(&self) -> usize {
        unsafe { self.inner.data.size }
    }
}

impl Drop for PmixOwnedValue {
    fn drop(&mut self) { free_value(&mut self.inner); }
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
    pub(crate) handle:pmix_proc_t,
    len: usize,
}

pub struct Info {
    handle: *mut pmix_info_t,
    len: usize,
}

struct InfoEntry {
    key: &'static[u8; 13],
    value: *const std::ffi::c_void,
    data_type: pmix_data_type_t,
}

pub struct InfoBuilder {
    infos: Vec<InfoEntry>,
}

impl InfoBuilder {
    pub fn new() -> Self {
        Self { infos: Vec::new() }
    }

    pub fn add(&mut self, key: &'static [u8; 13], value: *const std::ffi::c_void, data_type: pmix_data_type_t) {
        assert_ne!(key.as_ptr(), std::ptr::null());
        self.infos.push(InfoEntry{key, value, data_type})
    }
    pub fn collect_data(&mut self) -> &mut InfoBuilder {
        let collect = true;
        self.add(PMIX_COLLECT_DATA, &collect as *const bool as *const c_void, PMIX_BOOL as pmix_data_type_t);
        self
    }
    pub fn build(self) -> Info {
        let info_ptr: *mut pmix_info_t;
        let mut idx: usize = 0;
        unsafe {info_ptr = PMIx_Info_create(self.infos.len())}
        for info in &self.infos {
            let status = unsafe {PMIx_Info_load(info_ptr.add(idx), info.key.as_ptr().cast(),
                                                info.value, info.data_type)};
            if status != PMIX_SUCCESS as i32 {
                panic!("Error loading info: {}", status);
            }
            idx += 1;
        }
        Info { handle: info_ptr, len: idx }
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
        unsafe { PMIx_Load_nspace(handle.nspace.as_mut_ptr(), self.proc.handle.nspace.as_ptr()); }
        Ok(Proc{handle, len:1})
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
        finalize(None).unwrap();
    }
}

pub fn init(info: Option<Info>) -> Result<Context, PmixError> {
    let proc: pmix_proc_t;
    let mut uninit_proc = mem::MaybeUninit::<pmix_proc_t>::uninit();
    let status: pmix_status_t;
    match info {
        Some(info) => {unsafe {
            status = PMIx_Init(uninit_proc.as_mut_ptr(), info.handle, info.len);
        }}
        None => {unsafe {
            status = PMIx_Init(uninit_proc.as_mut_ptr(), ptr::null_mut(), 0);
        }}
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        unsafe { proc = uninit_proc.assume_init();}
        Ok(Context { proc: Proc{handle: proc, len: 1  }})
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

    unsafe { status = PmixStatus::from_raw(PMIx_Get(&proc.handle, CStr::from_bytes_with_nul(key).unwrap().as_ptr(), info_handle, ninfos, &mut value));}

    if status.is_success() {
        Ok(PmixOwnedValue{inner: unsafe{*value}})
    } else {
        if let Some(known) = status.known() {
            Err(known)
        } else {
            Err(PmixError::Error)
        }
    }
}

pub fn put_value(scope: pmix_scope_t, key: &CStr, value: &mut PmixOwnedValue) -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe { status = PMIx_Put(scope, key.as_ptr(), &mut value.inner);}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn commit() -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe { status = PMIx_Commit();}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn fence(proc: &Proc, info: Option<Info>) -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    let proc_handle: *const pmix_proc_t;
    let nprocs: usize;
    let info_handle: *const pmix_info_t;
    let ninfos: usize;

    proc_handle = &proc.handle;
    if proc_handle.is_null() {
        nprocs = 0;
    } else {
        nprocs = proc.len;
    }
    match info {
        Some(info) => {
            info_handle = info.handle;
            if proc_handle.is_null() {
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

    unsafe {status = PMIx_Fence(proc_handle, nprocs, info_handle, ninfos);}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn get_version() -> &'static str {
    let version: &CStr;
    unsafe {
        version = CStr::from_ptr(PMIx_Get_version());
    }
    version.to_str().unwrap()
}
pub fn progress() {
    unsafe {PMIx_Progress();}
}

pub fn finalize(info: Option<Info>) -> Result<(),pmix_status_t> {
    let status: pmix_status_t;
    match info {
        Some(x) => unsafe {status = PMIx_Finalize(x.handle, x.len);}
        None => unsafe {status = PMIx_Finalize(ptr::null_mut(), 0);}
    }
    if status as u32 == PMIX_SUCCESS { Result::Ok(())} else { Result::Err(status) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_version() {
        let _proc = init(None).unwrap();
    }
}