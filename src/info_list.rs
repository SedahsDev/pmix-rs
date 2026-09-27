//! Safe RAII wrapper for PMIx Info List operations.
//!
//! This module provides the `InfoList` struct which wraps the PMIx info list
//! API with automatic memory management. The info list is used to build
//! collections of key-value pairs that can be converted to `Info` arrays.
//!
//! # Example
//!
//! ```no_run
//! use pmix::info_list::InfoList;
//!
//! let mut list = InfoList::new().expect("failed to create info list");
//! list.add("pmix.get.refresh", true).expect("failed to add key");
//! list.add("custom.key", "value").expect("failed to add key");
//!
//! // Convert to Info array for use with PMIx APIs
//! let (info_ptr, count) = list.convert().expect("failed to convert to info array");
//! // Don't forget to free when done:
//! // unsafe { ffi::PMIx_Info_array_free(info_ptr, count) };
//! ```

use std::ffi::c_void;
use std::ptr;

use crate::ffi;
use crate::PmixError;

/// Safe RAII wrapper for `pmix_info_list_t`.
///
/// This struct manages the lifecycle of a PMIx info list, automatically
/// calling `PMIx_Info_list_release` when the list is dropped to prevent
/// memory leaks.
///
/// The info list API allows building collections of key-value pairs that
/// can later be converted to `pmix_info_t` arrays for use with PMIx functions.
#[derive(Debug)]
pub struct InfoList {
    ptr: *mut c_void,
}

impl InfoList {
    /// Create a new empty info list.
    ///
    /// # Errors
    ///
    /// Returns `Err(PmixError::Error)` if the info list allocation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pmix::info_list::InfoList;
    ///
    /// let list = InfoList::new().expect("failed to create info list");
    /// ```
    pub fn new() -> Result<Self, PmixError> {
        let ptr = unsafe { ffi::PMIx_Info_list_start() };
        if ptr.is_null() {
            return Err(PmixError::Error);
        }
        Ok(Self { ptr })
    }

    /// Create an `InfoList` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure `ptr` is a valid, allocated info list pointer
    /// or null. The resulting `InfoList` takes ownership and will call
    /// `PMIx_Info_list_release` on drop.
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self { ptr }
    }

    /// Check if the info list pointer is valid (non-null).
    pub fn is_valid(&self) -> bool {
        !self.ptr.is_null()
    }

    /// Create a null (invalid) `InfoList` for testing error paths.
    ///
    /// The list has a null pointer and `is_valid()` returns `false`.
    /// Dropping a null list is safe — it performs no operations.
    pub fn null() -> Self {
        Self {
            ptr: ptr::null_mut(),
        }
    }

    /// Add a key-value pair to the info list.
    ///
    /// This adds a new entry to the list with the given key and value.
    /// If the key already exists, multiple entries with the same key may
    /// be added (use `add_unique` to prevent duplicates).
    ///
    /// # Arguments
    ///
    /// * `key` - The key name (must be a valid C string)
    /// * `value` - The value to associate with the key
    ///
    /// # Errors
    ///
    /// Returns an error if the addition fails or if the key contains null bytes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pmix::info_list::InfoList;
    ///
    /// let mut list = InfoList::new().unwrap();
    /// list.add("pmix.get.refresh", true).unwrap();
    /// list.add("custom.string", "value").unwrap();
    /// list.add("custom.int", 42i32).unwrap();
    /// ```
    pub fn add_bool(&mut self, key: &str, value: bool) -> Result<(), PmixError> {
        let c_key = std::ffi::CString::new(key).map_err(|_| PmixError::ErrBadParam)?;
        let val: i32 = if value { 1 } else { 0 };
        
        unsafe {
            let status = ffi::PMIx_Info_list_add(
                self.ptr,
                c_key.as_ptr(),
                &val as *const i32 as *const c_void,
                ffi::PMIX_BOOL as u16,
            );
            if status != ffi::PMIX_SUCCESS as i32 {
                return Err(PmixError::from_raw(status).unwrap_or(PmixError::Error));
            }
        }
        Ok(())
    }

    pub fn add_int(&mut self, key: &str, value: i32) -> Result<(), PmixError> {
        let c_key = std::ffi::CString::new(key).map_err(|_| PmixError::ErrBadParam)?;
        
        unsafe {
            let status = ffi::PMIx_Info_list_add(
                self.ptr,
                c_key.as_ptr(),
                &value as *const i32 as *const c_void,
                ffi::PMIX_INT as u16,
            );
            if status != ffi::PMIX_SUCCESS as i32 {
                return Err(PmixError::from_raw(status).unwrap_or(PmixError::Error));
            }
        }
        Ok(())
    }

    pub fn add_string(&mut self, key: &str, value: &str) -> Result<(), PmixError> {
        let c_key = std::ffi::CString::new(key).map_err(|_| PmixError::ErrBadParam)?;
        let c_value = std::ffi::CString::new(value).map_err(|_| PmixError::ErrBadParam)?;
        
        unsafe {
            let status = ffi::PMIx_Info_list_add(
                self.ptr,
                c_key.as_ptr(),
                c_value.as_ptr() as *const c_void,
                ffi::PMIX_STRING as u16,
            );
            if status != ffi::PMIX_SUCCESS as i32 {
                return Err(PmixError::from_raw(status).unwrap_or(PmixError::Error));
            }
        }
        Ok(())
    }

    /// Add a unique key-value pair to the info list.
    ///
    /// This adds a new entry only if the key doesn't already exist in the list.
    /// If the key exists, the addition is silently ignored.
    ///
    /// # Arguments
    ///
    /// * `key` - The key name (must be a valid C string)
    /// * `value` - The value to associate with the key
    ///
    /// # Errors
    ///
    /// Returns an error if the addition fails or if the key contains null bytes.
    pub fn add_bool_unique(&mut self, key: &str, value: bool) -> Result<(), PmixError> {
        let c_key = std::ffi::CString::new(key).map_err(|_| PmixError::ErrBadParam)?;
        let val: i32 = if value { 1 } else { 0 };
        
        unsafe {
            let status = ffi::PMIx_Info_list_add_unique(
                self.ptr,
                c_key.as_ptr(),
                &val as *const i32 as *const c_void,
                ffi::PMIX_BOOL as u16,
                false,
            );
            if status != ffi::PMIX_SUCCESS as i32 {
                return Err(PmixError::from_raw(status).unwrap_or(PmixError::Error));
            }
        }
        Ok(())
    }

    /// Add a value to the info list with automatic type detection.
    ///
    /// This is a convenience method that automatically determines the PMIx
    /// data type from the Rust value type.
    ///
    /// # Arguments
    ///
    /// * `key` - The key name
    /// * `value` - The value to add
    ///
    /// # Errors
    ///
    /// Returns an error if the addition fails.


    /// Prepend a key-value pair to the info list.
    ///
    /// This adds the entry at the beginning of the list rather than the end.
    ///
    /// # Arguments
    ///
    /// * `key` - The key name
    /// * `value` - The value to add
    ///
    /// # Errors
    ///
    /// Returns an error if the addition fails.
    pub fn prepend_bool(&mut self, key: &str, value: bool) -> Result<(), PmixError> {
        let c_key = std::ffi::CString::new(key).map_err(|_| PmixError::ErrBadParam)?;
        let val: i32 = if value { 1 } else { 0 };
        
        unsafe {
            let status = ffi::PMIx_Info_list_prepend(
                self.ptr,
                c_key.as_ptr(),
                &val as *const i32 as *const c_void,
                ffi::PMIX_BOOL as u16,
            );
            if status != ffi::PMIX_SUCCESS as i32 {
                return Err(PmixError::from_raw(status).unwrap_or(PmixError::Error));
            }
        }
        Ok(())
    }


}
