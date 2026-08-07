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

/// Safe wrapper around a PMIx info linked list (`PMIx_Info_list_start`/Release).
///
/// The list is owned by this value and is deliberately not transferable between
/// threads. `iter` copies the raw entries returned by PMIx; it is intended for
/// inspection and does not create owned `Info` values.
#[derive(Debug)]
pub struct PmixInfoList {
    handle: std::ptr::NonNull<std::ffi::c_void>,
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl PmixInfoList {
    /// Start an empty PMIx info list.
    pub fn new() -> Result<Self, crate::PmixError> {
        let handle = crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_list_start() },
            real = unsafe { crate::ffi::PMIx_Info_list_start() },
        );
        std::ptr::NonNull::new(handle)
            .map(|handle| Self {
                handle,
                _not_thread_safe: std::marker::PhantomData,
            })
            .ok_or(crate::PmixError::ErrNomem)
    }

    /// Return the opaque PMIx list handle for FFI escape hatches.
    pub fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.handle.as_ptr()
    }

    /// Number of entries in the list.
    pub fn len(&self) -> usize {
        crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_list_get_size(self.as_ptr()) },
            real = unsafe { crate::ffi::PMIx_Info_list_get_size(self.as_ptr()) },
        )
    }

    /// Return whether the list has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy the current entries into raw snapshots. PMIx remains responsible
    /// for any nested allocations in the copied C structs.
    pub fn iter(&self) -> Vec<crate::ffi::pmix_info_t> {
        let mut result = Vec::new();
        let mut previous = std::ptr::null_mut();
        loop {
            let mut next = std::ptr::null_mut();
            let info = crate::pmix_ffi_or_mock!(
                mock = unsafe {
                    crate::mock_ffi::mock_info_list_get_info(self.as_ptr(), previous, &mut next)
                },
                real = unsafe {
                    crate::ffi::PMIx_Info_list_get_info(self.as_ptr(), previous, &mut next)
                },
            );
            if info.is_null() {
                break;
            }
            result.push(unsafe { info.read() });
            if next.is_null() || next == previous {
                break;
            }
            previous = next;
        }
        result
    }

    fn status(status: crate::ffi::pmix_status_t) -> Result<(), crate::PmixStatus> {
        let raw = status;
        if raw == crate::ffi::PMIX_SUCCESS as i32 {
            Ok(())
        } else {
            Err(crate::PmixStatus::from_raw(raw))
        }
    }

    fn key(key: &str) -> Result<std::ffi::CString, crate::PmixStatus> {
        std::ffi::CString::new(key).map_err(|_| crate::PmixStatus::from_raw(-27))
    }

    /// Add a value represented by bytes and a PMIx data type.
    pub fn add<V: AsRef<[u8]>>(
        &mut self,
        key: &str,
        value: V,
        ty: crate::ffi::pmix_data_type_t,
    ) -> Result<(), crate::PmixStatus> {
        let key = Self::key(key)?;
        let value = value.as_ref();
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_add(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                )
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_add(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                )
            },
        );
        Self::status(status)
    }

    /// Add a value, optionally overwriting an existing key.
    pub fn add_unique<V: AsRef<[u8]>>(
        &mut self,
        key: &str,
        value: V,
        ty: crate::ffi::pmix_data_type_t,
        overwrite: bool,
    ) -> Result<(), crate::PmixStatus> {
        let key = Self::key(key)?;
        let value = value.as_ref();
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_add_unique(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                    overwrite,
                )
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_add_unique(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                    overwrite,
                )
            },
        );
        Self::status(status)
    }

    /// Add an owned PMIx value.
    pub fn add_value(
        &mut self,
        key: &str,
        value: &crate::PmixOwnedValue,
    ) -> Result<(), crate::PmixStatus> {
        let key = Self::key(key)?;
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_add_value(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_raw(),
                )
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_add_value(self.as_ptr(), key.as_ptr(), value.as_raw())
            },
        );
        Self::status(status)
    }

    /// Add an owned PMIx value, optionally overwriting an existing key.
    pub fn add_value_unique(
        &mut self,
        key: &str,
        value: &crate::PmixOwnedValue,
        overwrite: bool,
    ) -> Result<(), crate::PmixStatus> {
        let key = Self::key(key)?;
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_add_value_unique(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_raw(),
                    overwrite,
                )
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_add_value_unique(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_raw(),
                    overwrite,
                )
            },
        );
        Self::status(status)
    }

    /// Add a value at the front of the list.
    pub fn prepend<V: AsRef<[u8]>>(
        &mut self,
        key: &str,
        value: V,
        ty: crate::ffi::pmix_data_type_t,
    ) -> Result<(), crate::PmixStatus> {
        let key = Self::key(key)?;
        let value = value.as_ref();
        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_prepend(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                )
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_prepend(
                    self.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr().cast(),
                    ty,
                )
            },
        );
        Self::status(status)
    }

    /// Insert an individually constructed info entry.
    pub fn insert(&mut self, info: &mut PmixInfo) -> Result<(), crate::PmixStatus> {
        Self::status(crate::pmix_ffi_or_mock!(
            mock =
                unsafe { crate::mock_ffi::mock_info_list_insert(self.as_ptr(), info.as_mut_ptr()) },
            real = unsafe { crate::ffi::PMIx_Info_list_insert(self.as_ptr(), info.as_mut_ptr()) },
        ))
    }

    /// Transfer an info array into the list.
    pub fn xfer(&mut self, info: &Info) -> Result<(), crate::PmixStatus> {
        Self::status(crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_list_xfer(self.as_ptr(), info.as_ptr()) },
            real = unsafe { crate::ffi::PMIx_Info_list_xfer(self.as_ptr(), info.as_ptr()) },
        ))
    }

    /// Transfer an info array, optionally overwriting duplicate keys.
    pub fn xfer_unique(&mut self, info: &Info, overwrite: bool) -> Result<(), crate::PmixStatus> {
        Self::status(crate::pmix_ffi_or_mock!(
            mock = unsafe {
                crate::mock_ffi::mock_info_list_xfer_unique(self.as_ptr(), info.as_ptr(), overwrite)
            },
            real = unsafe {
                crate::ffi::PMIx_Info_list_xfer_unique(self.as_ptr(), info.as_ptr(), overwrite)
            },
        ))
    }

    /// Convert the list to a PMIx data array. The returned raw array is owned
    /// by the caller according to PMIx's `pmix_data_array_t` ownership rules.
    pub fn convert(&mut self) -> Result<crate::ffi::pmix_data_array_t, crate::PmixStatus> {
        let mut array = unsafe { std::mem::zeroed() };
        Self::status(crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_list_convert(self.as_ptr(), &mut array) },
            real = unsafe { crate::ffi::PMIx_Info_list_convert(self.as_ptr(), &mut array) },
        ))
        .map(|()| array)
    }
}

impl Drop for PmixInfoList {
    fn drop(&mut self) {
        crate::pmix_ffi_or_mock!(
            mock = unsafe { crate::mock_ffi::mock_info_list_release(self.as_ptr()) },
            real = unsafe { crate::ffi::PMIx_Info_list_release(self.as_ptr()) },
        );
    }
}



#[cfg(test)]
mod info_list_tests {
    use super::*;

    #[test]
    fn mock_info_list_lifecycle_and_success_paths() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut list = PmixInfoList::new().expect("mock list");
        assert!(!list.as_ptr().is_null());
        assert_eq!(list.len(), 0);
        assert!(
            list.add("key", [1_u8, 2], crate::ffi::PMIX_BYTE as _)
                .is_ok()
        );
        assert!(
            list.add_unique("key", [1_u8], crate::ffi::PMIX_BYTE as _, true)
                .is_ok()
        );
        assert!(
            list.prepend("key", [1_u8], crate::ffi::PMIX_BYTE as _)
                .is_ok()
        );
        let value = crate::PmixValueBuilder::new().uint32(7).build().unwrap();
        assert!(list.add_value("key", &value).is_ok());
        assert!(list.add_value_unique("key", &value, true).is_ok());
        assert!(list.xfer(&empty()).is_ok());
        assert!(list.xfer_unique(&empty(), true).is_ok());
        assert!(list.insert(&mut PmixInfo::new()).is_ok());
        assert!(list.iter().is_empty());
        assert!(list.convert().is_ok());
    }
}
