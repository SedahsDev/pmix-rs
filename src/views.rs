//! Borrowed, zero-cost views over PMIx C structs (issues #520 / #541).
//!
//! PMIx host upcalls and event deliveries hand the caller raw C arrays
//! (`pmix_info_t *`, `pmix_proc_t *`, …). This module provides
//! `#[repr(transparent)]` view types over those structs plus the
//! `view_slice` / `view_slice_mut` helpers that turn a raw C pointer/length
//! pair into a native slice. Views borrow: they never free, copy, or retain
//! the underlying memory, and they are only valid while the C side keeps the
//! array alive (typically for the duration of the upcall).
//!
//! # Why `#[repr(transparent)]` is required
//!
//! `view_slice` builds a `&[InfoView]` (etc.) from a C array with
//! [`std::slice::from_raw_parts`]. That is only sound when the view type has
//! exactly the same size, alignment, and field layout as the C struct it
//! views — which `#[repr(transparent)]` guarantees for a single-field
//! newtype. Without it the slice cast would reinterpret memory with a
//! different layout.

use crate::enums::PmixDataType;
use crate::ffi;
use std::ffi::CStr;
use std::os::raw::c_char;

// ─────────────────────────────────────────────────────────────────────────────
// slice construction helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a borrowed slice of views from a raw C array.
///
/// A null pointer (with any length) maps to an empty slice — PMIx uses
/// `(NULL, 0)` for "no entries".
///
/// # Safety
///
/// `ptr` must be null or point to `len` initialized C elements of the
/// underlying type, and `V` must be `#[repr(transparent)]` over that type
/// (every view in this module is). The slice must not outlive the C
/// allocation.
pub(crate) unsafe fn view_slice<'a, V>(ptr: *const V, len: usize) -> &'a [V] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: the caller guarantees `len` initialized elements behind a
        // non-null, aligned pointer; V is repr(transparent) over the element.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Build a mutable slice of views from a raw C array.
///
/// A null pointer (with any length) maps to an empty slice.
///
/// # Safety
///
/// As [`view_slice`]; additionally no other reference to the array may exist
/// for the lifetime of the returned slice.
pub(crate) unsafe fn view_slice_mut<'a, V>(ptr: *mut V, len: usize) -> &'a mut [V] {
    if ptr.is_null() || len == 0 {
        &mut []
    } else {
        // SAFETY: the caller guarantees exclusive access to `len` initialized
        // elements behind a non-null, aligned pointer; V is
        // repr(transparent) over the element.
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

/// Read a NUL-terminated C string from a fixed-size key/nspace array.
fn fixed_str(bytes: &[c_char]) -> &str {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    // SAFETY: reading `end` bytes of the fixed-size array as `u8` is within
    // its allocation; PMIx keys and nspaces are NUL-terminated UTF-8.
    let raw = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), end) };
    std::str::from_utf8(raw).unwrap_or("")
}

// ─────────────────────────────────────────────────────────────────────────────
// InfoView — pmix_info_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_info_t` key/value entry.
///
/// Constructed by the crate at the FFI boundary (host upcalls, event
/// results) or borrowed from an owned [`Info`](crate::Info) array with
/// [`Info::view`](crate::Info::view). Never free a view — it borrows the
/// underlying C memory.
#[repr(transparent)]
pub struct InfoView {
    raw: ffi::pmix_info_t,
}

impl InfoView {
    /// View a single raw `pmix_info_t`.
    ///
    /// # Safety
    /// `ptr` must be null-free for this call: point to one initialized
    /// element that stays alive as long as the returned reference.
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_info_t) -> &'a Self {
        // SAFETY: InfoView is repr(transparent) over pmix_info_t, so the
        // pointer representation is identical; the caller guarantees the
        // pointee is initialized and outlives the borrow.
        unsafe { &*ptr.cast::<Self>() }
    }

    /// Wrap a by-value `pmix_info_t`.
    ///
    /// # Shallow-copy aliasing
    ///
    /// `from_value` copies only the POD (`key`, `flags`, `value` header).
    /// Any inner pointers of the copied `value` union (e.g. a PMIX_STRING
    /// `string` pointer) alias the original entry's allocations. The view
    /// is therefore only sound while those original allocations live —
    /// never free or mutate the source entry while a `from_value` view
    /// borrows from it.
    #[allow(dead_code)] // no 5.x/6.x server slot passes results by value; kept for by-value info copies
    pub(crate) fn from_value(raw: ffi::pmix_info_t) -> Self {
        Self { raw }
    }

    /// The info key (NUL-terminated attribute name).
    pub fn key(&self) -> &str {
        fixed_str(&self.raw.key)
    }

    /// The `pmix_info_directives_t` flags word.
    pub fn flags(&self) -> u32 {
        self.raw.flags
    }

    /// The declared type of [`Self::value`](Self).
    pub fn value_type(&self) -> PmixDataType {
        PmixDataType::from_raw(self.raw.value.type_)
    }

    /// `PMIX_BOOL` value, or `None` when the entry is not boolean.
    pub fn as_bool(&self) -> Option<bool> {
        if self.value_type() != PmixDataType::Bool {
            return None;
        }
        // SAFETY: value_type() is PMIX_BOOL, so the `flag` union arm is the
        // active one.
        Some(unsafe { self.raw.value.data.flag })
    }

    /// `PMIX_STRING` value, or `None` when the entry is not a string or the
    /// string pointer is null.
    ///
    /// A PMIX_STRING entry with a null `string` arm is not an error in PMIx;
    /// this returns `None` instead of dereferencing the null pointer
    /// (`CStr::from_ptr` on null is undefined behavior).
    pub fn as_str(&self) -> Option<&str> {
        if self.value_type() != PmixDataType::String {
            return None;
        }
        // SAFETY: value_type() is PMIX_STRING, so the `string` union arm is
        // the active one; reading it (even as a null check) is sound.
        if unsafe { self.raw.value.data.string }.is_null() {
            return None;
        }
        // SAFETY: value_type() is PMIX_STRING and the pointer is non-null, so
        // the `string` union arm is a valid NUL-terminated pointer for the
        // lifetime of the entry.
        unsafe { CStr::from_ptr(self.raw.value.data.string) }
            .to_str()
            .ok()
    }

    /// `PMIX_UINT32` value, or `None` when the entry is not a `uint32`.
    pub fn as_u32(&self) -> Option<u32> {
        if self.value_type() != PmixDataType::Uint32 {
            return None;
        }
        // SAFETY: value_type() is PMIX_UINT32 — the `uint32` arm is active.
        Some(unsafe { self.raw.value.data.uint32 })
    }

    /// `PMIX_SIZE` value, or `None` when the entry is not a `size_t`.
    pub fn as_usize(&self) -> Option<usize> {
        if self.value_type() != PmixDataType::Size {
            return None;
        }
        // SAFETY: value_type() is PMIX_SIZE — the `size` arm is active.
        Some(unsafe { self.raw.value.data.size })
    }
}

impl std::fmt::Debug for InfoView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InfoView")
            .field("key", &self.key())
            .field("value_type", &self.value_type())
            .finish_non_exhaustive()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ProcView — pmix_proc_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_proc_t` process identifier.
#[repr(transparent)]
pub struct ProcView {
    raw: ffi::pmix_proc_t,
}

impl ProcView {
    /// View a single raw `pmix_proc_t`.
    ///
    /// # Safety
    /// `ptr` must point to one initialized element that stays alive as long
    /// as the returned reference.
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_proc_t) -> &'a Self {
        // SAFETY: repr(transparent) over pmix_proc_t; caller guarantees
        // initialization and lifetime.
        unsafe { &*ptr.cast::<Self>() }
    }

    /// Borrow a view of an owned [`Proc`](crate::Proc) handle.
    ///
    /// The view borrows the handle: it must not outlive the `Proc`.
    pub fn from_proc(proc: &crate::Proc) -> &Self {
        // SAFETY: `handle` is the first field of `Proc` and ProcView is
        // repr(transparent) over that field's type, so casting a pointer to
        // the field to a pointer to the view preserves layout. The returned
        // reference is bound to `proc`'s lifetime.
        unsafe { Self::from_raw_ptr(&proc.handle) }
    }

    /// The process namespace.
    pub fn nspace(&self) -> &str {
        fixed_str(&self.raw.nspace)
    }

    /// The process rank.
    pub fn rank(&self) -> u32 {
        self.raw.rank
    }

    /// Raw pointer for completion callbacks that take a `pmix_proc_t *`.
    pub(crate) fn raw_ptr(&self) -> *const ffi::pmix_proc_t {
        &self.raw
    }
}

impl std::fmt::Debug for ProcView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProcView")
            .field("nspace", &self.nspace())
            .field("rank", &self.rank())
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NspaceView — pmix_nspace_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_nspace_t` namespace identifier.
#[repr(transparent)]
pub struct NspaceView {
    raw: ffi::pmix_nspace_t,
}

impl NspaceView {
    /// View a single raw `pmix_nspace_t`.
    ///
    /// # Safety
    /// `ptr` must point to one initialized element that stays alive as long
    /// as the returned reference.
    #[cfg_attr(not(test), allow(dead_code))] // no 5.x/6.x slot slices nspace arrays; used by unit tests
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_nspace_t) -> &'a Self {
        // SAFETY: repr(transparent) over the fixed-size C array type; the
        // caller guarantees initialization and lifetime.
        unsafe { &*ptr.cast::<Self>() }
    }

    /// The namespace string.
    pub fn name(&self) -> &str {
        fixed_str(&self.raw)
    }

    /// Copy out the raw C array (for completion callbacks taking the
    /// `pmix_nspace_t` by value).
    pub(crate) fn raw_copy(&self) -> ffi::pmix_nspace_t {
        self.raw
    }
}

impl std::fmt::Debug for NspaceView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NspaceView")
            .field("name", &self.name())
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PdataView — pmix_pdata_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_pdata_t` lookup target.
#[repr(transparent)]
pub struct PdataView {
    raw: ffi::pmix_pdata_t,
}

impl PdataView {
    /// View a single raw `pmix_pdata_t`.
    ///
    /// # Safety
    /// `ptr` must point to one initialized element that stays alive as long
    /// as the returned reference.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_pdata_t) -> &'a Self {
        // SAFETY: repr(transparent) over pmix_pdata_t; caller guarantees
        // initialization and lifetime.
        unsafe { &*ptr.cast::<Self>() }
    }

    /// The target process's namespace.
    pub fn nspace(&self) -> &str {
        fixed_str(&self.raw.proc_.nspace)
    }

    /// The target process's rank.
    pub fn rank(&self) -> u32 {
        self.raw.proc_.rank
    }

    /// The key to look up.
    pub fn key(&self) -> &str {
        fixed_str(&self.raw.key)
    }

    /// The declared type of the expected value.
    pub fn value_type(&self) -> PmixDataType {
        PmixDataType::from_raw(self.raw.value.type_)
    }
}

impl std::fmt::Debug for PdataView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PdataView")
            .field("nspace", &self.nspace())
            .field("rank", &self.rank())
            .field("key", &self.key())
            .finish_non_exhaustive()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AppView — pmix_app_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_app_t` spawn request.
#[repr(transparent)]
pub struct AppView {
    raw: ffi::pmix_app_t,
}

impl AppView {
    /// View a single raw `pmix_app_t`.
    ///
    /// # Safety
    /// `ptr` must point to one initialized element that stays alive as long
    /// as the returned reference.
    #[cfg_attr(not(test), allow(dead_code))] // the spawn trampoline slices whole arrays via `view_slice`
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_app_t) -> &'a Self {
        // SAFETY: repr(transparent) over pmix_app_t; caller guarantees
        // initialization and lifetime.
        unsafe { &*ptr.cast::<Self>() }
    }

    fn c_str<'a>(ptr: *const c_char) -> Option<&'a str> {
        if ptr.is_null() {
            return None;
        }
        // SAFETY: PMIx app strings are NUL-terminated and owned by the C
        // side for the duration of the upcall.
        unsafe { CStr::from_ptr(ptr) }.to_str().ok()
    }

    /// The executable command (may be unset).
    pub fn cmd(&self) -> Option<&str> {
        Self::c_str(self.raw.cmd)
    }

    /// The working directory (may be unset).
    pub fn cwd(&self) -> Option<&str> {
        Self::c_str(self.raw.cwd)
    }

    /// The maximum number of processes to spawn for this app.
    pub fn maxprocs(&self) -> i32 {
        self.raw.maxprocs
    }

    /// The app's info entries.
    pub fn info(&self) -> &[InfoView] {
        // SAFETY: PMIx supplies the app's initialized info array of `ninfo`
        // entries for the duration of the upcall; null maps to empty.
        unsafe { view_slice(self.raw.info.cast::<InfoView>(), self.raw.ninfo) }
    }

    /// The command-line arguments (`argv`).
    ///
    /// The returned strings borrow the C `argv` array that backs this
    /// view: the borrow is tied to `&self`, and the strings are only valid
    /// while the C side keeps the array alive (the duration of the
    /// upcall). Copy any string you need to keep before the upcall
    /// returns.
    pub fn argv(&self) -> Vec<&str> {
        // SAFETY: PMIx app argv arrays are NULL-terminated arrays of
        // NUL-terminated strings owned by the C side for the duration of
        // the upcall that backs this view.
        unsafe { read_str_array(self.raw.argv) }
    }

    /// The environment strings (`env`).
    ///
    /// Same borrow rules as [`Self::argv`]: valid only while the C `env`
    /// array lives (the duration of the upcall).
    pub fn env(&self) -> Vec<&str> {
        // SAFETY: PMIx app env arrays are NULL-terminated arrays of
        // NUL-terminated strings owned by the C side for the duration of
        // the upcall that backs this view.
        unsafe { read_str_array(self.raw.env) }
    }
}

/// Read a NULL-terminated C string-pointer array (`char **`) as borrowed
/// Rust strings.
///
/// # Safety contract
///
/// * `array` must be null or point to a NULL-terminated array of
///   NUL-terminated, UTF-8 C strings that stay alive for the chosen
///   lifetime `'a` (in practice: for the duration of the upcall that
///   owns them — hence the caller-provided lifetime, never `'static`).
/// * The loop stops at the first NULL array entry; an array without a
///   NULL terminator is undefined behavior on the C side and cannot be
///   recovered here. PMIx guarantees termination for `argv` / `env` /
///   `keys` arrays.
/// * Entries that are null or not valid UTF-8 are skipped, and reading
///   stops at the first such entry.
pub(crate) unsafe fn read_str_array<'a>(array: *mut *mut c_char) -> Vec<&'a str> {
    let mut out = Vec::new();
    if array.is_null() {
        return out;
    }
    let mut index = 0;
    // SAFETY: the caller guarantees a NULL-terminated pointer array whose
    // entries are NUL-terminated UTF-8 strings for lifetime `'a`.
    unsafe {
        while let Some(entry) = AppView::c_str(*array.add(index)) {
            out.push(entry);
            index += 1;
        }
    }
    out
}

impl std::fmt::Debug for AppView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppView")
            .field("cmd", &self.cmd())
            .field("maxprocs", &self.maxprocs())
            .finish_non_exhaustive()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QueryView — pmix_query_t
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_query_t` server query.
#[repr(transparent)]
pub struct QueryView {
    raw: ffi::pmix_query_t,
}

impl QueryView {
    /// View a single raw `pmix_query_t`.
    ///
    /// # Safety
    /// `ptr` must point to one initialized element that stays alive as long
    /// as the returned reference.
    #[cfg_attr(not(test), allow(dead_code))] // the query trampoline slices whole arrays via `view_slice`
    pub(crate) unsafe fn from_raw_ptr<'a>(ptr: *const ffi::pmix_query_t) -> &'a Self {
        // SAFETY: repr(transparent) over pmix_query_t; caller guarantees
        // initialization and lifetime.
        unsafe { &*ptr.cast::<Self>() }
    }

    /// The query keys (a NULL-terminated C array of strings).
    ///
    /// The returned strings borrow the C `keys` array: the borrow is tied
    /// to `&self` and is only valid for the duration of the upcall.
    pub fn keys(&self) -> Vec<&str> {
        // SAFETY: PMIx query key arrays are NULL-terminated arrays of
        // NUL-terminated strings owned by the C side for the duration of
        // the upcall that backs this view.
        unsafe { read_str_array(self.raw.keys) }
    }

    /// The query qualifiers.
    pub fn qualifiers(&self) -> &[InfoView] {
        // SAFETY: PMIx supplies the query's initialized qualifier array of
        // `nqual` entries for the duration of the upcall; null maps to empty.
        unsafe { view_slice(self.raw.qualifiers.cast::<InfoView>(), self.raw.nqual) }
    }
}

impl std::fmt::Debug for QueryView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QueryView")
            .field("keys", &self.keys())
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ResourceUnitView — pmix_resource_unit_t (PMIx 6.x)
// ─────────────────────────────────────────────────────────────────────────────

/// Borrowed view over a `ffi::pmix_resource_unit_t` resource-block unit
/// (PMIx 6.x `resource_block` upcall only).
#[cfg(pmix6)]
#[repr(transparent)]
pub struct ResourceUnitView {
    raw: ffi::pmix_resource_unit_t,
}

#[cfg(pmix6)]
impl ResourceUnitView {
    /// The unit's device type bitmask (`pmix_device_type_t`).
    pub fn device_type(&self) -> u64 {
        self.raw.type_
    }

    /// The number of units of this type.
    pub fn count(&self) -> usize {
        self.raw.count
    }
}

#[cfg(pmix6)]
impl std::fmt::Debug for ResourceUnitView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResourceUnitView")
            .field("device_type", &self.device_type())
            .field("count", &self.count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn key_str(raw: &mut ffi::pmix_info_t, key: &str) {
        for (dst, src) in raw.key.iter_mut().zip(key.as_bytes().iter()) {
            *dst = *src as c_char;
        }
    }

    fn set_type(raw: &mut ffi::pmix_value_t, ty: crate::ffi::pmix_data_type_t) {
        raw.type_ = ty;
    }

    #[test]
    fn info_view_reads_key_type_and_value() {
        let cvalue = CString::new("demo-value").expect("cstring");
        let mut raw: ffi::pmix_info_t = unsafe { std::mem::zeroed() };
        key_str(&mut raw, "pmix.demo");
        set_type(
            &mut raw.value,
            crate::ffi::PMIX_STRING as crate::ffi::pmix_data_type_t,
        );
        // SAFETY: PMIX_STRING arm becomes the active union arm.
        raw.value.data.string = cvalue.into_raw();

        // SAFETY: `raw` is an initialized pmix_info_t on the stack.
        let view = unsafe { InfoView::from_raw_ptr(&raw) };
        assert_eq!(view.key(), "pmix.demo");
        assert_eq!(view.value_type(), PmixDataType::String);
        assert_eq!(view.as_str(), Some("demo-value"));
        assert_eq!(view.as_bool(), None);
        assert_eq!(view.as_u32(), None);
    }

    #[test]
    fn info_view_reads_bool_and_uint32() {
        let mut raw: ffi::pmix_info_t = unsafe { std::mem::zeroed() };
        key_str(&mut raw, "pmix.flag");
        set_type(
            &mut raw.value,
            crate::ffi::PMIX_BOOL as crate::ffi::pmix_data_type_t,
        );
        // SAFETY: PMIX_BOOL arm is active.
        raw.value.data.flag = true;
        // SAFETY: `raw` is initialized.
        assert!(unsafe { InfoView::from_raw_ptr(&raw) }.as_bool().is_some());

        let mut raw: ffi::pmix_info_t = unsafe { std::mem::zeroed() };
        key_str(&mut raw, "pmix.count");
        set_type(
            &mut raw.value,
            crate::ffi::PMIX_UINT32 as crate::ffi::pmix_data_type_t,
        );
        // SAFETY: PMIX_UINT32 arm is active.
        raw.value.data.uint32 = 42;
        // SAFETY: `raw` is initialized.
        let view = unsafe { InfoView::from_raw_ptr(&raw) };
        assert_eq!(view.as_u32(), Some(42));
        assert_eq!(view.as_bool(), None);
    }

    #[test]
    fn proc_view_reads_nspace_and_rank() {
        let mut raw: ffi::pmix_proc_t = unsafe { std::mem::zeroed() };
        for (dst, src) in raw.nspace.iter_mut().zip(b"job.7".iter()) {
            *dst = *src as c_char;
        }
        raw.rank = 3;
        // SAFETY: `raw` is an initialized pmix_proc_t.
        let view = unsafe { ProcView::from_raw_ptr(&raw) };
        assert_eq!(view.nspace(), "job.7");
        assert_eq!(view.rank(), 3);
    }

    #[test]
    fn nspace_view_reads_name() {
        let mut raw: ffi::pmix_nspace_t = unsafe { std::mem::zeroed() };
        for (dst, src) in raw.iter_mut().zip(b"ns.1".iter()) {
            *dst = *src as c_char;
        }
        // SAFETY: `raw` is initialized.
        assert_eq!(unsafe { NspaceView::from_raw_ptr(&raw) }.name(), "ns.1");
    }

    #[test]
    fn pdata_view_reads_proc_key_and_type() {
        let mut raw: ffi::pmix_pdata_t = unsafe { std::mem::zeroed() };
        for (dst, src) in raw.proc_.nspace.iter_mut().zip(b"job.9".iter()) {
            *dst = *src as c_char;
        }
        raw.proc_.rank = 5;
        for (dst, src) in raw.key.iter_mut().zip(b"pmix.target".iter()) {
            *dst = *src as c_char;
        }
        set_type(
            &mut raw.value,
            crate::ffi::PMIX_STRING as crate::ffi::pmix_data_type_t,
        );
        // SAFETY: `raw` is initialized.
        let view = unsafe { PdataView::from_raw_ptr(&raw) };
        assert_eq!(view.nspace(), "job.9");
        assert_eq!(view.rank(), 5);
        assert_eq!(view.key(), "pmix.target");
        assert_eq!(view.value_type(), PmixDataType::String);
    }

    #[test]
    fn app_view_reads_cmd_argv_and_info() {
        let cmd = CString::new("/bin/true").expect("cstring");
        let arg = CString::new("-v").expect("cstring");
        let mut raw: ffi::pmix_app_t = unsafe { std::mem::zeroed() };
        raw.cmd = cmd.into_raw();
        raw.maxprocs = 4;
        // SAFETY: build a NULL-terminated argv array on the stack.
        let mut argv = [arg.into_raw(), std::ptr::null_mut()];
        raw.argv = argv.as_mut_ptr();
        // SAFETY: `raw` is initialized for the duration of the borrow.
        let view = unsafe { AppView::from_raw_ptr(&raw) };
        assert_eq!(view.cmd(), Some("/bin/true"));
        assert_eq!(view.maxprocs(), 4);
        assert_eq!(view.argv(), vec!["-v"]);
        assert!(view.info().is_empty());
    }

    #[test]
    fn query_view_reads_keys_and_qualifiers() {
        let key = CString::new("pmix.qry.demo").expect("cstring");
        let mut raw: ffi::pmix_query_t = unsafe { std::mem::zeroed() };
        // SAFETY: build a NULL-terminated keys array on the stack.
        let mut keys = [key.into_raw(), std::ptr::null_mut()];
        raw.keys = keys.as_mut_ptr();
        // SAFETY: `raw` is initialized for the duration of the borrow.
        let view = unsafe { QueryView::from_raw_ptr(&raw) };
        assert_eq!(view.keys(), vec!["pmix.qry.demo"]);
        assert!(view.qualifiers().is_empty());
    }

    #[test]
    fn null_pointer_and_zero_len_map_to_empty_slice() {
        // SAFETY: null pointers are explicitly mapped to empty slices.
        assert!(unsafe { view_slice::<InfoView>(std::ptr::null(), 0) }.is_empty());
        assert!(unsafe { view_slice::<ProcView>(std::ptr::null(), 0) }.is_empty());
        assert!(unsafe { view_slice_mut::<InfoView>(std::ptr::null_mut(), 0) }.is_empty());
        // A null pointer with a nonzero length is also treated as empty.
        assert!(unsafe { view_slice::<InfoView>(std::ptr::null(), 7) }.is_empty());
    }

    #[test]
    fn view_slice_converts_pointer_and_length() {
        let mut raws: [ffi::pmix_info_t; 2] = unsafe { std::mem::zeroed() };
        key_str(&mut raws[0], "pmix.zero");
        key_str(&mut raws[1], "pmix.one");
        // SAFETY: `raws` is an initialized two-element array.
        let slice = unsafe { view_slice(raws.as_ptr().cast::<InfoView>(), 2) };
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].key(), "pmix.zero");
        assert_eq!(slice[1].key(), "pmix.one");
    }
}
