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
            _not_thread_safe: std::marker::PhantomData,
        }
    }
    /// Return an immutable pointer to the underlying PMIx info value.
    pub fn as_ptr(&self) -> *const crate::ffi::pmix_info_t {
        &self.raw
    }
    /// Return a mutable pointer to the underlying PMIx info value.
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
        crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_destruct(&mut self.raw) },
            real = unsafe { crate::ffi::PMIx_Info_destruct(&mut self.raw) },
        );
    }
}

impl Info {
    fn first_ptr(&self) -> Option<*mut crate::ffi::pmix_info_t> {
        (!self.handle.is_null() && self.len > 0).then_some(self.handle)
    }
    /// Copy one PMIx info entry from `src` into this list's first entry.
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
    /// Return the serialized size of this list's first entry.
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
    /// Render this list's first entry as an owned string.
    pub fn info_string(&self) -> Result<String, PmixStatus> {
        let ptr = self.first_ptr().ok_or_else(|| PmixStatus::from_raw(-2))?;
        let c_ptr = crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_string(ptr) },
            real = unsafe { crate::ffi::PMIx_Info_string(ptr) },
        );
        if c_ptr.is_null() {
            return Err(PmixStatus::from_raw(-2));
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
    /// Return whether the first entry is marked required.
    pub fn is_required(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_required(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_required(p) }
            )
        })
    }
    /// Return whether the first entry is marked optional.
    pub fn is_optional(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_optional(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_optional(p) }
            )
        })
    }
    /// Return whether the first entry is marked persistent.
    pub fn is_persistent(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_persistent(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_persistent(p) }
            )
        })
    }
    /// Return whether the first entry is a qualifier.
    pub fn is_qualifier(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_qualifier(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_qualifier(p) }
            )
        })
    }
    /// Return whether the first entry is the end marker.
    pub fn is_end(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_is_end(p) },
                real = unsafe { crate::ffi::PMIx_Info_is_end(p) }
            )
        })
    }
    /// Return whether the first entry was processed.
    pub fn was_processed(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_was_processed(p) },
                real = unsafe { crate::ffi::PMIx_Info_was_processed(p) }
            )
        })
    }
    /// Return whether the first entry has PMIX_BOOL_TRUE state.
    pub fn is_true(&self) -> bool {
        self.first_ptr().is_some_and(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_true(p) },
                real = unsafe { crate::ffi::PMIx_Info_true(p) }
            ) == crate::ffi::pmix_boolean_t::PMIX_BOOL_TRUE
        })
    }
    fn set_with(&mut self, f: impl FnOnce(*mut crate::ffi::pmix_info_t)) {
        if let Some(p) = self.first_ptr() {
            f(p);
        }
    }
    /// Mark the first entry required.
    pub fn set_required(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_required(p) },
                real = unsafe { crate::ffi::PMIx_Info_required(p) }
            )
        });
    }
    /// Mark the first entry optional.
    pub fn set_optional(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_optional(p) },
                real = unsafe { crate::ffi::PMIx_Info_optional(p) }
            )
        });
    }
    /// Mark the first entry persistent.
    pub fn set_persistent(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_persistent(p) },
                real = unsafe { crate::ffi::PMIx_Info_persistent(p) }
            )
        });
    }
    /// Mark the first entry a qualifier.
    pub fn set_qualifier(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_qualifier(p) },
                real = unsafe { crate::ffi::PMIx_Info_qualifier(p) }
            )
        });
    }
    /// Mark the first entry processed.
    pub fn set_processed(&mut self) {
        self.set_with(|p| {
            crate::pmix_ffi_or_mock!(
                mock = unsafe { crate::mock_ffi::mock_info_processed(p) },
                real = unsafe { crate::ffi::PMIx_Info_processed(p) }
            )
        });
    }
    /// Mark the first entry as the end marker.
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

    #[test]
    fn mock_pmix_info_constructs_and_drops() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let info = PmixInfo::new();
        assert!(!info.as_ptr().is_null());
    }

    #[test]
    fn mock_info_helpers_cover_success_paths() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut dest = string_kv("k", "v");
        let src = string_kv("k2", "v2");
        assert!(dest.xfer_from(&src).is_ok());
        assert_eq!(dest.get_size().unwrap(), 0);
        assert_eq!(dest.info_string().unwrap(), "mock info");
        assert!(!dest.is_required());
        assert!(!dest.is_optional());
        assert!(!dest.is_persistent());
        assert!(!dest.is_qualifier());
        assert!(!dest.is_end());
        assert!(!dest.was_processed());
        assert!(!dest.is_true());
        dest.set_required(); dest.set_optional(); dest.set_persistent();
        dest.set_qualifier(); dest.set_processed(); dest.set_end();
    }

    #[test]
    fn mock_info_status_override_is_returned_as_error() {
        let config = crate::mock_ffi::MockConfig::new().with_function_status("PMIx_Info_get_size", crate::mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = crate::mock_ffi::MockGuard::with_config(config);
        let info = string_kv("k", "v");
        assert!(info.get_size().is_err());
    }
}
