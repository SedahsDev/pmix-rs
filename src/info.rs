//! PMIx Info helpers.
//!
//! Thin ergonomic layer over crate-root [`Info`] / [`InfoBuilder`].

pub use crate::{Info, InfoBuilder, InfoFlags, PmixStatus, info_with_string_key};

/// Create an empty `Info` list (length 0).
pub fn empty() -> Info {
    InfoBuilder::new().build()
}

/// Info list with `PMIX_COLLECT_DATA` set (common fence/get pattern).
pub fn with_collect_data() -> Info {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    builder.build()
}

/// Single string key/value info entry (no 13-byte key limit).
pub fn string_kv(key: &str, value: &str) -> Info {
    info_with_string_key(key, value)
}

/// Builder starting point (same as [`InfoBuilder::new`]).
pub fn builder() -> InfoBuilder {
    InfoBuilder::new()
}

/// Length of an info array.
pub fn len(info: &Info) -> usize {
    info.len()
}

/// RAII wrapper for one individually constructed `pmix_info_t`.
pub struct PmixInfo {
    raw: crate::ffi::pmix_info_t,
    constructed: bool,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixInfo {
    /// Construct a PMIx info value with `PMIx_Info_construct`.
    pub fn new() -> Self {
        // SAFETY: `raw` is a valid local pmix_info_t and PMIx initializes it.
        let mut raw = unsafe { std::mem::zeroed() };
        crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_construct(&mut raw) },
            real = unsafe { crate::ffi::PMIx_Info_construct(&mut raw) },
        );
        Self {
            raw,
            constructed: true,
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    pub fn as_ptr(&self) -> *const crate::ffi::pmix_info_t {
        &self.raw
    }
    pub fn as_mut_ptr(&mut self) -> *mut crate::ffi::pmix_info_t {
        &mut self.raw
    }
}
impl Default for PmixInfo {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for PmixInfo {
    fn drop(&mut self) {
        if self.constructed {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_destruct(&mut self.raw) },
                real = unsafe { crate::ffi::PMIx_Info_destruct(&mut self.raw) },
            );
            self.constructed = false;
        }
    }
}

impl Info {
    fn first_ptr(&self) -> Option<*mut crate::ffi::pmix_info_t> {
        (!self.handle.is_null() && self.len > 0).then_some(self.handle)
    }
    pub fn xfer_from(&mut self, src: &Info) -> Result<(), PmixStatus> {
        let dest = self.first_ptr().ok_or_else(|| PmixStatus::from_raw(-2))?;
        let source = src.first_ptr().ok_or_else(|| PmixStatus::from_raw(-2))?;
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_xfer(dest, source) },
            real = unsafe { crate::ffi::PMIx_Info_xfer(dest, source) },
        );
        (status == crate::ffi::PMIX_SUCCESS as i32)
            .then_some(())
            .ok_or_else(|| PmixStatus::from_raw(status))
    }
    pub fn get_size(&self) -> Result<usize, PmixStatus> {
        let ptr = self.first_ptr().ok_or_else(|| PmixStatus::from_raw(-2))?;
        let mut size = 0;
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_get_size(ptr, &mut size) },
            real = unsafe { crate::ffi::PMIx_Info_get_size(ptr, &mut size) },
        );
        if status == crate::ffi::PMIX_SUCCESS as i32 {
            Ok(size)
        } else {
            Err(PmixStatus::from_raw(status))
        }
    }
    pub fn info_string(&self) -> Result<String, crate::PmixError> {
        let ptr = self.first_ptr().ok_or(crate::PmixError::ErrBadParam)?;
        let c_ptr = crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_string(ptr) },
            real = unsafe { crate::ffi::PMIx_Info_string(ptr) },
        );
        if c_ptr.is_null() {
            return Err(crate::PmixError::Error);
        }
        // SAFETY: PMIx returns an allocated NUL-terminated string; copy then free it.
        let result = unsafe {
            let s = std::ffi::CStr::from_ptr(c_ptr)
                .to_string_lossy()
                .into_owned();
            libc::free(c_ptr.cast());
            s
        };
        Ok(result)
    }
    pub fn is_required(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_required(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_required(p) }
            )
        })
    }
    pub fn is_optional(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_optional(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_optional(p) }
            )
        })
    }
    pub fn is_persistent(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_persistent(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_persistent(p) }
            )
        })
    }
    pub fn is_qualifier(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_qualifier(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_qualifier(p) }
            )
        })
    }
    pub fn is_end(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_end(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_end(p) }
            )
        })
    }
    pub fn was_processed(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_was_processed(p) },
                real = unsafe { crate::ffi::PMIx_Info_was_processed(p) }
            )
        })
    }
    pub fn is_true(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_true(p) },
                real = unsafe { crate::ffi::PMIx_Info_true(p) }
            ) != crate::ffi::pmix_boolean_t::PMIX_BOOL_FALSE
        })
    }
    fn set_with(&mut self, f: impl FnOnce(*mut crate::ffi::pmix_info_t)) {
        if let Some(p) = self.first_ptr() {
            f(p);
        }
    }
    pub fn set_required(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_required(p) },
                real = unsafe { crate::ffi::PMIx_Info_required(p) }
            )
        });
    }
    pub fn set_optional(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_optional(p) },
                real = unsafe { crate::ffi::PMIx_Info_optional(p) }
            )
        });
    }
    pub fn set_persistent(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_persistent(p) },
                real = unsafe { crate::ffi::PMIx_Info_persistent(p) }
            )
        });
    }
    pub fn set_qualifier(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_qualifier(p) },
                real = unsafe { crate::ffi::PMIx_Info_qualifier(p) }
            )
        });
    }
    pub fn set_processed(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_processed(p) },
                real = unsafe { crate::ffi::PMIx_Info_processed(p) }
            )
        });
    }
    pub fn set_end(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_set_end(p) },
                real = unsafe { crate::ffi::PMIx_Info_set_end(p) }
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_info() {
        let info = empty();
        assert_eq!(len(&info), 0);
    }

    #[test]
    fn test_empty_info_is_empty() {
        let info = empty();
        assert!(info.is_empty());
    }

    #[test]
    fn test_with_collect_data_non_empty() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv() {
        let info = string_kv("pmix.srvr.uri", "tcp://127.0.0.1:1");
        assert_eq!(info.len(), 1);
    }

    #[test]
    fn test_info_as_ptr_returns_ptr() {
        let info = empty();
        let _ = info.as_ptr();
    }

    #[test]
    fn test_info_as_ptr_collect_data() {
        let info = with_collect_data();
        assert!(!info.as_ptr().is_null() || info.len() == 0);
    }

    #[test]
    fn test_info_is_empty_false_for_collect_data() {
        assert!(!with_collect_data().is_empty());
    }

    #[test]
    fn test_info_is_empty_false_for_string_kv() {
        assert!(!string_kv("k", "v").is_empty());
    }
}
