use crate::ffi;
use crate::ffi::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;
use crate::pmix_ffi_or_mock;
use crate::*;
use cstring_array::CStringArray;
use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::{c_char, c_void};
use std::ptr::{null, null_mut};
use std::{fmt, mem, ptr};

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
    /// Return the `PmixDataType` that matches this variant.
    pub fn type_tag(&self) -> PmixDataType {
        match self {
            Self::Undef => PmixDataType::Undef,
            Self::Bool(_) => PmixDataType::Bool,
            Self::Byte(_) => PmixDataType::Byte,
            Self::String(_) => PmixDataType::String,
            Self::Size(_) => PmixDataType::Size,
            Self::Pid(_) => PmixDataType::Pid,
            Self::Int(_) => PmixDataType::Int,
            Self::Int8(_) => PmixDataType::Int8,
            Self::Int16(_) => PmixDataType::Int16,
            Self::Int32(_) => PmixDataType::Int32,
            Self::Int64(_) => PmixDataType::Int64,
            Self::Uint(_) => PmixDataType::Uint,
            Self::Uint8(_) => PmixDataType::Uint8,
            Self::Uint16(_) => PmixDataType::Uint16,
            Self::Uint32(_) => PmixDataType::Uint32,
            Self::Uint64(_) => PmixDataType::Uint64,
            Self::Float(_) => PmixDataType::Float,
            Self::Double(_) => PmixDataType::Double,
            Self::Timeval(_) => PmixDataType::Timeval,
            Self::Status(_) => PmixDataType::Status,
            Self::Rank(_) => PmixDataType::ProcRank,
            Self::Persist(_) => PmixDataType::Persist,
            Self::Scope(_) => PmixDataType::Scope,
            Self::DataRange(_) => PmixDataType::DataRange,
            Self::ProcState(_) => PmixDataType::ProcState,
            Self::AllocDirective(_) => PmixDataType::AllocDirective,
            Self::IofChannel(_) => PmixDataType::IofChannel,
            Self::InfoDirectives(_) => PmixDataType::InfoDirectives,
            Self::Proc(_) => PmixDataType::Proc,
            Self::ByteObject(_) => PmixDataType::ByteObject,
            Self::Envar(_) => PmixDataType::Envar,
            Self::Pointer(_) => PmixDataType::Pointer,
            Self::DataArray { .. } => PmixDataType::DataArray,
        }
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
            pmix_owned: false,
            _not_thread_safe: std::marker::PhantomData,
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
        v.type_ = payload.type_tag().to_raw();
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
                let ptr = bytes.as_mut_ptr();
                std::mem::forget(bytes);
                dst.data.bo = pmix_byte_object_t {
                    // Bindgen emits *mut u8 or *mut i8 depending on the host headers.
                    bytes: ptr as *mut _,
                    size: len,
                };
            }

            // Write pmix_envar_t inline; transfer both CStrings.
            PmixPayload::Envar(e) => {
                dst.data.envar = pmix_envar_t {
                    envar: e.envar.into_raw(),
                    value: e.value.into_raw(),
                    // Bindgen emits u8 or i8 depending on the host headers.
                    separator: e.separator as _,
                };
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
/// `v` must have been produced by this crate's builder, or be a Rust-owned copy
/// whose nested payloads follow the same allocation discipline. It must not be
/// a value still owned by PMIx or a borrowed copy of a C-owned info array.
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

/// Destruct nested payloads allocated by PMIx.
pub(crate) unsafe fn destruct_pmix_value(v: &mut pmix_value_t) {
    pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_value_destruct(v) },
        real = unsafe { ffi::PMIx_Value_destruct(v) },
    );
}

/// Release a PMIx-allocated `pmix_value_t` struct and its nested payloads.
pub(crate) unsafe fn release_pmix_value(value: *mut pmix_value_t) {
    if value.is_null() {
        return;
    }
    pmix_ffi_or_mock!(
        mock = unsafe {
            mock_ffi::mock_value_destruct(value);
            mock_ffi::mock_value_free(value, 1);
        },
        real = unsafe {
            ffi::PMIx_Value_destruct(value);
            libc::free(value.cast());
        },
    );
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
    pub(crate) pmix_owned: bool,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    pub(crate) _not_thread_safe: std::marker::PhantomData<*mut u8>,
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

    /// Return the `PmixDataType` tag.
    pub fn type_tag(&self) -> PmixDataType {
        PmixDataType::from_raw(self.inner.type_)
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

    /// Read the value as an owned UTF-8 string.
    pub fn string_copy(&self) -> Result<String, std::str::Utf8Error> {
        // SAFETY: this accessor is only valid for a PMIX_STRING value, whose
        // active union arm is the string pointer.
        let ptr = unsafe { self.inner.data.string };
        if ptr.is_null() {
            // SAFETY: an empty C string is valid and never null.
            return Ok(String::new());
        }
        // SAFETY: PMIX_STRING values contain a valid NUL-terminated pointer
        // owned by this value for its lifetime.
        let string = unsafe { CStr::from_ptr(ptr) };
        string.to_str().map(str::to_owned)
    }
}

impl Drop for PmixOwnedValue {
    fn drop(&mut self) {
        if self.pmix_owned {
            // SAFETY: nested payloads were allocated by PMIx.
            unsafe { destruct_pmix_value(&mut self.inner) };
        } else {
            free_value(&mut self.inner);
        }
    }
}

impl std::fmt::Debug for PmixOwnedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixOwnedValue")
            .field("type_", &self.inner.type_)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Proc {
    pub(crate) handle: pmix_proc_t,
    pub(crate) len: usize,
}

pub struct Info {
    pub(crate) handle: *mut pmix_info_t,
    pub(crate) len: usize,
    /// Makes this type `!Send` + `!Sync` (PMIx handles are not free-threaded).
    pub(crate) _not_thread_safe: std::marker::PhantomData<*mut u8>,
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

    /// Transfer ownership of the raw PMIx info array to the caller.
    ///
    /// After this call, [`Drop`] will not free the handle — the caller must
    /// free it with `PMIx_Info_free` (or equivalent).
    pub fn into_raw(self) -> (*mut pmix_info_t, usize) {
        let handle = self.handle;
        let len = self.len;
        std::mem::forget(self);
        (handle, len)
    }
}

impl Drop for Info {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: handle came from PMIx_Info_create(len) in InfoBuilder /
            // info_with_string_key. PMIx_Info_free destructs and frees the array.
            unsafe {
                PMIx_Info_free(self.handle, self.len);
            }
            self.handle = std::ptr::null_mut();
            self.len = 0;
        }
    }
}

struct InfoEntry {
    key: &'static [u8; 13],
    value: *const std::ffi::c_void,
    data_type: pmix_data_type_t,
}

struct InfoEntryString {
    key: CString,
    value: CString,
    data_type: pmix_data_type_t,
}

/// String-key `PMIX_BOOL` entry with owned storage (`uint8_t` 0/1).
///
/// OpenPMIx loads `PMIX_BOOL` from a pointer to a byte, not a C string. Using
/// `add_string_key(..., "1", PMIX_BOOL)` mis-encodes the value and attributes
/// such as `PMIX_EXTERNAL_PROGRESS` are ignored (internal progress still runs).
struct InfoEntryBool {
    key: CString,
    /// `1` = true, `0` = false — matches PMIx `PMIX_BOOL` packing.
    value: u8,
}

/// String-key `PMIX_INT` entry with owned storage for long-key integer attributes.
struct InfoEntryInt {
    key: CString,
    value: i32,
}

#[derive(Default)]
pub struct InfoBuilder {
    infos: Vec<InfoEntry>,
    /// String-key entries for keys that exceed the 13-byte limit.
    string_infos: Vec<InfoEntryString>,
    /// String-key boolean attributes (`pmix.evext`, `pmix.bind.reqd`, …).
    bool_infos: Vec<InfoEntryBool>,
    /// String-key integer attributes such as `pmix.exit.code`.
    int_infos: Vec<InfoEntryInt>,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
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

    /// Add an info entry with a string key that exceeds the 13-byte limit.
    ///
    /// Use this for keys like `PMIX_BIND_REQUIRED` (`pmix.bind.reqd`, 15 bytes)
    /// which don't fit in [`InfoBuilder::add`].
    /// Returns `Err(NulError)` if the key or value contains a NUL byte.
    pub fn add_string_key(
        &mut self,
        key: &str,
        value: &str,
        data_type: pmix_data_type_t,
    ) -> Result<&mut Self, std::ffi::NulError> {
        let key_cstr = CString::new(key)?;
        let value_cstr = CString::new(value)?;
        self.string_infos.push(InfoEntryString {
            key: key_cstr,
            value: value_cstr,
            data_type,
        });
        Ok(self)
    }

    /// Append a string-key `PMIX_BOOL` attribute with correct scalar encoding.
    ///
    /// Use this for boolean-valued keys that exceed the 13-byte limit (e.g.
    /// `PMIX_QUERY_REFRESH_CACHE` = "pmix.qry.rfsh") which don't fit in
    /// [`InfoBuilder::add`] and can't be expressed as a string value.
    ///
    /// # Panics
    /// Panics if `key` contains a NUL byte (keys are static attribute names).
    pub fn add_bool_key(&mut self, key: &str, value: bool) -> &mut Self {
        let key_cstr = CString::new(key).expect("key must not contain null bytes");
        self.bool_infos.push(InfoEntryBool {
            key: key_cstr,
            value: u8::from(value),
        });
        self
    }

    /// Append a string-key `PMIX_INT` attribute with correct scalar encoding.
    ///
    /// Use this for integer-valued keys that exceed the 13-byte limit (e.g.
    /// `PMIX_EXIT_CODE` = `"pmix.exit.code"`) which don't fit in
    /// [`InfoBuilder::add`] and can't be expressed as a string value.
    /// Returns `Err(NulError)` if the key contains a NUL byte.
    pub fn add_int_key(&mut self, key: &str, value: i32) -> Result<&mut Self, std::ffi::NulError> {
        let key_cstr = CString::new(key)?;
        self.int_infos.push(InfoEntryInt {
            key: key_cstr,
            value,
        });
        Ok(self)
    }

    /// Set `PMIX_EXTERNAL_PROGRESS` attribute.
    ///
    /// When `true`, tells PMIx that the host provides its own event
    /// progress engine. **WARNING:** if you set this to `true` without
    /// calling [`progress()`] from your own event loop, the PMIx library
    /// will deadlock on blocking operations, as no internal progress
    /// thread is spawned.
    ///
    /// # C API
    /// `PMIX_EXTERNAL_PROGRESS` (`pmix.evext`) — `PMIX_BOOL`
    pub fn external_progress(&mut self, external: bool) -> &mut Self {
        self.add_bool_key("pmix.evext", external)
    }

    /// Set `PMIX_GET_REFRESH_CACHE` attribute.
    ///
    /// When `true`, instructs PMIx to automatically refresh its local cache
    /// before returning the requested value. This is useful when you need
    /// the most up-to-date data without manually querying again.
    ///
    /// # C API
    /// `PMIX_GET_REFRESH_CACHE` (`pmix.get.refresh`) — `PMIX_BOOL`
    ///
    /// # Example
    /// ```rust
    /// use pmix::InfoBuilder;
    /// let mut builder = InfoBuilder::new();
    /// builder.refresh_cache(true);
    /// let info = builder.build().expect("info");
    /// assert_eq!(info.len(), 1);
    /// ```
    pub fn refresh_cache(&mut self, refresh: bool) -> &mut Self {
        self.add_bool_key("pmix.get.refresh", refresh)
    }

    /// Set `PMIX_BIND_PROGRESS_THREAD` attribute.
    ///
    /// Specifies CPU ranges on which to bind the internal PMIx progress
    /// thread. The value is a CPU set string (e.g., `"0-3"`, `"4,5,6,7"`).
    ///
    /// **Note:** this only pins the PMIx **progress thread**, not the
    /// thread that calls `PMIx_Get` or other APIs — work is threadshifted
    /// onto the progress/`evbase` internally.
    ///
    /// Returns `Err(NulError)` if `cpus` contains a NUL byte.
    ///
    /// # C API
    /// `PMIX_BIND_PROGRESS_THREAD` (`pmix.bind.pt`) — `PMIX_STRING`
    pub fn bind_progress_thread(&mut self, cpus: &str) -> Result<&mut Self, std::ffi::NulError> {
        let cpus_cstr = CString::new(cpus)?;
        self.string_infos.push(InfoEntryString {
            key: CString::new("pmix.bind.pt").unwrap(),
            value: cpus_cstr,
            data_type: PMIX_STRING as pmix_data_type_t,
        });
        Ok(self)
    }

    /// Set `PMIX_BIND_REQUIRED` attribute.
    ///
    /// When `true`, the PMIx progress thread binding is mandatory —
    /// initialization fails if the requested CPUs are unavailable.
    ///
    /// # C API
    /// `PMIX_BIND_REQUIRED` (`pmix.bind.reqd`) — `PMIX_BOOL`
    pub fn bind_required(&mut self, required: bool) -> &mut Self {
        self.add_bool_key("pmix.bind.reqd", required)
    }

    /// Set `PMIX_PROGRESS_THREAD_FLUSH` attribute.
    ///
    /// Controls whether the progress thread flushes pending events before
    /// stopping. Pass `true` to force a flush of the event base before
    /// shutdown.
    ///
    /// # C API
    /// `PMIX_PROGRESS_THREAD_FLUSH` (`pmix.evflush`) — `PMIX_BOOL`
    pub fn progress_thread_flush(&mut self, flush: bool) -> &mut Self {
        self.add_bool_key("pmix.evflush", flush)
    }

    /// Set `PMIX_PROGRESS_THREAD_NAME` attribute.
    ///
    /// Gives the internal PMIx progress thread a custom name for debugging
    /// and profiling. The name is visible in thread listings (e.g.,
    /// `pthread_getname_np`, debuggers, `ps`).
    ///
    /// Returns `Err(NulError)` if `name` contains a NUL byte.
    ///
    /// # C API
    /// `PMIX_PROGRESS_THREAD_NAME` (`pmix.evname`) — `PMIX_STRING`
    pub fn progress_thread_name(&mut self, name: &str) -> Result<&mut Self, std::ffi::NulError> {
        self.add_string_key("pmix.evname", name, PMIX_STRING as pmix_data_type_t)
    }

    pub fn collect_data(&mut self) -> &mut InfoBuilder {
        // Owned storage: `add` only stores a raw pointer, so a stack bool
        // would dangle by `build()`. Use bool_infos for correct lifetime.
        self.add_bool_key("pmix.collect", true)
    }
    pub fn build(self) -> Result<Info, PmixStatus> {
        let n = self.infos.len()
            + self.string_infos.len()
            + self.bool_infos.len()
            + self.int_infos.len();
        // SAFETY: PMIx allocates an array of `n` initialized info records.
        let info_ptr = unsafe { PMIx_Info_create(n) };
        if info_ptr.is_null() && n > 0 {
            return Err(PmixStatus::from_raw(ffi::PMIX_ERR_NOMEM));
        }
        let mut idx: usize = 0;

        // Process fixed-key entries (13-byte keys)
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
                unsafe {
                    PMIx_Info_free(info_ptr, n);
                }
                return Err(PmixStatus::from_raw(status));
            }
            idx += 1;
        }

        // Process string-key entries (keys exceeding 13-byte limit)
        for info in &self.string_infos {
            let status = unsafe {
                PMIx_Info_load(
                    info_ptr.add(idx),
                    info.key.as_ptr(),
                    info.value.as_ptr() as *const c_void,
                    info.data_type,
                )
            };
            if status != PMIX_SUCCESS as i32 {
                unsafe {
                    PMIx_Info_free(info_ptr, n);
                }
                return Err(PmixStatus::from_raw(status));
            }
            idx += 1;
        }

        // Process string-key boolean attributes with owned u8 storage.
        for info in &self.bool_infos {
            let status = unsafe {
                PMIx_Info_load(
                    info_ptr.add(idx),
                    info.key.as_ptr(),
                    (&info.value as *const u8).cast(),
                    PMIX_BOOL as pmix_data_type_t,
                )
            };
            if status != PMIX_SUCCESS as i32 {
                unsafe {
                    PMIx_Info_free(info_ptr, n);
                }
                return Err(PmixStatus::from_raw(status));
            }
            idx += 1;
        }

        // Process string-key integer attributes with owned i32 storage.
        for info in &self.int_infos {
            let status = unsafe {
                PMIx_Info_load(
                    info_ptr.add(idx),
                    info.key.as_ptr(),
                    (&info.value as *const i32).cast(),
                    PMIX_INT as pmix_data_type_t,
                )
            };
            if status != PMIX_SUCCESS as i32 {
                unsafe {
                    PMIx_Info_free(info_ptr, n);
                }
                return Err(PmixStatus::from_raw(status));
            }
            idx += 1;
        }

        Ok(Info {
            handle: info_ptr,
            len: idx,
            _not_thread_safe: std::marker::PhantomData,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// InitOptions — builder for PMIx_Init options
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration options for [`init`].
///
/// Use this to configure PMIx initialization parameters in a type-safe way.
/// Call [`build`][InitOptions::build] to produce an [`Info`] array suitable
/// for passing to [`init`].
///
/// # Example
///
/// ```ignore
/// use pmix::InitOptions;
///
/// let mut options = InitOptions::new();
/// options.external_progress(true);
/// options.bind_progress_thread("0-3");
///
/// let info = options.build().expect("build info");
/// let handle = pmix::init(None, None, info)?;
/// ```
///
/// # External progress warning
///
/// When `external_progress(true)` is set, PMIx does **not** spawn an
/// internal progress thread. You **must** call [`progress`] periodically
/// from your own event loop, or PMIx will deadlock on any blocking
/// operation (e.g., `PMIx_Get`, `PMIx_Fence`). If in doubt, leave
/// external progress disabled (the default).
#[derive(Default, Debug)]
pub struct InitOptions {
    external_progress: Option<bool>,
    bind_progress_thread: Option<String>,
    bind_required: Option<bool>,
    progress_thread_flush: Option<bool>,
    progress_thread_name: Option<String>,
}

impl InitOptions {
    /// Create a new, empty `InitOptions` builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `PMIX_EXTERNAL_PROGRESS`.
    ///
    /// When `true`, tells PMIx that the host provides its own event
    /// progress engine. **WARNING:** if you set this to `true` without
    /// calling [`progress`] from your own event loop, the PMIx library
    /// will deadlock on blocking operations, as no internal progress
    /// thread is spawned.
    ///
    /// # C API
    /// `PMIX_EXTERNAL_PROGRESS` (`pmix.evext`) — `PMIX_BOOL`
    pub fn external_progress(&mut self, external: bool) -> &mut Self {
        self.external_progress = Some(external);
        self
    }

    /// Set `PMIX_BIND_PROGRESS_THREAD`.
    ///
    /// Specifies CPU ranges on which to bind the internal PMIx progress
    /// thread. The value is a CPU set string (e.g., `"0-3"`, `"4,5,6,7"`).
    ///
    /// **Note:** this only pins the PMIx **progress thread**, not the
    /// thread that calls `PMIx_Get` or other APIs — work is threadshifted
    /// onto the progress/`evbase` internally.
    ///
    /// # C API
    /// `PMIX_BIND_PROGRESS_THREAD` (`pmix.bind.pt`) — `PMIX_STRING`
    pub fn bind_progress_thread(&mut self, cpus: &str) -> &mut Self {
        self.bind_progress_thread = Some(cpus.to_string());
        self
    }

    /// Set `PMIX_BIND_REQUIRED`.
    ///
    /// When `true`, the PMIx progress thread binding is mandatory —
    /// initialization fails if the requested CPUs are unavailable.
    ///
    /// # C API
    /// `PMIX_BIND_REQUIRED` (`pmix.bind.reqd`) — `PMIX_BOOL`
    pub fn bind_required(&mut self, required: bool) -> &mut Self {
        self.bind_required = Some(required);
        self
    }

    /// Set `PMIX_PROGRESS_THREAD_FLUSH`.
    ///
    /// When `true`, the progress thread flushes pending events before
    /// stopping. Useful for ensuring in-flight operations complete during
    /// shutdown.
    ///
    /// # C API
    /// `PMIX_PROGRESS_THREAD_FLUSH` (`pmix.evflush`) — `PMIX_BOOL`
    pub fn progress_thread_flush(&mut self, flush: bool) -> &mut Self {
        self.progress_thread_flush = Some(flush);
        self
    }

    /// Set `PMIX_PROGRESS_THREAD_NAME`.
    ///
    /// Gives the internal PMIx progress thread a custom name for debugging
    /// and profiling. Visible in thread listings and debuggers.
    ///
    /// # C API
    /// `PMIX_PROGRESS_THREAD_NAME` (`pmix.evname`) — `PMIX_STRING`
    pub fn progress_thread_name(&mut self, name: &str) -> &mut Self {
        self.progress_thread_name = Some(name.to_string());
        self
    }

    /// Build an [`Info`] array from the configured options.
    ///
    /// Returns `Ok([`Info`])` suitable for passing to [`init`], or
    /// `Err(PmixStatus)` if the info array cannot be allocated or loaded.
    /// Unset options are simply omitted from the resulting array.
    pub fn build(&self) -> Result<Info, PmixStatus> {
        let mut builder = InfoBuilder::new();

        if let Some(external) = self.external_progress {
            builder.external_progress(external);
        }

        if let Some(ref cpus) = self.bind_progress_thread {
            builder
                .bind_progress_thread(cpus)
                .map_err(|_| PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM))?;
        }

        if let Some(required) = self.bind_required {
            builder.bind_required(required);
        }

        if let Some(flush) = self.progress_thread_flush {
            builder.progress_thread_flush(flush);
        }

        if let Some(ref name) = self.progress_thread_name {
            builder
                .progress_thread_name(name)
                .map_err(|_| PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM))?;
        }

        builder.build()
    }
}

/// Create an `Info` array with a single string key/value pair, bypassing
/// the 13-byte key limit of `InfoBuilder::add()`.
///
/// This is useful for keys like `"pmix.srvr.uri"` (14 bytes) which don't
/// fit in `InfoBuilder::add(key: &'static [u8; 13])`.
pub fn info_with_string_key(key: &str, value: &str) -> Result<Info, PmixStatus> {
    let key_cstr = CString::new(key).map_err(|_| PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM))?;
    let value_cstr =
        CString::new(value).map_err(|_| PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM))?;
    // SAFETY: PMIx allocates one initialized info record and returns a null
    // pointer on allocation failure.
    let info_ptr = unsafe { PMIx_Info_create(1) };
    if info_ptr.is_null() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_NOMEM));
    }
    // PMIx_Info_load copies key/value into the info array (PMIx 4+/5+/6+).
    // Keep CStrings alive only for the duration of the load call, then drop.
    // SAFETY: `info_ptr` is a valid PMIx info array, and the CString pointers
    // remain valid for the duration of the call.
    let status = unsafe {
        PMIx_Info_load(
            info_ptr,
            key_cstr.as_ptr(),
            value_cstr.as_ptr() as *const c_void,
            PMIX_STRING as pmix_data_type_t,
        )
    };
    // CStrings drop here — PMIx has already copied the bytes into the info array.
    drop(key_cstr);
    drop(value_cstr);
    if status != PMIX_SUCCESS as i32 {
        // Free the half-built array before returning the PMIx error.
        // SAFETY: `info_ptr` was allocated by PMIx_Info_create(1) above and
        // remains valid until it is freed here.
        unsafe {
            PMIx_Info_free(info_ptr, 1);
        }
        return Err(PmixStatus::from_raw(status));
    }
    Ok(Info {
        handle: info_ptr,
        len: 1,
        _not_thread_safe: std::marker::PhantomData,
    })
}
