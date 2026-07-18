#![allow(unused_imports)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::ptr_offset_with_cast)]

pub mod allocation;
pub mod cpu_locality;
pub mod data_ops;
pub mod data_serialization;
pub mod error;
pub mod events;
pub mod fabric;
#[allow(clippy::upper_case_acronyms, clippy::enum_variant_names)]
mod ffi;
pub mod groups;
pub mod info;
pub mod mock_ffi;
pub mod monitoring;
pub mod process_mgmt;
pub mod proc;
pub mod query_log;
pub mod security;
pub mod server;
pub mod tool;
pub mod types;
pub mod utility;
pub mod value;

pub use error::{PmixError, PmixStatus};
pub use info::{Info, InfoBuilder, InfoFlags, info_with_string_key};
pub use proc::Proc;
pub use types::{
    BuilderError, IOFChannelFlags, PmixAllocDirective, PmixDataRange, PmixDataType, PmixDeviceType,
    PmixJobState, PmixLinkState, PmixPersistence, PmixProcState, PmixScope,
};
pub use value::{
    PmixEnvar, PmixOwnedValue, PmixPayload, PmixTimeval, PmixValueBuilder, ValueError, free_value,
};

use crate::ffi::*;
use std::ffi::{CStr, CString, NulError};
use std::mem;
use std::os::raw::{c_char, c_void};
use std::ptr::{null, null_mut};
use std::{fmt, ptr};

pub const GLOBAL: u8 = PMIX_GLOBAL as u8;
pub const NUM_NODES: &[u8; 15] = PMIX_NUM_NODES;
pub const JOB_SIZE: &[u8; 14] = PMIX_JOB_SIZE;
pub const RANK_WILDCARD: u32 = PMIX_RANK_WILDCARD;

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
        finalize(None).expect("invariant: unwrap in lib.rs");
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
            CStr::from_bytes_with_nul(key).expect("invariant: unwrap in lib.rs").as_ptr(),
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
    version.to_str().expect("CStr to_str: invalid UTF-8 (lib.rs)")
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
