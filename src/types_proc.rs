use crate::ffi;
use crate::ffi::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;
use crate::pmix_ffi_or_mock;
use crate::*;
use cstring_array::CStringArray;
use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::{c_char, c_void};
use std::ptr::{null, null_mut};
use std::{fmt, mem, ptr};

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
    /// Create from `&str` arguments; returns `ErrBadParam` for invalid strings
    /// or separators.
    pub fn new(envar: &str, value: &str, separator: char) -> Result<Self, PmixError> {
        if u32::from(separator as u8) != separator as u32 || separator.is_ascii_control() {
            return Err(PmixError::ErrBadParam);
        }
        Ok(Self {
            envar: CString::new(envar).map_err(|_| PmixError::ErrBadParam)?,
            value: CString::new(value).map_err(|_| PmixError::ErrBadParam)?,
            separator: separator as u8,
        })
    }
}

impl From<Proc> for crate::ffi::pmix_proc_t {
    fn from(proc: Proc) -> Self {
        proc.handle
    }
}

impl TryFrom<crate::ffi::pmix_proc> for Proc {
    type Error = crate::PmixStatus;

    fn try_from(raw: crate::ffi::pmix_proc) -> Result<Self, Self::Error> {
        let nspace_len = raw
            .nspace
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(crate::PmixStatus::from_raw(-27))?;
        if nspace_len == 0 || nspace_len >= crate::ffi::PMIX_MAX_NSLEN as usize {
            return Err(crate::PmixStatus::from_raw(-27));
        }
        Ok(Proc {
            handle: raw,
            len: 1,
        })
    }
}

impl Proc {
    pub fn new(nspace: &str, rank: u32) -> Result<Self, PmixError> {
        if nspace.len() > ffi::PMIX_MAX_NSLEN as usize {
            return Err(PmixError::ErrBadParam);
        }
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        let c_name = CString::new(nspace).map_err(|_| PmixError::ErrBadParam)?;
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
// Safe wrappers for PMIx process, validation, process-info, and node-pid APIs.

impl Proc {
    /// Explicitly destruct this process object; `Proc` intentionally has no `Drop`.
    pub fn destruct(&mut self) {
        // SAFETY: `self.handle` is a valid initialized PMIx process object owned by `self`.
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_destruct(&mut self.handle) },
            real = unsafe { ffi::PMIx_Proc_destruct(&mut self.handle) }
        );
    }
    /// Load a namespace and rank into this process object.
    pub fn proc_load(&mut self, nspace: &str, rank: u32) -> Result<(), NulError> {
        let ns = CString::new(nspace)?;
        // SAFETY: `ns` is NUL-terminated and `self.handle` is valid for the PMIx call.
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_load(&mut self.handle, ns.as_ptr(), rank) },
            real = unsafe { ffi::PMIx_Proc_load(&mut self.handle, ns.as_ptr(), rank) }
        );
        Ok(())
    }
    /// Return the PMIx-owned printable representation of this process.
    pub fn proc_string(&self) -> Result<String, PmixStatus> {
        // SAFETY: `self.handle` is a valid initialized PMIx process object.
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_string(&self.handle) },
            real = unsafe { ffi::PMIx_Proc_string(&self.handle) }
        );
        if ptr.is_null() {
            return Err(PmixStatus::from_raw(-2));
        }
        // SAFETY: PMIx returns an allocated NUL-terminated string; it is freed exactly once.
        let value = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        // SAFETY: the pointer was allocated by PMIx/libc and is owned by this method.
        unsafe { libc::free(ptr.cast()) };
        Ok(value)
    }
    /// Copy a process identifier into this process object.
    pub fn xfer_procid(&mut self, src: &Proc) {
        /* SAFETY: both handles are valid initialized objects. */
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_xfer_procid(&mut self.handle, &src.handle) },
            real = unsafe { ffi::PMIx_Xfer_procid(&mut self.handle, &src.handle) }
        );
    }
    /// Compare this process identifier with another one.
    pub fn check_procid(&self, other: &Proc) -> bool {
        /* SAFETY: both handles are valid initialized objects. */
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_check_procid(&self.handle, &other.handle) },
            real = unsafe { ffi::PMIx_Check_procid(&self.handle, &other.handle) }
        )
    }
    /// Return whether this process identifier is invalid.
    pub fn procid_invalid(&self) -> bool {
        /* SAFETY: `self.handle` is a valid initialized object. */
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_procid_invalid(&self.handle) },
            real = unsafe { ffi::PMIx_Procid_invalid(&self.handle) }
        )
    }
}

fn cstring_pair(a: &str, b: &str) -> Option<(CString, CString)> {
    Some((CString::new(a).ok()?, CString::new(b).ok()?))
}
/// Compare two PMIx ranks.
pub fn check_rank(a: u32, b: u32) -> bool {
    /* SAFETY: scalar arguments satisfy the FFI ABI. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_check_rank(a, b) },
        real = unsafe { ffi::PMIx_Check_rank(a, b) }
    )
}
/// Compare two PMIx namespaces.
pub fn check_nspace(a: &str, b: &str) -> bool {
    let Some((a, b)) = cstring_pair(a, b) else {
        return false;
    }; /* SAFETY: both CStrings are valid NUL-terminated pointers for this call. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_check_nspace(a.as_ptr(), b.as_ptr()) },
        real = unsafe { ffi::PMIx_Check_nspace(a.as_ptr(), b.as_ptr()) }
    )
}
/// Compare a PMIx key against a string.
pub fn check_key(key: &str, s: &str) -> bool {
    let Some((key, s)) = cstring_pair(key, s) else {
        return false;
    }; /* SAFETY: both CStrings are valid NUL-terminated pointers for this call. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_check_key(key.as_ptr(), s.as_ptr()) },
        real = unsafe { ffi::PMIx_Check_key(key.as_ptr(), s.as_ptr()) }
    )
}
/// Return whether a key is reserved by PMIx.
pub fn check_reserved_key(key: &str) -> bool {
    let Ok(key) = CString::new(key) else {
        return false;
    }; /* SAFETY: `key` is a valid NUL-terminated pointer for this call. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_check_reserved_key(key.as_ptr()) },
        real = unsafe { ffi::PMIx_Check_reserved_key(key.as_ptr()) }
    )
}
/// Load a namespace and rank into a PMIx process object.
pub fn load_procid(dst: &mut Proc, nspace: &str, rank: u32) -> Result<(), NulError> {
    let ns = CString::new(nspace)?; /* SAFETY: `dst` is exclusively borrowed and `ns` is valid for this call. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_load_procid(&mut dst.handle, ns.as_ptr(), rank) },
        real = unsafe { ffi::PMIx_Load_procid(&mut dst.handle, ns.as_ptr(), rank) }
    );
    Ok(())
}
/// Return whether a rank is valid.
///
/// 6.x-only: `PMIx_Rank_valid` does not exist in OpenPMIx 5.0.
#[cfg(pmix6)]
pub fn rank_valid(rank: u32) -> bool {
    /* SAFETY: scalar argument satisfies the FFI ABI. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_rank_valid(rank) },
        real = unsafe { ffi::PMIx_Rank_valid(rank) }
    )
}
/// Return whether a namespace is invalid.
pub fn nspace_invalid(ns: &str) -> bool {
    let Ok(ns) = CString::new(ns) else {
        return true;
    }; /* SAFETY: `ns` is a valid NUL-terminated pointer for this call. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_nspace_invalid(ns.as_ptr()) },
        real = unsafe { ffi::PMIx_Nspace_invalid(ns.as_ptr()) }
    )
}
/// Copy a key into a NUL-terminated buffer without passing an oversized source to C.
pub fn load_key(dst: &mut [c_char], src: &str) {
    let Ok(src) = CString::new(src) else { return };
    if dst.is_empty() {
        return;
    };
    let n = src.as_bytes().len().min(dst.len() - 1);
    for (out, &byte) in dst[..n].iter_mut().zip(&src.as_bytes()[..n]) {
        *out = byte as c_char;
    }
    dst[n] = 0;
    let truncated = CString::new(&src.as_bytes()[..n]).expect("slice excludes NUL"); /* SAFETY: destination is valid and `truncated` is NUL-terminated; its length is bounded by the destination. */
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_load_key(dst.as_mut_ptr(), truncated.as_ptr()) },
        real = unsafe { ffi::PMIx_Load_key(dst.as_mut_ptr(), truncated.as_ptr()) }
    );
}

/// Owned array allocated by PMIx for process identifiers.
pub struct PmixProcArray {
    ptr: *mut pmix_proc_t,
    len: usize,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixProcArray {
    pub fn new(len: usize) -> Option<Self> {
        if len == 0 {
            return Some(Self {
                ptr: std::ptr::null_mut(),
                len,
                _not_thread_safe: std::marker::PhantomData,
            });
        }
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_create(len) },
            real = unsafe { ffi::PMIx_Proc_create(len) }
        );
        (!ptr.is_null()).then_some(Self {
            ptr,
            len,
            _not_thread_safe: std::marker::PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl Drop for PmixProcArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            /* SAFETY: pointer and length came from the matching PMIx allocator. */
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_proc_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Proc_free(self.ptr, self.len) }
            );
        }
    }
}

/// Owned, constructed PMIx process information object.
pub struct PmixProcInfo {
    raw: mem::MaybeUninit<pmix_proc_info_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixProcInfo {
    pub fn new() -> Self {
        let mut x = Self {
            raw: mem::MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        }; /* SAFETY: PMIx initializes the writable storage supplied by `x`. */
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_info_construct(x.raw.as_mut_ptr()) },
            real = unsafe { ffi::PMIx_Proc_info_construct(x.raw.as_mut_ptr()) }
        );
        x.constructed = true;
        x
    }
    #[cfg(test)]
    pub fn test_new() -> Self {
        Self {
            raw: mem::MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    fn raw(&self) -> &pmix_proc_info_t {
        /* SAFETY: `new` and `test_new` initialize the storage before access. */
        unsafe { self.raw.assume_init_ref() }
    }

    pub fn hostname(&self) -> Option<&str> {
        cstr_ref(self.raw().hostname)
    }
    pub fn executable_name(&self) -> Option<&str> {
        cstr_ref(self.raw().executable_name)
    }
    pub fn pid(&self) -> i32 {
        self.raw().pid
    }
    pub fn exit_code(&self) -> i32 {
        self.raw().exit_code
    }
    pub fn state(&self) -> pmix_proc_state_t {
        self.raw().state
    }
}
impl Default for PmixProcInfo {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixProcInfo {
    fn drop(&mut self) {
        if self.constructed {
            /* SAFETY: storage was initialized and is exclusively borrowed here. */
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_proc_info_destruct(self.raw.as_mut_ptr()) },
                real = unsafe { ffi::PMIx_Proc_info_destruct(self.raw.as_mut_ptr()) }
            );
            self.constructed = false;
        }
    }
}
fn cstr_ref<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }; // SAFETY: callers pass pointers to NUL-terminated fields owned by the borrowed wrapper.
    unsafe { CStr::from_ptr(ptr).to_str().ok() }
}

/// Owned array allocated by PMIx for process information objects.
pub struct PmixProcInfoArray {
    ptr: *mut pmix_proc_info_t,
    len: usize,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixProcInfoArray {
    pub fn new(len: usize) -> Option<Self> {
        if len == 0 {
            return Some(Self {
                ptr: std::ptr::null_mut(),
                len,
                _not_thread_safe: std::marker::PhantomData,
            });
        }
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_proc_info_create(len) },
            real = unsafe { ffi::PMIx_Proc_info_create(len) }
        );
        (!ptr.is_null()).then_some(Self {
            ptr,
            len,
            _not_thread_safe: std::marker::PhantomData,
        })
    }
    pub fn as_mut_ptr(&mut self) -> *mut pmix_proc_info_t {
        self.ptr
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl Drop for PmixProcInfoArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            /* SAFETY: pointer and length came from the matching PMIx allocator. */
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_proc_info_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Proc_info_free(self.ptr, self.len) }
            );
        }
    }
}

/// Owned, constructed PMIx node/pid object.
///
/// 6.x-only: `pmix_node_pid_t` does not exist in OpenPMIx 5.0.
#[cfg(pmix6)]
pub struct PmixNodePid {
    raw: mem::MaybeUninit<pmix_node_pid_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
#[cfg(pmix6)]
#[cfg(pmix6)]
impl PmixNodePid {
    pub fn new() -> Self {
        let mut x = Self {
            raw: mem::MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        }; /* SAFETY: PMIx initializes the writable storage supplied by `x`. */
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_node_pid_construct(x.raw.as_mut_ptr()) },
            real = unsafe { ffi::PMIx_Node_pid_construct(x.raw.as_mut_ptr()) }
        );
        x.constructed = true;
        x
    }
    #[cfg(test)]
    pub fn test_new() -> Self {
        Self {
            raw: mem::MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    fn raw(&self) -> &pmix_node_pid_t {
        /* SAFETY: `new` and `test_new` initialize the storage before access. */
        unsafe { self.raw.assume_init_ref() }
    }
    pub fn hostname(&self) -> Option<&str> {
        cstr_ref(self.raw().hostname)
    }
    pub fn nodeid(&self) -> u32 {
        self.raw().nodeid
    }
    pub fn pid(&self) -> i32 {
        self.raw().pid
    }
}
#[cfg(pmix6)]
impl Default for PmixNodePid {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(pmix6)]
impl Drop for PmixNodePid {
    fn drop(&mut self) {
        if self.constructed {
            /* SAFETY: storage was initialized and is exclusively borrowed here. */
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_node_pid_destruct(self.raw.as_mut_ptr()) },
                real = unsafe { ffi::PMIx_Node_pid_destruct(self.raw.as_mut_ptr()) }
            );
            self.constructed = false;
        }
    }
}
/// Owned array allocated by PMIx for node/pid objects.
#[cfg(pmix6)]
pub struct PmixNodePidArray {
    ptr: *mut pmix_node_pid_t,
    len: usize,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
#[cfg(pmix6)]
#[cfg(pmix6)]
impl PmixNodePidArray {
    pub fn new(len: usize) -> Option<Self> {
        if len == 0 {
            return Some(Self {
                ptr: std::ptr::null_mut(),
                len,
                _not_thread_safe: std::marker::PhantomData,
            });
        }
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_node_pid_create(len) },
            real = unsafe { ffi::PMIx_Node_pid_create(len) }
        );
        (!ptr.is_null()).then_some(Self {
            ptr,
            len,
            _not_thread_safe: std::marker::PhantomData,
        })
    }
    pub fn as_mut_ptr(&mut self) -> *mut pmix_node_pid_t {
        self.ptr
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
#[cfg(pmix6)]
#[cfg(pmix6)]
impl Drop for PmixNodePidArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            /* SAFETY: pointer and length came from the matching PMIx allocator. */
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_node_pid_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Node_pid_free(self.ptr, self.len) }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proc(nspace: &str, rank: u32) -> Proc {
        let mut handle = crate::ffi::pmix_proc_t {
            nspace: std::array::from_fn(|_| 0),
            rank,
        };
        for (dst, &byte) in handle.nspace.iter_mut().zip(nspace.as_bytes()) {
            *dst = byte as c_char;
        }
        Proc { handle, len: 1 }
    }

    #[test]
    fn test_pmix_proc_t_layout() {
        let raw: ffi::pmix_proc_t = test_proc("layout", 0).into();
        assert_eq!(mem::offset_of!(ffi::pmix_proc_t, nspace), 0);
        assert_eq!(
            mem::offset_of!(ffi::pmix_proc_t, rank),
            mem::size_of_val(&raw.nspace)
        );
        assert_eq!(mem::align_of_val(&raw), mem::align_of_val(&raw.rank));
    }

    #[test]
    fn test_proc_from_raw_roundtrip() {
        let proc = test_proc("test_namespace", 42);
        let raw: crate::ffi::pmix_proc_t = proc.clone().into();
        let proc2 = Proc::try_from(raw).unwrap();
        assert_eq!(proc.handle.nspace, proc2.handle.nspace);
        assert_eq!(proc.get_rank(), proc2.get_rank());
    }

    #[test]
    fn test_pmix_proc_size() {
        let raw: ffi::pmix_proc_t = test_proc("size", 0).into();
        assert_eq!(
            mem::size_of::<ffi::pmix_proc_t>(),
            mem::size_of_val(&raw.nspace) + mem::size_of_val(&raw.rank)
        );
    }

    #[test]
    fn test_proc_into_ffi_conversion() {
        let proc = test_proc("test_nspace", 42);
        let raw: crate::ffi::pmix_proc_t = proc.into();
        // Verify the raw struct has correct values
        let nspace_str = unsafe { std::ffi::CStr::from_ptr(raw.nspace.as_ptr()) }.to_string_lossy();
        assert_eq!(nspace_str, "test_nspace");
        assert_eq!(raw.rank, 42);
    }

    #[test]
    fn test_proc_try_from_empty_nspace_fails() {
        let raw: crate::ffi::pmix_proc_t = test_proc("", 0).into();
        assert!(Proc::try_from(raw).is_err());
    }

    #[test]
    fn test_proc_try_from_unterminated_nspace_fails() {
        let mut raw: crate::ffi::pmix_proc_t = test_proc("", 0).into();
        raw.nspace.fill(b'x' as c_char);
        assert!(Proc::try_from(raw).is_err());
    }

    #[test]
    fn test_load_procid_updates_proc() {
        let mut proc = test_proc("old_namespace", 1);
        load_procid(&mut proc, "new_namespace", 42).unwrap();
        let raw: crate::ffi::pmix_proc_t = proc.into();
        let nspace = unsafe { CStr::from_ptr(raw.nspace.as_ptr()) };
        assert_eq!(nspace.to_bytes(), b"new_namespace");
        assert_eq!(raw.rank, 42);
    }

    #[test]
    fn test_load_procid_rejects_interior_nul() {
        let mut proc = test_proc("old_namespace", 1);
        let original = proc.handle;
        assert!(load_procid(&mut proc, "invalid\0namespace", 42).is_err());
        assert_eq!(proc.handle.nspace, original.nspace);
        assert_eq!(proc.get_rank(), original.rank);
    }
}
