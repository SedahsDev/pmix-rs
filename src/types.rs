//! PMIx enumeration and flag types.

use crate::ffi::*;
use std::ffi::NulError;
use std::fmt;

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

