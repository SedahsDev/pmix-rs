//! Comprehensive integration tests for all 12 PMIx utility string conversion
//! functions in `src/utility.rs`.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — the string conversion functions only perform local lookups
//! of static string tables inside the library.
//!
//! Functions tested:
//!  1. error_string
//!  2. data_type_string
//!  3. data_range_string
//!  4. persistence_string
//!  5. scope_string
//!  6. proc_state_string
//!  7. job_state_string
//!  8. link_state_string
//!  9. iof_channel_string
//! 10. device_type_string
//! 11. info_directives_string
//! 12. alloc_directive_string

use pmix::{
    IOFChannelFlags, InfoFlags, PmixAllocDirective, PmixDataRange, PmixDataType, PmixDeviceType,
    PmixJobState, PmixLinkState, PmixPersistence, PmixProcState, PmixScope, PmixStatus, utility::*,
};

// ══════════════════════════════════════════════════════════════════════════════
// 1. error_string(status: PmixStatus) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_string_success() {
    let result = error_string(PmixStatus::from_raw(0));
    assert!(
        result.is_ok(),
        "error_string(SUCCESS) should return Ok, got {:?}",
        result
    );
    assert!(!result.unwrap().is_empty());
}

#[test]
fn error_string_generic_error() {
    let result = error_string(PmixStatus::from_raw(-1));
    assert!(
        result.is_ok(),
        "error_string(ERROR) should return Ok, got {:?}",
        result
    );
}

#[test]
fn error_string_all_known_error_codes() {
    // Every known PmixError variant, grouped by category
    let codes: Vec<i32> = vec![
        // Success / informational
        0, // Base error codes
        -1, -3, -4, -5, -6, -8, -11, -12, -15, -16, -18, -19, -20, -21, -23, -24, -25, -27, -28,
        -29, -31, -32, // Data / lookup errors
        -46, -47, -49, -50, -51, -52, -53, -58, -59, -60, -61, -62,
        // Job-control event codes
        -106, -107, -108, // Monitoring alert codes
        -109, -110, // Fabric / network event codes
        -113, // Internal / registration errors
        -144, // Job lifecycle event codes
        -145, // Operational-state codes
        -156, -157, -158, // Attribute / registration errors
        -171, // I/O-forwarding codes
        -172, -173, // Fabric status codes
        -175, -176, // Job-level error codes
        -177, -178, -179, -180, -181, -182, -183, -184, -185, -186, -187, -188, -189, -190,
        // Job-lifecycle event codes
        -191, -192, -193, // Process-level error codes
        -200, -201, // System-event codes
        -230, -231, -232, // Additional job-level errors
        -233, -234, -235, // System-event "other" range
        -330, // Event-handler return codes
        -331, -332, -333, -334, // Per-process error codes
        -400, -401, -402, -403, -404, // External boundary
        -3000,
    ];
    for code in codes {
        let status = PmixStatus::from_raw(code);
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string({}) should return Ok, got {:?}",
            code,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "error_string({}) should not return empty string",
            code
        );
    }
}

#[test]
fn error_string_unknown_status() {
    let status = PmixStatus::from_raw(-10001);
    let result = error_string(status);
    assert!(
        result.is_ok(),
        "error_string should handle unknown codes gracefully, got {:?}",
        result
    );
}

#[test]
fn error_string_distinct() {
    let success = error_string(PmixStatus::from_raw(0)).unwrap();
    let error = error_string(PmixStatus::from_raw(-1)).unwrap();
    assert_ne!(
        success, error,
        "SUCCESS and ERROR must produce different strings"
    );
}

#[test]
fn error_string_deterministic() {
    let first = error_string(PmixStatus::from_raw(-24)).unwrap();
    let second = error_string(PmixStatus::from_raw(-24)).unwrap();
    assert_eq!(first, second, "error_string must be deterministic");
}

#[test]
fn error_string_return_type() {
    let _r: Result<String, PmixStatus> = error_string(PmixStatus::from_raw(0));
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. data_type_string(ty: PmixDataType) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn data_type_string_scalar_types() {
    let types = [
        PmixDataType::Undef,
        PmixDataType::Bool,
        PmixDataType::Byte,
        PmixDataType::String,
        PmixDataType::Size,
        PmixDataType::Pid,
        PmixDataType::Int,
        PmixDataType::Int8,
        PmixDataType::Int16,
        PmixDataType::Int32,
        PmixDataType::Int64,
        PmixDataType::Uint,
        PmixDataType::Uint8,
        PmixDataType::Uint16,
        PmixDataType::Uint32,
        PmixDataType::Uint64,
        PmixDataType::Float,
        PmixDataType::Double,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "data_type_string({:?}) should not be empty",
            ty
        );
    }
}

#[test]
fn data_type_string_composite_types() {
    let types = [
        PmixDataType::Timeval,
        PmixDataType::Time,
        PmixDataType::Status,
        PmixDataType::Value,
        PmixDataType::Proc,
        PmixDataType::App,
        PmixDataType::Info,
        PmixDataType::Pdata,
        PmixDataType::ByteObject,
        PmixDataType::Kval,
        PmixDataType::Persist,
        PmixDataType::Pointer,
        PmixDataType::Scope,
        PmixDataType::DataRange,
        PmixDataType::Command,
        PmixDataType::InfoDirectives,
        PmixDataType::DataType,
        PmixDataType::ProcState,
        PmixDataType::ProcInfo,
        PmixDataType::DataArray,
        PmixDataType::ProcRank,
        PmixDataType::Query,
        PmixDataType::CompressedString,
        PmixDataType::AllocDirective,
        PmixDataType::IofChannel,
        PmixDataType::Envar,
        PmixDataType::Coord,
        PmixDataType::Regattr,
        PmixDataType::Regex,
        PmixDataType::JobState,
        PmixDataType::LinkState,
        PmixDataType::ProcCpuset,
        PmixDataType::Geometry,
        PmixDataType::DeviceDist,
        PmixDataType::Endpoint,
        PmixDataType::Topo,
        PmixDataType::Devtype,
        PmixDataType::LocType,
        PmixDataType::CompressedByteObject,
        PmixDataType::ProcNspace,
        PmixDataType::ProcStats,
        PmixDataType::DiskStats,
        PmixDataType::NetStats,
        PmixDataType::NodeStats,
        PmixDataType::DataBuffer,
        PmixDataType::StorMedium,
        PmixDataType::StorAccess,
        PmixDataType::StorPersist,
        PmixDataType::StorAccessType,
    ];
    for ty in types {
        let result = data_type_string(ty);
        assert!(
            result.is_ok(),
            "data_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "data_type_string({:?}) should not be empty",
            ty
        );
    }
}

#[test]
fn data_type_string_unknown() {
    let result = data_type_string(PmixDataType::Unknown);
    assert!(
        result.is_ok(),
        "data_type_string(Unknown) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn data_type_string_distinct() {
    let string_desc = data_type_string(PmixDataType::String).unwrap();
    let int_desc = data_type_string(PmixDataType::Int).unwrap();
    assert_ne!(string_desc, int_desc, "STRING and INT must differ");
}

#[test]
fn data_type_string_deterministic() {
    let first = data_type_string(PmixDataType::Int64).unwrap();
    let second = data_type_string(PmixDataType::Int64).unwrap();
    assert_eq!(first, second, "data_type_string must be deterministic");
}

#[test]
fn data_type_string_return_type() {
    let _r: Result<String, PmixStatus> = data_type_string(PmixDataType::Bool);
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. data_range_string(range: PmixDataRange) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn data_range_string_all_defined() {
    let values = [
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
        PmixDataRange::Session,
        PmixDataRange::Global,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
        PmixDataRange::Invalid,
    ];
    for v in values {
        let result = data_range_string(v);
        assert!(
            result.is_ok(),
            "data_range_string({:?}) should return Ok, got {:?}",
            v,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "data_range_string({:?}) should not be empty",
            v
        );
    }
}

#[test]
fn data_range_string_unknown() {
    let result = data_range_string(PmixDataRange::Unknown);
    assert!(
        result.is_ok(),
        "data_range_string(Unknown) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn data_range_string_distinct() {
    let local = data_range_string(PmixDataRange::Local).unwrap();
    let global = data_range_string(PmixDataRange::Global).unwrap();
    assert_ne!(local, global, "Local and Global must differ");
}

#[test]
fn data_range_string_deterministic() {
    let first = data_range_string(PmixDataRange::Session).unwrap();
    let second = data_range_string(PmixDataRange::Session).unwrap();
    assert_eq!(first, second, "data_range_string must be deterministic");
}

#[test]
fn data_range_string_return_type() {
    let _r: Result<String, PmixStatus> = data_range_string(PmixDataRange::Session);
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. persistence_string(persist: PmixPersistence) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn persistence_string_all_defined() {
    let values = [
        PmixPersistence::Indefinite,
        PmixPersistence::FirstRead,
        PmixPersistence::Process,
        PmixPersistence::Application,
        PmixPersistence::Session,
        PmixPersistence::Invalid,
    ];
    for v in values {
        let result = persistence_string(v);
        assert!(
            result.is_ok(),
            "persistence_string({:?}) should return Ok, got {:?}",
            v,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "persistence_string({:?}) should not be empty",
            v
        );
    }
}

#[test]
fn persistence_string_unknown() {
    let result = persistence_string(PmixPersistence::Unknown(42));
    assert!(
        result.is_ok(),
        "persistence_string(Unknown(42)) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn persistence_string_distinct() {
    let indef = persistence_string(PmixPersistence::Indefinite).unwrap();
    let first = persistence_string(PmixPersistence::FirstRead).unwrap();
    assert_ne!(indef, first, "Indefinite and FirstRead must differ");
}

#[test]
fn persistence_string_deterministic() {
    let first = persistence_string(PmixPersistence::Process).unwrap();
    let second = persistence_string(PmixPersistence::Process).unwrap();
    assert_eq!(first, second, "persistence_string must be deterministic");
}

#[test]
fn persistence_string_return_type() {
    let _r: Result<String, PmixStatus> = persistence_string(PmixPersistence::Indefinite);
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. scope_string(scope: PmixScope) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn scope_string_all_defined() {
    let scopes = [
        PmixScope::Undef,
        PmixScope::Local,
        PmixScope::Remote,
        PmixScope::Global,
        PmixScope::Internal,
    ];
    for scope in scopes {
        let result = scope_string(scope);
        assert!(
            result.is_ok(),
            "scope_string({:?}) should return Ok, got {:?}",
            scope,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "scope_string({:?}) should not return empty string",
            scope
        );
    }
}

#[test]
fn scope_string_unknown() {
    let result = scope_string(PmixScope::Unknown(99));
    assert!(
        result.is_ok(),
        "scope_string(Unknown(99)) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn scope_string_distinct() {
    let local = scope_string(PmixScope::Local).unwrap();
    let global = scope_string(PmixScope::Global).unwrap();
    assert_ne!(local, global, "Local and Global must differ");
}

#[test]
fn scope_string_deterministic() {
    let first = scope_string(PmixScope::Remote).unwrap();
    let second = scope_string(PmixScope::Remote).unwrap();
    assert_eq!(first, second, "scope_string must be deterministic");
}

#[test]
fn scope_string_return_type() {
    let _r: Result<String, PmixStatus> = scope_string(PmixScope::Local);
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. proc_state_string(state: PmixProcState) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

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
        let result = proc_state_string(state);
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
        let result = proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string({:?}) should succeed, got {:?}",
            state,
            result
        );
    }
}

#[test]
fn proc_state_string_unknown() {
    let result = proc_state_string(PmixProcState::Unknown(99));
    assert!(
        result.is_ok(),
        "proc_state_string(Unknown(99)) should succeed, got {:?}",
        result
    );
}

#[test]
fn proc_state_string_distinct() {
    let running = proc_state_string(PmixProcState::Running).unwrap();
    let terminated = proc_state_string(PmixProcState::Terminated).unwrap();
    assert_ne!(running, terminated, "Running and Terminated must differ");
}

#[test]
fn proc_state_string_deterministic() {
    let first = proc_state_string(PmixProcState::Terminated).unwrap();
    let second = proc_state_string(PmixProcState::Terminated).unwrap();
    assert_eq!(first, second, "proc_state_string must be deterministic");
}

#[test]
fn proc_state_string_return_type() {
    let _r: Result<String, PmixStatus> = proc_state_string(PmixProcState::Running);
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. job_state_string(state: PmixJobState) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

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
fn job_state_string_unknown() {
    let result = job_state_string(PmixJobState::Unknown(99));
    assert!(
        result.is_ok(),
        "job_state_string(Unknown(99)) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn job_state_string_distinct() {
    let running = job_state_string(PmixJobState::Running).unwrap();
    let term = job_state_string(PmixJobState::Terminated).unwrap();
    assert_ne!(running, term, "Running and Terminated must differ");
}

#[test]
fn job_state_string_deterministic() {
    let first = job_state_string(PmixJobState::Connected).unwrap();
    let second = job_state_string(PmixJobState::Connected).unwrap();
    assert_eq!(first, second, "job_state_string must be deterministic");
}

#[test]
fn job_state_string_return_type() {
    let _r: Result<String, PmixStatus> = job_state_string(PmixJobState::Running);
}

// ══════════════════════════════════════════════════════════════════════════════
// 8. link_state_string(state: PmixLinkState) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn link_state_string_all_defined() {
    let states = [
        PmixLinkState::UnknownState,
        PmixLinkState::LinkDown,
        PmixLinkState::LinkUp,
    ];
    for state in states {
        let result = link_state_string(state);
        assert!(
            result.is_ok(),
            "link_state_string({:?}) should return Ok, got {:?}",
            state,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "link_state_string({:?}) should not return empty string",
            state
        );
    }
}

#[test]
fn link_state_string_unknown() {
    let result = link_state_string(PmixLinkState::Unknown(99));
    assert!(
        result.is_ok(),
        "link_state_string(Unknown(99)) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn link_state_string_distinct() {
    let up = link_state_string(PmixLinkState::LinkUp).unwrap();
    let down = link_state_string(PmixLinkState::LinkDown).unwrap();
    assert_ne!(up, down, "LinkUp and LinkDown must differ");
}

#[test]
fn link_state_string_deterministic() {
    let first = link_state_string(PmixLinkState::LinkUp).unwrap();
    let second = link_state_string(PmixLinkState::LinkUp).unwrap();
    assert_eq!(first, second, "link_state_string must be deterministic");
}

#[test]
fn link_state_string_return_type() {
    let _r: Result<String, PmixStatus> = link_state_string(PmixLinkState::LinkUp);
}

// ══════════════════════════════════════════════════════════════════════════════
// 9. iof_channel_string(channel: IOFChannelFlags) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn iof_channel_string_all_defined() {
    let channels = [
        IOFChannelFlags::NO_CHANNELS,
        IOFChannelFlags::STDIN,
        IOFChannelFlags::STDOUT,
        IOFChannelFlags::STDERR,
        IOFChannelFlags::STDDIAG,
        IOFChannelFlags::ALL_CHANNELS,
    ];
    for c in channels {
        let result = iof_channel_string(c);
        assert!(
            result.is_ok(),
            "iof_channel_string({:?}) should return Ok, got {:?}",
            c,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "iof_channel_string({:?}) should not be empty",
            c
        );
    }
}

#[test]
fn iof_channel_string_combinations() {
    // Bitflags support combinations — test a few common combos
    let combos = [
        IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR,
        IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT,
        IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR,
    ];
    for c in combos {
        let result = iof_channel_string(c);
        assert!(
            result.is_ok(),
            "iof_channel_string({:?}) should return Ok for bitflag combos, got {:?}",
            c,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "iof_channel_string({:?}) should not be empty",
            c
        );
    }
}

#[test]
fn iof_channel_string_distinct() {
    let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
    let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
    assert_ne!(stdin, stdout, "STDIN and STDOUT must differ");
}

#[test]
fn iof_channel_string_deterministic() {
    let first = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
    let second = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
    assert_eq!(first, second, "iof_channel_string must be deterministic");
}

#[test]
fn iof_channel_string_return_type() {
    let _r: Result<String, PmixStatus> = iof_channel_string(IOFChannelFlags::STDOUT);
}

// ══════════════════════════════════════════════════════════════════════════════
// 10. device_type_string(ty: PmixDeviceType) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn device_type_string_all_defined() {
    let types = [
        PmixDeviceType::UnknownType,
        PmixDeviceType::Block,
        PmixDeviceType::Gpu,
        PmixDeviceType::Network,
        PmixDeviceType::OpenFabrics,
        PmixDeviceType::Dma,
        PmixDeviceType::Coproc,
    ];
    for ty in types {
        let result = device_type_string(ty);
        assert!(
            result.is_ok(),
            "device_type_string({:?}) should return Ok, got {:?}",
            ty,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "device_type_string({:?}) should not return empty string",
            ty
        );
    }
}

#[test]
fn device_type_string_unknown() {
    let result = device_type_string(PmixDeviceType::Unknown(0xFF));
    assert!(
        result.is_ok(),
        "device_type_string(Unknown(0xFF)) should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn device_type_string_distinct() {
    let gpu = device_type_string(PmixDeviceType::Gpu).unwrap();
    let network = device_type_string(PmixDeviceType::Network).unwrap();
    assert_ne!(gpu, network, "GPU and NETWORK must differ");
}

#[test]
fn device_type_string_deterministic() {
    let first = device_type_string(PmixDeviceType::Gpu).unwrap();
    let second = device_type_string(PmixDeviceType::Gpu).unwrap();
    assert_eq!(first, second, "device_type_string must be deterministic");
}

#[test]
fn device_type_string_return_type() {
    let _r: Result<String, PmixStatus> = device_type_string(PmixDeviceType::Gpu);
}

// ══════════════════════════════════════════════════════════════════════════════
// 11. info_directives_string(directives: InfoFlags) -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn info_directives_string_all_defined() {
    let directives = [
        InfoFlags::REQD,
        InfoFlags::QUALIFIER,
        InfoFlags::PERSISTENT,
        InfoFlags::REQD_PROCESSED,
    ];
    for d in directives {
        let result = info_directives_string(d);
        assert!(
            result.is_ok(),
            "info_directives_string({:?}) should return Ok, got {:?}",
            d,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "info_directives_string({:?}) should not be empty",
            d
        );
    }
}

#[test]
fn info_directives_string_combinations() {
    // Bitflags support combinations
    let combos = [
        InfoFlags::REQD | InfoFlags::PERSISTENT,
        InfoFlags::QUALIFIER | InfoFlags::REQD,
    ];
    for d in combos {
        let result = info_directives_string(d);
        assert!(
            result.is_ok(),
            "info_directives_string({:?}) should return Ok for combos, got {:?}",
            d,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "info_directives_string({:?}) should not be empty",
            d
        );
    }
}

#[test]
fn info_directives_string_distinct() {
    let reqd = info_directives_string(InfoFlags::REQD).unwrap();
    let persistent = info_directives_string(InfoFlags::PERSISTENT).unwrap();
    assert_ne!(reqd, persistent, "REQD and PERSISTENT must differ");
}

#[test]
fn info_directives_string_deterministic() {
    let first = info_directives_string(InfoFlags::REQD).unwrap();
    let second = info_directives_string(InfoFlags::REQD).unwrap();
    assert_eq!(
        first, second,
        "info_directives_string must be deterministic"
    );
}

#[test]
fn info_directives_string_return_type() {
    let _r: Result<String, PmixStatus> = info_directives_string(InfoFlags::REQD);
}

// ══════════════════════════════════════════════════════════════════════════════
// 12. alloc_directive_string(directive: PmixAllocDirective)
//     -> Result<String, PmixStatus>
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn alloc_directive_string_known() {
    let result = alloc_directive_string(PmixAllocDirective::AllocDirective);
    assert!(
        result.is_ok(),
        "alloc_directive_string(AllocDirective) should return Ok, got {:?}",
        result
    );
    assert!(
        !result.unwrap().is_empty(),
        "alloc_directive_string should not return empty string"
    );
}

#[test]
fn alloc_directive_string_unknown() {
    let result = alloc_directive_string(PmixAllocDirective::Unknown(99));
    assert!(
        result.is_ok(),
        "alloc_directive_string(Unknown(99)) should return Ok, got {:?}",
        result
    );
}

#[test]
fn alloc_directive_string_deterministic() {
    let first = alloc_directive_string(PmixAllocDirective::AllocDirective).unwrap();
    let second = alloc_directive_string(PmixAllocDirective::AllocDirective).unwrap();
    assert_eq!(
        first, second,
        "alloc_directive_string must be deterministic"
    );
}

#[test]
fn alloc_directive_string_return_type() {
    let _r: Result<String, PmixStatus> = alloc_directive_string(PmixAllocDirective::AllocDirective);
}

// ══════════════════════════════════════════════════════════════════════════════
// Cross-function: all string converters return non-empty for their zero values
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_string_converters_handle_zero_values() {
    // Every function should handle the "undefined" / zero value gracefully
    assert!(error_string(PmixStatus::from_raw(0)).unwrap().is_empty() == false);
    assert!(data_type_string(PmixDataType::Undef).unwrap().is_empty() == false);
    assert!(data_range_string(PmixDataRange::Undef).unwrap().is_empty() == false);
    assert!(
        persistence_string(PmixPersistence::Indefinite)
            .unwrap()
            .is_empty()
            == false
    );
    assert!(scope_string(PmixScope::Undef).unwrap().is_empty() == false);
    assert!(proc_state_string(PmixProcState::Undef).unwrap().is_empty() == false);
    assert!(job_state_string(PmixJobState::Undef).unwrap().is_empty() == false);
    assert!(
        link_state_string(PmixLinkState::UnknownState)
            .unwrap()
            .is_empty()
            == false
    );
    assert!(
        iof_channel_string(IOFChannelFlags::NO_CHANNELS)
            .unwrap()
            .is_empty()
            == false
    );
    assert!(
        device_type_string(PmixDeviceType::UnknownType)
            .unwrap()
            .is_empty()
            == false
    );
    // InfoFlags::default() is empty bitflags (0x0)
    assert!(
        info_directives_string(InfoFlags::default())
            .unwrap()
            .is_empty()
            == false
    );
    // alloc_directive_string has no zero variant — use the known one
    assert!(
        alloc_directive_string(PmixAllocDirective::AllocDirective)
            .unwrap()
            .is_empty()
            == false
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Cross-function: distinct outputs for different inputs across all converters
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_string_converters_produce_distinct_outputs() {
    // error_string
    assert_ne!(
        error_string(PmixStatus::from_raw(0)).unwrap(),
        error_string(PmixStatus::from_raw(-1)).unwrap()
    );
    // data_type_string
    assert_ne!(
        data_type_string(PmixDataType::String).unwrap(),
        data_type_string(PmixDataType::Int).unwrap()
    );
    // data_range_string
    assert_ne!(
        data_range_string(PmixDataRange::Local).unwrap(),
        data_range_string(PmixDataRange::Global).unwrap()
    );
    // persistence_string
    assert_ne!(
        persistence_string(PmixPersistence::Indefinite).unwrap(),
        persistence_string(PmixPersistence::FirstRead).unwrap()
    );
    // scope_string
    assert_ne!(
        scope_string(PmixScope::Local).unwrap(),
        scope_string(PmixScope::Global).unwrap()
    );
    // proc_state_string
    assert_ne!(
        proc_state_string(PmixProcState::Running).unwrap(),
        proc_state_string(PmixProcState::Terminated).unwrap()
    );
    // job_state_string
    assert_ne!(
        job_state_string(PmixJobState::Running).unwrap(),
        job_state_string(PmixJobState::Terminated).unwrap()
    );
    // link_state_string
    assert_ne!(
        link_state_string(PmixLinkState::LinkUp).unwrap(),
        link_state_string(PmixLinkState::LinkDown).unwrap()
    );
    // iof_channel_string
    assert_ne!(
        iof_channel_string(IOFChannelFlags::STDIN).unwrap(),
        iof_channel_string(IOFChannelFlags::STDOUT).unwrap()
    );
    // device_type_string
    assert_ne!(
        device_type_string(PmixDeviceType::Gpu).unwrap(),
        device_type_string(PmixDeviceType::Network).unwrap()
    );
    // info_directives_string
    assert_ne!(
        info_directives_string(InfoFlags::REQD).unwrap(),
        info_directives_string(InfoFlags::PERSISTENT).unwrap()
    );
    // alloc_directive_string — only one known variant; the library may return
    // the same string for AllocDirective and Unknown(0) ("UNSPECIFIED"),
    // so we skip the distinctness check for this one-function corner case.
    // Instead, just verify both produce non-empty results.
    {
        let known = alloc_directive_string(PmixAllocDirective::AllocDirective).unwrap();
        let unknown = alloc_directive_string(PmixAllocDirective::Unknown(0)).unwrap();
        assert!(
            !known.is_empty(),
            "AllocDirective string should not be empty"
        );
        assert!(!unknown.is_empty(), "Unknown(0) string should not be empty");
    }
}
