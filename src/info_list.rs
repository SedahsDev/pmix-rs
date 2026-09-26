//! Info list management — `PMIx_Info_list_*` API
//!
//! This module provides safe Rust wrappers around the PMIx Info list API,
//! which is used to construct lists of key-value pairs for various PMIx
//! operations such as initialization, queries, and notifications.
//!
//! # Info list lifecycle
//!
//! An `InfoList` is created via [`InfoList::new()`], populated with
//! key-value pairs using methods like [`add()`], and automatically released
//! when dropped.
//!
//! # Thread safety
//!
//! `InfoList` is `!Send` + `!Sync` because it owns C memory that is not
//! free-threaded. Share only behind your own synchronization primitives.
//!
//! # Examples
//!
//! ```no_run
//! use pmix::info_list::InfoList;
//!
//! let mut list = InfoList::new().expect("create list");
//! list.add("key1", "value1").expect("add string");
//! // list is automatically released when dropped
//! ```

use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr;

use crate::ffi;
use crate::PmixStatus;

/// A PMIx Info list for managing collections of key-value pairs.
///
/// Info lists are used to pass multiple configuration values to PMIx
/// functions like initialization and queries. The list manages its own
/// memory and is automatically released when dropped.
///
/// # C API Correspondence
///
/// This wrapper corresponds to the PMIx Info list API:
/// - `PMIx_Info_list_start()` → `InfoList::new()`
/// - `PMIx_Info_list_add()` → `InfoList::add()`
/// - `PMIx_Info_list_convert()` → `InfoList::convert()`
/// - `PMIx_Info_list_release()` → Drop implementation
///
/// # Thread Safety
///
/// `InfoList` is `!Send` + `!Sync` because it owns C memory that is not
/// free-threaded. Share only behind your own synchronization primitives.
///
/// # Examples
///
/// ```no_run
/// use pmix::info_list::InfoList;
///
/// let mut list = InfoList::new().expect("create list");
/// list.add("key1", "value1").expect("add key-value");
/// drop(list); // Automatically calls PMIx_Info_list_release
/// ```
pub struct InfoList {
    /// Raw pointer to the underlying C Info list
    ptr: *mut std::ffi::c_void,
    /// Makes this type `!Send` + `!Sync` (owns PMIx C memory)
    _not_thread_safe: PhantomData<*mut u8>,
}

// InfoList is not Send/Sync because it owns C memory
unsafe impl Send for InfoList {}
unsafe impl Sync for InfoList {}

impl InfoList {
    /// Create a new empty Info list.
    ///
    /// Allocates and initializes a PMIx Info list. Returns an error if
    /// allocation fails (null pointer returned).
    ///
    /// # C API
    /// `pmix_status_t PMIx_Info_list_start(void)`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pmix::info_list::InfoList;
    ///
    /// let list = InfoList::new().expect("failed to create list");
    /// ```
    pub fn new() -> Result<Self, PmixStatus> {
        // SAFETY: PMIx_Info_list_start takes no parameters and returns a
        // newly allocated Info list, or null on failure. No pointers are
        // dereferenced.
        let ptr = unsafe { ffi::PMIx_Info_list_start() };
        if ptr.is_null() {
            return Err(PmixStatus::Unknown(-1));
        }
        Ok(Self {
            ptr,
            _not_thread_safe: PhantomData,
        })
    }

    /// Check if the Info list pointer is valid (non-null).
    ///
    /// Returns `false` if the list has been dropped or never created.
    pub fn is_valid(&self) -> bool {
        !self.ptr.is_null()
    }

    /// Add a string key-value pair to the list.
    ///
    /// # C API
    /// `pmix_status_t PMIx_Info_list_add(pmix_info_list_t *list,`
    /// `                                  const char *key,`
    /// `                                  const void *value,`
    /// `                                  pmix_data_type_t type)`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pmix::info_list::InfoList;
    ///
    /// let mut list = InfoList::new().expect("create list");
    /// list.add("key", "value").expect("add string pair");
    /// ```
    pub fn add(&mut self, key: &str, value: &str) -> Result<(), PmixStatus> {
        if self.ptr.is_null() {
            return Err(PmixStatus::Unknown(-1));
        }

        let key_cstr = CString::new(key).map_err(|_| PmixStatus::Unknown(-2))?;
        let value_cstr = CString::new(value).map_err(|_| PmixStatus::Unknown(-3))?;

        // SAFETY: ptr is valid (checked above), key and value are valid C strings
        unsafe {
            let status = ffi::PMIx_Info_list_add(
                self.ptr,
                key_cstr.as_ptr(),
                value_cstr.as_ptr() as *const std::ffi::c_void,
                3, // PMIX_STRING constant
            );
            let status = PmixStatus::from_raw(status);
            if status.is_error() {
                Err(status)
            } else {
                Ok(())
            }
        }
    }

    /// Convert the Info list to a pmix_data_array_t.
    ///
    /// # C API
    /// `pmix_status_t PMIx_Info_list_convert(pmix_info_list_t *list,`
    /// `                                     pmix_data_array_t *array)`
    ///
    /// # Note
    ///
    /// After conversion, the Info list should not be used further.
    pub fn convert(&mut self, array: &mut crate::ffi::pmix_data_array_t) -> Result<(), PmixStatus> {
        if self.ptr.is_null() {
            return Err(PmixStatus::Unknown(-1));
        }

        // SAFETY: ptr is valid
        unsafe {
            let status = ffi::PMIx_Info_list_convert(self.ptr, array);
            let status = PmixStatus::from_raw(status);
            if status.is_error() {
                Err(status)
            } else {
                Ok(())
            }
        }
    }

    /// Get an iterator over the Info list entries.
    ///
    /// # C API
    /// `pmix_info_t *PMIx_Info_list_get_info(pmix_info_list_t *list,`
    /// `                                     pmix_info_t *prev,`
    /// `                                     pmix_info_t **next)`
    ///
    /// # Note
    ///
    /// This returns raw pointers to pmix_info_t structures. Use with caution.
    pub unsafe fn get_info_iter(
        &self,
        prev: *mut std::ffi::c_void,
    ) -> *mut crate::ffi::pmix_info_t {
        if self.ptr.is_null() {
            return ptr::null_mut();
        }
        unsafe { ffi::PMIx_Info_list_get_info(self.ptr, prev, std::ptr::null_mut()) }
    }
}

impl Drop for InfoList {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: The pointer is valid and was created by PMIx_Info_list_start.
            // PMIx_Info_list_release frees the Info list memory. Safe to call once.
            unsafe { ffi::PMIx_Info_list_release(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}

impl std::fmt::Debug for InfoList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ptr.is_null() {
            f.debug_struct("InfoList").field("ptr", &"null").finish()
        } else {
            f.debug_struct("InfoList")
                .field("ptr", &self.ptr)
                .finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infolist_create() {
        let list = InfoList::new().expect("should create list");
        assert!(list.is_valid());
    }

    #[test]
    fn test_infolist_add() {
        let mut list = InfoList::new().expect("create list");
        assert!(list.add("key", "value").is_ok());
    }

    #[test]
    fn test_infolist_drop() {
        let list = InfoList::new().expect("create list");
        // Should not panic on drop
        drop(list);
    }

    #[test]
    fn test_infolist_debug() {
        let list = InfoList::new().expect("create list");
        let debug_str = format!("{:?}", list);
        assert!(debug_str.contains("InfoList"));
    }
}
