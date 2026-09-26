//! Native Rust wrappers for PMIx struct types.
//!
//! This module provides owned Rust types that wrap PMIx's C structs
//! (proc, info, pdata, regattr) to prevent FFI type leakage into
//! the public API. Each wrapper provides safe constructors, accessors,
//! and conversion methods.

use crate::ffi::{pmix_info_t, pmix_pdata_t, pmix_proc_t};
use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::c_char;
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PmixProc - wrapper for pmix_proc_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_proc_t`.
///
/// Represents a process identifier in PMIx, containing a namespace
/// and rank. This wrapper provides safe Rust APIs for constructing
/// and manipulating process identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PmixProc {
    /// Namespace string (up to 255 characters + null terminator)
    nspace: String,
    /// Process rank (u32, with PMIX_RANK_WILDCARD as special value)
    rank: u32,
}

impl PmixProc {
    /// Create a new `PmixProc` from namespace and rank.
    pub fn new(nspace: &str, rank: u32) -> Self {
        // Ensure namespace fits in the C type (256 bytes max)
        let nspace_truncated = nspace.chars().take(255).collect::<String>();
        Self {
            nspace: nspace_truncated,
            rank,
        }
    }

    /// Create from namespace and wildcard rank.
    pub fn wildcard(nspace: &str) -> Self {
        Self::new(nspace, u32::MAX) // PMIX_RANK_WILDCARD
    }

    /// Get namespace as a string reference.
    pub fn nspace(&self) -> &str {
        &self.nspace
    }

    /// Get rank.
    pub fn rank(&self) -> u32 {
        self.rank
    }

    /// Convert to raw `pmix_proc_t` for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure `nspace` doesn't contain interior NUL bytes
    /// and that the resulting `pmix_proc_t` is used only for the duration
    /// of the FFI call.
    pub unsafe fn into_raw(self) -> pmix_proc_t {
        let mut proc = unsafe { mem::zeroed() };
        
        // Copy namespace into the fixed-size array
        let nspace_bytes = self.nspace.as_bytes();
        let copy_len = nspace_bytes.len().min(proc.nspace.len());
        ptr::copy_nonoverlapping(
            nspace_bytes.as_ptr() as *const c_char,
            proc.nspace.as_mut_ptr(),
            copy_len,
        );
        
        // Null-terminate
        if copy_len < proc.nspace.len() {
            proc.nspace[copy_len] = 0;
        }
        
        proc.rank = self.rank;
        proc
    }

    /// Create from raw `pmix_proc_t`.
    ///
    /// # Safety
    /// The caller must ensure `proc` is a valid `pmix_proc_t` with a
    /// null-terminated namespace.
    pub unsafe fn from_raw(proc: pmix_proc_t) -> Option<Self> {
        // Find the null terminator to get the actual namespace length
        let nspace_ptr = proc.nspace.as_ptr() as *const u8;
        let mut len = 0;
        while len < proc.nspace.len() && *nspace_ptr.add(len) != 0 {
            len += 1;
        }
        
        if len == 0 {
            return None;
        }
        
        let nspace = CStr::from_bytes_with_nul_unchecked(
            std::slice::from_raw_parts(nspace_ptr, len + 1),
        )
        .to_str()
        .ok()?
        .to_string();
        
        Some(Self {
            nspace,
            rank: proc.rank,
        })
    }

    /// Borrow as raw `pmix_proc_t` for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure the `PmixProc` outlives any FFI call
    /// that uses this pointer.
    pub unsafe fn as_raw(&self) -> pmix_proc_t {
        let mut proc = unsafe { mem::zeroed() };
        
        let nspace_bytes = self.nspace.as_bytes();
        let copy_len = nspace_bytes.len().min(proc.nspace.len());
        ptr::copy_nonoverlapping(
            nspace_bytes.as_ptr() as *const c_char,
            proc.nspace.as_mut_ptr(),
            copy_len,
        );
        
        if copy_len < proc.nspace.len() {
            proc.nspace[copy_len] = 0;
        }
        
        proc.rank = self.rank;
        proc
    }
}

impl Default for PmixProc {
    fn default() -> Self {
        Self::new("", 0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixInfo - wrapper for pmix_info_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_info_t`.
///
/// Represents a key-value pair in PMIx info arrays. This wrapper provides
/// safe constructors and accessors for building info arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmixInfo {
    /// Key string
    key: CString,
    /// Value string (stored as string for simplicity; other types use PmixValue)
    value: Option<String>,
}

impl PmixInfo {
    /// Create a new `PmixInfo` with a key and optional string value.
    pub fn new(key: &str, value: Option<&str>) -> Result<Self, std::ffi::NulError> {
        Ok(Self {
            key: CString::new(key)?,
            value: value.map(String::from),
        })
    }

    /// Create a key-only info entry.
    pub fn key_only(key: &str) -> Result<Self, std::ffi::NulError> {
        Self::new(key, None)
    }

    /// Create a key-value info entry.
    pub fn with_value(key: &str, value: &str) -> Result<Self, std::ffi::NulError> {
        Self::new(key, Some(value))
    }

    /// Get key as a string reference.
    pub fn key(&self) -> &str {
        self.key.to_str().unwrap_or("<invalid>")
    }

    /// Get value as an option.
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// Convert to raw `pmix_info_t` for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure the returned pointer is valid for the
    /// duration of the FFI call and that self outlives the call.
    pub unsafe fn into_raw(self) -> pmix_info_t {
        let mut info = unsafe { mem::zeroed() };
        
        info.key = self.key.as_ptr();
        
        if let Some(ref v) = self.value {
            info.value = Some(ptr::addr_of_mut!({
                // Create a temporary pmix_value_t to hold the string
                let mut val = ptr::zeroed();
                val.type_ = 30; // PMIX_STRING
                val.data.string = CString::new(v.as_str()).unwrap().into_raw();
                val
            }) as *mut _);
        } else {
            info.value = None;
        }
        
        info
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPdata - wrapper for pmix_pdata_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_pdata_t`.
///
/// Represents a process data attribute containing a process identifier,
/// key, and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmixPdata {
    /// Process identifier
    proc: PmixProc,
    /// Key string
    key: CString,
    /// Value string
    value: String,
}

impl PmixPdata {
    /// Create a new `PmixPdata` with process, key, and value.
    pub fn new(
        proc: &PmixProc,
        key: &str,
        value: &str,
    ) -> Result<Self, std::ffi::NulError> {
        Ok(Self {
            proc: proc.clone(),
            key: CString::new(key)?,
            value: String::from(value),
        })
    }

    /// Get process reference.
    pub fn proc(&self) -> &PmixProc {
        &self.proc
    }

    /// Get key as a string reference.
    pub fn key(&self) -> &str {
        self.key.to_str().unwrap_or("<invalid>")
    }

    /// Get value as a string reference.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Convert to raw `pmix_pdata_t` for FFI calls.
    ///
    /// # Safety
    /// The caller must ensure self outlives the FFI call.
    pub unsafe fn into_raw(self) -> pmix_pdata_t {
        let mut pdata = unsafe { mem::zeroed() };
        
        // Copy proc
        pdata.proc = self.proc.as_raw();
        
        // Copy key
        pdata.key = self.key.as_ptr();
        
        // Copy value
        pdata.value = CString::new(self.value).unwrap().into_raw();
        
        pdata
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixRegattr - wrapper for pmix_regattr_t (if it exists in PMIx)
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_regattr_t`.
///
/// Represents an attribute registration for PMIx server callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmixRegattr {
    /// Attribute name/key
    name: CString,
    /// Expected data type
    data_type: u16,
    /// Whether this attribute is required
    required: bool,
    /// Description (optional)
    description: Option<CString>,
}

impl PmixRegattr {
    /// Create a new `PmixRegattr`.
    pub fn new(
        name: &str,
        data_type: u16,
        required: bool,
        description: Option<&str>,
    ) -> Result<Self, std::ffi::NulError> {
        Ok(Self {
            name: CString::new(name)?,
            data_type,
            required,
            description: description
                .map(|s| CString::new(s))
                .transpose()?,
        })
    }

    /// Get name as a string reference.
    pub fn name(&self) -> &str {
        self.name.to_str().unwrap_or("<invalid>")
    }

    /// Get data type.
    pub fn data_type(&self) -> u16 {
        self.data_type
    }

    /// Check if required.
    pub fn required(&self) -> bool {
        self.required
    }

    /// Get description as an option.
    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(|s| s.to_str().unwrap_or("<invalid>"))
    }
}
