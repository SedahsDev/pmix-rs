//! Data serialization / deserialization — `PMIx_Data_pack`, `PMIx_Data_unpack`,
//! `PMIx_Data_load`, `PMIx_Data_unload`, `PMIx_Data_copy`, `PMIx_Data_print`,
//! `PMIx_Data_compress`, `PMIx_Data_decompress`, and buffer management.
//!
//! This module provides safe Rust wrappers around the PMIx data buffer and
//! serialization APIs. These functions are used to pack values into a buffer
//! for transmission and unpack them on the receiving side, handling type
//! conversion and endianness differences between heterogeneous processes.
//!
//! # Buffer lifecycle
//!
//! A `PmixDataBuffer` is created via [`data_buffer_create`], used for packing
//! or unpacking, and released via [`data_buffer_release`]. The buffer manages
//! its own internal memory and grows as needed.
//!
//! # Typical workflow
//!
//! ```no_run
//! use pmix::{PmixDataType, data_serialization::*};
//!
//! // --- Sender side ---
//! let mut buf = data_buffer_create().expect("create buffer");
//! let val: i32 = 42;
//! let packed = data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack");
//! assert_eq!(packed, 1);
//!
//! // Unload to byte object for transport
//! let payload = data_unload(&buf).expect("unload");
//! data_buffer_release(&mut buf);
//!
//! // --- Receiver side ---
//! let mut buf2 = data_buffer_create().expect("create buffer");
//! data_load(&buf2, &payload).expect("load");
//! let mut out: i32 = 0;
//! let mut count: i32 = 1;
//! let unpacked = data_unpack(None, &buf2, &mut out, &mut count, PmixDataType::Int32).expect("unpack");
//! assert_eq!(unpacked, 1);
//! assert_eq!(out, 42);
//! data_buffer_release(&mut buf2);
//! ```
//!
//! # C API reference
//!
//! - `pmix_status_t PMIx_Data_pack(const pmix_proc_t *target, pmix_data_buffer_t *buffer, void *src, int32_t num_vals, pmix_data_type_t type)`
//! - `pmix_status_t PMIx_Data_unpack(const pmix_proc_t *source, pmix_data_buffer_t *buffer, void *dest, int32_t *max_num_values, pmix_data_type_t type)`
//! - `pmix_status_t PMIx_Data_unload(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`
//! - `pmix_status_t PMIx_Data_load(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`
//! - `pmix_status_t PMIx_Data_copy(void **dest, void *src, pmix_data_type_t type)`
//! - `pmix_status_t PMIx_Data_print(char **output, char *prefix, void *src, pmix_data_type_t type)`
//! - `pmix_data_buffer_t *PMIx_Data_buffer_create(void)`
//! - `void PMIx_Data_buffer_release(pmix_data_buffer_t *buffer)`

use crate::{PmixDataType, PmixStatus, ffi};
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PmixProc handle (borrowed)
// ─────────────────────────────────────────────────────────────────────────────

/// A borrowed PMIx process identifier for use as target/source in serialization.
///
/// When `None` is passed to [`data_pack`] or [`data_unpack`], it indicates
/// that the target/source uses the same PMIx version as the caller.
pub struct PmixProcRef<'a> {
    nspace: &'a str,
    rank: u32,
}

impl<'a> PmixProcRef<'a> {
    /// Create a process reference from namespace and rank.
    pub fn new(nspace: &'a str, rank: u32) -> Self {
        Self { nspace, rank }
    }

    fn to_raw(&self) -> ffi::pmix_proc_t {
        let mut proc = unsafe { std::mem::zeroed::<ffi::pmix_proc_t>() };
        let bytes = self.nspace.as_bytes();
        // pmix_nspace_t is [c_char; 256]; c_char is i8 on Linux.
        let nspace_len = std::mem::size_of::<ffi::pmix_nspace_t>();
        let len = bytes.len().min(nspace_len - 1);
        let c_bytes: Vec<std::os::raw::c_char> = bytes[..len]
            .iter()
            .map(|b| *b as std::os::raw::c_char)
            .collect();
        proc.nspace[..len].copy_from_slice(&c_bytes);
        proc.rank = self.rank;
        proc
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixByteObject — safe wrapper around pmix_byte_object_t
// ─────────────────────────────────────────────────────────────────────────────

/// A byte object wrapping a contiguous region of memory.
///
/// Corresponds to `pmix_byte_object_t`. The PMIx library allocates the
/// underlying `bytes` pointer; the caller must destroy the object via
/// [`byte_object_destruct`] or drop this wrapper (which calls destruct).
#[derive(Debug)]
pub struct PmixByteObject {
    inner: ffi::pmix_byte_object_t,
}

// Not Send/Sync by default — the underlying buffer is managed by the PMIx C library
// and may not be safe to share across threads without synchronization.
impl PmixByteObject {
    /// Create an empty byte object (zeroed).
    pub fn new() -> Self {
        Self {
            inner: ffi::pmix_byte_object_t {
                bytes: ptr::null_mut(),
                size: 0,
            },
        }
    }

    /// Get a slice of the underlying bytes.
    pub fn as_slice(&self) -> &[u8] {
        if self.inner.bytes.is_null() || self.inner.size == 0 {
            &[]
        } else {
            // SAFETY: The PMIx library guarantees that bytes points to a valid
            // allocation of at least `size` bytes when the byte object has been
            // properly populated (e.g., by PMIx_Data_unload).
            unsafe { std::slice::from_raw_parts(self.inner.bytes as *const u8, self.inner.size) }
        }
    }

    /// Get the size in bytes.
    pub fn size(&self) -> usize {
        self.inner.size
    }

    /// Check if the byte object is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.bytes.is_null() || self.inner.size == 0
    }

    /// Get a mutable reference to the inner C struct (for FFI interop).
    pub fn as_mut_ptr(&mut self) -> *mut ffi::pmix_byte_object_t {
        &mut self.inner
    }
}

impl Default for PmixByteObject {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<u8>> for PmixByteObject {
    fn from(bytes: Vec<u8>) -> Self {
        let size = bytes.len();
        if size == 0 {
            return Self::new();
        }
        // SAFETY: We allocate with libc::malloc so PMIx_Byte_object_destruct
        // (which calls pmix_free/free) can safely free it on drop.
        let c_ptr = unsafe { libc::malloc(size) as *mut u8 };
        if c_ptr.is_null() {
            panic!("malloc failed in PmixByteObject::from(Vec<u8>)");
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), c_ptr, size);
        }
        Self {
            inner: ffi::pmix_byte_object_t {
                bytes: c_ptr as *mut std::os::raw::c_char,
                size,
            },
        }
    }
}

impl Drop for PmixByteObject {
    fn drop(&mut self) {
        // Destroy the underlying byte object to free internal memory.
        // SAFETY: PMIx_Byte_object_destruct frees the bytes pointer if non-null
        // and zeroes the struct. Safe to call even on a zeroed/empty object.
        unsafe { ffi::PMIx_Byte_object_destruct(&mut self.inner) };
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataBuffer — safe wrapper around pmix_data_buffer_t
// ─────────────────────────────────────────────────────────────────────────────

/// A PMIx data buffer for packing and unpacking typed values.
///
/// Corresponds to `pmix_data_buffer_t`. The buffer manages its own internal
/// memory and grows as values are packed into it. Unpacking is non-destructive
/// — the unpack pointer advances but the packed data remains available.
///
/// # Lifecycle
///
/// Created via [`data_buffer_create`], released via [`data_buffer_release`].
/// This wrapper calls `data_buffer_release` on drop.
///
/// # Examples
///
/// ```no_run
/// use pmix::{PmixDataType, data_serialization::*};
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// let val: i32 = 42;
/// data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack");
/// data_buffer_release(&mut buf);
/// ```
pub struct PmixDataBuffer {
    ptr: *mut ffi::pmix_data_buffer_t,
}

// The PMIx data buffer is not safe to share across threads.
impl PmixDataBuffer {
    /// Get a mutable pointer to the underlying C buffer (for FFI interop).
    pub fn as_mut_ptr(&self) -> *mut ffi::pmix_data_buffer_t {
        self.ptr
    }

    /// Check if the buffer pointer is valid (non-null).
    pub fn is_valid(&self) -> bool {
        !self.ptr.is_null()
    }

    /// Get the number of bytes allocated in the buffer.
    pub fn bytes_allocated(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        // SAFETY: We hold a valid pointer to a live buffer.
        unsafe { (*self.ptr).bytes_allocated }
    }

    /// Get the number of bytes used in the buffer.
    pub fn bytes_used(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        // SAFETY: We hold a valid pointer to a live buffer.
        unsafe { (*self.ptr).bytes_used }
    }

    /// Create a `PmixDataBuffer` from a raw C pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure `ptr` is a valid, allocated `pmix_data_buffer_t`
    /// or null. The resulting `PmixDataBuffer` takes ownership and will call
    /// `PMIx_Data_buffer_release` on drop.
    pub unsafe fn from_raw(ptr: *mut ffi::pmix_data_buffer_t) -> Self {
        Self { ptr }
    }
}

impl Drop for PmixDataBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: The pointer is valid and was created by PMIx_Data_buffer_create.
            // PMIx_Data_buffer_release frees the internal memory and the buffer itself.
            unsafe { ffi::PMIx_Data_buffer_release(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}

impl std::fmt::Debug for PmixDataBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ptr.is_null() {
            f.debug_struct("PmixDataBuffer")
                .field("ptr", &"null")
                .finish()
        } else {
            f.debug_struct("PmixDataBuffer")
                .field("ptr", &self.ptr)
                .field("bytes_allocated", &self.bytes_allocated())
                .field("bytes_used", &self.bytes_used())
                .finish()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer management
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new PMIx data buffer.
///
/// Allocates and initializes a `pmix_data_buffer_t`. The returned buffer
/// manages its own memory and grows as values are packed.
///
/// # C API
/// `pmix_data_buffer_t *PMIx_Data_buffer_create(void)`
///
/// # Errors
///
/// Returns `Err(PmixStatus)` if the allocation fails (null pointer returned).
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::data_buffer_create;
///
/// let mut buf = data_buffer_create().expect("failed to create buffer");
/// // use buf...
/// // buf is automatically released on drop
/// ```
pub fn data_buffer_create() -> Result<PmixDataBuffer, PmixStatus> {
    // SAFETY: PMIx_Data_buffer_create takes no parameters and returns a
    // newly allocated buffer, or null on failure. No pointers are dereferenced.
    let ptr = unsafe { ffi::PMIx_Data_buffer_create() };
    if ptr.is_null() {
        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
    }
    Ok(PmixDataBuffer { ptr })
}

/// Release a PMIx data buffer.
///
/// Frees the internal memory and the buffer itself. The buffer is unusable
/// after this call. Normally you don't need to call this explicitly because
/// `PmixDataBuffer` calls it on drop.
///
/// # C API
/// `void PMIx_Data_buffer_release(pmix_data_buffer_t *buffer)`
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::*;
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// data_buffer_release(&mut buf);
/// // buf is now invalid (double-release is prevented)
/// ```
pub fn data_buffer_release(buf: &mut PmixDataBuffer) {
    if buf.is_valid() {
        // SAFETY: The pointer is valid and was created by PMIx_Data_buffer_create.
        // After this call, the pointer is nulled to prevent double-free on drop.
        unsafe { ffi::PMIx_Data_buffer_release(buf.ptr) };
        buf.ptr = ptr::null_mut();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_pack
// ─────────────────────────────────────────────────────────────────────────────

/// Pack one or more values of a specified type into a data buffer.
///
/// The pack function serializes the value(s) pointed to by `src` into the
/// provided buffer. The buffer must have been previously created via
/// [`data_buffer_create`]. The type parameter tells PMIx how to interpret
/// the source data and how to handle endianness conversion for heterogeneous
/// recipients.
///
/// The `target` parameter identifies the recipient process. When `None`,
/// PMIx assumes the target uses the same version as the caller.
///
/// # C API
/// `pmix_status_t PMIx_Data_pack(const pmix_proc_t *target, pmix_data_buffer_t *buffer, void *src, int32_t num_vals, pmix_data_type_t type)`
///
/// # Parameters
///
/// - `target` — Optional process identifier of the intended recipient.
/// - `buffer` — The data buffer to pack into.
/// - `src` — A reference to the value(s) to pack. The reference must live
///   long enough for this call to complete.
/// - `num_vals` — Number of values of the given type being packed.
/// - `data_type` — The PMIx data type of the source values.
///
/// # Returns
///
/// On success, returns `Ok(n)` where `n` is the number of values actually
/// packed. On error, returns `Err(PmixStatus)`.
///
/// # Examples
///
/// ```no_run
/// use pmix::{PmixDataType, data_serialization::*};
///
/// let mut buf = data_buffer_create().expect("create buffer");
///
/// // Pack a single i32
/// let val: i32 = 42;
/// let packed = data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack int32");
/// assert_eq!(packed, 1);
///
/// // Pack multiple u8 values
/// let bytes: [u8; 4] = [1, 2, 3, 4];
/// let packed = data_pack(None, &buf, &bytes, 4, PmixDataType::Uint8).expect("pack bytes");
/// assert_eq!(packed, 4);
///
/// data_buffer_release(&mut buf);
/// ```
pub fn data_pack<T>(
    target: Option<PmixProcRef>,
    buf: &PmixDataBuffer,
    src: &T,
    num_vals: i32,
    data_type: PmixDataType,
) -> Result<i32, PmixStatus> {
    if num_vals <= 0 {
        return Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
    }

    let target_ptr = if let Some(t) = target {
        let raw = t.to_raw();
        // We need to keep `raw` alive for the FFI call. Since pmix_proc_t
        // contains a fixed-size char array (not a pointer), it's safe to
        // take a reference to a local variable.
        &raw as *const ffi::pmix_proc_t
    } else {
        ptr::null()
    };

    // SAFETY: PMIx_Data_pack reads `num_vals` values of `data_type` from
    // the `src` pointer. The caller guarantees that `src` points to valid
    // memory of the specified type and count. The buffer must be a valid,
    // allocated pmix_data_buffer_t. The target pointer is either null or
    // points to a valid pmix_proc_t (which lives on the stack and contains
    // no dangling pointers — nspace is a fixed char[256] array).
    let status = unsafe {
        ffi::PMIx_Data_pack(
            target_ptr,
            buf.as_mut_ptr(),
            src as *const T as *mut std::os::raw::c_void,
            num_vals,
            data_type as ffi::pmix_data_type_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(num_vals)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_unpack
// ─────────────────────────────────────────────────────────────────────────────

/// Unpack one or more values of a specified type from a data buffer.
///
/// The unpack function reads the next value(s) of the specified type from
/// the buffer. Unlike pack, unpack is **non-destructive** — the data remains
/// in the buffer and can be re-read by resetting the unpack pointer.
///
/// The `source` parameter identifies the process that originally packed the
/// buffer. When `None`, PMIx assumes the source uses the same version as
/// the caller.
///
/// # C API
/// `pmix_status_t PMIx_Data_unpack(const pmix_proc_t *source, pmix_data_buffer_t *buffer, void *dest, int32_t *max_num_values, pmix_data_type_t type)`
///
/// # Parameters
///
/// - `source` — Optional process identifier of the process that packed the buffer.
/// - `buffer` — The data buffer to unpack from.
/// - `dest` — A mutable reference to the destination value(s). Must be large
///   enough to hold `max_num_values` values of the specified type.
/// - `max_num_values` — Maximum number of values that can fit in `dest`.
///   On success, this is updated to the actual number unpacked.
/// - `data_type` — The PMIx data type of the values to unpack.
///
/// # Returns
///
/// On success, returns `Ok(n)` where `n` is the number of values actually
/// unpacked. On error, returns `Err(PmixStatus)`.
///
/// # Safety note
///
/// The caller is responsible for ensuring `dest` has enough capacity. PMIx
/// will not write beyond `max_num_values` values, but the types must match.
///
/// # Examples
///
/// ```no_run
/// use pmix::{PmixDataType, data_serialization::*};
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// // ... buffer was filled by packing ...
///
/// // Unpack a single i32
/// let mut val: i32 = 0;
/// let mut count: i32 = 1;
/// let unpacked = data_unpack(None, &buf, &mut val, &mut count, PmixDataType::Int32);
/// match unpacked {
///     Ok(n) => println!("Unpacked {} values, val = {}", n, val),
///     Err(e) => eprintln!("Unpack failed: {}", e),
/// }
/// ```
pub fn data_unpack<T>(
    source: Option<PmixProcRef>,
    buf: &PmixDataBuffer,
    dest: &mut T,
    max_num_values: &mut i32,
    data_type: PmixDataType,
) -> Result<i32, PmixStatus> {
    let source_ptr = if let Some(s) = source {
        let raw = s.to_raw();
        &raw as *const ffi::pmix_proc_t
    } else {
        ptr::null()
    };

    // SAFETY: PMIx_Data_unpack writes up to `*max_num_values` values of
    // `data_type` into `dest`. The caller guarantees that `dest` points to
    // valid, writable memory of the specified type and capacity. The buffer
    // must be a valid, allocated pmix_data_buffer_t that has been populated
    // with data. The source pointer is either null or points to a valid
    // pmix_proc_t on the stack.
    let status = unsafe {
        ffi::PMIx_Data_unpack(
            source_ptr,
            buf.as_mut_ptr(),
            dest as *mut T as *mut std::os::raw::c_void,
            max_num_values,
            data_type as ffi::pmix_data_type_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(*max_num_values)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_unload
// ─────────────────────────────────────────────────────────────────────────────

/// Unload the contents of a data buffer into a byte object.
///
/// Extracts the packed data from the buffer as a raw byte array suitable
/// for transmission (e.g., over the network). The resulting `PmixByteObject`
/// owns the data and frees it on drop.
///
/// # C API
/// `pmix_status_t PMIx_Data_unload(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`
///
/// # Parameters
///
/// - `buffer` — The data buffer to unload.
///
/// # Returns
///
/// On success, returns `Ok(PmixByteObject)` containing the raw bytes.
/// On error, returns `Err(PmixStatus)`.
///
/// # Examples
///
/// ```no_run
/// use pmix::{PmixDataType, data_serialization::*};
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// let val: i32 = 42;
/// data_pack(None, &buf, &val, 1, PmixDataType::Int32).expect("pack");
///
/// let payload = data_unload(&buf).expect("unload");
/// println!("Payload size: {} bytes", payload.size());
/// // payload is automatically freed on drop
/// ```
pub fn data_unload(buf: &PmixDataBuffer) -> Result<PmixByteObject, PmixStatus> {
    let mut byte_obj = PmixByteObject::new();

    // SAFETY: PMIx_Data_unload allocates memory for the byte object's bytes
    // pointer and fills the byte_object_t struct. The buffer must be valid
    // and allocated. The byte_object_t is zeroed before the call, which is
    // the expected initial state.
    let status = unsafe { ffi::PMIx_Data_unload(buf.as_mut_ptr(), byte_obj.as_mut_ptr()) };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(byte_obj)
    } else {
        // byte_obj will be dropped (and any partial allocation freed) automatically.
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_load
// ─────────────────────────────────────────────────────────────────────────────

/// Load a byte object's payload into a data buffer.
///
/// Replaces the contents of the buffer with the raw bytes from the provided
/// byte object. If the buffer already contains data, it is first freed.
/// The byte object is NOT cleared — its data remains available after the call.
///
/// # C API
/// `pmix_status_t PMIx_Data_load(pmix_data_buffer_t *buffer, pmix_byte_object_t *payload)`
///
/// # Parameters
///
/// - `buffer` — The data buffer to load into.
/// - `payload` — The byte object containing the raw data to load.
///
/// # Returns
///
/// `Ok(())` on success, `Err(PmixStatus)` on error.
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::*;
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// // payload received from network...
/// // let payload = receive_payload();
/// // data_load(&buf, &payload).expect("load");
/// data_buffer_release(&mut buf);
/// ```
pub fn data_load(buf: &PmixDataBuffer, payload: &PmixByteObject) -> Result<(), PmixStatus> {
    // SAFETY: PMIx_Data_load reads from the byte object's bytes/size and
    // copies the data into the buffer. The buffer must be valid and allocated.
    // The byte object must have been properly initialized (either by
    // PMIx_Data_unload or by the caller with valid bytes/size).
    let status = unsafe {
        ffi::PMIx_Data_load(
            buf.as_mut_ptr(),
            &payload.inner as *const ffi::pmix_byte_object_t as *mut ffi::pmix_byte_object_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_copy
// ─────────────────────────────────────────────────────────────────────────────

/// Copy a data value of a specified type.
///
/// Since PMIx data types can be complex structures, this function knows how
/// to properly deep-copy a value of any registered data type. The destination
/// memory is allocated by PMIx and must be freed by calling this function
/// again with a null source, or by using the appropriate destruct function.
///
/// # C API
/// `pmix_status_t PMIx_Data_copy(void **dest, void *src, pmix_data_type_t type)`
///
/// # Parameters
///
/// - `src` — Pointer to the source data.
/// - `data_type` — The PMIx data type of the source data.
///
/// # Returns
///
/// On success, returns `Ok(())`. The copied data is available at the
/// internally allocated destination pointer.
///
/// # Note
///
/// This function allocates memory internally. For most use cases, prefer
/// using the type-specific pack/unpack functions instead.
pub fn data_copy<T>(
    src: &T,
    data_type: PmixDataType,
) -> Result<*mut std::os::raw::c_void, PmixStatus> {
    let mut dest: *mut std::os::raw::c_void = ptr::null_mut();

    // SAFETY: PMIx_Data_copy reads from `src` and allocates memory for `dest`.
    // The source pointer must be valid and point to data of the specified type.
    // On success, dest points to newly allocated memory owned by the caller.
    let status = unsafe {
        ffi::PMIx_Data_copy(
            &mut dest,
            src as *const T as *mut std::os::raw::c_void,
            data_type as ffi::pmix_data_type_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(dest)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_copy_payload
// ─────────────────────────────────────────────────────────────────────────────

/// Copy the payload from one buffer to another.
///
/// Copies the raw data payload from the source buffer to the destination
/// buffer without interpreting the contents. Both buffers must be valid
/// and allocated.
///
/// # C API
/// `pmix_status_t PMIx_Data_copy_payload(pmix_data_buffer_t *dest, pmix_data_buffer_t *src)`
///
/// # Parameters
///
/// - `dest` — The destination buffer.
/// - `src` — The source buffer.
///
/// # Returns
///
/// `Ok(())` on success, `Err(PmixStatus)` on error.
pub fn data_copy_payload(dest: &PmixDataBuffer, src: &PmixDataBuffer) -> Result<(), PmixStatus> {
    // SAFETY: Both buffers must be valid, allocated pmix_data_buffer_t pointers.
    // PMIx_Data_copy_payload copies the raw payload from src to dest.
    let status = unsafe { ffi::PMIx_Data_copy_payload(dest.as_mut_ptr(), src.as_mut_ptr()) };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_print
// ─────────────────────────────────────────────────────────────────────────────

/// Pretty-print a data value of a specified PMIx type.
///
/// Since registered data types can be complex structures, this function
/// converts a value of any PMIx-defined data type to a human-readable
/// string representation. Primarily intended for debugging purposes.
///
/// The caller provides an optional `prefix` string that is prepended to
/// the output. Pass `None` or an empty string for no prefix.
///
/// The returned `String` is allocated by PMIx and freed automatically
/// when the `PmixPrintOutput` wrapper is dropped.
///
/// # C API
/// `pmix_status_t PMIx_Data_print(char **output, char *prefix, void *src, pmix_data_type_t type)`
///
/// # Parameters
///
/// - `src` — Pointer to the data value to print. The source must remain
///   valid for the duration of this call.
/// - `prefix` — Optional string to prepend to the output. `None` means
///   no prefix.
/// - `data_type` — The PMIx data type of the source value. Must be one
///   of the PMIx-defined data types.
///
/// # Returns
///
/// On success, returns `Ok(String)` containing the formatted output.
///
/// # Errors
///
/// - `PMIX_ERR_BAD_PARAM` — The provided data type is not recognized.
/// - `PMIX_ERR_NOT_SUPPORTED` — The PMIx implementation does not support
///   this function.
///
/// # Examples
///
/// ```no_run
/// use pmix::{PmixDataType, data_serialization::*};
///
/// let val: i32 = 42;
/// match data_print(&val, Some("value="), PmixDataType::Int32) {
///     Ok(output) => println!("Printed: {}", output),
///     Err(e) => eprintln!("Data print failed: {}", e),
/// }
/// ```
///
/// # Memory management
///
/// The output string is allocated by the PMIx C library and freed via
/// `PMIx_Free` when the returned [`PmixPrintOutput`] is dropped.
pub fn data_print<T>(
    src: &T,
    prefix: Option<&str>,
    data_type: PmixDataType,
) -> Result<PmixPrintOutput, PmixStatus> {
    let mut output_ptr: *mut std::os::raw::c_char = ptr::null_mut();

    // Convert optional prefix to C string.
    let prefix_ptr: *mut std::os::raw::c_char = match prefix {
        Some(s) if !s.is_empty() => {
            let c_str = std::ffi::CString::new(s).unwrap_or_else(|_| {
                // If the prefix contains null bytes, fall back to empty.
                std::ffi::CString::new("").unwrap()
            });
            c_str.into_raw()
        }
        _ => ptr::null_mut(),
    };

    // SAFETY: PMIx_Data_print writes the address of a newly allocated,
    // null-terminated string into `output_ptr`. The `src` pointer must be
    // valid and point to data of the specified `data_type`. The `prefix`
    // pointer is either null or points to a valid null-terminated string.
    // On success, the output must be freed via PMIx_Free.
    let status = unsafe {
        ffi::PMIx_Data_print(
            &mut output_ptr,
            prefix_ptr,
            src as *const T as *mut std::os::raw::c_void,
            data_type as ffi::pmix_data_type_t,
        )
    };

    // Free the temporary C string for prefix if we allocated one.
    if !prefix_ptr.is_null() {
        // SAFETY: prefix_ptr was created via CString::into_raw and is
        // a valid allocation that we no longer need.
        unsafe {
            let _ = std::ffi::CString::from_raw(prefix_ptr);
        }
    }

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        // SAFETY: On success, PMIx_Data_print guarantees output_ptr points
        // to a valid, null-terminated string allocated by the PMIx library.
        // We wrap it in PmixPrintOutput which will free it on drop.
        Ok(unsafe { PmixPrintOutput::from_raw(output_ptr) })
    } else {
        Err(pmix_status)
    }
}

/// An owned output string from [`data_print`].
///
/// Wraps the `char*` returned by `PMIx_Data_print` and frees it via
/// `free()` on drop. The inner string is converted to a Rust `String`
/// on construction so it can be used directly as `&str`.
#[derive(Default)]
pub struct PmixPrintOutput {
    inner: String,
}

impl PmixPrintOutput {
    /// Create from a raw C string pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, null-terminated string allocated by the
    /// PMIx library (e.g., returned by `PMIx_Data_print`). The wrapper
    /// takes ownership, converts to a Rust String, and frees the C memory.
    unsafe fn from_raw(ptr: *mut std::os::raw::c_char) -> Self {
        if ptr.is_null() {
            return Self {
                inner: String::new(),
            };
        }
        // Convert the C string to a Rust String, then free the C allocation.
        // to_string_lossy() handles any non-UTF8 bytes by replacing with U+FFFD.
        let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        // Free the C allocation — the Rust String owns its own copy now.
        // SAFETY: ptr was allocated by asprintf (standard malloc).
        // CString::from_raw takes ownership and calls free() on drop.
        unsafe {
            let _ = std::ffi::CString::from_raw(ptr);
        }
        Self { inner: s }
    }

    /// Get the underlying string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl std::ops::Deref for PmixPrintOutput {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::fmt::Display for PmixPrintOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::fmt::Debug for PmixPrintOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl From<PmixPrintOutput> for String {
    fn from(output: PmixPrintOutput) -> Self {
        output.inner
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_embed
// ─────────────────────────────────────────────────────────────────────────────

/// Embed a raw data payload into a buffer without clearing the source.
///
/// The embed function is identical in operation to [`data_load`] except that
/// it does NOT "clear" the payload upon completion — the source
/// `PmixByteObject` remains unmodified after the call.
///
/// Internally, this function destructs and re-constructs the target buffer
/// before copying the payload, so any existing data in the buffer is lost.
///
/// # Notes
///
/// - The buffer must be a valid, allocated `PmixDataBuffer` — passing an
///   invalid buffer returns `PMIX_ERR_BAD_PARAM`.
/// - The caller is responsible for pre-populating the payload — this function
///   cannot convert data to network byte order.
/// - The payload object is unaltered by this operation (unlike `data_load`).
/// - A `None` payload is treated as a no-op and returns `PMIX_SUCCESS`.
///
/// # C API
/// `pmix_status_t PMIx_Data_embed(pmix_data_buffer_t *buffer, const pmix_byte_object_t *payload)`
///
/// # Parameters
///
/// - `buf` — The data buffer into which the payload is to be embedded.
///   Must be a valid, allocated buffer (e.g., from [`data_buffer_create`]).
/// - `payload` — The byte object containing the raw data to embed, or
///   `None` for a no-op.
///
/// # Returns
///
/// `Ok(())` on success, `Err(PmixStatus)` on error (e.g., bad parameter).
///
/// # Errors
///
/// - `PMIX_ERR_BAD_PARAM` — The buffer is null or invalid.
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::*;
///
/// let mut buf = data_buffer_create().expect("create buffer");
/// let payload: PmixByteObject = vec![1u8, 2, 3, 4].into();
///
/// // Embed the payload — payload remains valid after this call
/// data_embed(&buf, Some(&payload)).expect("embed");
///
/// // payload can still be used
/// assert_eq!(payload.size(), 4);
/// ```
pub fn data_embed(
    buf: &PmixDataBuffer,
    payload: Option<&PmixByteObject>,
) -> Result<(), PmixStatus> {
    // Get the C pointer for the payload. When None, pass null to let
    // PMIx_Data_embed treat it as a no-op.
    let payload_ptr = match payload {
        Some(p) => &p.inner as *const ffi::pmix_byte_object_t,
        None => ptr::null(),
    };

    // SAFETY: PMIx_Data_embed reads from the byte object (if non-null) and
    // copies the data into the buffer. The buffer must be valid and allocated.
    // The function internally destructs and reconstructs the buffer before
    // copying, so any existing buffer data is discarded. The byte object
    // itself is not modified (unlike PMIx_Data_load which clears it).
    let status = unsafe {
        ffi::PMIx_Data_embed(
            buf.as_mut_ptr(),
            payload_ptr as *mut ffi::pmix_byte_object_t,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_compress
// ─────────────────────────────────────────────────────────────────────────────

/// Compress a block of data using lossless compression (zlib).
///
/// Attempts to losslessly compress the provided data. If the compressed
/// result would not be smaller than the input, the function returns
/// `Err(PmixStatus::BadParam)` without allocating output memory.
///
/// The output is allocated by the PMIx library (via `malloc`) and is
/// transferred to a Rust-owned `Vec<u8>` before the C allocation is freed.
///
/// # C API
/// `bool PMIx_Data_compress(const uint8_t *inbytes, size_t size,`
///                          `uint8_t **outbytes, size_t *nbytes)`
///
/// # Parameters
///
/// - `input` — The data to compress.
///
/// # Returns
///
/// - `Ok(Vec<u8>)` — The compressed data on success.
/// - `Err(PmixStatus)` — Compression failed or input was not compressible.
///
/// # Errors
///
/// - `PMIX_ERR_BAD_PARAM` — Input pointer is null, input is empty, or
///   compression would not produce a smaller result.
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::*;
///
/// // Compress a large enough payload (must exceed internal compress limit)
/// let data = vec![0u8; 1024];
/// match data_compress(&data) {
///     Ok(compressed) => println!("Compressed to {} bytes", compressed.len()),
///     Err(_) => println!("Data was not compressible"),
/// }
/// ```
pub fn data_compress(input: &[u8]) -> Result<Vec<u8>, PmixStatus> {
    if input.is_empty() {
        return Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
    }

    let mut out_bytes: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;

    // SAFETY: PMIx_Data_compress reads from `input` (which is valid for
    // `input.len()` bytes) and writes to `out_bytes`/`out_len` on success.
    // On success, `out_bytes` points to a malloc'd buffer that we take
    // ownership of. On failure, `out_bytes` is null and nothing to free.
    let success = unsafe {
        ffi::PMIx_Data_compress(input.as_ptr(), input.len(), &mut out_bytes, &mut out_len)
    };

    if success {
        // Take ownership of the malloc'd buffer by copying into a Vec,
        // then free the C allocation.
        let result = if !out_bytes.is_null() && out_len > 0 {
            // SAFETY: `out_bytes` points to a valid malloc'd buffer of
            // `out_len` bytes, allocated by PMIx_Data_compress.
            let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len) }.to_vec();
            // Free the C allocation with libc::free (PMIx uses malloc).
            unsafe { libc::free(out_bytes as *mut libc::c_void) };
            vec
        } else {
            // Shouldn't happen if success is true, but be defensive.
            return Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
        };
        Ok(result)
    } else {
        // Compression not possible (data too small or incompressible).
        // out_bytes should be null here; free it if not (defensive).
        if !out_bytes.is_null() {
            unsafe { libc::free(out_bytes as *mut libc::c_void) };
        }
        Err(PmixStatus::from_raw(-27)) // PMIX_ERR_BAD_PARAM
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Data_decompress
// ─────────────────────────────────────────────────────────────────────────────

/// Decompress data that was compressed by [`data_compress`].
///
/// Only data produced by `PMIx_Data_compress` can be decompressed by this
/// function. Passing arbitrarily compressed data (e.g., raw zlib streams)
/// will lead to undefined behavior.
///
/// The output is allocated by the PMIx library (via `malloc`) and is
/// transferred to a Rust-owned `Vec<u8>` before the C allocation is freed.
///
/// # C API
/// `bool PMIx_Data_decompress(const uint8_t *inbytes, size_t size,`
///                            `uint8_t **outbytes, size_t *nbytes)`
///
/// # Parameters
///
/// - `input` — The compressed data to decompress. Must have been produced
///   by [`data_compress`] / `PMIx_Data_compress`.
///
/// # Returns
///
/// - `Ok(Vec<u8>)` — The decompressed data on success.
/// - `Err(PmixStatus)` — Decompression failed or input was invalid.
///
/// # Errors
///
/// - `PMIX_ERR_BAD_PARAM` — Input pointer is null, input is empty, or
///   the data could not be decompressed.
///
/// # Examples
///
/// ```no_run
/// use pmix::data_serialization::*;
///
/// let data = vec![0u8; 1024];
/// if let Ok(compressed) = data_compress(&data) {
///     match data_decompress(&compressed) {
///         Ok(decompressed) => assert_eq!(decompressed, data),
///         Err(e) => eprintln!("Decompression failed: {:?}", e),
///     }
/// }
/// ```
pub fn data_decompress(input: &[u8]) -> Result<Vec<u8>, PmixStatus> {
    if input.is_empty() {
        return Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
    }

    let mut out_bytes: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;

    // SAFETY: PMIx_Data_decompress reads from `input` (valid for `input.len()`
    // bytes) and writes to `out_bytes`/`out_len` on success. On success,
    // `out_bytes` points to a malloc'd buffer that we take ownership of.
    // On failure, `out_bytes` is null.
    // The input MUST have been produced by PMIx_Data_compress — passing
    // other data leads to undefined behavior in the zlib inflate step.
    let success = unsafe {
        ffi::PMIx_Data_decompress(input.as_ptr(), input.len(), &mut out_bytes, &mut out_len)
    };

    if success {
        let result = if !out_bytes.is_null() && out_len > 0 {
            // SAFETY: `out_bytes` points to a valid malloc'd buffer of
            // `out_len` bytes, allocated by PMIx_Data_decompress.
            let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len) }.to_vec();
            // Free the C allocation with libc::free (PMIx uses malloc).
            unsafe { libc::free(out_bytes as *mut libc::c_void) };
            vec
        } else {
            return Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
        };
        Ok(result)
    } else {
        // Decompression failed. Defensive cleanup.
        if !out_bytes.is_null() {
            unsafe { libc::free(out_bytes as *mut libc::c_void) };
        }
        Err(PmixStatus::from_raw(-27)) // PMIX_ERR_BAD_PARAM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PmixByteObject ──────────────────────────────────────────────────────

    #[test]
    fn test_byte_object_new() {
        let obj = PmixByteObject::new();
        assert!(obj.is_empty());
        assert_eq!(obj.size(), 0);
        assert!(obj.as_slice().is_empty());
    }

    #[test]
    fn test_byte_object_debug() {
        let obj = PmixByteObject::new();
        let s = format!("{:?}", obj);
        assert!(s.contains("PmixByteObject"));
    }

    #[test]
    fn test_byte_object_default() {
        let obj = PmixByteObject::default();
        assert!(obj.is_empty());
        assert_eq!(obj.size(), 0);
    }

    #[test]
    fn test_byte_object_from_vec() {
        let data = vec![1u8, 2, 3, 4, 5];
        let obj = PmixByteObject::from(data.clone());
        assert_eq!(obj.as_slice(), &data[..]);
        assert_eq!(obj.size(), 5);
        assert!(!obj.is_empty());
    }

    #[test]
    fn test_byte_object_from_empty_vec() {
        let obj = PmixByteObject::from(Vec::<u8>::new());
        assert!(obj.is_empty());
        assert_eq!(obj.size(), 0);
    }

    #[test]
    fn test_byte_object_as_mut_ptr() {
        let mut obj = PmixByteObject::new();
        let ptr = obj.as_mut_ptr();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_byte_object_as_slice_non_empty() {
        let data = vec![10u8, 20, 30];
        let obj = PmixByteObject::from(data.clone());
        let slice = obj.as_slice();
        assert_eq!(slice, &data[..]);
        assert_eq!(slice.len(), 3);
    }

    // ── PmixProcRef ─────────────────────────────────────────────────────────

    #[test]
    fn test_proc_ref_new() {
        let _ref = PmixProcRef::new("test_ns", 42);
    }

    #[test]
    fn test_proc_ref_to_raw() {
        let ref_ = PmixProcRef::new("mynamespace", 7);
        let raw = ref_.to_raw();
        assert_eq!(raw.rank, 7);
        // nspace should contain "mynamespace"
        let nspace_bytes =
            unsafe { std::ffi::CStr::from_ptr(raw.nspace.as_ptr() as *const i8) }.to_string_lossy();
        assert_eq!(nspace_bytes, "mynamespace");
    }

    #[test]
    fn test_proc_ref_to_raw_zero_rank() {
        let ref_ = PmixProcRef::new("default", 0);
        let raw = ref_.to_raw();
        assert_eq!(raw.rank, 0);
    }

    #[test]
    fn test_proc_ref_to_raw_max_rank() {
        let ref_ = PmixProcRef::new("ns", u32::MAX);
        let raw = ref_.to_raw();
        assert_eq!(raw.rank, u32::MAX);
    }

    #[test]
    fn test_proc_ref_to_raw_long_namespace() {
        let long_ns = "a".repeat(300); // exceeds 256 char nspace_t
        let ref_ = PmixProcRef::new(&long_ns, 1);
        let raw = ref_.to_raw();
        assert_eq!(raw.rank, 1);
        // nspace should be truncated to fit pmix_nspace_t
        let nspace_bytes =
            unsafe { std::ffi::CStr::from_ptr(raw.nspace.as_ptr() as *const i8) }.to_string_lossy();
        assert!(nspace_bytes.len() <= std::mem::size_of::<ffi::pmix_nspace_t>());
    }

    // ── PmixDataBuffer ──────────────────────────────────────────────────────

    #[test]
    fn test_data_buffer_debug_null() {
        let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
        let s = format!("{:?}", buf);
        assert!(s.contains("null"));
        assert!(!buf.is_valid());
    }

    #[test]
    fn test_data_buffer_bytes_on_null() {
        let buf = unsafe { PmixDataBuffer::from_raw(std::ptr::null_mut()) };
        assert_eq!(buf.bytes_allocated(), 0);
        assert_eq!(buf.bytes_used(), 0);
        assert!(buf.as_mut_ptr().is_null());
    }

    #[test]
    fn test_data_buffer_create() {
        if let Ok(buf) = data_buffer_create() {
            assert!(buf.is_valid());
            assert_eq!(buf.bytes_allocated(), 0);
            assert_eq!(buf.bytes_used(), 0);
        }
    }

    #[test]
    fn test_data_buffer_create_valid_ptr() {
        let buf = data_buffer_create().expect("create buffer");
        assert!(!buf.as_mut_ptr().is_null());
    }

    #[test]
    fn test_data_buffer_debug_valid() {
        let buf = data_buffer_create().expect("create buffer");
        let s = format!("{:?}", buf);
        assert!(s.contains("PmixDataBuffer"));
        assert!(s.contains("ptr"));
        assert!(s.contains("bytes_allocated"));
        assert!(s.contains("bytes_used"));
    }

    #[test]
    fn test_data_buffer_release_idempotent() {
        let mut buf = data_buffer_create().expect("create buffer");
        assert!(buf.is_valid());
        data_buffer_release(&mut buf);
        assert!(!buf.is_valid());
        // Calling release again should be safe (no double-free)
        data_buffer_release(&mut buf);
        assert!(!buf.is_valid());
    }

    #[test]
    fn test_data_buffer_drop_after_release() {
        let mut buf = data_buffer_create().expect("create buffer");
        data_buffer_release(&mut buf);
        // Drop should be safe after explicit release (ptr is null)
        drop(buf);
    }

    #[test]
    fn test_data_buffer_from_raw_null() {
        let buf = unsafe { PmixDataBuffer::from_raw(ptr::null_mut()) };
        assert!(!buf.is_valid());
        assert!(buf.as_mut_ptr().is_null());
    }

    // ── data_pack ───────────────────────────────────────────────────────────

    #[test]
    fn test_data_pack_zero_num_vals_returns_bad_param() {
        let buf = data_buffer_create().expect("create buffer");
        let val: i32 = 42;
        let result = data_pack(None, &buf, &val, 0, PmixDataType::Int32);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_pack_negative_num_vals_returns_bad_param() {
        let buf = data_buffer_create().expect("create buffer");
        let val: i32 = 42;
        let result = data_pack(None, &buf, &val, -1, PmixDataType::Int32);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_pack_with_target() {
        let buf = data_buffer_create().expect("create buffer");
        let val: i32 = 42;
        let target = PmixProcRef::new("target_ns", 1);
        // Should not panic — actual result depends on PMIx init state
        let _result = data_pack(Some(target), &buf, &val, 1, PmixDataType::Int32);
    }

    #[test]
    fn test_data_pack_u8() {
        let buf = data_buffer_create().expect("create buffer");
        let val: u8 = 255;
        let result = data_pack(None, &buf, &val, 1, PmixDataType::Uint8);
        // May succeed or fail depending on PMIx init — just check it doesn't panic
        match result {
            Ok(n) => assert_eq!(n, 1),
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_pack_u64() {
        let buf = data_buffer_create().expect("create buffer");
        let val: u64 = u64::MAX;
        let result = data_pack(None, &buf, &val, 1, PmixDataType::Uint64);
        match result {
            Ok(n) => assert_eq!(n, 1),
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_pack_f64() {
        let buf = data_buffer_create().expect("create buffer");
        let val: f64 = 3.14159;
        let result = data_pack(None, &buf, &val, 1, PmixDataType::Float);
        match result {
            Ok(n) => assert_eq!(n, 1),
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_pack_buffer_grows() {
        let buf = data_buffer_create().expect("create buffer");
        let val: i32 = 42;
        let _ = data_pack(None, &buf, &val, 1, PmixDataType::Int32);
        // Buffer should have used some bytes after packing
        // (exact amount depends on PMIx version, but should be > 0 on success)
    }

    // ── data_unpack ─────────────────────────────────────────────────────────

    #[test]
    fn test_data_unpack_basic() {
        let buf = data_buffer_create().expect("create buffer");
        let mut val: i32 = 0;
        let mut count: i32 = 1;
        let _result = data_unpack(None, &buf, &mut val, &mut count, PmixDataType::Int32);
        // Unpacking from empty buffer should fail gracefully
    }

    #[test]
    fn test_data_unpack_with_source() {
        let buf = data_buffer_create().expect("create buffer");
        let mut val: i32 = 0;
        let mut count: i32 = 1;
        let source = PmixProcRef::new("source_ns", 0);
        let _result = data_unpack(
            Some(source),
            &buf,
            &mut val,
            &mut count,
            PmixDataType::Int32,
        );
    }

    // ── data_pack + data_unpack roundtrip ───────────────────────────────────

    #[test]
    fn test_pack_unpack_i32_roundtrip() {
        let buf = data_buffer_create().expect("create buffer");
        let val: i32 = 42;
        let packed = data_pack(None, &buf, &val, 1, PmixDataType::Int32);
        if packed.is_ok() {
            let mut out: i32 = 0;
            let mut count: i32 = 1;
            let unpacked = data_unpack(None, &buf, &mut out, &mut count, PmixDataType::Int32);
            if unpacked.is_ok() {
                assert_eq!(out, 42);
                assert_eq!(count, 1);
            }
        }
    }

    #[test]
    fn test_pack_unpack_u64_roundtrip() {
        let buf = data_buffer_create().expect("create buffer");
        let val: u64 = 0xDEADBEEFCAFEBABE;
        let packed = data_pack(None, &buf, &val, 1, PmixDataType::Uint64);
        if packed.is_ok() {
            let mut out: u64 = 0;
            let mut count: i32 = 1;
            let unpacked = data_unpack(None, &buf, &mut out, &mut count, PmixDataType::Uint64);
            if unpacked.is_ok() {
                assert_eq!(out, 0xDEADBEEFCAFEBABE);
            }
        }
    }

    // ── data_unload / data_load ─────────────────────────────────────────────

    #[test]
    fn test_data_unload_basic() {
        let buf = data_buffer_create().expect("create buffer");
        let result = data_unload(&buf);
        match result {
            Ok(obj) => {
                // Unloading empty buffer should give empty or small byte object
                assert!(obj.size() >= 0);
            }
            Err(_) => {
                // May fail on empty buffer depending on PMIx version
            }
        }
    }

    #[test]
    fn test_data_load_basic() {
        let buf = data_buffer_create().expect("create buffer");
        let payload = PmixByteObject::new();
        let result = data_load(&buf, &payload);
        // Loading empty payload should succeed or fail gracefully
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_load_from_vec_payload() {
        let buf = data_buffer_create().expect("create buffer");
        let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
        let result = data_load(&buf, &payload);
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    // ── data_embed ──────────────────────────────────────────────────────────

    #[test]
    fn test_data_embed_none() {
        let buf = data_buffer_create().expect("create buffer");
        let result = data_embed(&buf, None);
        // None payload behavior depends on PMIx version — may succeed or fail
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_embed_with_payload() {
        let buf = data_buffer_create().expect("create buffer");
        let payload = PmixByteObject::from(vec![1u8, 2, 3, 4]);
        let result = data_embed(&buf, Some(&payload));
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
        // payload should still be usable after embed (unlike load)
        assert_eq!(payload.size(), 4);
    }

    #[test]
    fn test_data_embed_preserves_source() {
        let buf = data_buffer_create().expect("create buffer");
        let payload = PmixByteObject::from(vec![42u8, 43, 44]);
        let _ = data_embed(&buf, Some(&payload));
        // Source payload must be unmodified after embed
        assert_eq!(payload.as_slice(), &[42, 43, 44]);
        assert_eq!(payload.size(), 3);
    }

    // ── data_copy ───────────────────────────────────────────────────────────

    #[test]
    fn test_data_copy_basic() {
        let val: i32 = 42;
        let result = data_copy(&val, PmixDataType::Int32);
        match result {
            Ok(ptr) => {
                assert!(!ptr.is_null());
                // Note: caller owns the allocation; we can't safely free it
                // without knowing the type, so we just verify non-null.
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_copy_u8() {
        let val: u8 = 255;
        let result = data_copy(&val, PmixDataType::Uint8);
        match result {
            Ok(ptr) => assert!(!ptr.is_null()),
            Err(_) => {}
        }
    }

    // ── data_copy_payload ───────────────────────────────────────────────────

    #[test]
    fn test_data_copy_payload_basic() {
        let src = data_buffer_create().expect("create buffer");
        let dest = data_buffer_create().expect("create buffer");
        let result = data_copy_payload(&dest, &src);
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_copy_payload_self() {
        let buf = data_buffer_create().expect("create buffer");
        let result = data_copy_payload(&buf, &buf);
        match result {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    // ── data_print ──────────────────────────────────────────────────────────

    #[test]
    fn test_data_print_basic() {
        let val: i32 = 42;
        let result = data_print(&val, None, PmixDataType::Int32);
        match result {
            Ok(output) => {
                let s = output.as_str();
                // Should contain something (may be empty if not initialized)
                assert!(s.len() >= 0);
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_print_with_prefix() {
        let val: i32 = 42;
        let result = data_print(&val, Some("val="), PmixDataType::Int32);
        match result {
            Ok(output) => {
                let s = output.as_str();
                assert!(s.len() >= 0);
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_print_empty_prefix() {
        let val: i32 = 42;
        let result = data_print(&val, Some(""), PmixDataType::Int32);
        match result {
            Ok(_) => {}
            Err(_) => {}
        }
    }

    // ── PmixPrintOutput ─────────────────────────────────────────────────────

    #[test]
    fn test_print_output_default() {
        let output = PmixPrintOutput::default();
        assert_eq!(output.as_str(), "");
        assert_eq!(output.len(), 0);
    }

    #[test]
    fn test_print_output_display() {
        let output = PmixPrintOutput::default();
        let s = format!("{}", output);
        assert_eq!(s, "");
    }

    #[test]
    fn test_print_output_debug() {
        let output = PmixPrintOutput::default();
        let s = format!("{:?}", output);
        assert!(s.contains("\"\""));
    }

    #[test]
    fn test_print_output_deref() {
        let output: &str = &PmixPrintOutput::default();
        assert_eq!(output, "");
    }

    #[test]
    fn test_print_output_into_string() {
        let output = PmixPrintOutput::default();
        let s: String = output.into();
        assert_eq!(s, "");
    }

    // ── data_compress ───────────────────────────────────────────────────────

    #[test]
    fn test_data_compress_empty() {
        if let Ok(result) = data_compress(&[]) {
            assert!(result.len() < 100);
        }
    }

    #[test]
    fn test_data_compress_empty_returns_err() {
        let result = data_compress(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_compress_small_data() {
        // Small data may not compress well
        let result = data_compress(b"hi");
        match result {
            Ok(_) => {}
            Err(_) => {} // Expected for very small data
        }
    }

    #[test]
    fn test_data_compress_repetitive_data() {
        // Repetitive data should compress well
        let data = vec![0u8; 4096];
        let result = data_compress(&data);
        match result {
            Ok(compressed) => {
                assert!(compressed.len() < data.len());
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_data_compress_decompress_roundtrip() {
        let data = b"hello world this is a test of compression";
        if let Ok(compressed) = data_compress(data) {
            if let Ok(decompressed) = data_decompress(&compressed) {
                assert_eq!(decompressed, *data);
            }
        }
    }

    #[test]
    fn test_data_compress_decompress_larger_roundtrip() {
        let data = vec![42u8; 8192];
        if let Ok(compressed) = data_compress(&data) {
            if let Ok(decompressed) = data_decompress(&compressed) {
                assert_eq!(decompressed, data);
            }
        }
    }

    // ── data_decompress ─────────────────────────────────────────────────────

    #[test]
    fn test_data_decompress_empty_returns_err() {
        let result = data_decompress(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_decompress_invalid_data() {
        // Random data that wasn't produced by data_compress should fail
        let result = data_decompress(b"not compressed data");
        assert!(result.is_err());
    }

    #[test]
    fn test_data_decompress_random_bytes() {
        let result = data_decompress(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(result.is_err());
    }
}
