//! PMIx value builders and owned values.

use crate::ffi::*;
use cstring_array::CStringArray;
use crate::{Info, Proc};
use std::ffi::{CStr, CString, NulError};
use std::os::raw::{c_char, c_void};
use std::{fmt, mem, ptr};

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// All errors the builder can produce.
#[derive(Debug, PartialEq, Eq)]
pub enum ValueError {
    /// A string argument contained an interior NUL byte.
    ContainsNul(NulError),
    /// `build()` / `build_raw()` was called before any payload setter.
    MissingPayload,
    /// A byte-object or data-array was given a length of zero when one or
    /// more bytes/elements were required.
    EmptyData,
}

impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContainsNul(e) => write!(f, "string contains interior NUL: {e}"),
            Self::MissingPayload => write!(f, "no payload set on PmixValueBuilder"),
            Self::EmptyData => write!(f, "data slice must not be empty"),
        }
    }
}

impl std::error::Error for ValueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContainsNul(e) => Some(e),
            _ => None,
        }
    }
}

impl From<NulError> for ValueError {
    fn from(e: NulError) -> Self {
        Self::ContainsNul(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixTimeval – Rust mirror of pmix_timeval_t
// ─────────────────────────────────────────────────────────────────────────────

/// Newtype so callers don't need to import `sys::pmix_timeval_t` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmixTimeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

impl From<PmixTimeval> for timeval {
    fn from(v: PmixTimeval) -> Self {
        Self {
            tv_sec: v.tv_sec,
            tv_usec: v.tv_usec,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixEnvar – Rust mirror of pmix_envar_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `pmix_envar_t`.
///
/// Both `envar` and `value` are stored as `CString` so interior-NUL checking
/// happens at construction time, not at `build()` time.
#[derive(Debug, Clone)]
pub struct PmixEnvar {
    pub envar: CString,
    pub value: CString,
    pub separator: u8,
}

impl PmixEnvar {
    /// Create from `&str` arguments; returns `NulError` if either string has
    /// an interior NUL.
    pub fn new(envar: &str, value: &str, separator: char) -> Result<Self, NulError> {
        Ok(Self {
            envar: CString::new(envar)?,
            value: CString::new(value)?,
            separator: separator as u8,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPayload – the Rust-side discriminated union
// ─────────────────────────────────────────────────────────────────────────────

/// Every discriminant of `pmix_val_data`, expressed as a safe Rust enum.
///
/// Variants that wrap heap-allocated C data (`String`, `Proc`, `ByteObject`,
/// `Envar`, `DataArray`) own their data here; `write_into` transfers ownership
/// to the raw `pmix_value_t`.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum PmixPayload {
    Undef,
    // ── Scalar types ──────────────────────────────────────────────────────
    Bool(bool),
    Byte(u8),
    /// NUL-terminated heap string (`PMIX_STRING`).
    String(CString),
    Size(usize),
    Pid(u32),
    Int(i32),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Uint(u32),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Float(f32),
    Double(f64),
    /// Elapsed / absolute time (`PMIX_TIMEVAL`).
    Timeval(PmixTimeval),
    Status(pmix_status_t),
    Rank(pmix_rank_t),
    // ── Enum-like scalar types (thin newtypes over integers) ──────────────
    Persist(pmix_persistence_t),
    Scope(pmix_scope_t),
    DataRange(pmix_data_range_t),
    ProcState(pmix_proc_state_t),
    AllocDirective(pmix_alloc_directive_t),
    IofChannel(pmix_iof_channel_t),
    InfoDirectives(pmix_info_directives_t),
    // ── Composite heap types ──────────────────────────────────────────────
    /// A `pmix_proc_t` (namespace + rank) stored as `Box<pmix_proc_t>`.
    Proc(Proc),
    /// Raw byte buffer (`PMIX_BYTE_OBJECT`).
    ByteObject(Vec<u8>),
    /// An environment variable modification (`PMIX_ENVAR`).
    Envar(PmixEnvar),
    /// Opaque pointer (`PMIX_POINTER`). Caller is fully responsible for
    /// the lifetime of the pointed-to data.
    Pointer(*mut std::ffi::c_void),
    /// Heterogeneous or homogeneous array of `pmix_value_t` structs
    /// (`PMIX_DATA_ARRAY`). Element type is recorded separately.
    DataArray {
        elem_type: pmix_data_type_t,
        elements: Vec<pmix_value_t>,
    },
}

impl PmixPayload {
    /// Return the `pmix_data_type_t` constant that matches this variant.
    pub fn type_tag(&self) -> pmix_data_type_t {
        (match self {
            Self::Undef => PMIX_UNDEF,
            Self::Bool(_) => PMIX_BOOL,
            Self::Byte(_) => PMIX_BYTE,
            Self::String(_) => PMIX_STRING,
            Self::Size(_) => PMIX_SIZE,
            Self::Pid(_) => PMIX_PID,
            Self::Int(_) => PMIX_INT,
            Self::Int8(_) => PMIX_INT8,
            Self::Int16(_) => PMIX_INT16,
            Self::Int32(_) => PMIX_INT32,
            Self::Int64(_) => PMIX_INT64,
            Self::Uint(_) => PMIX_UINT,
            Self::Uint8(_) => PMIX_UINT8,
            Self::Uint16(_) => PMIX_UINT16,
            Self::Uint32(_) => PMIX_UINT32,
            Self::Uint64(_) => PMIX_UINT64,
            Self::Float(_) => PMIX_FLOAT,
            Self::Double(_) => PMIX_DOUBLE,
            Self::Timeval(_) => PMIX_TIMEVAL,
            Self::Status(_) => PMIX_STATUS,
            Self::Rank(_) => PMIX_PROC_RANK,
            Self::Persist(_) => PMIX_PERSIST,
            Self::Scope(_) => PMIX_SCOPE,
            Self::DataRange(_) => PMIX_DATA_RANGE,
            Self::ProcState(_) => PMIX_PROC_STATE,
            Self::AllocDirective(_) => PMIX_ALLOC_DIRECTIVE,
            Self::IofChannel(_) => PMIX_IOF_CHANNEL,
            Self::InfoDirectives(_) => PMIX_INFO_DIRECTIVES,
            Self::Proc(_) => PMIX_PROC,
            Self::ByteObject(_) => PMIX_BYTE_OBJECT,
            Self::Envar(_) => PMIX_ENVAR,
            Self::Pointer(_) => PMIX_POINTER,
            Self::DataArray { .. } => PMIX_DATA_ARRAY,
        }) as pmix_data_type_t
    }
}
// ─────────────────────────────────────────────────────────────────────────────
// PmixValueBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Fluent builder for `pmix_value_t`.
///
/// Call one of the typed setter methods, then call [`build`][Self::build] for
/// a [`PmixOwnedValue`] (RAII) or [`build_raw`][Self::build_raw] for a raw
/// `pmix_value_t` whose heap data the caller must free manually.
///
/// # Example
/// ```
/// use pmix::{PmixValueBuilder, ValueError};
///
/// let owned = PmixValueBuilder::new().uint32(42).build().expect("build");
/// drop(owned);
/// ```
#[derive(Default)]
pub struct PmixValueBuilder {
    payload: Option<PmixPayload>,
}

impl PmixValueBuilder {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new() -> Self {
        Self::default()
    }

    // ── Generic payload setter ────────────────────────────────────────────

    /// Set the payload to any [`PmixPayload`] variant.
    pub fn payload(mut self, p: PmixPayload) -> Self {
        self.payload = Some(p);
        self
    }

    // ── Typed scalar setters ──────────────────────────────────────────────

    pub fn undef(self) -> Self {
        self.payload(PmixPayload::Undef)
    }
    pub fn bool(self, v: bool) -> Self {
        self.payload(PmixPayload::Bool(v))
    }
    pub fn byte(self, v: u8) -> Self {
        self.payload(PmixPayload::Byte(v))
    }
    pub fn size(self, v: usize) -> Self {
        self.payload(PmixPayload::Size(v))
    }
    pub fn pid(self, v: u32) -> Self {
        self.payload(PmixPayload::Pid(v))
    }
    pub fn int(self, v: i32) -> Self {
        self.payload(PmixPayload::Int(v))
    }
    pub fn int8(self, v: i8) -> Self {
        self.payload(PmixPayload::Int8(v))
    }
    pub fn int16(self, v: i16) -> Self {
        self.payload(PmixPayload::Int16(v))
    }
    pub fn int32(self, v: i32) -> Self {
        self.payload(PmixPayload::Int32(v))
    }
    pub fn int64(self, v: i64) -> Self {
        self.payload(PmixPayload::Int64(v))
    }
    pub fn uint(self, v: u32) -> Self {
        self.payload(PmixPayload::Uint(v))
    }
    pub fn uint8(self, v: u8) -> Self {
        self.payload(PmixPayload::Uint8(v))
    }
    pub fn uint16(self, v: u16) -> Self {
        self.payload(PmixPayload::Uint16(v))
    }
    pub fn uint32(self, v: u32) -> Self {
        self.payload(PmixPayload::Uint32(v))
    }
    pub fn uint64(self, v: u64) -> Self {
        self.payload(PmixPayload::Uint64(v))
    }
    pub fn float(self, v: f32) -> Self {
        self.payload(PmixPayload::Float(v))
    }
    pub fn double(self, v: f64) -> Self {
        self.payload(PmixPayload::Double(v))
    }
    pub fn timeval(self, v: PmixTimeval) -> Self {
        self.payload(PmixPayload::Timeval(v))
    }
    pub fn status(self, v: pmix_status_t) -> Self {
        self.payload(PmixPayload::Status(v))
    }
    pub fn rank(self, v: pmix_rank_t) -> Self {
        self.payload(PmixPayload::Rank(v))
    }
    pub fn persist(self, v: pmix_persistence_t) -> Self {
        self.payload(PmixPayload::Persist(v))
    }
    pub fn scope(self, v: pmix_scope_t) -> Self {
        self.payload(PmixPayload::Scope(v))
    }
    pub fn data_range(self, v: pmix_data_range_t) -> Self {
        self.payload(PmixPayload::DataRange(v))
    }
    pub fn proc_state(self, v: pmix_proc_state_t) -> Self {
        self.payload(PmixPayload::ProcState(v))
    }
    pub fn alloc_directive(self, v: pmix_alloc_directive_t) -> Self {
        self.payload(PmixPayload::AllocDirective(v))
    }
    pub fn iof_channel(self, v: pmix_iof_channel_t) -> Self {
        self.payload(PmixPayload::IofChannel(v))
    }
    pub fn info_directives(self, v: pmix_info_directives_t) -> Self {
        self.payload(PmixPayload::InfoDirectives(v))
    }

    // ── Typed heap setters ────────────────────────────────────────────────

    /// Set a NUL-terminated string payload (`PMIX_STRING`).
    ///
    /// Returns `Err(ValueError::ContainsNul)` if `s` contains an interior NUL.
    pub fn string(self, s: &str) -> Result<Self, ValueError> {
        Ok(self.payload(PmixPayload::String(CString::new(s)?)))
    }

    /// Set a `pmix_proc_t` payload (`PMIX_PROC`).
    pub fn proc_(self, p: Proc) -> Self {
        self.payload(PmixPayload::Proc(p))
    }

    /// Set a raw byte-buffer payload (`PMIX_BYTE_OBJECT`).
    ///
    /// Returns `Err(ValueError::EmptyData)` for a zero-length slice.
    pub fn byte_object(self, bytes: &[u8]) -> Result<Self, ValueError> {
        if bytes.is_empty() {
            return Err(ValueError::EmptyData);
        }
        Ok(self.payload(PmixPayload::ByteObject(bytes.to_vec())))
    }

    /// Set an environment-variable modification payload (`PMIX_ENVAR`).
    pub fn envar(self, e: PmixEnvar) -> Self {
        self.payload(PmixPayload::Envar(e))
    }

    /// Set an opaque pointer payload (`PMIX_POINTER`).
    ///
    /// # Safety
    /// The caller must ensure the pointed-to data outlives the resulting
    /// `pmix_value_t` and is freed independently.
    pub unsafe fn pointer(self, p: *mut std::ffi::c_void) -> Self {
        self.payload(PmixPayload::Pointer(p))
    }

    /// Set a heterogeneous `pmix_data_array_t` payload (`PMIX_DATA_ARRAY`).
    ///
    /// `elem_type` records the declared element type (use `PMIX_VALUE` /
    /// `PMIX_UNDEF` for a mixed array).  Every element must already be a
    /// fully-initialised `pmix_value_t`; use other builders to produce them.
    ///
    /// Returns `Err(ValueError::EmptyData)` for an empty slice.
    pub fn data_array(
        self,
        elem_type: pmix_data_type_t,
        elements: Vec<pmix_value_t>,
    ) -> Result<Self, ValueError> {
        if elements.is_empty() {
            return Err(ValueError::EmptyData);
        }
        Ok(self.payload(PmixPayload::DataArray {
            elem_type,
            elements,
        }))
    }

    // ── Static constructor: string_array ─────────────────────────────────
    //
    // This is the primary place where CStringArray is used: when a caller
    // has a list of string values (e.g. query keys, regex patterns) it wants
    // to pack into a PMIX_DATA_ARRAY of PMIX_STRING elements AND simultaneously
    // hold a char** view for PMIx list APIs.

    /// Build a `PMIX_DATA_ARRAY` of `PMIX_STRING` elements from a slice of
    /// `&str` values.
    ///
    /// Returns:
    /// - A [`PmixOwnedValue`] wrapping the resulting `pmix_value_t`.
    /// - A [`CStringArray`] whose `*const *const c_char` pointer covers the
    ///   same strings in the same order, for passing to PMIx list APIs that
    ///   accept `char**`.
    ///
    /// Both objects independently own their strings; the `CStringArray` is
    /// a separate copy intended for the C API call's lifetime only.
    ///
    /// ```
    /// use pmix::PmixValueBuilder;
    ///
    /// let (val, keys) = PmixValueBuilder::string_array(&["pmix.timeout", "pmix.collect"]).expect("build");
    /// let _pp: *const *const std::ffi::c_char = keys.as_ptr();
    /// ```
    pub fn string_array(strings: &[&str]) -> Result<(PmixOwnedValue, CStringArray), ValueError> {
        if strings.is_empty() {
            return Err(ValueError::EmptyData);
        }

        // Build the pmix_value_t elements (each a PMIX_STRING).
        let elements: Result<Vec<pmix_value_t>, ValueError> = strings
            .iter()
            .map(|s| PmixValueBuilder::new().string(s)?.build_raw())
            .collect();

        let owned = PmixValueBuilder::new()
            .data_array(PMIX_STRING as pmix_data_type_t, elements?)?
            .build()?;

        // Build CStringArray from the same strings. CStringArray owns its own
        // CString copies, so the two lifetimes are fully independent.
        let cstrings: Result<Vec<CString>, NulError> =
            strings.iter().map(|s| CString::new(*s)).collect();
        let key_array =
            CStringArray::from_cstrings(cstrings?).expect("strings are already validated CStrings");

        Ok((owned, key_array))
    }

    // ── Build ─────────────────────────────────────────────────────────────

    /// Validate and return a [`PmixOwnedValue`] (RAII — heap data freed on
    /// `Drop`).
    pub fn build(self) -> Result<PmixOwnedValue, ValueError> {
        Ok(PmixOwnedValue {
            inner: self.build_raw()?,
        })
    }

    /// Validate and write directly into a `pmix_value_t`.
    ///
    /// **Ownership note:** any heap allocations (string, proc, envar,
    /// byte-object, data-array) are now owned by the returned struct.  The
    /// caller must call [`free_value`] or use [`build`][Self::build] instead.
    pub fn build_raw(self) -> Result<pmix_value_t, ValueError> {
        let payload = self.payload.ok_or(ValueError::MissingPayload)?;
        // SAFETY: zeroed gives a valid starting state for every scalar field
        // and every pointer field (NULL).
        let mut v: pmix_value_t = unsafe { std::mem::zeroed() };
        v.type_ = payload.type_tag();
        write_payload(&mut v, payload);
        Ok(v)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// write_payload – transfer PmixPayload into a pmix_val_data union
// ─────────────────────────────────────────────────────────────────────────────

/// Write `payload` into `dst`.
///
/// For heap variants (String, Proc, ByteObject, Envar, DataArray) ownership
/// is transferred; the caller is responsible for freeing via [`free_value`].
fn write_payload(dst: &mut pmix_value, payload: PmixPayload) {
    //let &mut dst = &mut dst_in.data;
    // SAFETY: caller has already set v.type_; we write exactly the matching
    // union arm. All other arms remain as zeroed bytes from `mem::zeroed`.
    unsafe {
        match payload {
            PmixPayload::Undef => { /* data stays zeroed */ }
            PmixPayload::Bool(v) => dst.data.flag = v,
            PmixPayload::Byte(v) => dst.data.byte = v,
            PmixPayload::Size(v) => dst.data.size = v,
            PmixPayload::Pid(v) => dst.data.pid = v as pid_t,
            PmixPayload::Int(v) => dst.data.integer = v,
            PmixPayload::Int8(v) => dst.data.int8 = v,
            PmixPayload::Int16(v) => dst.data.int16 = v,
            PmixPayload::Int32(v) => dst.data.int32 = v,
            PmixPayload::Int64(v) => dst.data.int64 = v,
            PmixPayload::Uint(v) => dst.data.uint = v,
            PmixPayload::Uint8(v) => dst.data.uint8 = v,
            PmixPayload::Uint16(v) => dst.data.uint16 = v,
            PmixPayload::Uint32(v) => dst.data.uint32 = v,
            PmixPayload::Uint64(v) => dst.data.uint64 = v,
            PmixPayload::Float(v) => dst.data.fval = v,
            PmixPayload::Double(v) => dst.data.dval = v,
            PmixPayload::Timeval(v) => dst.data.tv = v.into(),
            PmixPayload::Status(v) => dst.data.status = v,
            PmixPayload::Rank(v) => dst.data.rank = v,
            PmixPayload::Persist(v) => dst.data.persist = v,
            PmixPayload::Scope(v) => dst.data.scope = v,
            PmixPayload::DataRange(v) => dst.data.range = v,
            PmixPayload::ProcState(v) => dst.data.state = v,
            PmixPayload::AllocDirective(v) => dst.data.adir = v,
            PmixPayload::IofChannel(v) => dst.data.uint16 = v,
            PmixPayload::InfoDirectives(v) => dst.data.uint32 = v,

            // Transfer CString → *mut c_char (null-termination included).
            PmixPayload::String(cs) => {
                dst.data.string = cs.into_raw();
            }

            // Heap-allocate pmix_proc_t, fill nspace (fixed char array), set rank.
            PmixPayload::Proc(p) => {
                let mut raw_proc: pmix_proc_t = std::mem::zeroed();
                let copy_len = p.handle.nspace.len().min(raw_proc.nspace.len());
                ptr::copy_nonoverlapping(
                    p.handle.nspace.as_ptr() as *const c_void,
                    raw_proc.nspace.as_mut_ptr() as *mut c_void,
                    copy_len,
                );
                raw_proc.rank = p.handle.rank;
                dst.data.proc_ = Box::into_raw(Box::new(raw_proc));
            }

            // Leak Vec<u8> into pmix_byte_object_t.
            PmixPayload::ByteObject(mut bytes) => {
                bytes.shrink_to_fit();
                let len = bytes.len();
                let ptr = bytes.as_mut_ptr() as *mut i8;
                std::mem::forget(bytes);
                dst.data.bo = pmix_byte_object_t {
                    bytes: ptr,
                    size: len,
                };
            }

            // Heap-allocate pmix_envar_t; transfer both CStrings.
            PmixPayload::Envar(e) => {
                let raw = Box::new(pmix_envar_t {
                    envar: e.envar.into_raw(),
                    value: e.value.into_raw(),
                    separator: e.separator as i8,
                });
                dst.data.envar = *Box::into_raw(raw);
            }

            // Opaque pointer – no allocation here, caller owns data.
            PmixPayload::Pointer(p) => {
                dst.data.ptr = p;
            }

            // Leak Vec<pmix_value_t> into a heap-allocated pmix_data_array_t.
            PmixPayload::DataArray {
                elem_type,
                mut elements,
            } => {
                elements.shrink_to_fit();
                let len = elements.len();
                let arr = elements.as_mut_ptr() as *mut std::ffi::c_void;
                std::mem::forget(elements);
                let darray = Box::new(pmix_data_array_t {
                    type_: elem_type,
                    size: len,
                    array: arr,
                });
                dst.data.darray = Box::into_raw(darray);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// free_value – release heap data inside a pmix_value_t
// ─────────────────────────────────────────────────────────────────────────────

/// Free any heap-allocated data inside `v` that was created by this builder.
///
/// After this call `v` is zeroed and safe to drop or reuse.
///
/// # Safety
/// `v` must have been produced by this crate's builder (or have equivalent
/// allocation discipline).  Calling this on a `pmix_value_t` produced by the C
/// library (and therefore managed by `PMIX_VALUE_RELEASE`) is a double-free.
pub fn free_value(v: &mut pmix_value_t) {
    // SAFETY: type_ was set by write_payload; we access only the matching arm.
    unsafe {
        match v.type_ as u32 {
            t if t == PMIX_STRING => {
                if !v.data.string.is_null() {
                    drop(CString::from_raw(v.data.string));
                    v.data.string = ptr::null_mut();
                }
            }
            t if t == PMIX_PROC => {
                if !v.data.proc_.is_null() {
                    drop(Box::from_raw(v.data.proc_));
                    v.data.proc_ = ptr::null_mut();
                }
            }
            t if t == PMIX_BYTE_OBJECT => {
                if !v.data.bo.bytes.is_null() && v.data.bo.size > 0 {
                    let _ = Vec::from_raw_parts(
                        v.data.bo.bytes as *mut u8,
                        v.data.bo.size,
                        v.data.bo.size,
                    );
                    v.data.bo.bytes = ptr::null_mut();
                    v.data.bo.size = 0;
                }
            }
            t if t == PMIX_ENVAR => {
                // envar is embedded in the union (not heap-allocated).
                // Only free the CString pointers inside it.
                if !v.data.envar.envar.is_null() {
                    drop(CString::from_raw(v.data.envar.envar));
                    v.data.envar.envar = ptr::null_mut();
                }
                if !v.data.envar.value.is_null() {
                    drop(CString::from_raw(v.data.envar.value));
                    v.data.envar.value = ptr::null_mut();
                }
            }
            t if t == PMIX_DATA_ARRAY && !v.data.darray.is_null() => {
                let darray = Box::from_raw(v.data.darray);
                if !darray.array.is_null() && darray.size > 0 {
                    // Reconstruct the Vec to let each element's Drop run.
                    let elements = Vec::from_raw_parts(
                        darray.array as *mut pmix_value_t,
                        darray.size,
                        darray.size,
                    );
                    // Each element may itself own heap data; free recursively.
                    for mut elem in elements {
                        free_value(&mut elem);
                    }
                }
                v.data.darray = ptr::null_mut();
            }
            // PMIX_POINTER – caller owns; we never free it.
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixOwnedValue – RAII wrapper with automatic cleanup
// ─────────────────────────────────────────────────────────────────────────────

/// Owns a `pmix_value_t` and calls [`free_value`] when dropped.
///
/// Use [`as_raw`][Self::as_raw] / [`as_raw_mut`][Self::as_raw_mut] to borrow
/// the struct for FFI calls, or [`into_raw`][Self::into_raw] to transfer full
/// ownership to a C API that calls `PMIX_VALUE_RELEASE` itself.
pub struct PmixOwnedValue {
    pub(crate) inner: pmix_value_t,
}

impl PmixOwnedValue {
    /// Immutable raw pointer.
    pub fn as_raw(&self) -> *const pmix_value_t {
        &self.inner
    }

    /// Mutable raw pointer.
    pub fn as_raw_mut(&mut self) -> *mut pmix_value_t {
        &mut self.inner
    }

    /// Return the `pmix_data_type_t` tag.
    pub fn type_tag(&self) -> pmix_data_type_t {
        self.inner.type_
    }

    /// Transfer ownership out of RAII; caller must free via `PMIX_VALUE_RELEASE`
    /// or [`free_value`].
    pub fn into_raw(self) -> pmix_value_t {
        let inner = self.inner;
        std::mem::forget(self);
        inner
    }

    pub fn bytes(&self) -> (*const c_void, usize) {
        let bytes = unsafe { self.inner.data.bo }; //.bytes.cast_const() as *const c_void
        (bytes.bytes.cast_const() as *const c_void, bytes.size)
    }

    pub fn size(&self) -> usize {
        unsafe { self.inner.data.size }
    }

    /// Read the value as a u32.
    pub fn uint32(&self) -> u32 {
        unsafe { self.inner.data.uint32 }
    }

    /// Read the value as a u64.
    pub fn uint64(&self) -> u64 {
        unsafe { self.inner.data.uint64 }
    }

    /// Read the byte object as a Vec<u8> copy.
    pub fn bytes_copy(&self) -> Vec<u8> {
        let (ptr, len) = self.bytes();
        if ptr.is_null() || len == 0 {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len).to_vec() }
    }
}

impl Drop for PmixOwnedValue {
    fn drop(&mut self) {
        free_value(&mut self.inner);
    }
}

impl std::fmt::Debug for PmixOwnedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixOwnedValue")
            .field("type_", &self.inner.type_)
            .finish_non_exhaustive()
    }
}
