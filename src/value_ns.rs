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
// PMIx_Value_* safe utilities
// ─────────────────────────────────────────────────────────────────────────────

/// A constructed PMIx value whose C-owned state is released on drop.
pub struct PmixValue {
    raw: std::mem::MaybeUninit<pmix_value_t>,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixValue {
    pub fn new() -> Self {
        let mut value = Self {
            raw: std::mem::MaybeUninit::uninit(),
            constructed: false,
            _not_thread_safe: std::marker::PhantomData,
        };
        let ptr = value.raw.as_mut_ptr();
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_construct(ptr) },
            real = unsafe { ffi::PMIx_Value_construct(ptr) }
        );
        value.constructed = true;
        value
    }

    pub fn test_new() -> Self {
        Self {
            raw: std::mem::MaybeUninit::zeroed(),
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }

    pub fn as_raw(&self) -> *const pmix_value_t {
        self.raw.as_ptr()
    }
    pub fn as_mut_ptr(&mut self) -> *mut pmix_value_t {
        self.raw.as_mut_ptr()
    }

    /// Loads a serialized payload into this value. `data.len()` is the payload
    /// length in bytes passed to the C API.
    pub fn value_load(&mut self, data: &[u8], ty: pmix_data_type_t) -> Result<(), PmixStatus> {
        let status = pmix_ffi_or_mock!(
            mock =
                unsafe { mock_ffi::mock_value_load(self.as_mut_ptr(), data.as_ptr().cast(), ty) },
            real = unsafe { ffi::PMIx_Value_load(self.as_mut_ptr(), data.as_ptr().cast(), ty) }
        );
        (status == PMIX_SUCCESS as i32)
            .then_some(())
            .ok_or_else(|| PmixStatus::from_raw(status))
    }

    /// Unloads the value into an owned copy. The returned `Vec` owns its bytes;
    /// the buffer allocated by the C API is freed before this method returns.
    pub fn value_unload(&mut self) -> Result<(Vec<u8>, usize), PmixStatus> {
        let mut data = std::ptr::null_mut();
        let mut size = 0;
        let status = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_unload(self.as_mut_ptr(), &mut data, &mut size) },
            real = unsafe { ffi::PMIx_Value_unload(self.as_mut_ptr(), &mut data, &mut size) }
        );
        if status != PMIX_SUCCESS as i32 {
            return Err(PmixStatus::from_raw(status));
        }
        if data.is_null() && size != 0 {
            return Err(PmixStatus::from_raw(-2));
        }
        let bytes = if size == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec() }
        };
        if !data.is_null() {
            unsafe { libc::free(data) };
        }
        Ok((bytes, size))
    }

    pub fn value_xfer(dest: &mut Self, src: &Self) -> Result<(), PmixStatus> {
        let status = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_xfer(dest.as_mut_ptr(), src.as_raw()) },
            real = unsafe { ffi::PMIx_Value_xfer(dest.as_mut_ptr(), src.as_raw()) }
        );
        (status == PMIX_SUCCESS as i32)
            .then_some(())
            .ok_or_else(|| PmixStatus::from_raw(status))
    }
    /// Writes the requested numeric representation into `dest`. The buffer
    /// must be large enough for the requested PMIx type.
    ///
    /// 6.x-only: `PMIx_Value_get_number` does not exist in OpenPMIx 5.0.
    #[cfg(pmix6)]
    pub fn value_get_number(
        &self,
        dest: &mut [u8],
        ty: pmix_data_type_t,
    ) -> Result<(), PmixStatus> {
        let status = pmix_ffi_or_mock!(
            mock = unsafe {
                mock_ffi::mock_value_get_number(self.as_raw(), dest.as_mut_ptr().cast(), ty)
            },
            real =
                unsafe { ffi::PMIx_Value_get_number(self.as_raw(), dest.as_mut_ptr().cast(), ty) }
        );
        (status == PMIX_SUCCESS as i32)
            .then_some(())
            .ok_or_else(|| PmixStatus::from_raw(status))
    }
    pub fn value_get_size(&self) -> Result<usize, PmixStatus> {
        let mut size = 0;
        let status = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_get_size(self.as_raw(), &mut size) },
            real = unsafe { ffi::PMIx_Value_get_size(self.as_raw(), &mut size) }
        );
        if status == PMIX_SUCCESS as i32 {
            Ok(size)
        } else {
            Err(PmixStatus::from_raw(status))
        }
    }
    /// Compares two values. PMIx declares this C API as taking mutable
    /// pointers, although it does not mutate either value; the const-casts
    /// therefore only adapt the inaccurate C signature.
    pub fn value_compare(a: &Self, b: &Self) -> pmix_value_cmp_t {
        pmix_ffi_or_mock!(
            mock =
                unsafe { mock_ffi::mock_value_compare(a.as_raw() as *mut _, b.as_raw() as *mut _) },
            real = unsafe { ffi::PMIx_Value_compare(a.as_raw() as *mut _, b.as_raw() as *mut _) }
        )
    }
    pub fn value_string(&self) -> Result<String, PmixStatus> {
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_string(self.as_raw()) },
            real = unsafe { ffi::PMIx_Value_string(self.as_raw()) }
        );
        if ptr.is_null() {
            return Err(PmixStatus::from_raw(-2));
        }
        let result = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { libc::free(ptr.cast()) };
        Ok(result)
    }
    pub fn value_true(&self) -> bool {
        pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_true(self.as_raw()) },
            real = unsafe { ffi::PMIx_Value_true(self.as_raw()) }
        ) == pmix_boolean_t::PMIX_BOOL_TRUE
    }
}

impl Default for PmixValue {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixValue {
    fn drop(&mut self) {
        if self.constructed {
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_value_destruct(self.raw.as_mut_ptr()) },
                real = unsafe { ffi::PMIx_Value_destruct(self.raw.as_mut_ptr()) }
            );
            self.constructed = false;
        }
    }
}

pub fn value_comparison_string(cmp: pmix_value_cmp_t) -> &'static str {
    let ptr = pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_value_comparison_string(cmp) },
        real = unsafe { ffi::PMIx_Value_comparison_string(cmp) }
    );
    if ptr.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(ptr).to_str().unwrap_or("") }
    }
}

/// Owns an array allocated by PMIx with [`PMIx_Value_create`]. The allocation
/// is released by [`PMIx_Value_free`] when this wrapper is dropped.
pub struct PmixValueArray {
    ptr: *mut pmix_value_t,
    len: usize,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}
impl PmixValueArray {
    pub fn new(len: usize) -> Option<Self> {
        if len == 0 {
            return Some(Self {
                ptr: std::ptr::null_mut(),
                len,
                _not_thread_safe: std::marker::PhantomData,
            });
        }
        let ptr = pmix_ffi_or_mock!(
            mock = unsafe { mock_ffi::mock_value_create(len) },
            real = unsafe { ffi::PMIx_Value_create(len) }
        );
        (!ptr.is_null()).then_some(Self {
            ptr,
            len,
            _not_thread_safe: std::marker::PhantomData,
        })
    }
    pub fn as_mut_ptr(&mut self) -> *mut pmix_value_t {
        self.ptr
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl Drop for PmixValueArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_value_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Value_free(self.ptr, self.len) }
            );
            self.ptr = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod value_utils_tests {
    use crate::*;
    use crate::*;
    #[test]
    fn mock_value_utilities() {
        let _guard = mock_ffi::MockGuard::new();
        let value = PmixValue::new();
        assert_eq!(value.value_string().unwrap(), "mock");
        assert!(value.value_true());
        assert_eq!(value.value_get_size().unwrap(), 4);
        assert_eq!(
            value_comparison_string(pmix_value_cmp_t::PMIX_EQUAL),
            "equal"
        );
    }
    #[test]
    fn mock_value_array_and_unload() {
        let _guard = mock_ffi::MockGuard::new();
        let mut array = PmixValueArray::new(2).unwrap();
        assert_eq!(array.len(), 2);
        assert!(!array.as_mut_ptr().is_null());
        let mut value = PmixValue::new();
        assert_eq!(value.value_unload().unwrap().0, b"mock");
    }
    #[test]
    fn mock_value_test_new_has_zeroed_accessors() {
        let _guard = mock_ffi::MockGuard::new();
        let value = PmixValue::test_new();
        assert_eq!(value.value_get_size().unwrap(), 4);
        assert_eq!(value.value_string().unwrap(), "mock");
        assert!(value.value_true());
    }
    #[test]
    fn mock_value_xfer_succeeds() {
        let _guard = mock_ffi::MockGuard::new();
        let mut dest = PmixValue::new();
        let src = PmixValue::new();
        assert!(PmixValue::value_xfer(&mut dest, &src).is_ok());
    }
    #[test]
    fn mock_value_compare_returns_equal() {
        let _guard = mock_ffi::MockGuard::new();
        let a = PmixValue::new();
        let b = PmixValue::new();
        assert_eq!(
            PmixValue::value_compare(&a, &b),
            pmix_value_cmp_t::PMIX_EQUAL
        );
    }
    #[test]
    fn mock_value_unload_propagates_error_status() {
        let _guard = mock_ffi::MockGuard::new();
        mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Value_unload", mock_ffi::PMIX_ERR_BAD_PARAM)
            .apply();
        let mut value = PmixValue::new();
        assert!(value.value_unload().is_err());
    }
}

/// Safe wrapper around an opaque PMIx info linked list.
pub use info::PmixInfoList;

#[cfg(test)]
mod envar_utils_tests {
    use super::PMIX_MAX_NSLEN;
    use crate::*;

    #[test]
    fn envar_load_round_trip() {
        let _guard = mock_ffi::MockGuard::new();
        let value = envar_load("PATH", "/bin", ':').unwrap();
        assert_eq!(value.envar.as_c_str().to_str().unwrap(), "PATH");
        assert_eq!(value.value.as_c_str().to_str().unwrap(), "/bin");
        assert_eq!(value.separator, b':');
    }

    #[test]
    fn envar_create_allocates_and_drops() {
        let _guard = mock_ffi::MockGuard::new();
        let mut array = envar_create(2).unwrap();
        assert_eq!(array.len(), 2);
        assert!(!array.as_mut_ptr().is_null());
    }

    #[test]
    fn envar_create_zero_is_empty_and_safe() {
        let _guard = mock_ffi::MockGuard::new();
        let array = envar_create(0).unwrap();
        assert!(array.is_empty());
        assert!(array.ptr.is_null());
    }

    #[test]
    fn envar_destruct_does_not_free_rust_owned_strings() {
        let _guard = mock_ffi::MockGuard::new();
        let mut value = PmixEnvar::new("NAME", "VALUE", '=').unwrap();
        envar_destruct(&mut value);
        assert_eq!(value.envar, CString::new("NAME").unwrap());
        assert_eq!(value.value, CString::new("VALUE").unwrap());
    }

    #[test]
    fn setenv_replaces_vec_environment() {
        let _guard = mock_ffi::MockGuard::new();
        let mut env = vec!["A=B".to_owned()];
        setenv("C", "D", true, &mut env).unwrap();
        assert_eq!(env, vec!["A=B", "C=D"]);
    }

    #[test]
    fn setenv_rejects_later_interior_nul_without_panicking() {
        let _guard = mock_ffi::MockGuard::new();
        let mut env = vec!["A=B".to_owned(), "BAD\0=VALUE".to_owned()];
        assert!(setenv("C", "D", true, &mut env).is_err());
        assert_eq!(env, vec!["A=B", "BAD\0=VALUE"]);
    }

    #[test]
    fn multicluster_parse_returns_mock_buffers() {
        let _guard = mock_ffi::MockGuard::new();
        assert_eq!(
            multicluster_nspace_parse("cluster:nspace").unwrap(),
            ("cluster".into(), "nspace".into())
        );
    }

    #[test]
    fn multicluster_construct_writes_target() {
        let _guard = mock_ffi::MockGuard::new();
        assert_eq!(
            multicluster_nspace_construct("cluster", "nspace").unwrap(),
            "cluster:nspace"
        );
    }

    #[test]
    fn multicluster_rejects_oversized_components() {
        let _guard = mock_ffi::MockGuard::new();
        let component = "x".repeat(PMIX_MAX_NSLEN + 1);
        assert!(multicluster_nspace_construct(&component, "nspace").is_err());
        assert!(multicluster_nspace_parse(&format!("{}:nspace", component)).is_err());
    }
}

// END TDD TESTS

// ─────────────────────────────────────────────────────────────────────────────
// Safe wrappers for PMIx_Envar_*, PMIx_Setenv, and multicluster namespaces.

/// Construct a Rust-owned environment variable using the PMIx construct contract.
pub fn envar_construct() -> PmixEnvar {
    let mut raw = unsafe { mem::zeroed::<pmix_envar_t>() };
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_envar_construct(&mut raw) },
        real = unsafe { ffi::PMIx_Envar_construct(&mut raw) },
    );
    PmixEnvar::new("", "", '=').expect("empty strings cannot contain NUL")
}

/// Apply the PMIx environment-variable load operation to a validated value.
pub fn envar_load(envar: &str, value: &str, separator: char) -> Result<PmixEnvar, PmixError> {
    let result = PmixEnvar::new(envar, value, separator)?;
    let mut raw = unsafe { mem::zeroed::<pmix_envar_t>() };
    let var = CString::new(envar).map_err(|_| PmixError::ErrBadParam)?;
    let val = CString::new(value).map_err(|_| PmixError::ErrBadParam)?;
    pmix_ffi_or_mock!(
        mock = unsafe {
            mock_ffi::mock_envar_load(
                &mut raw,
                var.as_ptr().cast_mut(),
                val.as_ptr().cast_mut(),
                separator as c_char,
            )
        },
        real = unsafe {
            ffi::PMIx_Envar_load(
                &mut raw,
                var.as_ptr().cast_mut(),
                val.as_ptr().cast_mut(),
                separator as c_char,
            )
        },
    );
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_envar_destruct(&mut raw) },
        real = unsafe { ffi::PMIx_Envar_destruct(&mut raw) },
    );
    Ok(result)
}

/// Release hook for a [`PmixEnvar`].
///
/// `PmixEnvar` owns both [`CString`] fields. Calling the PMIx destructor on a
/// temporary raw value would not affect this value, while calling it on a raw
/// view of these fields would make PMIx free Rust-owned allocations. Therefore
/// this function intentionally performs no FFI call; normal Rust `Drop` owns
/// destruction of the strings.
pub fn envar_destruct(_envar: &mut PmixEnvar) {}

/// Owned array allocated by PMIx_Envar_create.
pub struct PmixEnvarArray {
    ptr: *mut pmix_envar_t,
    len: usize,
}

impl PmixEnvarArray {
    pub fn as_mut_ptr(&mut self) -> *mut pmix_envar_t {
        self.ptr
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for PmixEnvarArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            pmix_ffi_or_mock!(
                mock = unsafe { mock_ffi::mock_envar_free(self.ptr, self.len) },
                real = unsafe { ffi::PMIx_Envar_free(self.ptr, self.len) },
            );
            self.ptr = ptr::null_mut();
        }
    }
}

pub fn envar_create(len: usize) -> Result<PmixEnvarArray, PmixError> {
    if len == 0 {
        return Ok(PmixEnvarArray {
            ptr: ptr::null_mut(),
            len,
        });
    }
    let ptr = pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_envar_create(len) },
        real = unsafe { ffi::PMIx_Envar_create(len) },
    );
    if ptr.is_null() {
        Err(PmixError::ErrNomem)
    } else {
        Ok(PmixEnvarArray { ptr, len })
    }
}

fn c_string_vector(values: &[String]) -> Result<*mut *mut c_char, PmixStatus> {
    let slots = values
        .len()
        .checked_add(1)
        .ok_or(PmixStatus::from_raw(PmixError::ErrBadParam as i32))?;
    let argv = unsafe { libc::calloc(slots, mem::size_of::<*mut c_char>()) as *mut *mut c_char };
    if argv.is_null() {
        return Err(PmixStatus::from_raw(PmixError::ErrNomem as i32));
    }
    for (index, value) in values.iter().enumerate() {
        let c = match CString::new(value.as_str()) {
            Ok(c) => c,
            Err(_) => {
                unsafe {
                    for initialized in 0..index {
                        libc::free((*argv.add(initialized)).cast());
                    }
                    libc::free(argv.cast());
                }
                return Err(PmixStatus::from_raw(PmixError::ErrBadParam as i32));
            }
        };
        unsafe {
            *argv.add(index) = c.into_raw();
        }
    }
    Ok(argv)
}

/// Set an environment entry while preserving the caller's Vec<String> API.
pub fn setenv(
    name: &str,
    value: &str,
    overwrite: bool,
    env: &mut Vec<String>,
) -> Result<(), PmixStatus> {
    let name =
        CString::new(name).map_err(|_| PmixStatus::from_raw(PmixError::ErrBadParam as i32))?;
    let value =
        CString::new(value).map_err(|_| PmixStatus::from_raw(PmixError::ErrBadParam as i32))?;
    let mut argv = c_string_vector(env)?;
    let status = pmix_ffi_or_mock!(
        mock =
            unsafe { mock_ffi::mock_setenv(name.as_ptr(), value.as_ptr(), overwrite, &mut argv) },
        real = unsafe { ffi::PMIx_Setenv(name.as_ptr(), value.as_ptr(), overwrite, &mut argv) },
    );
    let result = PmixStatus::from_raw(status as i32);
    if result.is_success() {
        let mut copied = Vec::new();
        unsafe {
            let mut index = 0;
            while !(*argv.add(index)).is_null() {
                copied.push(
                    CStr::from_ptr(*argv.add(index))
                        .to_string_lossy()
                        .into_owned(),
                );
                index += 1;
            }
        }
        *env = copied;
    }
    // PMIx_Argv_free is the matching deallocator even when PMIx_Setenv replaced argv.
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_argv_free(argv) },
        real = unsafe { ffi::PMIx_Argv_free(argv) },
    );
    if result.is_success() {
        Ok(())
    } else {
        Err(result)
    }
}

const PMIX_MAX_NSLEN: usize = 255;

pub fn multicluster_nspace_construct(cluster: &str, nspace: &str) -> Result<String, PmixError> {
    if cluster.len() > PMIX_MAX_NSLEN
        || nspace.len() > PMIX_MAX_NSLEN
        || cluster
            .len()
            .checked_add(nspace.len())
            .and_then(|len| len.checked_add(1))
            .is_none_or(|len| len > PMIX_MAX_NSLEN)
    {
        return Err(PmixError::ErrBadParam);
    }
    let target = [0 as c_char; PMIX_MAX_NSLEN + 1];
    let cluster = CString::new(cluster).map_err(|_| PmixError::ErrBadParam)?;
    let nspace = CString::new(nspace).map_err(|_| PmixError::ErrBadParam)?;
    pmix_ffi_or_mock!(
        mock = unsafe {
            mock_ffi::mock_multicluster_nspace_construct(
                target.as_ptr().cast_mut(),
                cluster.as_ptr().cast_mut(),
                nspace.as_ptr().cast_mut(),
            )
        },
        real = unsafe {
            ffi::PMIx_Multicluster_nspace_construct(
                target.as_ptr().cast_mut(),
                cluster.as_ptr().cast_mut(),
                nspace.as_ptr().cast_mut(),
            )
        },
    );
    CStr::from_bytes_until_nul(unsafe {
        std::slice::from_raw_parts(target.as_ptr().cast::<u8>(), target.len())
    })
    .map_err(|_| PmixError::ErrBadParam)
    .and_then(|s| {
        s.to_str()
            .map(str::to_owned)
            .map_err(|_| PmixError::ErrBadParam)
    })
}

pub fn multicluster_nspace_parse(target: &str) -> Result<(String, String), PmixError> {
    if target.len() > PMIX_MAX_NSLEN {
        return Err(PmixError::ErrBadParam);
    }
    let target = CString::new(target).map_err(|_| PmixError::ErrBadParam)?;
    let cluster = [0 as c_char; PMIX_MAX_NSLEN + 1];
    let nspace = [0 as c_char; PMIX_MAX_NSLEN + 1];
    pmix_ffi_or_mock!(
        mock = unsafe {
            mock_ffi::mock_multicluster_nspace_parse(
                target.as_ptr().cast_mut(),
                cluster.as_ptr().cast_mut(),
                nspace.as_ptr().cast_mut(),
            )
        },
        real = unsafe {
            ffi::PMIx_Multicluster_nspace_parse(
                target.as_ptr().cast_mut(),
                cluster.as_ptr().cast_mut(),
                nspace.as_ptr().cast_mut(),
            )
        },
    );
    let cluster = CStr::from_bytes_until_nul(unsafe {
        std::slice::from_raw_parts(cluster.as_ptr().cast::<u8>(), cluster.len())
    })
    .map_err(|_| PmixError::ErrNotFound)?;
    let nspace = CStr::from_bytes_until_nul(unsafe {
        std::slice::from_raw_parts(nspace.as_ptr().cast::<u8>(), nspace.len())
    })
    .map_err(|_| PmixError::ErrNotFound)?;
    if cluster.to_bytes().is_empty() && nspace.to_bytes().is_empty() {
        return Err(PmixError::ErrNotFound);
    }
    Ok((
        cluster.to_string_lossy().into_owned(),
        nspace.to_string_lossy().into_owned(),
    ))
}

/// Look up a PMIx error by its symbolic name.
pub fn error_code(name: &str) -> Option<PmixError> {
    let name = CString::new(name).ok()?;
    let raw = pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_error_code(name.as_ptr()) },
        real = unsafe { ffi::PMIx_Error_code(name.as_ptr()) }
    );
    PmixError::from_raw(raw)
}

#[cfg(test)]
#[test]
fn test_misc_error_code_wrapper() {
    let _guard = mock_ffi::MockGuard::new();
    assert_eq!(error_code("PMIX_SUCCESS"), Some(PmixError::Success));
    assert!(error_code("bad\0name").is_none());
}
