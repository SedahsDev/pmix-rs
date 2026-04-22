#![allow(unused_imports)]

mod ffi;
mod info;

use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::c_void;
use std::ptr;
use std::ptr::{null, null_mut};
use crate::ffi::*;
use cstring_array::CStringArray;

// get_version()
// info lists
// PMIx_Get()
// PMIx_Put()
// PMIx_Commit()
// PMIx_Fence()

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
// PmixValue — safe typed payload for pmix_value_t
// ─────────────────────────────────────────────────────────────────────────────

/// Typed payload, mirroring the `pmix_val_data` union discriminated by
/// `pmix_data_type_t`.
///
/// `CString`-based variants own their data; all other variants are `Copy`-able
/// scalars. `DataArray` boxes a `Vec<pmix_info_t>` for nested arrays.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PmixValue {
    Undef,
    Bool(bool),
    Byte(u8),
    /// Heap-allocated C string. The builder will call `into_raw()` to hand
    /// ownership to the C struct; the `Drop` impl on `PmixOwnedInfo` frees it.
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
    Status(pmix_status_t),
    Rank(pmix_rank_t),
}

impl PmixValue {
    /// Convenience: build a `PmixValue::String` from any `&str`, propagating
    /// `NulError` if an interior NUL is present.
    pub fn from_str(s: &str) -> Result<Self, NulError> {
        CString::new(s).map(Self::String)
    }

    /// Return the `pmix_data_type_t` discriminant for this variant.
    pub fn type_tag(&self) -> pmix_data_type_t {
        match self {
            Self::Undef       => PMIX_UNDEF   as _,
            Self::Bool(_)     => PMIX_BOOL    as _,
            Self::Byte(_)     => PMIX_BYTE    as _,
            Self::String(_)   => PMIX_STRING  as _,
            Self::Size(_)     => PMIX_SIZE    as _,
            Self::Pid(_)      => PMIX_PID     as _,
            Self::Int(_)      => PMIX_INT     as _,
            Self::Int8(_)     => PMIX_INT8    as _,
            Self::Int16(_)    => PMIX_INT16   as _,
            Self::Int32(_)    => PMIX_INT32   as _,
            Self::Int64(_)    => PMIX_INT64   as _,
            Self::Uint(_)     => PMIX_UINT    as _,
            Self::Uint8(_)    => PMIX_UINT8   as _,
            Self::Uint16(_)   => PMIX_UINT16  as _,
            Self::Uint32(_)   => PMIX_UINT32  as _,
            Self::Uint64(_)   => PMIX_UINT64  as _,
            Self::Float(_)    => PMIX_FLOAT   as _,
            Self::Double(_)   => PMIX_DOUBLE  as _,
            Self::Status(_)   => PMIX_STATUS  as _,
            Self::Rank(_)     => PMIX_PROC_RANK as _,
        }
    }
}

pub struct Proc {
    pub(crate) handle: pmix_proc_t,
    len: usize,
}

pub struct Info {
    handle: pmix_info_t,
    len: usize,
}

struct InfoEntry {
    key: CString,
    value: std::ffi::c_void,
    data_type: pmix_data_type_t,
}

pub struct InfoBuilder {
    infos: Vec<InfoEntry>,
}

impl InfoBuilder {
    pub fn new() -> Self {
        Self { infos: Vec::new() }
    }

    pub fn add(&mut self, key: CString, value: std::ffi::c_void, data_type: pmix_data_type_t) {
        assert_ne!(key.as_ptr(), std::ptr::null());
        self.infos.push(InfoEntry{key, value, data_type})
    }
    pub fn build(self) -> Info {
        let info_ptr: *mut pmix_info_t;
        let mut idx: usize = 0;
        unsafe {info_ptr = PMIx_Info_create(self.infos.len())}
        for info in &self.infos {
            let status = unsafe {PMIx_Info_load(info_ptr.add(idx), info.key.as_ptr(),
                                                &info.value, info.data_type)};
            if status != PMIX_SUCCESS as i32 {
                panic!("Error loading info: {}", status);
            }
            idx += 1;
        }
        Info { handle: unsafe{ *info_ptr }, len: idx }
    }
}

pub fn init(info: Option<Info>) -> Result<Proc, pmix_status_t>{
    let proc: *mut pmix_proc_t = null_mut();
    let status: pmix_status_t;
    match info {
        Some(mut info) => {unsafe { status = PMIx_Init(proc, &mut info.handle, info.len);}}
        None => {unsafe { status = PMIx_Init(proc, ptr::null_mut(), 0);}}
    }

    if status as u32 == PMIX_SUCCESS {
        Ok(Proc{ handle: unsafe{*proc}, len: 1 })
    } else {
        Err(status)
    }
}

pub fn get_value(proc: Proc, key: pmix_key_t, info: Option<Info>) -> Result<pmix_value_t, pmix_status_t> {
    let status: pmix_status_t;
    let mut value: *mut pmix_value_t = null_mut();
    match info {
        Some(info) => {unsafe { status = PMIx_Get(&proc.handle, key.as_ptr(), &info.handle, info.len, &mut value) ;}}
        None => {unsafe { status = PMIx_Get(&proc.handle, key.as_ptr(), ptr::null_mut(), 0, &mut value) ;}}
    }
    if status as u32 == PMIX_SUCCESS {
        Ok(unsafe{*value})
    } else {
        Err(status)
    }
}

pub fn put_value(scope: pmix_scope_t, key: pmix_key_t, mut value: pmix_value_t) -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe { status = PMIx_Put(scope, key.as_ptr(), &mut value);}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn commit() -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe { status = PMIx_Commit();}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn fence(proc: Proc, info: Info) -> Result<(), pmix_status_t> {
    let status: pmix_status_t;
    unsafe { status = PMIx_Fence(&proc.handle, proc.len, &info.handle, info.len);}
    if status as u32 == PMIX_SUCCESS { Ok(()) } else { Err(status) }
}

pub fn get_version() -> &'static CStr {
    let version: &CStr;
    unsafe {
        version = CStr::from_ptr(PMIx_Get_version());
    }
    version
}
pub fn progress() {
    unsafe {PMIx_Progress();}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_version() {
        let _proc = init(None).unwrap();
    }
}