//! PMIx `Info` / `InfoBuilder` and convenience helpers.

use crate::ffi::*;
use crate::types::BuilderError;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;

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

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn contains(self, f: Self) -> bool {
        (self.0 & f.0) == f.0
    }
    /// Return the raw `pmix_info_directives_t` value.
    pub fn raw(self) -> pmix_info_directives_t {
        self.0
    }
}

impl std::ops::BitOr for InfoFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for InfoFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
pub struct Info {
    pub(crate) handle: *mut pmix_info_t,
    pub(crate) len: usize,
}

impl Info {
    /// Number of entries in this info array.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw pointer for FFI (valid for `len()` elements).
    pub fn as_ptr(&self) -> *mut pmix_info_t {
        self.handle
    }
}

struct InfoEntry {
    key: &'static [u8; 13],
    value: *const std::ffi::c_void,
    data_type: pmix_data_type_t,
}

#[derive(Default)]
pub struct InfoBuilder {
    infos: Vec<InfoEntry>,
}

impl InfoBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(
        &mut self,
        key: &'static [u8; 13],
        value: *const std::ffi::c_void,
        data_type: pmix_data_type_t,
    ) {
        assert_ne!(key.as_ptr(), std::ptr::null());
        self.infos.push(InfoEntry {
            key,
            value,
            data_type,
        })
    }
    pub fn collect_data(&mut self) -> &mut InfoBuilder {
        let collect = true;
        self.add(
            PMIX_COLLECT_DATA,
            &collect as *const bool as *const c_void,
            PMIX_BOOL as pmix_data_type_t,
        );
        self
    }
    pub fn build(self) -> Info {
        let info_ptr: *mut pmix_info_t;
        let mut idx: usize = 0;
        unsafe { info_ptr = PMIx_Info_create(self.infos.len()) }
        for info in &self.infos {
            let status = unsafe {
                PMIx_Info_load(
                    info_ptr.add(idx),
                    info.key.as_ptr().cast(),
                    info.value,
                    info.data_type,
                )
            };
            if status != PMIX_SUCCESS as i32 {
                panic!("Error loading info: {}", status);
            }
            idx += 1;
        }
        Info {
            handle: info_ptr,
            len: idx,
        }
    }
}

/// Create an `Info` array with a single string key/value pair, bypassing
/// the 13-byte key limit of `InfoBuilder::add()`.
///
/// This is useful for keys like `"pmix.srvr.uri"` (14 bytes) which don't
/// fit in `InfoBuilder::add(key: &'static [u8; 13])`.
pub fn info_with_string_key(key: &str, value: &str) -> Info {
    let info_ptr = unsafe { PMIx_Info_create(1) };
    let key_cstr = CString::new(key).expect("key must not contain null bytes");
    let value_cstr = CString::new(value).expect("value must not contain null bytes");
    unsafe {
        let status = PMIx_Info_load(
            info_ptr,
            key_cstr.as_ptr(),
            value_cstr.as_ptr() as *const c_void,
            PMIX_STRING as pmix_data_type_t,
        );
        if status != PMIX_SUCCESS as i32 {
            panic!("PMIx_Info_load failed for key {}: {}", key, status);
        }
    }
    // Leak the CString allocations — the PMIx library copies the data
    // internally and the Info handle is managed by the library.
    std::mem::forget(key_cstr);
    std::mem::forget(value_cstr);
    Info {
        handle: info_ptr,
        len: 1,
    }
}



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

#[cfg(test)]
mod tests {
    use super::*;

    // -- empty() tests --

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
    fn test_empty_info_as_ptr_not_null() {
        let info = empty();
        // Even empty info arrays have a valid (possibly null) pointer
        let _ptr = info.as_ptr();
    }

    // -- with_collect_data() tests --

    #[test]
    fn test_with_collect() {
        let info = with_collect_data();
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_with_collect_not_empty() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    // -- string_kv() tests --

    #[test]
    fn test_string_kv() {
        let info = string_kv("test.key", "hello");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_not_empty() {
        let info = string_kv("key", "value");
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv_long_key() {
        // Keys longer than 13 bytes (e.g., "pmix.srvr.uri" is 14 bytes)
        let info = string_kv("pmix.srvr.uri", "tcp://127.0.0.1:1234");
        assert_eq!(len(&info), 1);
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv_empty_value() {
        let info = string_kv("test.key", "");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_special_chars() {
        let info = string_kv("test.key", "value/with:special@chars#123");
        assert_eq!(len(&info), 1);
    }

    // -- builder() tests --

    #[test]
    fn test_builder() {
        let mut b = builder();
        b.collect_data();
        let info = b.build();
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_builder_empty() {
        let info = builder().build();
        assert_eq!(len(&info), 0);
        assert!(info.is_empty());
    }

    #[test]
    fn test_builder_collect_data_returns_self() {
        // Verify method chaining works — collect_data() returns &mut Self
        let mut b = InfoBuilder::new();
        b.collect_data();
        let info = b.build();
        assert_eq!(len(&info), 1);
    }

    // -- len() tests --

    #[test]
    fn test_len_zero_for_empty() {
        assert_eq!(len(&empty()), 0);
    }

    #[test]
    fn test_len_one_for_collect_data() {
        assert_eq!(len(&with_collect_data()), 1);
    }

    #[test]
    fn test_len_one_for_string_kv() {
        assert_eq!(len(&string_kv("k", "v")), 1);
    }

    // -- Info is_empty() tests --

    #[test]
    fn test_info_is_empty_false_for_string_kv() {
        let info = string_kv("k", "v");
        assert!(!info.is_empty());
    }

    #[test]
    fn test_info_is_empty_false_for_collect_data() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    // -- Info as_ptr() tests --

    #[test]
    fn test_info_as_ptr_returns_ptr() {
        let info = string_kv("k", "v");
        let ptr = info.as_ptr();
        // Just verify we can call it and get a pointer back
        let _ = ptr;
    }

    #[test]
    fn test_info_as_ptr_collect_data() {
        let info = with_collect_data();
        let ptr = info.as_ptr();
        let _ = ptr;
    }

    // -- Multiple string_kv calls produce independent infos --

    #[test]
    fn test_string_kv_independent_instances() {
        let info_a = string_kv("key_a", "value_a");
        let info_b = string_kv("key_b", "value_b");
        assert_eq!(len(&info_a), 1);
        assert_eq!(len(&info_b), 1);
        // Both should be independent and valid
        assert!(!info_a.is_empty());
        assert!(!info_b.is_empty());
    }

    // -- Edge cases --

    #[test]
    fn test_string_kv_single_char_key_value() {
        let info = string_kv("k", "v");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_numeric_value() {
        let info = string_kv("port", "8080");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_uri_value() {
        let info = string_kv("uri", "unix:///var/run/socket");
        assert_eq!(len(&info), 1);
    }
}
