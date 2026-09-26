//! Native Rust wrappers for PMIx scalar types and enums.
//!
//! This module provides pure Rust types that wrap PMIx's C scalar types
//! (status, data types, scopes, etc.) to prevent FFI type leakage into
//! the public API. All types are `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`,
//! and have proper `From`/`TryFrom` conversions with their FFI counterparts.

use crate::ffi::{
    pmix_alloc_directive_t, pmix_data_range_t, pmix_data_type_t, pmix_iof_channel_t,
    pmix_proc_state_t, pmix_persistence_t, pmix_rank_t, pmix_scope_t, pmix_status_t,
    pmix_info_directives_t,
};

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus - wrapper for pmix_status_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_status_t`.
///
/// This is a newtype wrapper around `i32` that provides a safe Rust API
/// for PMIx status codes. It implements `From<i32>` for conversion from
/// raw C status codes and provides methods to check status categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub struct PmixStatus(pub i32);

impl PmixStatus {
    /// Create from raw `pmix_status_t` value.
    pub fn from_raw(value: pmix_status_t) -> Self {
        Self(value as i32)
    }

    /// Convert to raw `pmix_status_t` value.
    pub fn into_raw(self) -> pmix_status_t {
        self.0 as pmix_status_t
    }

    /// Check if this status indicates success.
    pub fn is_success(&self) -> bool {
        self.0 >= 0
    }

    /// Check if this status indicates an error.
    pub fn is_error(&self) -> bool {
        self.0 < 0
    }
}

impl From<i32> for PmixStatus {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl From<PmixStatus> for i32 {
    fn from(status: PmixStatus) -> Self {
        status.0
    }
}

impl From<PmixStatus> for pmix_status_t {
    fn from(status: PmixStatus) -> Self {
        status.into_raw()
    }
}

impl From<pmix_status_t> for PmixStatus {
    fn from(status: pmix_status_t) -> Self {
        Self::from_raw(status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataType - wrapper for pmix_data_type_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_data_type_t`.
///
/// Represents PMIx data type enumerators in a type-safe Rust API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub struct PmixDataType(pub u16);

impl PmixDataType {
    /// Create from raw `pmix_data_type_t` value.
    pub fn from_raw(value: pmix_data_type_t) -> Self {
        Self(value as u16)
    }

    /// Convert to raw `pmix_data_type_t` value.
    pub fn into_raw(self) -> pmix_data_type_t {
        self.0 as pmix_data_type_t
    }
}

impl From<u16> for PmixDataType {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<PmixDataType> for u16 {
    fn from(dt: PmixDataType) -> Self {
        dt.0
    }
}

impl From<PmixDataType> for pmix_data_type_t {
    fn from(dt: PmixDataType) -> Self {
        dt.into_raw()
    }
}

impl From<pmix_data_type_t> for PmixDataType {
    fn from(dt: pmix_data_type_t) -> Self {
        Self::from_raw(dt)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixRank - wrapper for pmix_rank_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_rank_t`.
///
/// Represents process rank in a type-safe Rust API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub struct PmixRank(pub u32);

impl PmixRank {
    /// Create from raw `pmix_rank_t` value.
    pub fn from_raw(value: pmix_rank_t) -> Self {
        Self(value as u32)
    }

    /// Convert to raw `pmix_rank_t` value.
    pub fn into_raw(self) -> pmix_rank_t {
        self.0 as pmix_rank_t
    }
}

impl From<u32> for PmixRank {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PmixRank> for u32 {
    fn from(rank: PmixRank) -> Self {
        rank.0
    }
}

impl From<PmixRank> for pmix_rank_t {
    fn from(rank: PmixRank) -> Self {
        rank.into_raw()
    }
}

impl From<pmix_rank_t> for PmixRank {
    fn from(rank: pmix_rank_t) -> Self {
        Self::from_raw(rank)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixScope - wrapper for pmix_scope_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_scope_t`.
///
/// Represents PMIx scope enumerators (local, global, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub struct PmixScope(pub u8);

impl PmixScope {
    /// Create from raw `pmix_scope_t` value.
    pub fn from_raw(value: pmix_scope_t) -> Self {
        Self(value as u8)
    }

    /// Convert to raw `pmix_scope_t` value.
    pub fn into_raw(self) -> pmix_scope_t {
        self.0 as pmix_scope_t
    }
}

impl From<u8> for PmixScope {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<PmixScope> for u8 {
    fn from(scope: PmixScope) -> Self {
        scope.0
    }
}

impl From<PmixScope> for pmix_scope_t {
    fn from(scope: PmixScope) -> Self {
        scope.into_raw()
    }
}

impl From<pmix_scope_t> for PmixScope {
    fn from(scope: pmix_scope_t) -> Self {
        Self::from_raw(scope)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixPersistence - wrapper for pmix_persistence_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_persistence_t`.
///
/// Represents PMIx persistence enumerators (temporary, persistent, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub struct PmixPersistence(pub u8);

impl PmixPersistence {
    /// Create from raw `pmix_persistence_t` value.
    pub fn from_raw(value: pmix_persistence_t) -> Self {
        Self(value as u8)
    }

    /// Convert to raw `pmix_persistence_t` value.
    pub fn into_raw(self) -> pmix_persistence_t {
        self.0 as pmix_persistence_t
    }
}

impl From<u8> for PmixPersistence {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<PmixPersistence> for u8 {
    fn from(persist: PmixPersistence) -> Self {
        persist.0
    }
}

impl From<PmixPersistence> for pmix_persistence_t {
    fn from(persist: PmixPersistence) -> Self {
        persist.into_raw()
    }
}

impl From<pmix_persistence_t> for PmixPersistence {
    fn from(persist: pmix_persistence_t) -> Self {
        Self::from_raw(persist)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixDataRange - wrapper for pmix_data_range_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_data_range_t`.
///
/// Represents PMIx data range enumerators (local, global, keyed, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub struct PmixDataRange(pub u8);

impl PmixDataRange {
    /// Create from raw `pmix_data_range_t` value.
    pub fn from_raw(value: pmix_data_range_t) -> Self {
        Self(value as u8)
    }

    /// Convert to raw `pmix_data_range_t` value.
    pub fn into_raw(self) -> pmix_data_range_t {
        self.0 as pmix_data_range_t
    }
}

impl From<u8> for PmixDataRange {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<PmixDataRange> for u8 {
    fn from(range: PmixDataRange) -> Self {
        range.0
    }
}

impl From<PmixDataRange> for pmix_data_range_t {
    fn from(range: PmixDataRange) -> Self {
        range.into_raw()
    }
}

impl From<pmix_data_range_t> for PmixDataRange {
    fn from(range: pmix_data_range_t) -> Self {
        Self::from_raw(range)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixProcState - wrapper for pmix_proc_state_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_proc_state_t`.
///
/// Represents PMIx process state enumerators (spawned, terminated, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub struct PmixProcState(pub u8);

impl PmixProcState {
    /// Create from raw `pmix_proc_state_t` value.
    pub fn from_raw(value: pmix_proc_state_t) -> Self {
        Self(value as u8)
    }

    /// Convert to raw `pmix_proc_state_t` value.
    pub fn into_raw(self) -> pmix_proc_state_t {
        self.0 as pmix_proc_state_t
    }
}

impl From<u8> for PmixProcState {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<PmixProcState> for u8 {
    fn from(state: PmixProcState) -> Self {
        state.0
    }
}

impl From<PmixProcState> for pmix_proc_state_t {
    fn from(state: PmixProcState) -> Self {
        state.into_raw()
    }
}

impl From<pmix_proc_state_t> for PmixProcState {
    fn from(state: pmix_proc_state_t) -> Self {
        Self::from_raw(state)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixAllocDirective - wrapper for pmix_alloc_directive_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_alloc_directive_t`.
///
/// Represents PMIx allocation directive enumerators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub struct PmixAllocDirective(pub u8);

impl PmixAllocDirective {
    /// Create from raw `pmix_alloc_directive_t` value.
    pub fn from_raw(value: pmix_alloc_directive_t) -> Self {
        Self(value as u8)
    }

    /// Convert to raw `pmix_alloc_directive_t` value.
    pub fn into_raw(self) -> pmix_alloc_directive_t {
        self.0 as pmix_alloc_directive_t
    }
}

impl From<u8> for PmixAllocDirective {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<PmixAllocDirective> for u8 {
    fn from(dir: PmixAllocDirective) -> Self {
        dir.0
    }
}

impl From<PmixAllocDirective> for pmix_alloc_directive_t {
    fn from(dir: PmixAllocDirective) -> Self {
        dir.into_raw()
    }
}

impl From<pmix_alloc_directive_t> for PmixAllocDirective {
    fn from(dir: pmix_alloc_directive_t) -> Self {
        Self::from_raw(dir)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixIofChannel - wrapper for pmix_iof_channel_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_iof_channel_t`.
///
/// Represents PMIx I/O forwarding channel enumerators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub struct PmixIofChannel(pub u16);

impl PmixIofChannel {
    /// Create from raw `pmix_iof_channel_t` value.
    pub fn from_raw(value: pmix_iof_channel_t) -> Self {
        Self(value as u16)
    }

    /// Convert to raw `pmix_iof_channel_t` value.
    pub fn into_raw(self) -> pmix_iof_channel_t {
        self.0 as pmix_iof_channel_t
    }
}

impl From<u16> for PmixIofChannel {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<PmixIofChannel> for u16 {
    fn from(ch: PmixIofChannel) -> Self {
        ch.0
    }
}

impl From<PmixIofChannel> for pmix_iof_channel_t {
    fn from(ch: PmixIofChannel) -> Self {
        ch.into_raw()
    }
}

impl From<pmix_iof_channel_t> for PmixIofChannel {
    fn from(ch: pmix_iof_channel_t) -> Self {
        Self::from_raw(ch)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixInfoDirectives - wrapper for pmix_info_directives_t
// ─────────────────────────────────────────────────────────────────────────────

/// Native Rust wrapper for `pmix_info_directives_t`.
///
/// Represents PMIx info directives (flags for info array operations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub struct PmixInfoDirectives(pub u32);

impl PmixInfoDirectives {
    /// Create from raw `pmix_info_directives_t` value.
    pub fn from_raw(value: pmix_info_directives_t) -> Self {
        Self(value as u32)
    }

    /// Convert to raw `pmix_info_directives_t` value.
    pub fn into_raw(self) -> pmix_info_directives_t {
        self.0 as pmix_info_directives_t
    }
}

impl From<u32> for PmixInfoDirectives {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PmixInfoDirectives> for u32 {
    fn from(dir: PmixInfoDirectives) -> Self {
        dir.0
    }
}

impl From<PmixInfoDirectives> for pmix_info_directives_t {
    fn from(dir: PmixInfoDirectives) -> Self {
        dir.into_raw()
    }
}

impl From<pmix_info_directives_t> for PmixInfoDirectives {
    fn from(dir: pmix_info_directives_t) -> Self {
        Self::from_raw(dir)
    }
}
