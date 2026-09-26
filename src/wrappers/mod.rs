//! Native Rust wrappers for PMIx FFI types.
//!
//! This module and its submodules provide pure Rust types that wrap PMIx's
//! C types to prevent FFI type leakage into the public API. The wrappers
//! provide safe constructors, accessors, and conversions between Rust and
//! C types.
//!
//! # Submodules
//!
//! - `scalars`: Wrapper types for scalar C types (status, rank, scope, etc.)
//! - `structs`: Wrapper types for C structs (proc, info, pdata, regattr)

pub mod scalars;
pub mod structs;

// Re-export for convenience
pub use scalars::*;
pub use structs::*;
