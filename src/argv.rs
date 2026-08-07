//! Safe wrappers for PMIx `PMIx_Argv_*` string-array utilities.

use crate::ffi;
use crate::{PmixError, PmixStatus};
use std::ffi::{CStr, CString, NulError};
use std::os::raw::{c_char, c_int};
use std::ptr;

#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

struct CArgv {
    ptr: *mut *mut c_char,
}

impl Drop for CArgv {
    fn drop(&mut self) {
        free_c_argv(self.ptr);
    }
}

fn invalid_argument(_: NulError) -> PmixError {
    PmixError::Error
}

fn to_c_argv(argv: &[String]) -> Result<CArgv, PmixError> {
    // SAFETY: calloc returns aligned storage for the pointers plus a zeroed
    // NULL terminator; ownership is released by free_c_argv on every path.
    let array = unsafe {
        libc::calloc(
            argv.len().saturating_add(1),
            std::mem::size_of::<*mut c_char>(),
        ) as *mut *mut c_char
    };
    if array.is_null() {
        return Err(PmixError::ErrNomem);
    }

    for (index, value) in argv.iter().enumerate() {
        let cvalue = match CString::new(value.as_str()) {
            Ok(value) => value,
            Err(error) => {
                free_c_argv(array);
                return Err(invalid_argument(error));
            }
        };
        // SAFETY: cvalue is a live NUL-terminated CString and strdup creates
        // an independently allocated entry owned by the argv array.
        let duplicate = unsafe { libc::strdup(cvalue.as_ptr()) };
        if duplicate.is_null() {
            free_c_argv(array);
            return Err(PmixError::ErrNomem);
        }
        // SAFETY: index is within the calloc'd array and each slot is written
        // once while constructing the NULL-terminated argv.
        unsafe { *array.add(index) = duplicate };
    }
    Ok(CArgv { ptr: array })
}

fn free_c_argv(argv: *mut *mut c_char) {
    if !argv.is_null() {
        crate::pmix_ffi_or_mock!(
            // SAFETY: argv is non-null and points to a PMIx-compatible NULL-terminated array.
            mock = unsafe { mock_ffi::mock_argv_free(argv) },
            // SAFETY: argv is non-null and points to a PMIx-compatible NULL-terminated array.
            real = unsafe { ffi::PMIx_Argv_free(argv) },
        );
    }
}

fn read_c_argv(argv: *mut *mut c_char) -> Result<Vec<String>, PmixError> {
    if argv.is_null() {
        return Err(PmixError::Error);
    }
    let mut result = Vec::new();
    let mut index = 0;
    loop {
        // SAFETY: argv is a valid NULL-terminated array and index advances only
        // over entries read before the terminator.
        let entry = unsafe { *argv.add(index) };
        if entry.is_null() {
            break;
        }
        // SAFETY: entry is a non-NULL pointer to a valid NUL-terminated string.
        let value = unsafe { CStr::from_ptr(entry) }
            .to_str()
            .map_err(|_| PmixError::Error)?
            .to_owned();
        result.push(value);
        index += 1;
    }
    Ok(result)
}

fn split_impl(src: &str, delimiter: char, kind: SplitKind) -> Result<Vec<String>, PmixError> {
    let source = CString::new(src).map_err(invalid_argument)?;
    let raw = crate::pmix_ffi_or_mock!(
        // SAFETY: source is a live NUL-terminated CString and the delimiter is a valid C int.
        mock = unsafe {
            match kind {
                SplitKind::Normal => mock_ffi::mock_argv_split(source.as_ptr(), delimiter as c_int),
                SplitKind::WithEmpty => {
                    mock_ffi::mock_argv_split_with_empty(source.as_ptr(), delimiter as c_int)
                }
                SplitKind::Inter(include_empty) => mock_ffi::mock_argv_split_inter(
                    source.as_ptr(),
                    delimiter as c_int,
                    include_empty,
                ),
            }
        },
        // SAFETY: source is a live NUL-terminated CString and the delimiter is a valid C int.
        real = unsafe {
            match kind {
                SplitKind::Normal => ffi::PMIx_Argv_split(source.as_ptr(), delimiter as c_int),
                SplitKind::WithEmpty => {
                    ffi::PMIx_Argv_split_with_empty(source.as_ptr(), delimiter as c_int)
                }
                SplitKind::Inter(include_empty) => {
                    ffi::PMIx_Argv_split_inter(source.as_ptr(), delimiter as c_int, include_empty)
                }
            }
        },
    );
    let result = read_c_argv(raw);
    free_c_argv(raw);
    result
}

enum SplitKind {
    Normal,
    WithEmpty,
    Inter(bool),
}

/// Split a string using PMIx's argv parser.
///
/// Only single-byte (ASCII/Latin-1) delimiters are supported. Splitting an
/// empty string returns `Err(PmixError::Error)`, matching PMIx's NULL result.
pub fn split(src: &str, delimiter: char) -> Result<Vec<String>, PmixError> {
    split_impl(src, delimiter, SplitKind::Normal)
}

/// Split a string while retaining empty fields.
///
/// Only single-byte (ASCII/Latin-1) delimiters are supported. An empty source
/// returns `Err(PmixError::Error)`, matching PMIx's NULL result.
pub fn split_with_empty(src: &str, delimiter: char) -> Result<Vec<String>, PmixError> {
    split_impl(src, delimiter, SplitKind::WithEmpty)
}

/// Split a string, optionally retaining empty fields.
///
/// Only single-byte (ASCII/Latin-1) delimiters are supported. An empty source
/// returns `Err(PmixError::Error)`, matching PMIx's NULL result.
pub fn split_inter(
    src: &str,
    delimiter: char,
    include_empty: bool,
) -> Result<Vec<String>, PmixError> {
    split_impl(src, delimiter, SplitKind::Inter(include_empty))
}

/// Join strings with PMIx's argv join utility.
pub fn join(argv: &[String], delimiter: char) -> Result<String, PmixError> {
    let cargv = to_c_argv(argv)?;
    let joined = crate::pmix_ffi_or_mock!(
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        mock = unsafe { mock_ffi::mock_argv_join(cargv.ptr, delimiter as c_int) },
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        real = unsafe { ffi::PMIx_Argv_join(cargv.ptr, delimiter as c_int) },
    );
    if joined.is_null() {
        return Err(PmixError::Error);
    }
    // SAFETY: joined was checked non-null and is the NUL-terminated allocation returned by PMIx.
    let result = unsafe { CStr::from_ptr(joined) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| PmixError::Error);
    // SAFETY: joined is the allocation returned by PMIx_Argv_join and is freed exactly once.
    unsafe { libc::free(joined.cast()) };
    result
}

/// Deep-copy an argv string array.
pub fn copy(argv: &[String]) -> Result<Vec<String>, PmixError> {
    let cargv = to_c_argv(argv)?;
    let copied = crate::pmix_ffi_or_mock!(
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        mock = unsafe { mock_ffi::mock_argv_copy(cargv.ptr) },
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        real = unsafe { ffi::PMIx_Argv_copy(cargv.ptr) },
    );
    let result = read_c_argv(copied);
    free_c_argv(copied);
    result
}

/// Count entries. For a Rust slice this is equal to `argv.len()`.
pub fn count(argv: &[String]) -> usize {
    let Ok(cargv) = to_c_argv(argv) else {
        return 0;
    };
    let count = crate::pmix_ffi_or_mock!(
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        mock = unsafe { mock_ffi::mock_argv_count(cargv.ptr) },
        // SAFETY: cargv.ptr is a valid NULL-terminated array owned by cargv.
        real = unsafe { ffi::PMIx_Argv_count(cargv.ptr) },
    );
    usize::try_from(count).unwrap_or(0)
}

fn update_with<F>(argv: &mut Vec<String>, arg: &str, call: F) -> Result<(), PmixStatus>
where
    F: FnOnce(*mut *mut *mut c_char, *const c_char) -> i32,
{
    let mut cargv = to_c_argv(argv).map_err(PmixStatus::from)?;
    let carg = CString::new(arg).map_err(|_| PmixStatus::from(PmixError::ErrBadParam))?;
    let mut raw = cargv.ptr;
    let status = call(&mut raw, carg.as_ptr());
    cargv.ptr = raw;
    if status != 0 {
        return Err(PmixStatus::from_raw(status));
    }
    let updated = read_c_argv(cargv.ptr).map_err(PmixStatus::from)?;
    *argv = updated;
    Ok(())
}

/// Append `arg`, even when it is already present.
pub fn append(argv: &mut Vec<String>, arg: &str) -> Result<(), PmixStatus> {
    update_with(argv, arg, |ptr, value| {
        crate::pmix_ffi_or_mock!(
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            mock = unsafe { mock_ffi::mock_argv_append_nosize(ptr, value) },
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            real = unsafe { ffi::PMIx_Argv_append_nosize(ptr, value) },
        )
    })
}

/// Append `arg` only when it is not already present.
pub fn append_unique(argv: &mut Vec<String>, arg: &str) -> Result<(), PmixStatus> {
    update_with(argv, arg, |ptr, value| {
        crate::pmix_ffi_or_mock!(
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            mock = unsafe { mock_ffi::mock_argv_append_unique_nosize(ptr, value) },
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            real = unsafe { ffi::PMIx_Argv_append_unique_nosize(ptr, value) },
        )
    })
}

/// Prepend `arg` to the array.
pub fn prepend(argv: &mut Vec<String>, arg: &str) -> Result<(), PmixStatus> {
    update_with(argv, arg, |ptr, value| {
        crate::pmix_ffi_or_mock!(
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            mock = unsafe { mock_ffi::mock_argv_prepend_nosize(ptr, value) },
            // SAFETY: ptr references the live owned argv pointer and value is a live CString.
            real = unsafe { ffi::PMIx_Argv_prepend_nosize(ptr, value) },
        )
    })
}

/// No-op for Rust-owned slices; provided for API completeness.
pub fn free(_argv: &[String]) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_ffi::MockGuard;

    #[test]
    fn split_copy_join_and_count_use_deterministic_mocks() {
        let _guard = MockGuard::new();
        let values = vec!["one".to_owned(), "two".to_owned()];
        assert_eq!(split("one,two", ',').unwrap(), vec!["a"]);
        assert_eq!(split_with_empty("one,two", ',').unwrap(), vec!["a"]);
        assert_eq!(split_inter("one,two", ',', true).unwrap(), vec!["a"]);
        assert_eq!(copy(&values).unwrap(), values);
        assert_eq!(join(&values, ',').unwrap(), "joined");
        assert_eq!(count(&values), 2);
    }

    #[test]
    fn mutation_and_free_are_safe_with_mocks() {
        let _guard = MockGuard::new();
        let mut values = vec!["one".to_owned()];
        append(&mut values, "two").unwrap();
        append_unique(&mut values, "two").unwrap();
        prepend(&mut values, "zero").unwrap();
        assert_eq!(values, vec!["zero", "one", "two"]);
        free(&values);
    }
}
