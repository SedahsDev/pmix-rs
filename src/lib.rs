#![allow(unused_imports)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::ptr_offset_with_cast)]

//!
//! # Concurrency model
//!
//! OpenPMIx **≥ 6.1** threadshifts most `PMIx_*` entry points onto the progress
//! engine, so concurrent **calls** into C are intended to be OK while progress
//! runs. Rust still owns session lifetime and C-handle discipline:
//!
//! - **Only** client session API: process-wide [`PmixClient`] (`Clone + Send + Sync`).
//!   Use [`PmixClient::connect_new`] / [`connect`](PmixClient::connect) /
//!   [`disconnect`](PmixClient::disconnect). There is no legacy `Context` / `init`.
//! - Do **not** lock every put/get in Rust by default — OpenPMIx ≥ 6.1 serializes entry.
//!   Lock only Rust-owned shared state (session lifecycle updates use a write lock;
//!   cached `Proc` reads use shared read locks).
//! - Data ops stay as free functions ([`put_value`], [`get_value`], [`commit`],
//!   [`fence`], …). Pass [`Proc`] from the client; build [`Info`] per call.
//! - [`Info`] and other C-owned handles are **`!Send` + `!Sync`**
//!   (`PhantomData<*mut u8>`). Keep them on their owner thread, or convert
//!   them to Rust-owned snapshots before sending data to another thread.
//!   `Arc<Mutex<T>>` does not make a `!Send` wrapper transferable: `T` must
//!   still be `Send` for the mutex to cross threads.
//! - One logical connect/disconnect per process. **Drop does not finalize**
//!   (clones must not each run `PMIx_Finalize`). Call [`disconnect`](PmixClient::disconnect)
//!   or [`finalize`].
//! - Server module upcalls ([`server::PmixServerModule`]) run in **progress
//!   context** — keep them short, hop before blocking PMIx, complete via the
//!   provided `cbfunc` later. They are **not** CPU-pin targets (pin progress
//!   via [`InitOptions::bind_progress_thread`]). See
//!   `examples/server_upcall_hop.rs` and [THREADING.md](../THREADING.md) §4.1.
//! - `_nb` completions and events are delivered on the **progress thread**.
//!   Hop off before any blocking work with the [`threading`] helpers
//!   (`spawn_from_callback`, `CallbackChannel`); never call blocking PMIx
//!   APIs from inside a callback. See `examples/callback_hop.rs` and
//!   [THREADING.md](../THREADING.md).
//!
//! See [THREADING.md](../THREADING.md) in the repo for the full model.
//!
use std::fmt::Debug;
pub mod allocation;
pub mod argv;
pub mod cbdata;
pub mod cpu_locality;
pub mod data_ops;
pub mod data_serialization;
pub mod events;
pub mod fabric;
#[path = ""]
#[allow(hidden_glob_reexports)]
pub mod ffi {
    #[allow(clippy::upper_case_acronyms, clippy::enum_variant_names)]
    #[path = "ffi.rs"]
    mod raw;
    pub use raw::*;
    pub(crate) use raw::{pmix_event_notification_cbfunc_fn_t, pmix_op_cbfunc_t};
}
pub mod groups;
pub mod info;
pub mod info_list;
#[cfg(any(test, feature = "mock_ffi"))]
pub mod mock_ffi;
pub mod monitoring;
pub mod process_mgmt;
pub mod query_log;
pub mod security;
pub mod server;
pub mod threading;
#[cfg(test)]
mod threading_assert;
pub mod tool;
pub mod utility;

/// Dispatch a PMIx FFI call to the mock implementation when the `mock_ffi`
/// feature (or `cfg(test)`) is active **and** mocks are enabled at runtime.
///
/// In production builds without the feature, only the real call is compiled.
#[macro_export]
#[doc(hidden)]
macro_rules! pmix_ffi_or_mock {
    (mock = $mock:expr, real = $real:expr $(,)?) => {{
        #[cfg(any(test, feature = "mock_ffi"))]
        {
            if $crate::mock_ffi::is_mock_enabled() {
                $mock
            } else {
                $real
            }
        }
        #[cfg(not(any(test, feature = "mock_ffi")))]
        {
            $real
        }
    }};
}

use crate::ffi::*;
use cstring_array::CStringArray;
use std::ffi::{CStr, CString, NulError};
use std::mem::zeroed;
use std::os::raw::{c_char, c_void};
use std::ptr::{null, null_mut};
use std::{fmt, mem, ptr};

pub const GLOBAL: u8 = PMIX_GLOBAL as u8;
pub const NUM_NODES: &[u8; 15] = PMIX_NUM_NODES;
pub const JOB_SIZE: &[u8; 14] = PMIX_JOB_SIZE;
pub const RANK_WILDCARD: u32 = PMIX_RANK_WILDCARD;

pub mod client;
pub mod enums;
pub mod error;
pub mod types_proc;
pub mod value_kinds;
pub mod value_ns;
pub use client::*;
pub use enums::*;
pub use error::*;
pub use types_proc::*;
pub use value_kinds::*;
pub use value_ns::setenv;
pub use value_ns::*;

#[cfg(test)]
mod tests;
