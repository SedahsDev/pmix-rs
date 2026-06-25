//! Security operations — `PMIx_Get_credential` and `PMIx_Validate_credential`.
//!
//! This module provides safe Rust wrappers for PMIx credential management APIs.
//! Credentials are opaque byte objects issued by the host environment's security
//! system (e.g., Munge, Kerberos, X.509). The caller does not interpret the
//! credential — it is passed to [`validate_credential`] or forwarded to peers
//! for authentication.
//!
//! # Credential lifecycle
//!
//! 1. Client calls [`get_credential`] to obtain an opaque credential from the
//!    PMIx server / host environment.
//! 2. The credential (a [`PmixCredential`]) can be stored, transmitted, or
//!    passed to [`validate_credential`] to verify its validity.
//! 3. Non-blocking variants ([`get_credential_nb`], [`validate_credential_nb`])
//!    accept a callback trait and return immediately.
//!
//! # C API reference
//!
//! ```c
//! pmix_status_t PMIx_Get_credential(const pmix_info_t info[], size_t ninfo,
//!                                    pmix_byte_object_t *credential);
//! pmix_status_t PMIx_Get_credential_nb(const pmix_info_t info[], size_t ninfo,
//!                                       pmix_credential_cbfunc_t cbfunc, void *cbdata);
//! pmix_status_t PMIx_Validate_credential(const pmix_byte_object_t *cred,
//!                                        const pmix_info_t info[], size_t ninfo,
//!                                        pmix_info_t **results, size_t *nresults);
//! pmix_status_t PMIx_Validate_credential_nb(const pmix_byte_object_t *cred,
//!                                           const pmix_info_t info[], size_t ninfo,
//!                                           pmix_validation_cbfunc_t cbfunc, void *cbdata);
//! ```

use std::os::raw::{c_uchar, c_void};
use std::ptr;
use std::sync::{LazyLock, Mutex};

use crate::ffi;
use crate::{Info, PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixCredential — safe wrapper for pmix_byte_object_t
// ─────────────────────────────────────────────────────────────────────────────

/// An opaque credential returned by [`get_credential`].
///
/// Stores the credential bytes in a Rust-owned `Vec<u8>` and provides
/// a safe API for constructing and passing credentials to PMIx APIs.
/// The credential bytes are opaque to the caller — they are only
/// meaningful to the security system that issued them.
///
/// # C API
/// `typedef struct { char *bytes; size_t size; } pmix_byte_object_t;`
#[derive(Debug, Clone)]
pub struct PmixCredential {
    bytes: Vec<u8>,
}

impl PmixCredential {
    /// Create a credential from the given opaque bytes (copies the data).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }

    /// Create a credential from a `Vec<u8>` (takes ownership).
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Create an empty credential.
    pub fn empty() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Get a reference to the raw credential bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert to a C `pmix_byte_object_t` for FFI calls.
    ///
    /// Returns a heap-allocated `pmix_byte_object_t` whose `bytes` field
    /// points to a copy of our data. The caller must free the result
    /// with `free_c_ptr()`.
    fn as_c_mut_ptr(&self) -> *mut ffi::pmix_byte_object_t {
        let c_bytes = if self.bytes.is_empty() {
            ptr::null_mut()
        } else {
            // Allocate a copy of the bytes using libc malloc so we can
            // free it later with libc free.
            let layout = std::alloc::Layout::array::<u8>(self.bytes.len()).unwrap();
            let buf = unsafe { std::alloc::alloc(layout) as *mut std::os::raw::c_char };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.bytes.as_ptr(),
                    buf as *mut u8,
                    self.bytes.len(),
                );
            }
            buf
        };

        // Build the C struct on the heap using Box.
        let c_bo = Box::new(ffi::pmix_byte_object_t {
            bytes: c_bytes,
            size: self.bytes.len(),
        });
        Box::into_raw(c_bo)
    }

    /// Free a C `pmix_byte_object_t` that was created by `as_c_mut_ptr()`.
    ///
    /// # Safety
    /// `ptr` must have been returned by `as_c_mut_ptr()` and must not
    /// have been freed already.
    unsafe fn free_c_ptr(ptr: *mut ffi::pmix_byte_object_t) {
        if !ptr.is_null() {
            let bo = unsafe { Box::from_raw(ptr) };
            // Free the internal bytes buffer if non-null.
            if !bo.bytes.is_null() {
                let layout = std::alloc::Layout::array::<u8>(bo.size).unwrap();
                unsafe {
                    std::alloc::dealloc(bo.bytes as *mut u8, layout);
                }
            }
            // Box drop frees the struct itself.
        }
    }

    /// Get a raw `*const pmix_byte_object_t` for FFI calls.
    ///
    /// Returns a pointer to a leaked `pmix_byte_object_t` that points
    /// directly to our Rust-owned bytes buffer. The pointer is valid
    /// for as long as `self` is alive (the struct is leaked, but the
    /// bytes are owned by the Vec inside self).
    ///
    /// WARNING: The returned pointer should not be freed by the caller.
    /// Use `as_c_mut_ptr()` for operations that need a mutable copy.
    pub fn as_raw(&self) -> *const ffi::pmix_byte_object_t {
        let bo = Box::leak(Box::new(ffi::pmix_byte_object_t {
            bytes: if self.bytes.is_empty() {
                ptr::null_mut()
            } else {
                self.bytes.as_ptr() as *mut std::os::raw::c_char
            },
            size: self.bytes.len(),
        }));
        bo as *const ffi::pmix_byte_object_t
    }

    /// `true` if the credential has no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Length of the credential in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: copy bytes from a PMIx-allocated pmix_byte_object_t into a Vec<u8>
// and free the PMIx-allocated struct.
// ─────────────────────────────────────────────────────────────────────────────

/// Copy bytes from a PMIx-allocated `pmix_byte_object_t` into a Rust-owned
/// `Vec<u8>`, then free the PMIx-allocated memory.
///
/// # Safety
/// - `cred` must be a valid, non-null pointer to a `pmix_byte_object_t`
///   allocated by PMIx (i.e., via `pmix_malloc`).
/// - The caller must ensure the struct is valid and not already freed.
/// - Copy bytes from a PMIx-allocated `pmix_byte_object_t` into a Rust-owned
///   `Vec<u8>`, then free the PMIx-allocated memory.
///
/// # Safety
/// - `cred` must be a valid, non-null pointer to a `pmix_byte_object_t`
///   allocated by PMIx (i.e., via `pmix_malloc` / libc `malloc`).
/// - The caller must ensure the struct is valid and not already freed.
unsafe fn copy_and_free_pmix_byte_object(cred: *mut ffi::pmix_byte_object_t) -> Vec<u8> {
    let obj = unsafe { &*cred };
    let bytes = if !obj.bytes.is_null() && obj.size > 0 {
        unsafe {
            let slice = std::slice::from_raw_parts(obj.bytes as *const c_uchar, obj.size);
            slice.to_vec()
        }
    } else {
        Vec::new()
    };
    // Free the internal bytes buffer using libc free.
    if !obj.bytes.is_null() {
        unsafe {
            ffi::free(obj.bytes as *mut std::ffi::c_void);
        }
    }
    // Free the struct itself using libc free.
    unsafe {
        ffi::free(cred as *mut std::ffi::c_void);
    }
    bytes
}

// ─────────────────────────────────────────────────────────────────────────────
// get_credential — safe wrapper for PMIx_Get_credential
// ─────────────────────────────────────────────────────────────────────────────

/// Request a credential from the PMIx server or host environment.
///
/// This is a blocking call — it returns only after the server has obtained
/// and returned the credential (or an error). The credential is an opaque
/// byte object whose format depends on the host environment's security
/// system (e.g., Munge, Kerberos).
///
/// # Parameters
/// - `info`: Optional info entries that direct the credential request.
///   Common keys include:
///   - `PMIX_CRED_TYPE` — prioritized, comma-separated list of credential
///     types to request (e.g., `"munge,kerberos"`).
///   - `PMIX_TIMEOUT` — maximum time in seconds to wait for the credential.
///
/// # Returns
/// - `Ok(PmixCredential)` containing the opaque credential bytes. The
///   credential is owned by the caller.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_NOT_SUPPORTED` — the host environment does not support
///     credential generation.
///   - `PMIX_ERR_TIMEOUT` — the request timed out.
///   - `PMIX_ERR_BAD_CRED` — the credential could not be generated.
///
/// # C API
/// `pmix_status_t PMIx_Get_credential(const pmix_info_t info[], size_t ninfo,`
/// `  pmix_byte_object_t *credential);`
pub fn get_credential(info: &[Info]) -> Result<PmixCredential, PmixStatus> {
    let ninfo = info.len();

    // Allocate a pmix_byte_object_t on the stack for the output credential.
    // We use Box to get a heap-allocated struct that we can pass to PMIx.
    let cred_box = Box::new(ffi::pmix_byte_object_t {
        bytes: ptr::null_mut(),
        size: 0,
    });
    let cred_ptr = Box::into_raw(cred_box);

    // Collect raw handles from the Info objects.
    let info_handles: Vec<*mut ffi::pmix_info_t> = info.iter().map(|i| i.handle).collect();
    let info_ptr = if ninfo > 0 {
        info_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    let status = unsafe {
        // SAFETY: PMIx_Get_credential is a synchronous PMIx API call.
        // - info_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows passed by the caller (or is null if empty).
        // - cred_ptr is a valid, allocated pmix_byte_object_t that PMIx
        //   will populate with the credential bytes.
        // - PMIx does not retain info_ptr after this call returns.
        // - The caller must keep info alive until this function returns.
        ffi::PMIx_Get_credential(info_ptr, ninfo, cred_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        unsafe {
            // SAFETY: cred_ptr is valid. PMIx has populated it with
            // the credential bytes (allocated by pmix_malloc).
            // We copy the bytes into a Rust Vec and free the C memory.
            let bytes = copy_and_free_pmix_byte_object(cred_ptr);
            Ok(PmixCredential::from_vec(bytes))
        }
    } else {
        // On error, free the allocated struct.
        unsafe {
            // The struct was allocated by Box::into_raw, so we recover it.
            // The bytes field should be null on error, but be safe.
            let cred = Box::from_raw(cred_ptr);
            if !cred.bytes.is_null() {
                ffi::free(cred.bytes as *mut std::ffi::c_void);
            }
            // Box drop frees the struct.
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait and registry for PMIx_Get_credential_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for [`get_credential_nb`].
///
/// Implement this trait to receive the result of a non-blocking credential
/// request. The `on_complete` method receives the `PmixStatus` and, on
/// success, the [`PmixCredential`] returned by the server.
///
/// The callback also receives optional `Info` results (e.g., credential
/// metadata) and their count. The info array is owned by the caller after
/// the callback returns.
pub trait CredentialCallback: Send {
    fn on_complete(
        self: Box<Self>,
        status: PmixStatus,
        credential: Option<PmixCredential>,
        results: CredentialResults,
    );
}

/// Results returned by the credential callback, wrapping the info array
/// and any associated metadata.
#[derive(Default)]
pub struct CredentialResults {
    /// Info entries returned by the server (e.g., credential metadata,
    /// effective userid/groupid).
    info: Vec<Info>,
    /// Number of info entries.
    ninfo: usize,
}

impl CredentialResults {
    /// Get the info entries.
    pub fn info(&self) -> &[Info] {
        &self.info
    }

    /// Number of info entries.
    pub fn len(&self) -> usize {
        self.ninfo
    }

    /// `true` if there are no info entries.
    pub fn is_empty(&self) -> bool {
        self.ninfo == 0
    }
}

/// Global registry mapping request IDs to pending credential callbacks.
type CredentialRegistry = std::collections::HashMap<usize, Box<dyn CredentialCallback>>;
static CREDENTIAL_REGISTRY: LazyLock<Mutex<CredentialRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing credential request ID counter.
static CREDENTIAL_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_credential_cbfunc_t`.
///
/// Called by PMIx when the non-blocking credential request completes.
/// The callback signature is:
/// ```c
/// void cred_cbfunc(pmix_status_t status, pmix_byte_object_t *credential,
///                  pmix_info_t info[], size_t ninfo, void *cbdata);
/// ```
///
/// The `cbdata` parameter encodes the request ID. We look up the registered
/// Rust closure and invoke it with the result.
///
/// Ownership notes:
/// - `credential` is allocated by PMIx — we copy its bytes into a Vec<u8>
///   and free the PMIx-allocated memory.
/// - `info` is allocated by PMIx — we copy the entries into a Vec<Info>
///   and then free the original array via `PMIx_Info_free`.
extern "C" fn credential_callback_bridge(
    status: ffi::pmix_status_t,
    credential: *mut ffi::pmix_byte_object_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        // Free resources if callback data is missing.
        if !credential.is_null() {
            unsafe {
                // Copy bytes and free PMIx memory.
                let _bytes = copy_and_free_pmix_byte_object(credential);
            }
        }
        if !info.is_null() && ninfo > 0 {
            unsafe {
                ffi::PMIx_Info_free(info, ninfo);
            }
        }
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = CREDENTIAL_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — free resources to avoid leak.
            if !credential.is_null() {
                unsafe {
                    let _bytes = copy_and_free_pmix_byte_object(credential);
                }
            }
            if !info.is_null() && ninfo > 0 {
                unsafe {
                    ffi::PMIx_Info_free(info, ninfo);
                }
            }
            return;
        }
    };

    let pmix_status = PmixStatus::from_raw(status);

    // Take ownership of the credential — copy bytes and free PMIx memory.
    let cred = if !credential.is_null() {
        unsafe {
            let bytes = copy_and_free_pmix_byte_object(credential);
            Some(PmixCredential::from_vec(bytes))
        }
    } else {
        None
    };

    // Copy info entries into Rust-owned Info objects, then free the C array.
    let results = if !info.is_null() && ninfo > 0 {
        unsafe {
            // SAFETY: info points to a valid pmix_info_t array of length ninfo
            // allocated by PMIx. We copy each entry's fields into Rust Info
            // objects, then free the original array.
            let mut info_vec: Vec<Info> = Vec::with_capacity(ninfo);
            for i in 0..ninfo {
                let entry = std::ptr::read_unaligned(info.add(i) as *const crate::ffi::pmix_info_t);
                // Create a new Info handle that points to a freshly allocated
                // pmix_info_t with copied fields.
                let new_info = ffi::PMIx_Info_create(1);
                if !new_info.is_null() {
                    std::ptr::copy_nonoverlapping(&entry as *const ffi::pmix_info_t, new_info, 1);
                    info_vec.push(Info {
                        handle: new_info,
                        len: 1,
                    });
                }
            }
            // Free the original C-allocated info array.
            ffi::PMIx_Info_free(info, ninfo);
            let n = info_vec.len();
            CredentialResults {
                info: info_vec,
                ninfo: n,
            }
        }
    } else {
        CredentialResults::default()
    };

    cb.on_complete(pmix_status, cred, results);
}

/// Non-blocking request for a credential from the PMIx server.
///
/// Submit an asynchronous credential request. The `callback` closure is
/// invoked once the operation completes, receiving the status, the
/// credential (if successful), and any additional info results.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # C API
/// `pmix_status_t PMIx_Get_credential_nb(const pmix_info_t info[], size_t ninfo,`
/// `  pmix_credential_cbfunc_t cbfunc, void *cbdata);`
pub fn get_credential_nb(
    info: &[Info],
    callback: Box<dyn CredentialCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = CREDENTIAL_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };
    {
        let mut registry = CREDENTIAL_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    let ninfo = info.len();

    // Collect raw handles from the Info objects.
    let info_handles: Vec<*mut ffi::pmix_info_t> = info.iter().map(|i| i.handle).collect();
    let info_ptr = if ninfo > 0 {
        info_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    let status = unsafe {
        // SAFETY: PMIx_Get_credential_nb is an async PMIx API call.
        // - info_ptr points to a valid pmix_info_t array owned by the
        //   Info borrows (or is null if empty).
        // - cbfunc is a valid extern "C" function pointer.
        // - cbdata encodes the request ID; PMIx passes it back unchanged.
        // - PMIx does not retain info_ptr after this call returns.
        // - The caller must keep info alive until the callback is invoked.
        ffi::PMIx_Get_credential_nb(info_ptr, ninfo, Some(credential_callback_bridge), cbdata)
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request rejected — remove the callback from the registry.
        let mut registry = CREDENTIAL_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_credential — safe wrapper for PMIx_Validate_credential
// ─────────────────────────────────────────────────────────────────────────────

/// Owned result set from [`validate_credential`].
///
/// Wraps the `pmix_info_t*` array returned by `PMIx_Validate_credential`
/// and automatically frees it via `PMIx_Info_free` on drop.
#[derive(Debug)]
pub struct ValidationResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
}

impl ValidationResults {
    /// Create an empty `ValidationResults` with no info entries.
    ///
    /// Useful for testing and as a default value when validation
    /// has not yet been performed.
    pub fn empty() -> Self {
        Self {
            handle: ptr::null_mut(),
            len: 0,
        }
    }

    /// Number of info entries in this result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for ValidationResults {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.len > 0 {
            unsafe {
                // SAFETY: handle was returned by PMIx_Validate_credential
                // as an allocated pmix_info_t array. PMIx_Info_free releases it.
                ffi::PMIx_Info_free(self.handle, self.len);
                self.handle = ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

/// Validate a credential obtained from [`get_credential`].
///
/// This is a blocking call — it returns only after the server has validated
/// the credential and returned results (or an error). The results include
/// metadata about the credential holder (e.g., effective userid, groupid,
/// and any authorizations).
///
/// # Parameters
/// - `credential`: The credential to validate, obtained from [`get_credential`].
/// - `info`: Optional info entries that direct the validation.
///   Common keys include:
///   - `PMIX_TIMEOUT` — maximum time in seconds to wait.
///
/// # Returns
/// - `Ok(ValidationResults)` containing the validation metadata.
/// - `Err(PmixStatus)` on failure:
///   - `PMIX_ERR_INIT` — PMIx has not been initialized.
///   - `PMIX_ERR_INVALID_CRED` — the credential is invalid or expired.
///   - `PMIX_ERR_NOT_SUPPORTED` — credential validation is not supported.
///
/// # C API
/// `pmix_status_t PMIx_Validate_credential(const pmix_byte_object_t *cred,`
/// `  const pmix_info_t info[], size_t ninfo,`
/// `  pmix_info_t **results, size_t *nresults);`
pub fn validate_credential(
    credential: &PmixCredential,
    info: &[Info],
) -> Result<ValidationResults, PmixStatus> {
    let ninfo = info.len();

    // Collect raw handles from the Info objects.
    let info_handles: Vec<*mut ffi::pmix_info_t> = info.iter().map(|i| i.handle).collect();
    let info_ptr = if ninfo > 0 {
        info_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    // Create a C pmix_byte_object_t for the credential.
    let cred_c = credential.as_c_mut_ptr();

    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    let status = unsafe {
        // SAFETY: PMIx_Validate_credential is a synchronous PMIx API call.
        // - cred_c points to a valid pmix_byte_object_t with our credential bytes.
        // - info_ptr points to a valid pmix_info_t array (or is null if empty).
        // - results and nresults are valid mutable references for output.
        // - PMIx does not retain cred_c or info_ptr after this call returns.
        ffi::PMIx_Validate_credential(
            cred_c as *const ffi::pmix_byte_object_t,
            info_ptr,
            ninfo,
            &mut results,
            &mut nresults,
        )
    };

    // Free the C credential struct.
    unsafe {
        PmixCredential::free_c_ptr(cred_c);
    }

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        Ok(ValidationResults {
            handle: results,
            len: nresults,
        })
    } else {
        // On error, free any results that may have been allocated.
        if !results.is_null() && nresults > 0 {
            unsafe {
                ffi::PMIx_Info_free(results, nresults);
            }
        }
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callback trait and registry for PMIx_Validate_credential_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for [`validate_credential_nb`].
///
/// Implement this trait to receive the result of a non-blocking credential
/// validation request. The `on_complete` method receives the `PmixStatus`
/// and, on success, the [`ValidationResults`] containing metadata about
/// the credential holder.
pub trait ValidationCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, results: ValidationResults);
}

/// Global registry mapping request IDs to pending validation callbacks.
type ValidationRegistry = std::collections::HashMap<usize, Box<dyn ValidationCallback>>;
static VALIDATION_REGISTRY: LazyLock<Mutex<ValidationRegistry>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Map from request ID to the C-allocated credential pointer for async validation.
/// We store as usize to avoid Send/Sync issues with raw pointers in a shared HashMap.
type ValidationCredMap = std::collections::HashMap<usize, usize>;
static VALIDATION_CRED_MAP: LazyLock<Mutex<ValidationCredMap>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Monotonically increasing validation request ID counter.
static VALIDATION_SEQ: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

/// C bridge for `pmix_validation_cbfunc_t`.
///
/// Called by PMIx when the non-blocking validation request completes.
/// The callback signature is:
/// ```c
/// void validation_cbfunc(pmix_status_t status, pmix_info_t info[],
///                        size_t ninfo, void *cbdata);
/// ```
///
/// The `cbdata` parameter encodes the request ID. We look up the registered
/// Rust closure and invoke it with the result.
extern "C" fn validation_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        if !info.is_null() && ninfo > 0 {
            unsafe {
                ffi::PMIx_Info_free(info, ninfo);
            }
        }
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    let req_id = (cbdata as usize) >> 2;

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = VALIDATION_REGISTRY.lock().unwrap();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — free resources to avoid leak.
            if !info.is_null() && ninfo > 0 {
                unsafe {
                    ffi::PMIx_Info_free(info, ninfo);
                }
            }
            // Also free the C credential if it was stored.
            let mut cred_map = VALIDATION_CRED_MAP.lock().unwrap();
            if let Some(cred_ptr) = cred_map.remove(&req_id) {
                unsafe {
                    PmixCredential::free_c_ptr(cred_ptr as *mut ffi::pmix_byte_object_t);
                }
            }
            return;
        }
    };

    // Free the C credential struct that was passed to PMIx.
    {
        let mut cred_map = VALIDATION_CRED_MAP.lock().unwrap();
        if let Some(cred_ptr) = cred_map.remove(&req_id) {
            unsafe {
                PmixCredential::free_c_ptr(cred_ptr as *mut ffi::pmix_byte_object_t);
            }
        }
    }

    let pmix_status = PmixStatus::from_raw(status);

    // Build ValidationResults from the info array.
    let results = if !info.is_null() && ninfo > 0 {
        ValidationResults {
            handle: info,
            len: ninfo,
        }
    } else {
        ValidationResults {
            handle: ptr::null_mut(),
            len: 0,
        }
    };

    cb.on_complete(pmix_status, results);
}

/// Non-blocking validation of a credential obtained from [`get_credential`].
///
/// Submit an asynchronous validation request. The `callback` closure is
/// invoked once the operation completes, receiving the status and any
/// validation results.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized). The callback will NOT be called.
///
/// # C API
/// `pmix_status_t PMIx_Validate_credential_nb(const pmix_byte_object_t *cred,`
/// `  const pmix_info_t info[], size_t ninfo,`
/// `  pmix_validation_cbfunc_t cbfunc, void *cbdata);`
pub fn validate_credential_nb(
    credential: &PmixCredential,
    info: &[Info],
    callback: Box<dyn ValidationCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = {
        let mut seq = VALIDATION_SEQ.lock().unwrap();
        *seq += 1;
        *seq
    };

    // Create a C pmix_byte_object_t for the credential.
    // It needs to stay alive until the callback fires, so we store it
    // in a separate registry keyed by req_id.
    let cred_c = credential.as_c_mut_ptr();
    {
        let mut cred_map = VALIDATION_CRED_MAP.lock().unwrap();
        cred_map.insert(req_id, cred_c as usize);
    }
    {
        let mut registry = VALIDATION_REGISTRY.lock().unwrap();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = (req_id << 2) as *mut c_void;

    let ninfo = info.len();

    // Collect raw handles from the Info objects.
    let info_handles: Vec<*mut ffi::pmix_info_t> = info.iter().map(|i| i.handle).collect();
    let info_ptr = if ninfo > 0 {
        info_handles.as_ptr() as *const ffi::pmix_info_t
    } else {
        ptr::null()
    };

    let status = unsafe {
        // SAFETY: PMIx_Validate_credential_nb is an async PMIx API call.
        ffi::PMIx_Validate_credential_nb(
            cred_c as *const ffi::pmix_byte_object_t,
            info_ptr,
            ninfo,
            Some(validation_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // Request rejected — remove the callback and free the C credential.
        let mut registry = VALIDATION_REGISTRY.lock().unwrap();
        registry.remove(&req_id);
        let mut cred_map = VALIDATION_CRED_MAP.lock().unwrap();
        if let Some(cred_ptr) = cred_map.remove(&req_id) {
            unsafe {
                PmixCredential::free_c_ptr(cred_ptr as *mut ffi::pmix_byte_object_t);
            }
        }
        Err(pmix_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_from_bytes() {
        let cred = PmixCredential::from_bytes(b"test credential data");
        assert_eq!(cred.as_bytes(), b"test credential data");
        assert_eq!(cred.len(), 20);
        assert!(!cred.is_empty());
    }

    #[test]
    fn test_credential_from_vec() {
        let data = vec![1u8, 2, 3, 4, 5];
        let cred = PmixCredential::from_vec(data.clone());
        assert_eq!(cred.as_bytes(), &data);
        assert_eq!(cred.len(), 5);
    }

    #[test]
    fn test_credential_empty() {
        let cred = PmixCredential::empty();
        assert!(cred.is_empty());
        assert_eq!(cred.len(), 0);
        assert!(cred.as_bytes().is_empty());
    }

    #[test]
    fn test_credential_as_raw() {
        let cred = PmixCredential::from_bytes(b"test");
        let ptr = cred.as_raw();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_validation_results_empty() {
        let results = ValidationResults::empty();
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }
}
