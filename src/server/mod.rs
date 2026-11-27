//! Server-side PMIx APIs — `PMIx_server_init`, `PMIx_server_finalize`,
//! and the `PmixServerModule` / `PmixServerHandle` safe wrappers.
//!
//! This module provides safe Rust wrappers around the PMIx server APIs
//! that allow a resource manager (RM) or daemon to act as a PMIx server.
//!
//! # Overview
//!
//! The PMIx server library is a separate initialization path from the
//! client/tool path. Instead of calling `PMIx_Init`, a server calls
//! `PMIx_server_init` and provides a server module structure whose
//! function pointers implement the callbacks the PMIx library will
//! invoke when clients connect, publish, spawn, etc.
//!
//! # Example
//!
//! ```no_run
//! use pmix::server::{PmixServer, PmixServerModule};
//! use pmix::InfoBuilder;
//!
//! let module = PmixServerModule::default();
//! let server = PmixServer::connect_new(Some(&module), &InfoBuilder::new().build().expect("build info"))
//!     .expect("server connect");
//! // Clone is cheap (Arc); Drop does **not** finalize.
//! let worker = server.clone();
//! // ... serve clients ...
//! server.disconnect().expect("server disconnect");
//! ```
//!
//! # Server module upcalls (progress context)
//!
//! Host callbacks in [`PmixServerModule`] (`fence_nb`, `direct_modex`,
//! `publish`, …) run on the PMIx **progress thread** (or another host
//! progress context OpenPMIx chooses) — **not** on an application worker
//! you pin. That has two consequences:
//!
//! 1. **Do not block back into PMIx from an upcall.** Waiting on a
//!    blocking `PMIx_*` entry (or joining a thread that does) deadlocks
//!    progress. Hop first with
//!    [`crate::threading::spawn_from_callback`] /
//!    [`crate::threading::CallbackChannel`], then finish the request by
//!    invoking the provided `cbfunc` **later** from an app thread.
//! 2. **Upcalls are not CPU-pin targets.** Pin the progress engine via
//!    [`InitOptions::bind_progress_thread`](crate::InitOptions::bind_progress_thread)
//!    (see [THREADING.md](../../THREADING.md) §4). Do not try to pin
//!    which core runs `fence_nb` / `direct_modex` themselves.
//!
//! Full hop-off policy and forbidden APIs:
//! [`crate::threading::ProgressContext`]. Worked fence/modex pattern:
//! `examples/server_upcall_hop.rs`. Client `_nb` / events counterpart:
//! `examples/callback_hop.rs` ([#51](https://github.com/SedahsDev/pmix-rs/issues/51)).
//!
//! # Callbacks
//!
//! The `PmixServerModule` struct exposes host upcall functions that the
//! PMIx library may invoke. All fields default to `None` (null in C),
//! meaning the callback is not implemented. The PMIx library checks for
//! null before calling, so it is safe to provide a minimal module that
//! only implements the callbacks you need.
//!
//! # C API
//!
//! ```c
//! pmix_status_t PMIx_server_init(void *module,
//!                                 pmix_info_t info[], size_t ninfo);
//! pmix_status_t PMIx_server_finalize(void);
//! ```

use crate::cbdata::Registry;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::security::PmixCredential;
use crate::server::callbacks::{ModexCallback, ReleaseCallback};
use crate::threading::invoke_user_callback;
#[cfg(pmix6)]
use crate::views::ResourceUnitView;
use crate::views::{AppView, InfoView, ProcView, QueryView, view_slice};
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc, ffi};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

#[allow(dead_code)]
struct RetainedServerData {
    infos: Vec<ffi::pmix_info_t>,
    directives: Vec<ffi::pmix_info_t>,
    source: Option<Box<ffi::pmix_proc_t>>,
    bo: Option<Box<ffi::pmix_byte_object_t>>,
    bo_bytes: Option<Vec<u8>>,
}

unsafe impl Send for RetainedServerData {}

static SERVER_RETAINED_DATA: LazyLock<Mutex<HashMap<usize, RetainedServerData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn retained_info(info: &Info) -> Vec<ffi::pmix_info_t> {
    if info.handle.is_null() || info.len == 0 {
        Vec::new()
    } else {
        // SAFETY: Info owns a valid array of `len` initialized PMIx entries.
        unsafe { std::slice::from_raw_parts(info.handle, info.len) }
            .iter()
            .map(|entry| unsafe { std::ptr::read(entry) })
            .collect()
    }
}

fn release_server_retained_data(req_id: usize) {
    SERVER_RETAINED_DATA
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&req_id);
}

fn retain_server_info(req_id: usize, info: &Info) -> (*mut ffi::pmix_info_t, usize) {
    let infos = retained_info(info);
    let ninfo = infos.len();
    let ptr = if infos.is_empty() {
        ptr::null_mut()
    } else {
        infos.as_ptr() as *mut _
    };
    SERVER_RETAINED_DATA
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            req_id,
            RetainedServerData {
                infos,
                directives: Vec::new(),
                source: None,
                bo: None,
                bo_bytes: None,
            },
        );
    (ptr, ninfo)
}

fn retain_server_iof(
    req_id: usize,
    source: &Proc,
    bo: &crate::data_serialization::PmixByteObject,
    info: &Info,
) -> (
    *const ffi::pmix_proc_t,
    *const ffi::pmix_byte_object_t,
    *mut ffi::pmix_info_t,
    usize,
) {
    let infos = retained_info(info);
    let ninfo = infos.len();
    let bo_bytes = bo.as_slice().to_vec();
    let source = Box::new(source.handle);
    let bo = Box::new(ffi::pmix_byte_object_t {
        bytes: if bo_bytes.is_empty() {
            ptr::null_mut()
        } else {
            bo_bytes.as_ptr() as *mut c_char
        },
        size: bo_bytes.len(),
    });
    let source_ptr = source.as_ref() as *const _;
    let bo_ptr = bo.as_ref() as *const _;
    let info_ptr = if infos.is_empty() {
        ptr::null_mut()
    } else {
        infos.as_ptr() as *mut _
    };
    SERVER_RETAINED_DATA
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            req_id,
            RetainedServerData {
                infos,
                directives: Vec::new(),
                source: Some(source),
                bo: Some(bo),
                bo_bytes: Some(bo_bytes),
            },
        );
    (source_ptr, bo_ptr, info_ptr, ninfo)
}

#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

mod cred;
mod data;
mod pset;
mod session;
#[cfg(test)]
mod tests;

pub use session::{PmixServer, PmixServerState};

// Re-export submodule items at server:: for stable paths.
pub use callbacks::free_op_callback_cbdata;
pub use cred::*;
pub use data::*;
pub use pset::*;

/// Operation completion callback utilities.
#[path = "callbacks.rs"]
mod native_callbacks;
pub use native_callbacks::*;

pub use callbacks::ServerFenceNbCallback;

pub mod callbacks {
    use super::CompletionTaken;
    pub use super::OpCallbackWrapper;
    pub use super::native_callbacks::*;
    use crate::{PmixStatus, ffi};
    use std::os::raw::{c_char, c_void};

    /// An owned, one-shot release callback supplied by PMIx.
    pub struct ReleaseCallback(
        Box<dyn FnOnce(*mut c_void) + Send + 'static>,
        ffi::pmix_release_cbfunc_t,
    );

    impl ReleaseCallback {
        /// Wrap a Rust release callback for invocation with [`Self::call`].
        pub fn new(callback: impl FnOnce(*mut c_void) + Send + 'static) -> Self {
            Self(Box::new(callback), None)
        }

        /// Invoke the release callback, consuming this wrapper.
        ///
        /// # Safety
        /// `cbdata` must satisfy the original callback's lifetime and
        /// thread requirements.
        pub unsafe fn call(self, cbdata: *mut c_void) {
            (self.0)(cbdata);
        }

        /// Restore the original C callback, consuming this wrapper.
        ///
        /// Returns `None` for a Rust closure created with [`Self::new`].
        pub fn into_c_callback(self) -> Option<unsafe extern "C" fn(*mut c_void)> {
            self.1
        }
    }

    impl Default for ReleaseCallback {
        fn default() -> Self {
            Self(Box::new(|_| {}), None)
        }
    }

    impl From<ffi::pmix_release_cbfunc_t> for ReleaseCallback {
        fn from(callback: ffi::pmix_release_cbfunc_t) -> Self {
            Self(
                Box::new(move |cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ReleaseCallback> for ffi::pmix_release_cbfunc_t {
        fn from(callback: ReleaseCallback) -> Self {
            callback.into_c_callback()
        }
    }

    impl std::fmt::Debug for ReleaseCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("ReleaseCallback")
                .finish_non_exhaustive()
        }
    }

    /// Release callback data when an operation is rejected without a callback.
    ///
    /// # Safety
    /// `cbdata` must come from `OpCallbackWrapper::into_c` and must not
    /// have been consumed or be used by a subsequent callback.
    pub unsafe fn free_op_callback_cbdata(cbdata: *mut std::ffi::c_void) {
        if !cbdata.is_null() {
            unsafe { drop(Box::from_raw(cbdata.cast::<OpCallbackWrapper>())) };
        }
    }

    type ModexClosure = dyn FnOnce(PmixStatus, *const c_char, usize, *mut c_void, ReleaseCallback, *mut c_void)
        + Send
        + 'static;

    type RawModexCallback = ffi::pmix_modex_cbfunc_t;

    /// An owned, one-shot modex completion callback supplied by PMIx.
    pub struct ModexCallback(Box<ModexClosure>, RawModexCallback, Option<CompletionTaken>);

    impl ModexCallback {
        /// Wrap a Rust modex callback for invocation with [`Self::call`].
        pub fn new(
            callback: impl FnOnce(
                PmixStatus,
                *const c_char,
                usize,
                *mut c_void,
                ReleaseCallback,
                *mut c_void,
            ) + Send
            + 'static,
        ) -> Self {
            Self(Box::new(callback), None, None)
        }

        /// Complete the modex operation, consuming this wrapper.
        ///
        /// # Safety
        /// The data buffer and opaque pointers must satisfy the original
        /// callback's lifetime, ownership and thread requirements.
        pub unsafe fn call(
            self,
            status: PmixStatus,
            data: *const c_char,
            len: usize,
            cbdata: *mut c_void,
            release: ReleaseCallback,
            release_cbdata: *mut c_void,
        ) {
            (self.0)(status, data, len, cbdata, release, release_cbdata);
        }

        /// Restore the original C callback, consuming this wrapper.
        ///
        /// For a tracked wrapper (server trampoline origin) the extract
        /// CLAIMS the shared [`CompletionTaken`] one-shot completion
        /// right: the tracking closure is dropped and the bare C
        /// function pointer escapes as the single owner.
        ///
        /// Returns `None` for a Rust closure created with [`Self::new`]
        /// and when the claim is lost (the trampoline's panic path or an
        /// earlier completion already owned the right): in the lost
        /// case the caller must NOT invoke anything and must NOT free
        /// `cbdata` — the claim winner owns both.
        pub fn into_c_callback(
            self,
        ) -> Option<
            unsafe extern "C" fn(
                i32,
                *const c_char,
                usize,
                *mut c_void,
                Option<unsafe extern "C" fn(*mut c_void)>,
                *mut c_void,
            ),
        > {
            let Self(_, callback, taken) = self;
            if let Some(taken) = taken {
                if !taken.claim() {
                    // Someone else (the trampoline's panic path or an earlier
                    // completion) already owned the one-shot completion right.
                    // `None` means "already completed elsewhere; do not call".
                    return None;
                }
            }
            callback
        }
    }

    impl ModexCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw_tracked(
            callback: ffi::pmix_modex_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(callback: RawModexCallback, taken: Option<CompletionTaken>) -> Self {
            let closure_taken = taken.clone();
            Self(
                Box::new(move |status, data, len, cbdata, release, release_cbdata| {
                    if let Some(callback) = callback {
                        // Prepare the release function BEFORE claiming so
                        // a later fallible prep can never take the
                        // one-shot right without a call following it.
                        let release_fn = release.into_c_callback();
                        if let Some(taken) = &closure_taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                data,
                                len,
                                cbdata,
                                release_fn,
                                release_cbdata,
                            );
                        }
                    }
                }),
                callback,
                taken,
            )
        }
    }

    impl From<ffi::pmix_modex_cbfunc_t> for ModexCallback {
        fn from(callback: ffi::pmix_modex_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl From<ModexCallback> for ffi::pmix_modex_cbfunc_t {
        fn from(callback: ModexCallback) -> Self {
            callback.into_c_callback()
        }
    }

    impl std::fmt::Debug for ModexCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("ModexCallback")
                .finish_non_exhaustive()
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Completion-callback wrappers for the remaining server cbfunc types
    // (#520 / #541): one native wrapper per distinct C signature, following
    // the ModexCallback / OpCallbackWrapper / ReleaseCallback pattern. The
    // bindgen fn-pointer types stay inside these wrappers; public surfaces
    // use native view types.
    // ─────────────────────────────────────────────────────────────────────

    /// An owned, one-shot direct-modex response callback supplied by PMIx
    /// (`pmix_dmodex_response_fn_t`).
    ///
    /// Complete a `direct_modex` host upcall by handing PMIx the requested
    /// modex blob (or an error status) via [`Self::call`].
    #[allow(clippy::type_complexity)]
    pub struct DmodexResponseCallback(
        Box<dyn FnOnce(PmixStatus, *mut c_char, usize, *mut c_void) + Send + 'static>,
        ffi::pmix_dmodex_response_fn_t,
    );

    impl DmodexResponseCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, *mut c_char, usize, *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the direct-modex request, consuming this wrapper.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; `data`/`sz` must satisfy PMIx's ownership
        /// requirements for the response blob.
        pub unsafe fn call(
            self,
            status: PmixStatus,
            data: *mut c_char,
            sz: usize,
            cbdata: *mut c_void,
        ) {
            (self.0)(status, data, sz, cbdata);
        }

        /// Restore the original C callback, consuming this wrapper.
        ///
        /// Returns `None` for a Rust closure created with [`Self::new`].
        pub fn into_c_callback(
            self,
        ) -> Option<unsafe extern "C" fn(i32, *mut c_char, usize, *mut c_void)> {
            self.1
        }
    }

    impl From<ffi::pmix_dmodex_response_fn_t> for DmodexResponseCallback {
        fn from(callback: ffi::pmix_dmodex_response_fn_t) -> Self {
            Self(
                Box::new(move |status, data, sz, cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(status.to_raw(), data, sz, cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<DmodexResponseCallback> for ffi::pmix_dmodex_response_fn_t {
        fn from(callback: DmodexResponseCallback) -> Self {
            callback.into_c_callback()
        }
    }

    impl std::fmt::Debug for DmodexResponseCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("DmodexResponseCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot lookup completion callback supplied by PMIx
    /// (`pmix_lookup_cbfunc_t`).
    #[allow(clippy::type_complexity)]
    pub struct LookupCallback(
        Box<dyn FnOnce(PmixStatus, &[crate::views::PdataView], *mut c_void) + Send + 'static>,
        #[allow(dead_code)] ffi::pmix_lookup_cbfunc_t,
    );

    impl LookupCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, &[crate::views::PdataView], *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the lookup, consuming this wrapper.
        ///
        /// Pass the requested pdata back (the same view slice the upcall
        /// received, or an empty slice).
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; `data` must point to PMIx-compatible
        /// `pmix_pdata_t` storage.
        pub unsafe fn call(
            self,
            status: PmixStatus,
            data: &[crate::views::PdataView],
            cbdata: *mut c_void,
        ) {
            (self.0)(status, data, cbdata);
        }
    }

    impl LookupCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_lookup_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(callback: ffi::pmix_lookup_cbfunc_t, taken: Option<CompletionTaken>) -> Self {
            Self(
                Box::new(move |status, data, cbdata| {
                    if let Some(callback) = callback {
                        // An empty slice must cross the FFI boundary as
                        // (NULL, 0): `slice::as_ptr` on `&[]` is a dangling
                        // non-null pointer, and PMIx null-checks the pointer
                        // before the length.
                        let (data, ndata) = if data.is_empty() {
                            (std::ptr::null_mut::<ffi::pmix_pdata_t>(), 0)
                        } else {
                            (
                                data.as_ptr().cast::<ffi::pmix_pdata_t>().cast_mut(),
                                data.len(),
                            )
                        };
                        if let Some(taken) = &taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(status.to_raw(), data, ndata, cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ffi::pmix_lookup_cbfunc_t> for LookupCallback {
        fn from(callback: ffi::pmix_lookup_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl std::fmt::Debug for LookupCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("LookupCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot spawn completion callback supplied by PMIx
    /// (`pmix_spawn_cbfunc_t`).
    #[allow(clippy::type_complexity)]
    pub struct SpawnCallback(
        Box<dyn FnOnce(PmixStatus, &crate::views::NspaceView, *mut c_void) + Send + 'static>,
        ffi::pmix_spawn_cbfunc_t,
        Option<CompletionTaken>,
    );

    impl SpawnCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, &crate::views::NspaceView, *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None, None)
        }

        /// Complete the spawn, consuming this wrapper, with the new job's
        /// nspace.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback.
        pub unsafe fn call(
            self,
            status: PmixStatus,
            nspace: &crate::views::NspaceView,
            cbdata: *mut c_void,
        ) {
            (self.0)(status, nspace, cbdata);
        }

        /// Restore the original C callback, consuming this wrapper.
        ///
        /// For a tracked wrapper (server trampoline origin) the extract
        /// CLAIMS the shared [`CompletionTaken`] one-shot completion
        /// right: the tracking closure is dropped and the bare C
        /// function pointer escapes as the single owner.
        ///
        /// Returns `None` for a Rust closure created with [`Self::new`]
        /// and when the claim is lost (the trampoline's panic path or an
        /// earlier completion already owned the right): in the lost
        /// case the caller must NOT invoke anything and must NOT free
        /// `cbdata` — the claim winner owns both.
        pub fn into_c_callback(
            self,
        ) -> Option<unsafe extern "C" fn(i32, *mut c_char, *mut c_void)> {
            let Self(_, callback, taken) = self;
            if let Some(taken) = taken {
                if !taken.claim() {
                    // Someone else (the trampoline's panic path or an earlier
                    // completion) already owned the one-shot completion right.
                    // `None` means "already completed elsewhere; do not call".
                    return None;
                }
            }
            callback
        }
    }

    impl SpawnCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_spawn_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(callback: ffi::pmix_spawn_cbfunc_t, taken: Option<CompletionTaken>) -> Self {
            let closure_taken = taken.clone();
            Self(
                Box::new(move |status, nspace, cbdata| {
                    if let Some(callback) = callback {
                        // The nspace copy lives in this closure frame, so it
                        // stays valid across the synchronous one-shot C call.
                        // SAFETY contract: the C callback copies the nspace
                        // into its own storage before returning; the borrow
                        // ends when this closure returns.
                        let mut nspace_copy = nspace.raw_copy();
                        if let Some(taken) = &closure_taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(status.to_raw(), nspace_copy.as_mut_ptr(), cbdata) };
                    }
                }),
                callback,
                taken,
            )
        }
    }

    impl From<ffi::pmix_spawn_cbfunc_t> for SpawnCallback {
        fn from(callback: ffi::pmix_spawn_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl From<SpawnCallback> for ffi::pmix_spawn_cbfunc_t {
        fn from(callback: SpawnCallback) -> Self {
            callback.into_c_callback()
        }
    }

    impl std::fmt::Debug for SpawnCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("SpawnCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot client-connection completion callback supplied by
    /// PMIx (`pmix_connection_cbfunc_t`).
    #[allow(clippy::type_complexity)]
    pub struct ConnectionCallback(
        Box<dyn FnOnce(std::os::raw::c_int, *mut c_void) + Send + 'static>,
        ffi::pmix_connection_cbfunc_t,
    );

    impl ConnectionCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(std::os::raw::c_int, *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the connection handshake, consuming this wrapper, with
        /// the accepted socket descriptor.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; `incoming_sd` must be a live socket.
        pub unsafe fn call(self, incoming_sd: std::os::raw::c_int, cbdata: *mut c_void) {
            (self.0)(incoming_sd, cbdata);
        }

        /// Restore the original C callback, consuming this wrapper.
        ///
        /// Returns `None` for a Rust closure created with [`Self::new`].
        pub fn into_c_callback(
            self,
        ) -> Option<unsafe extern "C" fn(std::os::raw::c_int, *mut c_void)> {
            self.1
        }
    }

    impl ConnectionCallback {
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_connection_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_connection_cbfunc_t> for ConnectionCallback {
        fn from(callback: ffi::pmix_connection_cbfunc_t) -> Self {
            Self(
                Box::new(move |incoming_sd, cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(incoming_sd, cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ConnectionCallback> for ffi::pmix_connection_cbfunc_t {
        fn from(callback: ConnectionCallback) -> Self {
            callback.into_c_callback()
        }
    }

    impl std::fmt::Debug for ConnectionCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("ConnectionCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot info-array completion callback supplied by PMIx
    /// (`pmix_info_cbfunc_t`) — used by `query`, `allocate`, `job_control`,
    /// `monitor`, `group`, `fabric`, and `session_control`.
    #[allow(clippy::type_complexity)]
    pub struct InfoCallback(
        Box<
            dyn FnOnce(
                    PmixStatus,
                    &[crate::views::InfoView],
                    *mut c_void,
                    ReleaseCallback,
                    *mut c_void,
                ) + Send
                + 'static,
        >,
        #[allow(dead_code)] ffi::pmix_info_cbfunc_t,
    );

    impl InfoCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(
                PmixStatus,
                &[crate::views::InfoView],
                *mut c_void,
                ReleaseCallback,
                *mut c_void,
            ) + Send
            + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the request, consuming this wrapper.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; `info` must point to PMIx-compatible
        /// `pmix_info_t` storage, and the release pair must be the one PMIx
        /// supplied (or `ReleaseCallback::default()` with null cbdata when
        /// there is none).
        pub unsafe fn call(
            self,
            status: PmixStatus,
            info: &[crate::views::InfoView],
            cbdata: *mut c_void,
            release: ReleaseCallback,
            release_cbdata: *mut c_void,
        ) {
            (self.0)(status, info, cbdata, release, release_cbdata);
        }
    }

    impl InfoCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_info_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(callback: ffi::pmix_info_cbfunc_t, taken: Option<CompletionTaken>) -> Self {
            Self(
                Box::new(move |status, info, cbdata, release, release_cbdata| {
                    if let Some(callback) = callback {
                        // An empty slice must cross the FFI boundary as
                        // (NULL, 0): `slice::as_ptr` on `&[]` is a dangling
                        // non-null pointer, and PMIx null-checks the pointer
                        // before the length.
                        let (info, ninfo) = if info.is_empty() {
                            (std::ptr::null_mut::<ffi::pmix_info_t>(), 0)
                        } else {
                            (
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                            )
                        };
                        // Prepare the release function BEFORE claiming so
                        // a fallible prep can never take the one-shot
                        // right without a call following it.
                        let release_fn = release.into_c_callback();
                        if let Some(taken) = &taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                info,
                                ninfo,
                                cbdata,
                                release_fn,
                                release_cbdata,
                            )
                        };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ffi::pmix_info_cbfunc_t> for InfoCallback {
        fn from(callback: ffi::pmix_info_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl std::fmt::Debug for InfoCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("InfoCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot credential completion callback supplied by PMIx
    /// (`pmix_credential_cbfunc_t`).
    #[allow(clippy::type_complexity)]
    pub struct CredentialCallback(
        Box<
            dyn FnOnce(PmixStatus, Option<&[u8]>, &[crate::views::InfoView], *mut c_void)
                + Send
                + 'static,
        >,
        #[allow(dead_code)] ffi::pmix_credential_cbfunc_t,
    );

    impl CredentialCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, Option<&[u8]>, &[crate::views::InfoView], *mut c_void)
            + Send
            + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the credential request, consuming this wrapper.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; the credential and info arrays must remain
        /// valid for the synchronous duration of the call (PMIx copies them).
        pub unsafe fn call(
            self,
            status: PmixStatus,
            credential: Option<&[u8]>,
            info: &[crate::views::InfoView],
            cbdata: *mut c_void,
        ) {
            (self.0)(status, credential, info, cbdata);
        }
    }

    impl CredentialCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_credential_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(
            callback: ffi::pmix_credential_cbfunc_t,
            taken: Option<CompletionTaken>,
        ) -> Self {
            Self(
                Box::new(move |status, credential, info, cbdata| {
                    if let Some(callback) = callback {
                        // A stack byte object mirrors the credential for the
                        // duration of the synchronous one-shot call. The C
                        // type takes `pmix_byte_object_t *` (mutable), but
                        // PMIx only reads it and copies the bytes before
                        // returning — it never writes through the pointer.
                        // The `bytes` field therefore aliases the caller's
                        // shared slice for the call only.
                        let bo = credential.map(|bytes| ffi::pmix_byte_object_t {
                            bytes: if bytes.is_empty() {
                                std::ptr::null_mut()
                            } else {
                                bytes.as_ptr() as *mut c_char
                            },
                            size: bytes.len(),
                        });
                        // An empty info slice must cross the FFI boundary as
                        // (NULL, 0): `slice::as_ptr` on `&[]` is a dangling
                        // non-null pointer.
                        let (info, ninfo) = if info.is_empty() {
                            (std::ptr::null_mut::<ffi::pmix_info_t>(), 0)
                        } else {
                            (
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                            )
                        };
                        if let Some(taken) = &taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                bo.as_ref()
                                    .map_or(std::ptr::null_mut(), |bo| bo as *const _ as *mut _),
                                info,
                                ninfo,
                                cbdata,
                            )
                        };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ffi::pmix_credential_cbfunc_t> for CredentialCallback {
        fn from(callback: ffi::pmix_credential_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl std::fmt::Debug for CredentialCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("CredentialCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot I/O-forwarding completion callback supplied by PMIx
    /// (`pmix_iof_cbfunc_t`).
    #[allow(clippy::type_complexity)]
    pub struct IofCallback(
        Box<
            dyn FnOnce(usize, u32, &crate::views::ProcView, &[u8], &[crate::views::InfoView])
                + Send
                + 'static,
        >,
        #[allow(dead_code)] ffi::pmix_iof_cbfunc_t,
    );

    impl IofCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(usize, u32, &crate::views::ProcView, &[u8], &[crate::views::InfoView])
            + Send
            + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the I/O-forwarding registration, consuming this wrapper.
        ///
        /// # Safety
        ///
        /// The source view, payload, and info array must remain valid for the
        /// synchronous duration of the call; `iofhdlr` must be the handle
        /// PMIx assigned.
        pub unsafe fn call(
            self,
            iofhdlr: usize,
            channel: u32,
            source: &crate::views::ProcView,
            payload: &[u8],
            info: &[crate::views::InfoView],
        ) {
            (self.0)(iofhdlr, channel, source, payload, info);
        }
    }

    impl From<ffi::pmix_iof_cbfunc_t> for IofCallback {
        fn from(callback: ffi::pmix_iof_cbfunc_t) -> Self {
            Self(
                Box::new(move |iofhdlr, channel, source, payload, info| {
                    if let Some(callback) = callback {
                        // Stack byte object mirrors the payload for the
                        // synchronous one-shot call. The C type takes
                        // `pmix_byte_object_t *` / `pmix_proc_t *` (mutable),
                        // but PMIx only reads them and copies before
                        // returning — it never writes through the pointers,
                        // so aliasing the caller's shared borrows is sound
                        // for the duration of the call.
                        let bo = ffi::pmix_byte_object_t {
                            bytes: if payload.is_empty() {
                                std::ptr::null_mut()
                            } else {
                                payload.as_ptr() as *mut c_char
                            },
                            size: payload.len(),
                        };
                        // An empty info slice must cross the FFI boundary as
                        // (NULL, 0): `slice::as_ptr` on `&[]` is a dangling
                        // non-null pointer.
                        let (info, ninfo) = if info.is_empty() {
                            (std::ptr::null_mut::<ffi::pmix_info_t>(), 0)
                        } else {
                            (
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                            )
                        };
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                iofhdlr,
                                channel as ffi::pmix_iof_channel_t,
                                source.raw_ptr().cast_mut(),
                                &bo as *const _ as *mut _,
                                info,
                                ninfo,
                            )
                        };
                    }
                }),
                callback,
            )
        }
    }

    impl std::fmt::Debug for IofCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("IofCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot tool-connection completion callback supplied by
    /// PMIx (`pmix_tool_connection_cbfunc_t`) — used by the `tool_connected`
    /// and (6.x) `tool_connected2` host upcalls.
    ///
    /// The C callback takes the tool's assigned identity by pointer: the
    /// host writes the nspace/rank it assigned to the tool before the
    /// callback returns. The wrapper exposes that as native
    /// `(nspace: &str, rank: u32)` arguments and owns the intermediate
    /// `pmix_proc_t` storage for the synchronous one-shot call.
    #[allow(clippy::type_complexity)]
    pub struct ToolConnectionCallback(
        Box<dyn FnOnce(PmixStatus, &str, u32, *mut c_void) + Send + 'static>,
        #[allow(dead_code)] ffi::pmix_tool_connection_cbfunc_t,
    );

    impl ToolConnectionCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, &str, u32, *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the tool connection with the identity the host assigned
        /// to the tool, consuming this wrapper.
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback.
        pub unsafe fn call(self, status: PmixStatus, nspace: &str, rank: u32, cbdata: *mut c_void) {
            (self.0)(status, nspace, rank, cbdata);
        }
    }

    impl ToolConnectionCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_tool_connection_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(
            callback: ffi::pmix_tool_connection_cbfunc_t,
            taken: Option<CompletionTaken>,
        ) -> Self {
            Self(
                Box::new(move |status, nspace, rank, cbdata| {
                    if let Some(callback) = callback {
                        // Stack proc storage for the synchronous one-shot
                        // call; PMIx copies the identity before returning.
                        let mut proc: ffi::pmix_proc_t = unsafe { std::mem::zeroed() };
                        let end = nspace.len().min(proc.nspace.len() - 1);
                        for (dst, src) in proc.nspace.iter_mut().zip(nspace.as_bytes()) {
                            *dst = *src as c_char;
                        }
                        proc.nspace[end] = 0;
                        proc.rank = rank;
                        if let Some(taken) = &taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(status.to_raw(), &mut proc, cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ffi::pmix_tool_connection_cbfunc_t> for ToolConnectionCallback {
        fn from(callback: ffi::pmix_tool_connection_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl std::fmt::Debug for ToolConnectionCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("ToolConnectionCallback")
                .finish_non_exhaustive()
        }
    }

    /// An owned, one-shot credential-validation completion callback supplied
    /// by PMIx (`pmix_validation_cbfunc_t`) — used by the
    /// `validate_credential` host upcall.
    ///
    /// `pmix_validation_cbfunc_t` is `(status, info, ninfo, cbdata)` — it
    /// does NOT carry the validated credential byte object (that belongs to
    /// `pmix_credential_cbfunc_t` / [`CredentialCallback`]).
    #[allow(clippy::type_complexity)]
    pub struct ValidationCallback(
        Box<dyn FnOnce(PmixStatus, &[crate::views::InfoView], *mut c_void) + Send + 'static>,
        #[allow(dead_code)] ffi::pmix_validation_cbfunc_t,
    );

    impl ValidationCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, &[crate::views::InfoView], *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
        }

        /// Complete the validation request, consuming this wrapper.
        ///
        /// Pass the validation-result info entries back (or an empty slice
        /// for none).
        ///
        /// # Safety
        ///
        /// `cbdata` must be the opaque pointer PMIx supplied with this
        /// completion callback; `info` must point to PMIx-compatible
        /// `pmix_info_t` storage.
        pub unsafe fn call(
            self,
            status: PmixStatus,
            info: &[crate::views::InfoView],
            cbdata: *mut c_void,
        ) {
            (self.0)(status, info, cbdata);
        }
    }

    impl ValidationCallback {
        /// Wrap the optional C completion callback supplied by PMIx,
        /// claiming `taken` at the instant the wrapper invokes the C
        /// completion (see [`CompletionTaken`]).
        pub(crate) fn from_raw(
            callback: ffi::pmix_validation_cbfunc_t,
            taken: CompletionTaken,
        ) -> Option<Self> {
            if callback.is_some() {
                Some(Self::wrap_raw(callback, Some(taken)))
            } else {
                None
            }
        }

        fn wrap_raw(
            callback: ffi::pmix_validation_cbfunc_t,
            taken: Option<CompletionTaken>,
        ) -> Self {
            Self(
                Box::new(move |status, info, cbdata| {
                    if let Some(callback) = callback {
                        // An empty slice must cross the FFI boundary as
                        // (NULL, 0): `slice::as_ptr` on `&[]` is a dangling
                        // non-null pointer.
                        let (info, ninfo) = if info.is_empty() {
                            (std::ptr::null_mut::<ffi::pmix_info_t>(), 0)
                        } else {
                            (
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                            )
                        };
                        if let Some(taken) = &taken {
                            if !taken.claim() {
                                // A racing trampoline panic path already
                                // completed the chain — invoking the C
                                // callback again would double-release
                                // cbdata.
                                return;
                            }
                        }
                        // SAFETY: call requires the original callback contract.
                        unsafe { callback(status.to_raw(), info, ninfo, cbdata) };
                    }
                }),
                callback,
            )
        }
    }

    impl From<ffi::pmix_validation_cbfunc_t> for ValidationCallback {
        fn from(callback: ffi::pmix_validation_cbfunc_t) -> Self {
            Self::wrap_raw(callback, None)
        }
    }

    impl std::fmt::Debug for ValidationCallback {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("ValidationCallback")
                .finish_non_exhaustive()
        }
    }
}

/// An owned, one-shot operation completion callback.
pub struct OpCallbackWrapper(Box<dyn FnOnce(PmixStatus) + Send + 'static>);

impl OpCallbackWrapper {
    /// Wrap a Rust operation completion callback.
    pub fn new(callback: impl FnOnce(PmixStatus) + Send + 'static) -> Self {
        Self(Box::new(callback))
    }

    /// Complete the operation, consuming the callback.
    pub fn call(self, status: PmixStatus) {
        (self.0)(status);
    }

    /// Transfer ownership to a one-shot C callback and its opaque data.
    ///
    /// Invoke the callback exactly once, or release the data with
    /// [`callbacks::free_op_callback_cbdata`] if the operation is rejected.
    pub fn into_c(self) -> (Option<unsafe extern "C" fn(i32, *mut c_void)>, *mut c_void) {
        unsafe extern "C" fn bridge(status: i32, cbdata: *mut c_void) {
            if cbdata.is_null() {
                return;
            }
            // SAFETY: into_c transfers a boxed wrapper to this one-shot bridge.
            let callback = unsafe { Box::from_raw(cbdata.cast::<OpCallbackWrapper>()) };
            let _ = invoke_user_callback("server", move || {
                callback.call(PmixStatus::from_raw(status));
            });
        }

        (Some(bridge), Box::into_raw(Box::new(self)).cast())
    }

    /// Transfer ownership to C; see [`Self::into_c`] for the ownership contract.
    pub fn into_c_callback(self) -> (Option<unsafe extern "C" fn(i32, *mut c_void)>, *mut c_void) {
        self.into_c()
    }

    // SAFETY: The caller must ensure PMIx permits this completion callback
    // and its opaque data to be retained and invoked from another thread.
    unsafe fn from_raw(
        callback: ffi::pmix_op_cbfunc_t,
        cbdata: *mut c_void,
        taken: CompletionTaken,
    ) -> Option<Self> {
        callback.map(|callback| {
            let cbdata = cbdata as usize;
            Self::new(move |status| unsafe {
                // Claim the one-shot right to invoke the C completion the
                // instant before the call: a trampoline panic path racing
                // this (possibly async) completion must not invoke the
                // C callback a second time.
                if !taken.claim() {
                    return;
                }
                callback(status.to_raw(), cbdata as *mut c_void);
            })
        })
    }
}

impl std::fmt::Debug for OpCallbackWrapper {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpCallbackWrapper")
            .finish_non_exhaustive()
    }
}

/// Host `publish` upcall: `(proc, info[], op_cbfunc, cbdata)` from
/// `pmix_server_publish_fn_t`.
///
/// `T` is the borrowed view type of the entries (`InfoView` for publish).
pub type ServerOpCallback<T> = unsafe fn(&ProcView, &[T], Option<OpCallbackWrapper>, *mut c_void);

/// Host `client_connected` upcall (`pmix_server_client_connected_fn_t`):
/// `(proc, server_object, op_cbfunc, cbdata)`.
///
/// OpenPMIx blocks the connecting client until the host completes through
/// the supplied [`OpCallbackWrapper`].
pub type ServerClientConnectedCallback =
    unsafe fn(&ProcView, *mut c_void, Option<OpCallbackWrapper>, *mut c_void);

/// Host `client_finalized` upcall (`pmix_server_client_finalized_fn_t`):
/// `(proc, server_object, op_cbfunc, cbdata)`.
pub type ServerClientFinalizedCallback =
    unsafe fn(&ProcView, *mut c_void, Option<OpCallbackWrapper>, *mut c_void);

/// Host `client_connected2` upcall (`pmix_server_client_connected2_fn_t`):
/// `(proc, server_object, info[], op_cbfunc, cbdata)`.
pub type ServerClientConnected2Callback =
    unsafe fn(&ProcView, *mut c_void, &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `abort` upcall (`pmix_server_abort_fn_t`):
/// `(proc, server_object, status, msg, procs[], nprocs, op_cbfunc, cbdata)`.
///
/// `msg` is `None` when the C message pointer is null. An empty `procs`
/// slice is the C NULL array — kill all ranks in the client's nspace.
pub type ServerAbortCallback = unsafe fn(
    &ProcView,
    *mut c_void,
    i32,
    Option<&str>,
    &[ProcView],
    Option<OpCallbackWrapper>,
    *mut c_void,
);

/// Host `fence_nb` upcall (`pmix_server_fencenb_fn_t`):
/// `(procs[], nprocs, info[], ninfo, data, ndata, modex_cbfunc, cbdata)`.
///
/// `procs` is the participant array (`&[ProcView]`, never a nspace list);
/// an empty slice is the C NULL array (all ranks in the caller's nspace).
/// The collective payload is the modex blob — a raw
/// `(*mut c_char, usize)` pair owned by PMIx for the upcall. Complete via
/// the supplied [`ModexCallback`] once the host has shared the blob with
/// its peer daemons.
pub type ServerModexCallback =
    unsafe fn(&[ProcView], &[InfoView], *mut c_char, usize, Option<ModexCallback>, *mut c_void);

/// Host `direct_modex` upcall (`pmix_server_dmodex_req_fn_t`):
/// `(proc, info[], ninfo, modex_cbfunc, cbdata)`.
///
/// Completes via `pmix_modex_cbfunc_t` — the same
/// [`ModexCallback`] type as [`ServerModexCallback`]. (The
/// `pmix_dmodex_response_fn_t` shape belongs to
/// [`crate::server::server_dmodex_request`], not this slot.)
pub type ServerDirectModexCallback =
    unsafe fn(&ProcView, &[InfoView], Option<ModexCallback>, *mut c_void);

/// Host `lookup` upcall (`pmix_server_lookup_fn_t`):
/// `(proc, keys, info[], ninfo, lookup_cbfunc, cbdata)`.
///
/// `keys` is the C NULL-terminated `char **` key list, borrowed for the
/// synchronous duration of the upcall. The pdata array is an OUTPUT of
/// the completion — the host hands the results back through the supplied
/// [`LookupCallback`](callbacks::LookupCallback), not through this upcall's
/// arguments.
pub type ServerLookupCallback =
    unsafe fn(&ProcView, &[&str], &[InfoView], Option<callbacks::LookupCallback>, *mut c_void);

/// Host `unpublish` upcall (`pmix_server_unpublish_fn_t`):
/// `(proc, keys, info[], ninfo, op_cbfunc, cbdata)`.
///
/// `keys` is the C NULL-terminated `char **` key list, borrowed for the
/// synchronous duration of the upcall.
pub type ServerUnpublishCallback =
    unsafe fn(&ProcView, &[&str], &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `spawn` upcall (`pmix_server_spawn_fn_t`):
/// `(proc, job_info[], ninfo, apps[], napps, spawn_cbfunc, cbdata)`.
pub type ServerSpawnCallback =
    unsafe fn(&ProcView, &[InfoView], &[AppView], Option<callbacks::SpawnCallback>, *mut c_void);

/// Host `connect` / `disconnect` upcalls (`pmix_server_connect_fn_t` /
/// `pmix_server_disconnect_fn_t`): `(procs[], nprocs, info[], ninfo,
/// op_cbfunc, cbdata)`.
///
/// The procs array is `&[ProcView]` — never a nspace list. Both complete
/// via `pmix_op_cbfunc_t`.
pub type ServerConnectCallback =
    unsafe fn(&[ProcView], &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `register_events` upcall (`pmix_server_register_events_fn_t`):
/// `(codes[], ncodes, info[], ninfo, op_cbfunc, cbdata)`.
pub type ServerRegisterEventsCallback =
    unsafe fn(&[i32], &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `deregister_events` upcall (`pmix_server_deregister_events_fn_t`):
/// `(codes[], ncodes, op_cbfunc, cbdata)`.
pub type ServerDeregisterEventsCallback = unsafe fn(&[i32], Option<OpCallbackWrapper>, *mut c_void);

/// Host `listener` upcall (`pmix_server_listener_fn_t`):
/// `(listening_sd, connection_cbfunc, cbdata)`.
///
/// The host accepts connections on the listening socket and hands each
/// accepted socket descriptor back through the supplied
/// [`ConnectionCallback`](callbacks::ConnectionCallback) — the connection
/// callback belongs to this slot (not `connect`).
pub type ServerListenerCallback =
    unsafe fn(c_int, Option<callbacks::ConnectionCallback>, *mut c_void);

/// Host `notify_event` upcall (`pmix_server_notify_event_fn_t`):
/// `(code, source, range, info[], ninfo, op_cbfunc, cbdata)`.
///
/// There is no results array in the 5.x/6.x slot; the notification info
/// is input data. Completion happens through the supplied
/// [`OpCallbackWrapper`].
pub type ServerNotifyEventCallback =
    unsafe fn(i32, &ProcView, u8, &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `query` upcall (`pmix_server_query_fn_t`):
/// `(proc, queries[], nqueries, info_cbfunc, cbdata)`.
pub type ServerQueryCallback =
    unsafe fn(&ProcView, &[QueryView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `tool_connected` upcall (`pmix_server_tool_connection_fn_t`):
/// `(info[], ninfo, tool_connection_cbfunc, cbdata)` — void-returning.
///
/// The host assigns the tool its nspace/rank and completes through the
/// supplied [`ToolConnectionCallback`](callbacks::ToolConnectionCallback).
pub type ServerToolConnectedCallback =
    unsafe fn(&[InfoView], Option<callbacks::ToolConnectionCallback>, *mut c_void);

/// Host `log` upcall (`pmix_server_log_fn_t`): `(client, data[], ndata,
/// directives[], ndirs, op_cbfunc, cbdata)` — void-returning.
pub type ServerLogCallback =
    unsafe fn(&ProcView, &[InfoView], &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `allocate` upcall (`pmix_server_alloc_fn_t`):
/// `(client, alloc_directive, data[], ndata, info_cbfunc, cbdata)`.
///
/// The allocation directive is the raw `pmix_alloc_directive_t` value.
pub type ServerAllocateCallback =
    unsafe fn(&ProcView, u8, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `job_control` upcall (`pmix_server_job_control_fn_t`):
/// `(requestor, targets[], ntargets, directives[], ndirs, info_cbfunc, cbdata)`.
///
/// An empty targets slice is the C NULL array — apply to all targets in
/// the requestor's nspace.
pub type ServerJobControlCallback =
    unsafe fn(&ProcView, &[ProcView], &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `monitor` upcall (`pmix_server_monitor_fn_t`):
/// `(requestor, monitor, error, directives[], ndirs, info_cbfunc, cbdata)`.
///
/// `monitor` is the single `pmix_info_t` entry describing what to monitor
/// (`None` when the C pointer is null).
pub type ServerMonitorCallback = unsafe fn(
    &ProcView,
    Option<&InfoView>,
    i32,
    &[InfoView],
    Option<callbacks::InfoCallback>,
    *mut c_void,
);

/// Host `get_credential` upcall (`pmix_server_get_cred_fn_t`):
/// `(proc, directives[], ndirs, credential_cbfunc, cbdata)`.
pub type ServerCredentialCallback =
    unsafe fn(&ProcView, &[InfoView], Option<callbacks::CredentialCallback>, *mut c_void);

/// Host `validate_credential` upcall (`pmix_server_validate_cred_fn_t`):
/// `(proc, cred, directives[], ndirs, validation_cbfunc, cbdata)`.
///
/// `cred` is the credential byte object borrowed for the upcall (`None`
/// when the C pointer is null). Completes via
/// [`ValidationCallback`](callbacks::ValidationCallback) — the validation
/// callback takes `(status, info, ninfo, cbdata)`, not the credential
/// byte-object callback.
pub type ServerValidateCredentialCallback = unsafe fn(
    &ProcView,
    Option<&[u8]>,
    &[InfoView],
    Option<callbacks::ValidationCallback>,
    *mut c_void,
);

/// Host `iof_pull` upcall (`pmix_server_iof_fn_t`):
/// `(procs[], nprocs, directives[], ndirs, channels, op_cbfunc, cbdata)`.
///
/// Registration completes via `pmix_op_cbfunc_t`; the IOF delivery
/// callback (`pmix_iof_cbfunc_t`) is the client-side callback.
pub type ServerIofPullCallback =
    unsafe fn(&[ProcView], &[InfoView], u16, Option<OpCallbackWrapper>, *mut c_void);

/// Host `push_stdin` upcall (`pmix_server_stdin_fn_t`):
/// `(source, targets[], ntargets, directives[], ndirs, bo, op_cbfunc, cbdata)`.
///
/// `bo` is the stdin byte object borrowed for the upcall (`None` when the
/// C pointer is null).
pub type ServerPushStdinCallback = unsafe fn(
    &ProcView,
    &[ProcView],
    &[InfoView],
    Option<&[u8]>,
    Option<OpCallbackWrapper>,
    *mut c_void,
);

/// Host `group` upcall (`pmix_server_grp_fn_t`):
/// `(op, group name, procs[], nprocs, directives[], ndirs, info_cbfunc, cbdata)`.
///
/// The group name is borrowed for the synchronous duration of the upcall
/// (`None` when the C pointer is null); `op` is the raw
/// `pmix_group_operation_t` value.
pub type ServerGroupCallback = unsafe fn(
    u32,
    Option<&str>,
    &[ProcView],
    &[InfoView],
    Option<callbacks::InfoCallback>,
    *mut c_void,
);

/// Host `fabric` upcall (`pmix_server_fabric_fn_t`):
/// `(requestor, fabric_op, directives[], ndirs, info_cbfunc, cbdata)`.
///
/// `op` is the raw `pmix_fabric_operation_t` value.
pub type ServerFabricCallback =
    unsafe fn(&ProcView, u32, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `session_control` upcall (`pmix_server_session_control_fn_t`):
/// `(requestor, sessionID, directives[], ndirs, info_cbfunc, cbdata)`.
pub type ServerSessionControlCallback =
    unsafe fn(&ProcView, u32, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `resource_block` upcall (6.x-only,
/// `pmix_server_resource_block_fn_t`):
/// `(requestor, directive, block, units[], nunit, info[], ninfo,
/// op_cbfunc, cbdata)`.
///
/// `block` is the block name borrowed for the upcall (`None` when the C
/// pointer is null); `directive` is the raw
/// `pmix_resource_block_directive_t` value.
#[cfg(pmix6)]
pub type ServerResourceBlockCallback = unsafe fn(
    &ProcView,
    u8,
    Option<&str>,
    &[ResourceUnitView],
    &[InfoView],
    Option<OpCallbackWrapper>,
    *mut c_void,
);

/// Host `log2` upcall (6.x-only, `pmix_server_log2_fn_t`):
/// `(client, data[], ndata, directives[], ndirs, op_cbfunc, cbdata)`.
#[cfg(pmix6)]
pub type ServerLog2Callback =
    unsafe fn(&ProcView, &[InfoView], &[InfoView], Option<OpCallbackWrapper>, *mut c_void);

/// Host `tool_connected2` upcall (6.x-only,
/// `pmix_server_tool_connection2_fn_t`): `(info[], ninfo,
/// tool_connection_cbfunc, cbdata)`.
#[cfg(pmix6)]
pub type ServerToolConnected2Callback =
    unsafe fn(&[InfoView], Option<callbacks::ToolConnectionCallback>, *mut c_void);

/// Zero-sized marker for shared storage of converted C callback tables.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct ServerModuleCallbackState;

// Keep returned C table addresses stable when the vector grows. The table
// stores the pinned `ffi::pmix_server_module_t` per (owner address,
// installed-slot mask); the trampolines are static functions, so equal
// masks mean identical C contents.
#[allow(clippy::type_complexity)]
static SERVER_MODULE_CALLBACK_TABLES: Mutex<
    Vec<(usize, u64, std::pin::Pin<Box<ffi::pmix_server_module_t>>)>,
> = Mutex::new(Vec::new());

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerModule — safe wrapper around pmix_server_module_t
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust wrapper around the server module used during `PMIx_server_init`.
///
/// Each field corresponds to a host upcall the PMIx server library will
/// invoke when client requests arrive. All fields default to `None`,
/// meaning the upcall is not implemented by the server. The PMIx library
/// checks for null before calling each upcall, so it is safe to set only
/// the ones you need.
///
/// # Progress-thread rules (read before implementing any field)
///
/// OpenPMIx invokes these host upcalls from **progress context** (typically
/// the PMIx progress / event-engine thread). They are **not** a place to
/// pin application CPUs — pin progress separately
/// ([THREADING.md](../../THREADING.md) §4; helpers in [`crate::threading`]).
///
/// | Do | Don't |
/// |----|--------|
/// | Return quickly from the upcall | Call blocking PMIx APIs in-handler |
/// | Hop off with [`spawn_from_callback`](crate::threading::spawn_from_callback) or [`CallbackChannel`](crate::threading::CallbackChannel) | Join / park waiting for progress |
/// | Invoke the provided `cbfunc` **later** when RM work finishes | Block the upcall until remote collectives complete |
/// | Copy C buffers you need before hopping (`!Send` / lifetime ends at return) | Hold crate registry locks across user/RM code |
///
/// ```rust,ignore
/// // ❌ NEVER — blocks progress on a PMIx round-trip from a server upcall
/// let _ = pmix::data_ops::get(&proc, "pmix.job.size", None);
///
/// // ❌ NEVER — waits in-handler for work that needs progress
/// handle.join().unwrap();
///
/// // ✅ Hop, then complete asynchronously via cbfunc
/// let _ctx = pmix::threading::ProgressContext;
/// let cb = cbfunc; // capture
/// let chain = cbdata as usize; // *mut c_void is !Send
/// let _ = pmix::threading::spawn_from_callback(move || {
///     // RM / network work is safe here
///     if let Some(cb) = cb {
///         unsafe { cb.call(PmixStatus::from_raw(PMIX_SUCCESS), /* … */, chain as *mut _, ReleaseCallback::default(), std::ptr::null_mut()); }
///     }
/// });
/// // return PMIX_SUCCESS from the upcall immediately
/// ```
///
/// See `examples/server_upcall_hop.rs` for a complete fence_nb / direct_modex
/// pattern, and [`crate::threading`] / `examples/callback_hop.rs` for the
/// shared hop-off helpers used by client `_nb` completions and events.
///
/// # Using the module
///
/// Construct the module with the upcalls you need:
///
/// ```rust
/// use pmix::server::PmixServerModule;
///
/// let mut module = PmixServerModule::default();
/// // module.client_connected = Some(my_client_connected_fn);
/// ```
///
/// Pass it to [`crate::server::server_init`] or [`crate::server::PmixServer::connect_new`].
///
/// # Safety
///
/// Callback functions must remain valid until `PMIx_server_finalize` is
/// called (or the server is disconnected). Do not drop closures or
/// function pointers while the server is active.
#[repr(C)]
#[derive(Debug, Default)]
pub struct PmixServerModule {
    /// Called when a client process connects to this server.
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    ///
    /// Called when a client process connects to this server.
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    pub client_connected: Option<ServerClientConnectedCallback>,

    /// Called when a client process disconnects from this server
    /// (`client_finalized` upcall). Complete via the provided
    /// `pmix_op_cbfunc_t` wrapper after hopping if work blocks.
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    pub client_finalized: Option<ServerClientFinalizedCallback>,

    /// Called when a client requests an abort (`abort` upcall). The
    /// `procs` slice lists the processes to terminate — an empty slice is
    /// the C NULL array (all ranks in the client's nspace).
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    /// Complete via the provided `pmix_op_cbfunc_t` after hopping if work blocks.
    pub abort: Option<ServerAbortCallback>,

    /// Non-blocking fence / collective upcall (`PMIx_Fence` / `PMIx_Fence_nb`).
    ///
    /// OpenPMIx calls this once local participants have contributed. The
    /// host must share the provided modex blob (`data` / `ndata`) with peer
    /// daemons and complete via the provided [`ModexCallback`] — typically
    /// **after** hopping off progress (network / collective work must not
    /// block the upcall).
    ///
    /// **Not a CPU-pin target** — progress pinning is separate ([THREADING.md](../../THREADING.md)).
    /// Pattern: `examples/server_upcall_hop.rs`.
    pub fence_nb: Option<ServerModexCallback>,

    /// Direct modex request — fetch a remote proc's modex blob via the host.
    ///
    /// Same progress-thread rules as [`Self::fence_nb`]: schedule the remote
    /// fetch off-progress and invoke the provided [`ModexCallback`] when
    /// the blob is ready (or on error). See `examples/server_upcall_hop.rs`.
    pub direct_modex: Option<ServerDirectModexCallback>,

    /// Publish callback — client requests to publish data.
    pub publish: Option<ServerOpCallback<InfoView>>,

    /// Lookup callback — client requests to lookup data.
    ///
    /// The keys arrive as a borrowed string slice; the pdata results are
    /// handed back through the provided `pmix_lookup_cbfunc_t` wrapper.
    pub lookup: Option<ServerLookupCallback>,

    /// Unpublish callback — client requests to remove published data.
    pub unpublish: Option<ServerUnpublishCallback>,

    /// Spawn callback — client requests to spawn new processes.
    pub spawn: Option<ServerSpawnCallback>,

    /// Connect callback — client requests to establish a connection
    /// (completes via the `pmix_op_cbfunc_t` wrapper).
    pub connect: Option<ServerConnectCallback>,

    /// Disconnect callback — client requests to disconnect.
    pub disconnect: Option<ServerConnectCallback>,

    /// Register events callback.
    pub register_events: Option<ServerRegisterEventsCallback>,

    /// Deregister events callback.
    pub deregister_events: Option<ServerDeregisterEventsCallback>,

    /// Listener callback — register a host socket for connection
    /// harvesting; hand accepted sockets back via the provided
    /// `pmix_connection_cbfunc_t` wrapper.
    pub listener: Option<ServerListenerCallback>,

    /// Notify event callback — deliver notifications to the server.
    pub notify_event: Option<ServerNotifyEventCallback>,

    /// Query callback — client requests server-side query.
    pub query: Option<ServerQueryCallback>,

    /// Tool connection callback — accepts tool connections; the host
    /// assigns the tool's nspace/rank and completes via the provided
    /// `pmix_tool_connection_cbfunc_t` wrapper.
    pub tool_connected: Option<ServerToolConnectedCallback>,

    /// Log callback — client requests logging.
    pub log: Option<ServerLogCallback>,

    /// Allocation callback — client requests resource allocation.
    pub allocate: Option<ServerAllocateCallback>,

    /// Job control callback.
    pub job_control: Option<ServerJobControlCallback>,

    /// Monitoring callback — client requests monitoring.
    pub monitor: Option<ServerMonitorCallback>,

    /// Get credential callback.
    pub get_credential: Option<ServerCredentialCallback>,

    /// Validate credential callback (completes via the
    /// `pmix_validation_cbfunc_t` wrapper).
    pub validate_credential: Option<ServerValidateCredentialCallback>,

    /// I/O forwarding pull callback (registration completes via the
    /// `pmix_op_cbfunc_t` wrapper).
    pub iof_pull: Option<ServerIofPullCallback>,

    /// Push stdin callback.
    pub push_stdin: Option<ServerPushStdinCallback>,

    /// Group operations callback.
    pub group: Option<ServerGroupCallback>,

    /// Fabric operations callback.
    pub fabric: Option<ServerFabricCallback>,

    /// Client connected v2 callback.
    pub client_connected2: Option<ServerClientConnected2Callback>,

    /// Tool connection v2 callback.
    ///
    /// 6.x-only: absent from the OpenPMIx 5.0 server module.
    #[cfg(pmix6)]
    pub tool_connected2: Option<ServerToolConnected2Callback>,

    /// Log v2 callback.
    ///
    /// 6.x-only: absent from the OpenPMIx 5.0 server module.
    #[cfg(pmix6)]
    pub log2: Option<ServerLog2Callback>,

    /// Session control callback.
    pub session_control: Option<ServerSessionControlCallback>,

    /// Resource block callback.
    ///
    /// 6.x-only: absent from the OpenPMIx 5.0 server module.
    #[cfg(pmix6)]
    pub resource_block: Option<ServerResourceBlockCallback>,

    /// Internal marker for the converted C callback table storage.
    #[doc(hidden)]
    pub callback_state: ServerModuleCallbackState,
}

/// Registry machinery for the server-module host upcalls: one slot per
/// `PmixServerModule` callback field, storing the native (Rust-ABI) user
/// callback. The C-ABI trampolines below convert PMIx's raw arguments into
/// native view types and forward to the slot.
macro_rules! server_op_upcalls {
    ($($(#[$attribute:meta])* $field:ident: $callback:ty;)*) => {
        #[derive(Default)]
        struct ServerOpUpcalls {
            fence_nb: Option<ServerModexCallback>,
            $( $(#[$attribute])* $field: $callback, )*
        }

        impl ServerOpUpcalls {
            fn from_module(module: &PmixServerModule) -> Self {
                Self {
                    fence_nb: module.fence_nb,
                    $( $(#[$attribute])* $field: module.$field, )*
                }
            }
        }

        static SERVER_OP_UPCALLS: LazyLock<Mutex<ServerOpUpcalls>> =
            LazyLock::new(|| Mutex::new(ServerOpUpcalls::default()));
    };
}

server_op_upcalls! {
    client_connected: Option<ServerClientConnectedCallback>;
    client_finalized: Option<ServerClientFinalizedCallback>;
    abort: Option<ServerAbortCallback>;
    direct_modex: Option<ServerDirectModexCallback>;
    publish: Option<ServerOpCallback<InfoView>>;
    lookup: Option<ServerLookupCallback>;
    unpublish: Option<ServerUnpublishCallback>;
    spawn: Option<ServerSpawnCallback>;
    connect: Option<ServerConnectCallback>;
    disconnect: Option<ServerConnectCallback>;
    register_events: Option<ServerRegisterEventsCallback>;
    deregister_events: Option<ServerDeregisterEventsCallback>;
    listener: Option<ServerListenerCallback>;
    notify_event: Option<ServerNotifyEventCallback>;
    query: Option<ServerQueryCallback>;
    tool_connected: Option<ServerToolConnectedCallback>;
    log: Option<ServerLogCallback>;
    allocate: Option<ServerAllocateCallback>;
    job_control: Option<ServerJobControlCallback>;
    monitor: Option<ServerMonitorCallback>;
    get_credential: Option<ServerCredentialCallback>;
    validate_credential: Option<ServerValidateCredentialCallback>;
    iof_pull: Option<ServerIofPullCallback>;
    push_stdin: Option<ServerPushStdinCallback>;
    group: Option<ServerGroupCallback>;
    fabric: Option<ServerFabricCallback>;
    client_connected2: Option<ServerClientConnected2Callback>;
    #[cfg(pmix6)]
    tool_connected2: Option<ServerToolConnected2Callback>;
    #[cfg(pmix6)]
    log2: Option<ServerLog2Callback>;
    session_control: Option<ServerSessionControlCallback>;
    #[cfg(pmix6)]
    resource_block: Option<ServerResourceBlockCallback>;
}

// ─────────────────────────────────────────────────────────────────────────────
// C-ABI trampolines. Each trampoline's `unsafe extern "C"` signature is
// IDENTICAL to its bindgen `pmix_server_module_t` slot typedef (argument
// order, argument types, cbfunc parameter types, and the
// `-> ffi::pmix_status_t` return) — `PmixServerModule::as_c_ptr` assigns
// them into the typed `ffi::pmix_server_module_t`, so rustc verifies every
// signature against the bindgen type at compile time, under both the
// pmix5 and pmix6 bindings. Each trampoline converts raw pointers/lengths
// into native view types, wraps the completion callback, and forwards to
// the user callback registered in `SERVER_OP_UPCALLS`.
//
// Status-return semantics — the one-call-then-SUCCESS policy. OpenPMIx
// has BOTH caller shapes for these upcalls (verified in the 5.x and the
// 6.1.0 trees):
//
// * Callers that HONOR the returned status and release/complete the
//   request themselves on any non-success return — src/server/
//   pmix_server_ops.c (publish/lookup/unpublish/spawn/abort),
//   src/server/pmix_server_fence.c (fence_nb calls trk->modexcbfunc
//   itself), src/server/pmix_server_connect.c, src/mca/ptl/base/
//   ptl_base_connection_hdlr.c, src/mca/ptl/base/ptl_base_sendrecv.c
//   (fence/connect/disconnect PMIX_RELEASE(trk)), and src/server/
//   pmix_server.c client_finalized (invokes op_cbfunc2 itself unless
//   the return is PMIX_SUCCESS). For these, a host that both calls the
//   C cbfunc AND returns non-success gets a double release.
// * Callers that IGNORE the returned status and hang unless the host
//   invokes the C cbfunc — src/server/pmix_server.c
//   pmix_server_execute_collective, a LIVE path (nspace registration
//   pushes completed trackers through PMIX_EXECUTE_COLLECTIVE) that
//   calls fence_nb, connect, and disconnect and discards rc. For
//   these, a host that returns non-success without calling the C
//   cbfunc leaks the tracker and hangs the collective.
//
// One policy satisfies both lists: a trampoline panic BEFORE
// completion invokes the C cbfunc exactly ONCE with PMIX_ERROR and
// returns PMIX_SUCCESS (when no C cbfunc was supplied at all, see the
// PMIX_ERROR arm below). Status-checking callers treat PMIX_SUCCESS as
// "the host owns the callback" and do not release or complete again;
// execute_collective ignores the status but still gets the callback,
// so the collective unblocks. A panic AFTER completion (the
// [`CompletionTaken`] claim already won by the host's own completion)
// calls nothing and also returns PMIX_SUCCESS — the completion already
// released cbdata, and a non-success return would make the
// status-checking callers release it a second time.
//
// * `PMIX_SUCCESS` — the host callback accepted the request and will
//   complete it via the supplied completion callback; OR it completed
//   through the wrapper and THEN panicked; OR it panicked and the
//   trampoline completed the C cbfunc with PMIX_ERROR itself. In
//   every case the C cbfunc is invoked at most once and the PMIx
//   caller treats the return as "the host owns the callback".
// * `PMIX_ERROR` — the host panicked, the trampoline won the
//   [`CompletionTaken`] claim, but no C completion callback was
//   supplied, so `complete_*_on_panic` invoked nothing. Returning
//   SUCCESS here would leak `cbdata` (the status-honoring callers
//   would read it as "host owns the callback" and never release);
//   PMIX_ERROR makes them release it themselves. This arm cannot
//   double-release: nothing was invoked.
// * `PMIX_ERR_NOT_SUPPORTED` — the field is `None`; cbdata is NOT
//   touched (PMIx does not expect a completion in this case).
//
// The claim is a compare-exchange on the [`CompletionTaken`] flag each
// trampoline shares with its completion wrapper: the wrapper invokes
// the C cbfunc only when IT wins the claim, the trampoline's panic
// path invokes it only when the wrapper has not claimed, and the
// async-handoff race (completion running on another thread while the
// upcall thread panics) still leaves exactly one winner. Void-returning
// slots (`tool_connected`, `log`) have no status to signal with: on a
// panic BEFORE completion they still complete the C cbfunc with the
// panic status — the only way to unblock the requester — and never
// once the claim is lost.
// ─────────────────────────────────────────────────────────────────────────────

/// `PMIX_SUCCESS` for trampoline returns.
fn status_success() -> ffi::pmix_status_t {
    PmixStatus::Known(PmixError::Success).to_raw()
}

/// `PMIX_ERR_NOT_SUPPORTED` for trampoline returns (field is `None`).
fn status_not_supported() -> ffi::pmix_status_t {
    PmixStatus::Known(PmixError::ErrNotSupported).to_raw()
}

/// `PMIX_ERROR` — the status a trampoline-supplied panic completion
/// carries into the C completion callback, and the trampoline's own
/// return when a won claim found no C callback to invoke (see the
/// status-return policy above).
fn status_panic() -> ffi::pmix_status_t {
    PmixStatus::Known(PmixError::Error).to_raw()
}

/// One-shot completion-ownership claim shared between a server-module
/// trampoline and the completion wrapper it hands to the host callback.
///
/// Both sides invoke the C completion callback only after winning the
/// [`Self::claim`] compare-exchange, so the C `cbdata` has exactly one
/// owner even when an asynchronous completion races a trampoline panic:
///
/// * **wrapper wins** — the host completed through the wrapper (or via
///   the raw pointer extracted with `into_c_callback`, which CLAIMS the
///   right on extract). The trampoline's panic path loses the claim,
///   calls nothing, and returns `PMIX_SUCCESS`: the completion already
///   released `cbdata`, and a non-success return would make the PMIx
///   caller release it a second time.
/// * **trampoline wins** — the host panicked without completing. The
///   trampoline completes the C callback exactly once with `PMIX_ERROR`
///   and returns `PMIX_SUCCESS`: status-honoring PMIx callers treat
///   SUCCESS as "the host owns the callback", while
///   pmix_server_execute_collective ignores the status and hangs
///   unless the callback fires. If NO C callback was supplied, nothing
///   can be invoked and the trampoline returns `PMIX_ERROR` instead,
///   so the status-honoring callers release `cbdata` themselves.
#[derive(Clone, Debug, Default)]
pub(crate) struct CompletionTaken(Arc<AtomicBool>);

impl CompletionTaken {
    /// A fresh, unclaimed flag.
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Claim the one-shot right to invoke the C completion callback:
    /// the first claim wins, every later caller (the trampoline's panic
    /// path, the raw-pointer extract path, or a second completion
    /// attempt) loses and must invoke nothing. The wrapper invokes the C callback only if it wins; the
    /// trampoline's panic path invokes it only if it wins — the loser
    /// never calls, closing the async-handoff double-completion window.
    pub(crate) fn claim(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

/// The return status for a status-returning trampoline after the user
/// callback ran, keeping exactly one owner of the C `cbdata`:
///
/// * `ok` — the host accepted the request and will complete it (or has
///   already completed it): `PMIX_SUCCESS`.
/// * panicked — the trampoline tries to [`CompletionTaken::claim`] the
///   one-shot completion right. If it wins (the host never completed),
///   it invokes the C completion exactly once with `PMIX_ERROR` via
///   `complete_on_panic` and returns `PMIX_SUCCESS`: status-honoring
///   PMIx callers (pmix_server_ops.c, pmix_server_fence.c,
///   pmix_server_connect.c, ptl_base_sendrecv.c) treat SUCCESS as
///   "the host owns the callback" and do not release `cbdata` again,
///   while the status-ignoring pmix_server_execute_collective still
///   gets the callback it needs to unblock the collective. If the
///   claim wins but no C completion callback was supplied,
///   `complete_on_panic` reports that nothing was invoked and the
///   trampoline returns `PMIX_ERROR` instead, so a status-honoring
///   caller releases `cbdata` itself rather than leaking it.
/// * panicked with the claim lost (the host completed through the
///   wrapper before or while panicking): nothing is invoked and the
///   return is `PMIX_SUCCESS` — the completion right was the host's.
fn status_after_panic(
    ok: bool,
    taken: &CompletionTaken,
    complete_on_panic: impl FnOnce() -> bool,
) -> ffi::pmix_status_t {
    // Short-circuit order matters: claim() runs only when the host did not
    // complete (or panic), and the completion runs only when the claim wins.
    if !ok && taken.claim() && !complete_on_panic() {
        // The claim was won but there was no C completion callback to
        // invoke: report the failure so a status-honoring PMIx caller
        // (pmix_server_ops.c & friends) releases `cbdata` itself instead
        // of leaking it on a SUCCESS it reads as "host owns the callback".
        return status_panic();
    }
    status_success()
}

/// Complete a raw `pmix_op_cbfunc_t` chain with the panic status. Used
/// by every status-returning trampoline whose completion is an
/// `pmix_op_cbfunc_t` (client_connected/finalized, abort, publish,
/// unpublish, connect, disconnect, register/deregister_events,
/// notify_event, iof_pull, push_stdin, log2, resource_block) and by the
/// void-returning `log` slot. Callers must have WON the
/// [`CompletionTaken`] claim first — this helper invokes the C
/// completion unconditionally when the pointer is non-null.
unsafe fn complete_op_on_panic(cbfunc: ffi::pmix_op_cbfunc_t, cbdata: *mut c_void) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe { cbfunc(status_panic(), cbdata) };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_modex_cbfunc_t` chain with the panic status: no
/// blob and no release pair. `_mdxcbfunc` checks the status before it
/// touches the data, so a (NULL, 0) payload is safe on the error path —
/// pmix_server_fence.c makes the same call itself when fence_nb returns
/// non-success. Callers must have WON the [`CompletionTaken`] claim.
unsafe fn complete_modex_on_panic(cbfunc: ffi::pmix_modex_cbfunc_t, cbdata: *mut c_void) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe {
            cbfunc(
                status_panic(),
                ptr::null(),
                0,
                cbdata,
                None,
                ptr::null_mut(),
            )
        };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_lookup_cbfunc_t` chain with the panic status:
/// no pdata array (`lookup_cbfunc` NULL-checks it before copying).
/// Callers must have WON the [`CompletionTaken`] claim.
unsafe fn complete_lookup_on_panic(cbfunc: ffi::pmix_lookup_cbfunc_t, cbdata: *mut c_void) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe { cbfunc(status_panic(), ptr::null_mut(), 0, cbdata) };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_spawn_cbfunc_t` chain with the panic status: no
/// nspace (`spawn_cbfunc` NULL-checks it before strdup). Callers must
/// have WON the [`CompletionTaken`] claim.
unsafe fn complete_spawn_on_panic(cbfunc: ffi::pmix_spawn_cbfunc_t, cbdata: *mut c_void) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe { cbfunc(status_panic(), ptr::null_mut(), cbdata) };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_info_cbfunc_t` chain with the panic status: no
/// info array and no release pair (`query_cbfunc` and siblings
/// NULL-check before dereferencing). Used by query, allocate,
/// job_control, monitor, group, fabric, and session_control. Callers
/// must have WON the [`CompletionTaken`] claim.
unsafe fn complete_info_on_panic(cbfunc: ffi::pmix_info_cbfunc_t, cbdata: *mut c_void) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe {
            cbfunc(
                status_panic(),
                ptr::null_mut(),
                0,
                cbdata,
                None,
                ptr::null_mut(),
            )
        };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_credential_cbfunc_t` chain with the panic
/// status. The credential must be a VALID zeroed
/// `pmix_byte_object_t`, not NULL: `cred_cbfunc` copies it
/// unconditionally in both the 5.x and 6.1.0 trees. Callers must have
/// WON the [`CompletionTaken`] claim.
unsafe fn complete_credential_on_panic(
    cbfunc: ffi::pmix_credential_cbfunc_t,
    cbdata: *mut c_void,
) -> bool {
    if let Some(cbfunc) = cbfunc {
        let credential = ffi::pmix_byte_object_t {
            bytes: ptr::null_mut(),
            size: 0,
        };
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion; the zeroed byte object is
        // valid input for the copy cred_cbfunc performs.
        unsafe {
            cbfunc(
                status_panic(),
                &credential as *const _ as *mut _,
                ptr::null_mut(),
                0,
                cbdata,
            )
        };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_validation_cbfunc_t` chain with the panic
/// status: no info array (`validate_cbfunc` NULL-checks it). Callers
/// must have WON the [`CompletionTaken`] claim.
unsafe fn complete_validation_on_panic(
    cbfunc: ffi::pmix_validation_cbfunc_t,
    cbdata: *mut c_void,
) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback and its opaque
        // data for exactly one completion.
        unsafe { cbfunc(status_panic(), ptr::null_mut(), 0, cbdata) };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Complete a raw `pmix_tool_connection_cbfunc_t` chain with the panic
/// status. Used by the void-returning `tool_connected` slot and the
/// (6.x) status-returning `tool_connected2` slot. Callers must have WON
/// the [`CompletionTaken`] claim.
unsafe fn complete_tool_connection_on_panic(
    cbfunc: ffi::pmix_tool_connection_cbfunc_t,
    cbdata: *mut c_void,
) -> bool {
    if let Some(cbfunc) = cbfunc {
        // SAFETY: PMIx supplied this completion callback for exactly one
        // completion. `cnct_cbfunc` (ptl_base_connection_hdlr.c) logs
        // proc->nspace BEFORE its NULL check, so pass a zeroed
        // pmix_proc_t — a NULL proc would crash under verbosity >= 2.
        let mut proc: ffi::pmix_proc_t = unsafe { std::mem::zeroed() };
        unsafe { cbfunc(status_panic(), &mut proc, cbdata) };
        true
    } else {
        // No C completion callback was supplied: nothing was
        // invoked, so the trampoline reports PMIX_ERROR to
        // let status-honoring callers release cbdata.
        false
    }
}

/// Borrow a nullable C string as a native `&str` for the synchronous
/// duration of an upcall.
unsafe fn borrow_cstr<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: PMIx upcall strings are NUL-terminated and live for the
    // duration of the synchronous upcall.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Borrow a nullable `pmix_byte_object_t` payload for the synchronous
/// duration of an upcall.
unsafe fn borrow_byte_object<'a>(bo: *const ffi::pmix_byte_object_t) -> Option<&'a [u8]> {
    if bo.is_null() {
        return None;
    }
    // SAFETY: PMIx supplies the initialized byte object for the duration
    // of the synchronous upcall; null bytes + zero size maps to empty.
    unsafe {
        let bo = &*bo;
        Some(view_slice(bo.bytes.cast::<u8>().cast_const(), bo.size))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Slot trampolines (slot order matches pmix_server_module_t)
// ─────────────────────────────────────────────────────────────────────────────

/// `client_connected` slot: `(proc, server_object, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn client_connected(
    proc: *const ffi::pmix_proc_t,
    server_object: *mut c_void,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_connected;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the connecting client's proc for the duration
    // of this upcall.
    let proc = unsafe { ProcView::from_raw_ptr(proc) };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, server_object, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `client_finalized` slot: `(proc, server_object, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn client_finalized(
    proc: *const ffi::pmix_proc_t,
    server_object: *mut c_void,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_finalized;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the finalizing client's proc for this upcall.
    let proc = unsafe { ProcView::from_raw_ptr(proc) };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, server_object, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `abort` slot: `(proc, server_object, status, msg, procs, nprocs,
/// op_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn abort(
    proc: *const ffi::pmix_proc_t,
    server_object: *mut c_void,
    status: c_int,
    msg: *const c_char,
    procs: *mut ffi::pmix_proc_t,
    nprocs: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .abort;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the aborting client's proc, the message string,
    // and the procs-to-kill array (NULL + 0 = all ranks in the nspace) for
    // this upcall.
    let (proc, msg, procs) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            borrow_cstr(msg),
            view_slice(procs.cast::<ProcView>().cast_const(), nprocs),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, server_object, status, msg, procs, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `fence_nb` slot: `(procs, nprocs, info, ninfo, data, ndata,
/// modex_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn fence_nb(
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    data: *mut c_char,
    ndata: usize,
    cbfunc: ffi::pmix_modex_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .fence_nb;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the participant array, the info array, and the
    // modex blob for the duration of this upcall; null + 0 maps to an
    // empty slice.
    let (procs, info) = unsafe {
        (
            view_slice(procs.cast::<ProcView>(), nprocs),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    let taken = CompletionTaken::new();
    let completion = ModexCallback::from_raw_tracked(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        // SAFETY: raw arguments are converted to native views at the FFI
        // boundary; the completion callback is wrapped.
        callback(procs, info, data, ndata, completion, cbdata);
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_modex_on_panic(cbfunc, cbdata) }
    })
}

/// `direct_modex` slot: `(proc, info, ninfo, modex_cbfunc, cbdata) ->
/// pmix_status_t`. The slot's completion is `pmix_modex_cbfunc_t` — the
/// same shape as `fence_nb`'s, NOT `pmix_dmodex_response_fn_t`.
unsafe extern "C" fn direct_modex(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_modex_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .direct_modex;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc and the info array for
    // this upcall; null + 0 maps to an empty slice.
    let (proc, info) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    let taken = CompletionTaken::new();
    let completion = ModexCallback::from_raw_tracked(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_modex_on_panic(cbfunc, cbdata) }
    })
}

/// `publish` slot: `(proc, info, ninfo, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn publish(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .publish;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies valid proc/info arrays for this upcall; null +
    // 0 maps to an empty slice.
    let (proc, info) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `lookup` slot: `(proc, keys, info, ninfo, lookup_cbfunc, cbdata) ->
/// pmix_status_t`. The pdata array is an OUTPUT of the completion — the
/// keys arrive as a borrowed string slice.
unsafe extern "C" fn lookup(
    proc: *const ffi::pmix_proc_t,
    keys: *mut *mut c_char,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_lookup_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .lookup;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the NULL-terminated keys
    // array, and the info array for this upcall; null + 0 maps to an
    // empty slice. The borrowed key strings live on this trampoline's
    // stack for the synchronous duration of the host call.
    let (proc, info, key_vec) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(info.cast::<InfoView>(), ninfo),
            crate::views::read_str_array(keys),
        )
    };
    let keys = key_vec.as_slice();
    let taken = CompletionTaken::new();
    let completion = callbacks::LookupCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, keys, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_lookup_on_panic(cbfunc, cbdata) }
    })
}

/// `unpublish` slot: `(proc, keys, info, ninfo, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn unpublish(
    proc: *const ffi::pmix_proc_t,
    keys: *mut *mut c_char,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .unpublish;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the NULL-terminated keys
    // array, and the info array for this upcall; null + 0 maps to an
    // empty slice. The borrowed key strings live on this trampoline's
    // stack for the synchronous duration of the host call.
    let (proc, info, key_vec) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(info.cast::<InfoView>(), ninfo),
            crate::views::read_str_array(keys),
        )
    };
    let keys = key_vec.as_slice();
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, keys, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `spawn` slot: `(proc, job_info, ninfo, apps, napps, spawn_cbfunc,
/// cbdata) -> pmix_status_t`.
unsafe extern "C" fn spawn(
    proc: *const ffi::pmix_proc_t,
    job_info: *const ffi::pmix_info_t,
    ninfo: usize,
    apps: *const ffi::pmix_app_t,
    napps: usize,
    cbfunc: ffi::pmix_spawn_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .spawn;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the spawning proc, the job info array, and the
    // app array for this upcall; null + 0 maps to an empty slice.
    let (proc, job_info, apps) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(job_info.cast::<InfoView>(), ninfo),
            view_slice(apps.cast::<AppView>(), napps),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::SpawnCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, job_info, apps, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_spawn_on_panic(cbfunc, cbdata) }
    })
}

/// `connect` slot: `(procs, nprocs, info, ninfo, op_cbfunc, cbdata) ->
/// pmix_status_t`. Connect completes via `pmix_op_cbfunc_t` — the
/// connection accept callback belongs to the `listener` slot.
unsafe extern "C" fn connect(
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .connect;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the connecting procs array and the info array
    // for this upcall; null + 0 maps to an empty slice. Procs are
    // `pmix_proc_t` elements — ProcView, never a nspace view.
    let (procs, info) = unsafe {
        (
            view_slice(procs.cast::<ProcView>(), nprocs),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(procs, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `disconnect` slot: `(procs, nprocs, info, ninfo, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn disconnect(
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .disconnect;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the disconnecting procs array and the info
    // array for this upcall; null + 0 maps to an empty slice.
    let (procs, info) = unsafe {
        (
            view_slice(procs.cast::<ProcView>(), nprocs),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(procs, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `register_events` slot: `(codes, ncodes, info, ninfo, op_cbfunc,
/// cbdata) -> pmix_status_t`.
unsafe extern "C" fn register_events(
    codes: *mut ffi::pmix_status_t,
    ncodes: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .register_events;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the status-code array and the info array for
    // this upcall; null + 0 maps to an empty slice. `pmix_status_t` is
    // `c_int`, so the elements are borrowed as `i32`.
    let (codes, info) = unsafe {
        (
            view_slice(codes.cast::<i32>().cast_const(), ncodes),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(codes, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `deregister_events` slot: `(codes, ncodes, op_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn deregister_events(
    codes: *mut ffi::pmix_status_t,
    ncodes: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .deregister_events;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the status-code array for this upcall; null + 0
    // maps to an empty slice.
    let codes = unsafe { view_slice(codes.cast::<i32>().cast_const(), ncodes) };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(codes, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `listener` slot: `(listening_sd, connection_cbfunc, cbdata) ->
/// pmix_status_t`.
///
/// The completion is the connection accept callback — it hands accepted
/// socket descriptors back to PMIx and cannot express an error status. If
/// the user callback panics, the registration is dropped (PMIx falls back
/// to its own listener) and `PMIX_ERROR` is returned.
unsafe extern "C" fn listener(
    listening_sd: c_int,
    cbfunc: ffi::pmix_connection_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .listener;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    let completion = callbacks::ConnectionCallback::from_raw(cbfunc);
    let ok = invoke_user_callback("server", move || unsafe {
        callback(listening_sd, completion, cbdata)
    });
    if ok {
        status_success()
    } else {
        // `pmix_connection_cbfunc_t` takes a socket descriptor, not a
        // status — there is no error completion to deliver here.
        status_panic()
    }
}

/// `notify_event` slot: `(code, source, range, info, ninfo, op_cbfunc,
/// cbdata) -> pmix_status_t`. There is no results array in this slot —
/// the notification info is input data only.
unsafe extern "C" fn notify_event(
    code: ffi::pmix_status_t,
    source: *const ffi::pmix_proc_t,
    range: ffi::pmix_data_range_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .notify_event;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the generating proc and the notification info
    // array for this upcall; null + 0 maps to an empty slice.
    let (source, info) = unsafe {
        (
            ProcView::from_raw_ptr(source),
            view_slice(info.cast::<InfoView>().cast_const(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(code, source, range, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `query` slot: `(proct, queries, nqueries, info_cbfunc, cbdata) ->
/// pmix_status_t`.
unsafe extern "C" fn query(
    proct: *mut ffi::pmix_proc_t,
    queries: *mut ffi::pmix_query_t,
    nqueries: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .query;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the querying proc and the query array for
    // this upcall; null + 0 maps to an empty slice.
    let (proc, queries) = unsafe {
        (
            ProcView::from_raw_ptr(proct.cast_const()),
            view_slice(queries.cast::<QueryView>().cast_const(), nqueries),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, queries, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `tool_connected` slot: `(info, ninfo, tool_connection_cbfunc, cbdata)`
/// — void-returning (the 5.x/6.x slot has no status return).
unsafe extern "C" fn tool_connected(
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_tool_connection_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .tool_connected;
    let Some(callback) = callback else {
        // The slot returns void; nothing to report. cbdata is untouched.
        return;
    };
    // SAFETY: PMIx supplies the tool's info array for this upcall; null +
    // 0 maps to an empty slice.
    let info = unsafe { view_slice(info.cast::<InfoView>().cast_const(), ninfo) };
    let taken = CompletionTaken::new();
    let completion = callbacks::ToolConnectionCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(info, completion, cbdata)
    });
    if !ok && taken.claim() {
        // No status return on this slot, so completing the C cbfunc is
        // the only way to unblock the tool: the host panicked before
        // completing, so complete with the panic status here — but only
        // when the claim is won. The claim is shared with the completion
        // wrapper (and its raw-pointer extraction), so a host that
        // completed first — or a completion racing this panic path from
        // another thread — leaves the C cbfunc called exactly once.
        unsafe { complete_tool_connection_on_panic(cbfunc, cbdata) };
    }
}

/// `log` slot: `(client, data, ndata, directives, ndirs, op_cbfunc,
/// cbdata)` — void-returning.
unsafe extern "C" fn log(
    client: *const ffi::pmix_proc_t,
    data: *const ffi::pmix_info_t,
    ndata: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .log;
    let Some(callback) = callback else {
        return;
    };
    // SAFETY: PMIx supplies the logging client plus both info arrays for
    // this upcall; null + 0 maps to an empty slice.
    let (client, data, directives) = unsafe {
        (
            ProcView::from_raw_ptr(client),
            view_slice(data.cast::<InfoView>(), ndata),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(client, data, directives, completion, cbdata)
    });
    if !ok && taken.claim() {
        // No status return on this slot, so completing the C cbfunc is
        // the only way to unblock the requester: the host panicked
        // before completing, so complete with the panic status here —
        // but only when the claim is won. The claim is shared with the
        // completion wrapper (and its raw-pointer extraction), so a
        // host that completed first — or a completion racing this
        // panic path from another thread — leaves the C cbfunc called
        // exactly once.
        unsafe { complete_op_on_panic(cbfunc, cbdata) };
    }
}

/// `allocate` slot: `(client, alloc_directive, data, ndata, info_cbfunc,
/// cbdata) -> pmix_status_t`.
unsafe extern "C" fn allocate(
    client: *const ffi::pmix_proc_t,
    directive: ffi::pmix_alloc_directive_t,
    data: *const ffi::pmix_info_t,
    ndata: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .allocate;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting client and the info array for
    // this upcall; null + 0 maps to an empty slice.
    let (client, data) = unsafe {
        (
            ProcView::from_raw_ptr(client),
            view_slice(data.cast::<InfoView>(), ndata),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(client, directive, data, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `job_control` slot: `(requestor, targets, ntargets, directives, ndirs,
/// info_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn job_control(
    requestor: *const ffi::pmix_proc_t,
    targets: *const ffi::pmix_proc_t,
    ntargets: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .job_control;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the target procs array,
    // and the directives array for this upcall; null + 0 maps to an empty
    // slice (empty targets = all ranks in the requestor's nspace).
    let (requestor, targets, directives) = unsafe {
        (
            ProcView::from_raw_ptr(requestor),
            view_slice(targets.cast::<ProcView>(), ntargets),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(requestor, targets, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `monitor` slot: `(requestor, monitor, error, directives, ndirs,
/// info_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn monitor(
    requestor: *const ffi::pmix_proc_t,
    monitor: *const ffi::pmix_info_t,
    error: ffi::pmix_status_t,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .monitor;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the single monitor info
    // entry, and the directives array for this upcall; null maps to
    // None / an empty slice.
    let (requestor, monitor, directives) = unsafe {
        (
            ProcView::from_raw_ptr(requestor),
            (!monitor.is_null()).then(|| InfoView::from_raw_ptr(monitor)),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(requestor, monitor, error, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `get_credential` slot: `(proc, directives, ndirs, credential_cbfunc,
/// cbdata) -> pmix_status_t`.
unsafe extern "C" fn get_credential(
    proc: *const ffi::pmix_proc_t,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_credential_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_credential;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc and the directives array
    // for this upcall; null + 0 maps to an empty slice.
    let (proc, directives) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::CredentialCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_credential_on_panic(cbfunc, cbdata) }
    })
}

/// `validate_credential` slot: `(proc, cred, directives, ndirs,
/// validation_cbfunc, cbdata) -> pmix_status_t`. The completion is
/// `pmix_validation_cbfunc_t` — it takes `(status, info, ninfo, cbdata)`,
/// NOT the credential byte object.
unsafe extern "C" fn validate_credential(
    proc: *const ffi::pmix_proc_t,
    cred: *const ffi::pmix_byte_object_t,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_validation_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .validate_credential;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the credential byte
    // object, and the directives array for this upcall; null maps to
    // None / an empty slice.
    let (proc, cred, directives) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            borrow_byte_object(cred),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::ValidationCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, cred, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_validation_on_panic(cbfunc, cbdata) }
    })
}

/// `iof_pull` slot: `(procs, nprocs, directives, ndirs, channels,
/// op_cbfunc, cbdata) -> pmix_status_t`. Registration completes via
/// `pmix_op_cbfunc_t`; `pmix_iof_cbfunc_t` is the client-side delivery
/// callback, not this slot's completion.
unsafe extern "C" fn iof_pull(
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    channels: ffi::pmix_iof_channel_t,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iof_pull;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the source procs array and the directives
    // array for this upcall; null + 0 maps to an empty slice.
    let (procs, directives) = unsafe {
        (
            view_slice(procs.cast::<ProcView>(), nprocs),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(procs, directives, channels, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `push_stdin` slot: `(source, targets, ntargets, directives, ndirs, bo,
/// op_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn push_stdin(
    source: *const ffi::pmix_proc_t,
    targets: *const ffi::pmix_proc_t,
    ntargets: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    bo: *const ffi::pmix_byte_object_t,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push_stdin;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the source proc, the target procs array, the
    // directives array, and the stdin byte object for this upcall; null
    // maps to None / an empty slice.
    let (source, targets, directives, bo) = unsafe {
        (
            ProcView::from_raw_ptr(source),
            view_slice(targets.cast::<ProcView>(), ntargets),
            view_slice(directives.cast::<InfoView>(), ndirs),
            borrow_byte_object(bo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(source, targets, directives, bo, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `group` slot: `(op, grp, procs, nprocs, directives, ndirs,
/// info_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn group(
    op: ffi::pmix_group_operation_t,
    grp: *mut c_char,
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .group;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the group name string, the member procs array
    // (ProcView elements — never a nspace list), and the directives array
    // for this upcall; null maps to None / an empty slice.
    let (grp, procs, directives) = unsafe {
        (
            borrow_cstr(grp.cast_const()),
            view_slice(procs.cast::<ProcView>(), nprocs),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(op as u32, grp, procs, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `fabric` slot: `(requestor, op, directives, ndirs, info_cbfunc,
/// cbdata) -> pmix_status_t`.
unsafe extern "C" fn fabric(
    requestor: *const ffi::pmix_proc_t,
    op: ffi::pmix_fabric_operation_t,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .fabric;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc and the directives array
    // for this upcall; null + 0 maps to an empty slice.
    let (requestor, directives) = unsafe {
        (
            ProcView::from_raw_ptr(requestor),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(requestor, op as u32, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `client_connected2` slot: `(proc, server_object, info, ninfo,
/// op_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn client_connected2(
    proc: *const ffi::pmix_proc_t,
    server_object: *mut c_void,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_connected2;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the connecting client's proc and info array
    // for the duration of this upcall; null + 0 maps to an empty slice.
    let (proc, info) = unsafe {
        (
            ProcView::from_raw_ptr(proc),
            view_slice(info.cast::<InfoView>().cast_const(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(proc, server_object, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `tool_connected2` slot (6.x-only): `(info, ninfo,
/// tool_connection_cbfunc, cbdata) -> pmix_status_t`.
#[cfg(pmix6)]
unsafe extern "C" fn tool_connected2(
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_tool_connection_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .tool_connected2;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the tool's info array for this upcall; null +
    // 0 maps to an empty slice.
    let info = unsafe { view_slice(info.cast::<InfoView>().cast_const(), ninfo) };
    let taken = CompletionTaken::new();
    let completion = callbacks::ToolConnectionCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_tool_connection_on_panic(cbfunc, cbdata) }
    })
}

/// `log2` slot (6.x-only): `(client, data, ndata, directives, ndirs,
/// op_cbfunc, cbdata) -> pmix_status_t`.
#[cfg(pmix6)]
unsafe extern "C" fn log2(
    client: *const ffi::pmix_proc_t,
    data: *const ffi::pmix_info_t,
    ndata: usize,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .log2;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the logging client plus both info arrays for
    // this upcall; null + 0 maps to an empty slice.
    let (client, data, directives) = unsafe {
        (
            ProcView::from_raw_ptr(client),
            view_slice(data.cast::<InfoView>(), ndata),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(client, data, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

/// `session_control` slot: `(requestor, sessionID, directives, ndirs,
/// info_cbfunc, cbdata) -> pmix_status_t`.
unsafe extern "C" fn session_control(
    requestor: *const ffi::pmix_proc_t,
    session_id: u32,
    directives: *const ffi::pmix_info_t,
    ndirs: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .session_control;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc and the directives array
    // for this upcall; null + 0 maps to an empty slice.
    let (requestor, directives) = unsafe {
        (
            ProcView::from_raw_ptr(requestor),
            view_slice(directives.cast::<InfoView>(), ndirs),
        )
    };
    let taken = CompletionTaken::new();
    let completion = callbacks::InfoCallback::from_raw(cbfunc, taken.clone());
    let ok = invoke_user_callback("server", move || unsafe {
        callback(requestor, session_id, directives, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_info_on_panic(cbfunc, cbdata) }
    })
}

/// `resource_block` slot (6.x-only): `(requestor, directive, block, units,
/// nunit, info, ninfo, op_cbfunc, cbdata) -> pmix_status_t`.
#[cfg(pmix6)]
unsafe extern "C" fn resource_block(
    requestor: *const ffi::pmix_proc_t,
    directive: ffi::pmix_resource_block_directive_t,
    block: *const c_char,
    units: *const ffi::pmix_resource_unit_t,
    nunit: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) -> ffi::pmix_status_t {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .resource_block;
    let Some(callback) = callback else {
        return status_not_supported();
    };
    // SAFETY: PMIx supplies the requesting proc, the block name string,
    // the units array, and the info array for this upcall; null maps to
    // None / an empty slice.
    let (requestor, block, units, info) = unsafe {
        (
            ProcView::from_raw_ptr(requestor),
            borrow_cstr(block),
            view_slice(units.cast::<ResourceUnitView>(), nunit),
            view_slice(info.cast::<InfoView>(), ninfo),
        )
    };
    // SAFETY: PMIx supplies the completion callback and its opaque data.
    let taken = CompletionTaken::new();
    let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata, taken.clone()) };
    let ok = invoke_user_callback("server", move || unsafe {
        callback(requestor, directive, block, units, info, completion, cbdata)
    });
    status_after_panic(ok, &taken, || {
        // SAFETY: PMIx supplied this completion callback and its
        // opaque data for exactly one completion; the claim was
        // won above, so this is the single panic completion.
        unsafe { complete_op_on_panic(cbfunc, cbdata) }
    })
}

impl PmixServerModule {
    /// Bitmask of the installed host upcalls (one bit per slot, in
    /// `pmix_server_module_t` order). The trampolines are static functions,
    /// so two conversions with the same mask install identical C contents.
    fn callback_mask(&self) -> u64 {
        let mut mask: u64 = 0;
        macro_rules! slot {
            ($field:ident, $bit:expr) => {
                if self.$field.is_some() {
                    mask |= 1 << $bit;
                }
            };
        }
        slot!(client_connected, 0);
        slot!(client_finalized, 1);
        slot!(abort, 2);
        slot!(fence_nb, 3);
        slot!(direct_modex, 4);
        slot!(publish, 5);
        slot!(lookup, 6);
        slot!(unpublish, 7);
        slot!(spawn, 8);
        slot!(connect, 9);
        slot!(disconnect, 10);
        slot!(register_events, 11);
        slot!(deregister_events, 12);
        slot!(listener, 13);
        slot!(notify_event, 14);
        slot!(query, 15);
        slot!(tool_connected, 16);
        slot!(log, 17);
        slot!(allocate, 18);
        slot!(job_control, 19);
        slot!(monitor, 20);
        slot!(get_credential, 21);
        slot!(validate_credential, 22);
        slot!(iof_pull, 23);
        slot!(push_stdin, 24);
        slot!(group, 25);
        slot!(fabric, 26);
        slot!(client_connected2, 27);
        #[cfg(pmix6)]
        slot!(tool_connected2, 28);
        #[cfg(pmix6)]
        slot!(log2, 29);
        slot!(session_control, 30);
        #[cfg(pmix6)]
        slot!(resource_block, 31);
        mask
    }

    /// Convert this safe module into the raw C server module pointer.
    ///
    /// The returned pointer points to shared callback-table storage valid for
    /// the duration of the `PMIx_server_init` call. Do not store the
    /// pointer beyond the call — the PMIx library copies the struct
    /// internally during initialization.
    ///
    /// # Type safety
    ///
    /// The conversion builds a `ffi::pmix_server_module_t` directly and
    /// assigns each slot with the matching trampoline as a TYPED function
    /// pointer — the compiler verifies every trampoline signature against
    /// the bindgen slot typedef (for both the pmix5 and pmix6 bindings).
    /// A slot whose field is `None` stays null.
    ///
    /// # Safety
    /// The caller must ensure that any callback functions stored in
    /// this module remain valid for the lifetime of the PMIx server
    /// library (i.e., until `PMIx_server_finalize` is called).
    /// Because the PMIx server is process-wide, do not select a different
    /// module while the server is active.
    pub fn as_c_ptr(&self) -> *const c_void {
        // Publish the native host upcalls before the trampolines can fire.
        *SERVER_OP_UPCALLS
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = ServerOpUpcalls::from_module(self);

        let owner = self as *const Self as usize;
        let mask = self.callback_mask();

        // Every slot is assigned a TYPED fn pointer: each line below fails
        // to compile unless the trampoline's `unsafe extern "C"` signature
        // is identical to the bindgen typedef of that slot.
        let mut module: ffi::pmix_server_module_t = unsafe { std::mem::zeroed() };
        module.client_connected = self.client_connected.map(|_| client_connected as _);
        module.client_finalized = self.client_finalized.map(|_| client_finalized as _);
        module.abort = self.abort.map(|_| abort as _);
        module.fence_nb = self.fence_nb.map(|_| fence_nb as _);
        module.direct_modex = self.direct_modex.map(|_| direct_modex as _);
        module.publish = self.publish.map(|_| publish as _);
        module.lookup = self.lookup.map(|_| lookup as _);
        module.unpublish = self.unpublish.map(|_| unpublish as _);
        module.spawn = self.spawn.map(|_| spawn as _);
        module.connect = self.connect.map(|_| connect as _);
        module.disconnect = self.disconnect.map(|_| disconnect as _);
        module.register_events = self.register_events.map(|_| register_events as _);
        module.deregister_events = self.deregister_events.map(|_| deregister_events as _);
        module.listener = self.listener.map(|_| listener as _);
        module.notify_event = self.notify_event.map(|_| notify_event as _);
        module.query = self.query.map(|_| query as _);
        module.tool_connected = self.tool_connected.map(|_| tool_connected as _);
        module.log = self.log.map(|_| log as _);
        module.allocate = self.allocate.map(|_| allocate as _);
        module.job_control = self.job_control.map(|_| job_control as _);
        module.monitor = self.monitor.map(|_| monitor as _);
        module.get_credential = self.get_credential.map(|_| get_credential as _);
        module.validate_credential = self.validate_credential.map(|_| validate_credential as _);
        module.iof_pull = self.iof_pull.map(|_| iof_pull as _);
        module.push_stdin = self.push_stdin.map(|_| push_stdin as _);
        module.group = self.group.map(|_| group as _);
        module.fabric = self.fabric.map(|_| fabric as _);
        module.client_connected2 = self.client_connected2.map(|_| client_connected2 as _);
        #[cfg(pmix6)]
        {
            module.tool_connected2 = self.tool_connected2.map(|_| tool_connected2 as _);
            module.log2 = self.log2.map(|_| log2 as _);
        }
        module.session_control = self.session_control.map(|_| session_control as _);
        #[cfg(pmix6)]
        {
            module.resource_block = self.resource_block.map(|_| resource_block as _);
        }

        let mut modules = SERVER_MODULE_CALLBACK_TABLES
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        // Reuse the pinned table for an unchanged (owner, installed-set)
        // pair so repeated conversions return a stable pointer.
        if let Some((_, _, module)) = modules
            .iter()
            .find(|(table_owner, table_mask, _)| *table_owner == owner && *table_mask == mask)
        {
            return module.as_ref().get_ref() as *const ffi::pmix_server_module_t as *const c_void;
        }

        let module = Box::pin(module);
        let pointer =
            module.as_ref().get_ref() as *const ffi::pmix_server_module_t as *const c_void;
        modules.push((owner, mask, module));
        pointer
    }
}

#[cfg(test)]
#[test]
fn test_server_module_all_slots_null_by_default() {
    let module = PmixServerModule::default();
    let raw = module.as_c_ptr().cast::<ffi::pmix_server_module_t>();
    // SAFETY: the module remains alive while its retained callback table is
    // inspected, and as_c_ptr has just installed the (all-null) table.
    let raw = unsafe { &*raw };
    macro_rules! assert_slot_null {
        ($($field:ident),* $(,)?) => {
            $(assert!(
                raw.$field.is_none(),
                concat!(stringify!($field), " must be null for a default module")
            );)*
        };
    }
    assert_slot_null!(
        client_connected,
        client_finalized,
        abort,
        fence_nb,
        direct_modex,
        publish,
        lookup,
        unpublish,
        spawn,
        connect,
        disconnect,
        register_events,
        deregister_events,
        listener,
        notify_event,
        query,
        tool_connected,
        log,
        allocate,
        job_control,
        monitor,
        get_credential,
        validate_credential,
        iof_pull,
        push_stdin,
        group,
        fabric,
        client_connected2,
        session_control,
    );
    #[cfg(pmix6)]
    assert_slot_null!(tool_connected2, log2, resource_block);
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixServerHandle — alias for process-wide [`PmixServer`] session
// ─────────────────────────────────────────────────────────────────────────────

/// Process-wide server session handle ([`PmixServer`]).
///
/// **Drop does not finalize.** Use [`server_finalize`] or [`PmixServer::disconnect`].
pub use session::PmixServerHandle;

// ─────────────────────────────────────────────────────────────────────────────
// server_init
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize the PMIx server library.
///
/// This function initializes the server-side PMIx library and registers
/// the provided callback module. The server library will invoke the
/// callbacks in `module` when client processes connect, request data,
/// spawn processes, etc.
///
/// The `info` array can be used to pass server-specific configuration,
/// such as:
///
/// * `PMIX_SERVER_TOOL_SUPPORT` — indicate the server accepts tool connections.
/// * `PMIX_SERVER_SCHEDULER` — indicate the server is a scheduler.
/// * `PMIX_SOCKET_MODE` — file permissions for the Unix domain socket.
/// * `PMIX_HOSTNAME` — hostname for this server instance.
///
/// # Parameters
/// * `module` — the server callback module (may be `None` for a minimal
///   server with no callbacks).
/// * `info` — optional configuration info keys.
///
/// # Returns
/// * `Ok(PmixServerHandle)` — server initialized successfully.
/// * `Err(PmixStatus)` — initialization failed (e.g., PMIx library not
///   available, invalid module, or conflicting initialization).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_init(void *module,
///                                 pmix_info_t info[], size_t ninfo);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, PmixServerModule};
/// use pmix::InfoBuilder;;
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build().expect("build info")).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_init(
    module: Option<&PmixServerModule>,
    info: &Info,
) -> Result<PmixServerHandle, PmixStatus> {
    PmixServer::connect_new(module, info)
}

/// Initialize the PMIx server library with no info keys.
///
/// Convenience wrapper around [`server_init`] that passes no
/// configuration info to the server library.
///
/// # Parameters
/// * `module` — the server callback module (may be `None` for a minimal
///   server with no callbacks).
///
/// # Returns
/// * `Ok(PmixServerHandle)` — server initialized successfully.
/// * `Err(PmixStatus)` — initialization failed.
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init_minimal, server_finalize, PmixServerModule};
///
/// let module = PmixServerModule::default();
/// let handle = server_init_minimal(Some(&module)).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_init_minimal(
    module: Option<&PmixServerModule>,
) -> Result<PmixServerHandle, PmixStatus> {
    PmixServer::connect_new_minimal(module)
}

// ─────────────────────────────────────────────────────────────────────────────
// server_finalize
// ─────────────────────────────────────────────────────────────────────────────

/// Finalize the PMIx server library.
///
/// This function shuts down the server-side PMIx library, releasing
/// all internal resources, closing communication channels, and freeing
/// memory. After calling this, the server library is no longer usable
/// until `PMIx_server_init` is called again.
///
/// # Parameters
/// * `handle` — the handle returned by [`server_init`]. Consumed by value
///   to prevent use-after-finalize.
///
/// # Returns
/// * `Ok(())` — server finalized successfully.
/// * `Err(PmixStatus)` — finalization failed (should not happen under
///   normal circumstances).
///
/// # C API
/// `pmix_status_t PMIx_server_finalize(void)`
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, PmixServerModule};
/// use pmix::InfoBuilder;;
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build().expect("build info")).expect("server_init failed");
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn server_finalize(handle: PmixServerHandle) -> Result<(), PmixStatus> {
    // Explicit finalize on the process session (Drop no longer finalizes).
    handle.disconnect()
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_register_nspace — register a job nspace with the server library
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_register_nspace_nb`.
///
/// Implement this trait to receive the result of a non-blocking nspace
/// registration. The `on_complete` method receives the `PmixStatus` result.
pub trait RegisterNspaceCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending register_nspace callbacks.
static REGISTER_NS_SPACE_REGISTRY: LazyLock<Registry<Box<dyn RegisterNspaceCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (register_nspace completion).
///
/// Called by PMIx when the non-blocking nspace registration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn register_nspace_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    release_server_retained_data(req_id);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Register an nspace (job namespace) with the PMIx server library.
///
/// This function informs the PMIx server library about a new job
/// namespace. The server must register ALL nspaces that will participate
/// in collective operations with local processes — even if no local
/// processes belong to that nspace, as long as any local process might
/// perform a collective involving processes from that nspace.
///
/// The `nlocalprocs` parameter tells the library how many local processes
/// will be launched within this nspace. This is required for correct
/// collective handling, because a collective call can occur before all
/// processes have started.
///
/// The `info` array can contain per-process information such as:
///
/// * `PMIX_LOCAL_RANK` — local rank of each process.
/// * `PMIX_PROC_RANK` — global rank within the job.
/// * `PMIX_NODE_RANK` — rank on the local node.
/// * `PMIX_HOSTNAME` — hostname where the process runs.
/// * `PMIX_NODEID` — numeric identifier of the node.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `nspace` — the job namespace identifier (string, max 255 chars).
/// * `nlocalprocs` — number of local processes in this nspace.
/// * `info` — optional per-process info keys.
/// * `callback` — invoked when registration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid
///   nspace, PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_register_nspace(const pmix_nspace_t nspace,
///                                           int nlocalprocs,
///                                           pmix_info_t info[], size_t ninfo,
///                                           pmix_op_cbfunc_t cbfunc,
///                                           void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_register_nspace, PmixServerModule, server_init, server_finalize};
/// use pmix::InfoBuilder;
/// use pmix::PmixStatus;
///
/// struct MyNspaceCallback;
/// impl pmix::server::RegisterNspaceCallback for MyNspaceCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("register_nspace completed: {:?}", status);
///     }
/// }
///
/// let module = PmixServerModule::default();
/// let _handle = server_init(Some(&module), &InfoBuilder::new().build().expect("build info")).expect("server_init failed");
///
/// server_register_nspace("myjob.12345", 4, &InfoBuilder::new().build().expect("build info"), Box::new(MyNspaceCallback))
///     .expect("register_nspace request rejected");
/// ```
pub fn server_register_nspace(
    nspace: &str,
    nlocalprocs: i32,
    info: &Info,
    callback: Box<dyn RegisterNspaceCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => return Err(PmixStatus::from_raw(-1)), // PMIX_ERROR — contains NUL
    };

    // Allocate a unique request ID and register the callback.
    let req_id = REGISTER_NS_SPACE_REGISTRY.next_req_id();
    {
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Prepare info parameters.
    let (info_ptr, ninfo) = retain_server_info(req_id, info);

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_register_nspace is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array or null (checked above).
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        ffi::PMIx_server_register_nspace(
            nspace_c.as_ptr(),
            nlocalprocs,
            info_ptr,
            ninfo,
            Some(register_nspace_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = REGISTER_NS_SPACE_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// is_server_initialized
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the PMIx server library has been initialized.
///
/// This checks whether `PMIx_server_init` has been called and not yet
/// finalized. Note that this is distinct from [`crate::utility::initialized`],
/// which checks the client-side initialization state.
///
/// # C API
/// Uses `PMIx_Initialized()` internally, which checks the global PMIx
/// initialization state (valid for both client and server paths).
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_finalize, is_server_initialized, PmixServerModule};
/// use pmix::InfoBuilder;
///
/// assert!(!is_server_initialized());
///
/// let module = PmixServerModule::default();
/// let handle = server_init(Some(&module), &InfoBuilder::new().build().expect("build info")).expect("server_init failed");
/// assert!(is_server_initialized());
///
/// server_finalize(handle).expect("server_finalize failed");
/// ```
pub fn is_server_initialized() -> bool {
    PmixServer::new().is_live()
}

// PMIx_server_deregister_nspace — deregister a job nspace and purge its data
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_deregister_nspace` (non-blocking mode).
///
/// Implement this trait to receive the result of a non-blocking nspace
/// deregistration. The `on_complete` method receives the `PmixStatus` result.
pub trait DeregisterNspaceCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending deregister_nspace callbacks.
static DEREGISTER_NS_SPACE_REGISTRY: LazyLock<Registry<Box<dyn DeregisterNspaceCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (deregister_nspace completion).
///
/// Called by PMIx when the non-blocking nspace deregistration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn deregister_nspace_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DEREGISTER_NS_SPACE_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Deregister an nspace (job namespace) and purge all related data.
///
/// This function tells the PMIx server library to delete all client
/// information for the specified namespace, including any published
/// data, process records, and other objects associated with that job.
///
/// This is intended to support persistent PMIx servers by providing
/// an opportunity for the host resource manager (RM) to tell the PMIx
/// server library to release all memory for a completed job.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`. If you need blocking behavior, pass
/// `None` for the callback (the C API accepts a NULL callback for
/// blocking execution).
///
/// # Parameters
///
/// * `nspace` — the job namespace identifier to deregister.
/// * `callback` — invoked when deregistration completes. Pass `None`
///   for blocking behavior (not recommended in async contexts).
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback` (if provided).
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # C API
/// ```c
/// void PMIx_server_deregister_nspace(const pmix_nspace_t nspace,
///                                    pmix_op_cbfunc_t cbfunc,
///                                    void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_deregister_nspace, DeregisterNspaceCallback};
/// use pmix::PmixStatus;
///
/// struct MyDeregisterCallback;
/// impl DeregisterNspaceCallback for MyDeregisterCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("deregister_nspace completed: {:?}", status);
///     }
/// }
///
/// // Deregister a completed job's namespace
/// server_deregister_nspace("myjob.12345", Some(Box::new(MyDeregisterCallback)));
/// ```
pub fn server_deregister_nspace(nspace: &str, callback: Option<Box<dyn DeregisterNspaceCallback>>) {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            // Since the C API returns void, we can't report this
            // through a return value. If a callback was provided,
            // invoke it with an error status immediately.
            if let Some(cb) = callback {
                cb.on_complete(PmixStatus::from_raw(-1)); // PMIX_ERROR
            }
            return;
        }
    };

    match callback {
        Some(cb) => {
            // Non-blocking mode: register callback and pass bridge to FFI.
            let req_id = DEREGISTER_NS_SPACE_REGISTRY.next_req_id();
            {
                let mut registry = DEREGISTER_NS_SPACE_REGISTRY.lock();
                registry.insert(req_id, cb);
            }

            // Encode the request ID as a non-null pointer for cbdata.
            let cbdata = crate::cbdata::encode_req_id(req_id);

            // SAFETY: PMIx_server_deregister_nspace is a non-blocking server API.
            // - nspace_c.as_ptr() is a valid null-terminated string for the
            //   duration of this call (PMIx copies it internally).
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            // - The C function returns void; no return value to check.
            unsafe {
                ffi::PMIx_server_deregister_nspace(
                    nspace_c.as_ptr(),
                    Some(deregister_nspace_callback_bridge),
                    cbdata,
                );
            }
        }
        None => {
            // Blocking mode: pass NULL callback. The C API documents
            // that a NULL cbfunc means the function executes as a
            // blocking operation.
            //
            // SAFETY: nspace_c.as_ptr() is a valid null-terminated string.
            // NULL callback means blocking execution.
            unsafe {
                ffi::PMIx_server_deregister_nspace(nspace_c.as_ptr(), None, std::ptr::null_mut());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_register_client — register a client process with the server
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_register_client`.
///
/// Implement this trait to receive the result of a non-blocking client
/// registration. The `on_complete` method receives the `PmixStatus` result.
pub trait RegisterClientCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending register_client callbacks.
static REGISTER_CLIENT_REGISTRY: LazyLock<Registry<Box<dyn RegisterClientCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (register_client completion).
///
/// Called by PMIx when the non-blocking client registration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn register_client_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = REGISTER_CLIENT_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Register a client process with the PMIx server library.
///
/// This function informs the PMIx server about a specific client process
/// that has been launched. The `uid` and `gid` parameters help the server
/// library authenticate clients as they connect — the library requires
/// the actual credentials of connecting processes to match the registered
/// values.
///
/// The `server_object` parameter allows the host resource manager to
/// associate an opaque pointer with this client. The PMIx library will
/// return this pointer in server callbacks (e.g., when the client calls
/// finalize), allowing the host server to access its own per-client
/// state without performing a lookup.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `proc` — the process identifier (namespace + rank) of the client.
/// * `uid` — expected user ID of the client process for authentication.
/// * `gid` — expected group ID of the client process for authentication.
/// * `server_object` — opaque pointer associated with this client, returned
///   in server callbacks. Pass `None` if not needed.
/// * `callback` — invoked when registration completes.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid proc,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_register_client(const pmix_proc_t *proc,
///                                           uid_t uid, gid_t gid,
///                                           void *server_object,
///                                           pmix_op_cbfunc_t cbfunc,
///                                           void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_register_client, RegisterClientCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyClientCallback;
/// impl RegisterClientCallback for MyClientCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("register_client completed: {:?}", status);
///     }
/// }
///
/// let proc = Proc::new("myjob.12345", 0).expect("invalid nspace");
/// server_register_client(&proc, 1000, 1000, None, Box::new(MyClientCallback))
///     .expect("register_client request rejected");
/// ```
pub fn server_register_client(
    proc: &Proc,
    uid: u32,
    gid: u32,
    server_object: Option<*mut c_void>,
    callback: Box<dyn RegisterClientCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = REGISTER_CLIENT_REGISTRY.next_req_id();
    {
        let mut registry = REGISTER_CLIENT_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // The server_object is an opaque pointer the RM associates with this client.
    let server_obj_ptr = match server_object {
        Some(ptr) => ptr,
        None => ptr::null_mut(),
    };

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_register_client is a non-blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - uid and gid are passed by value.
        // - server_obj_ptr is either a valid pointer owned by the caller or null.
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        ffi::PMIx_server_register_client(
            proc_ptr,
            uid,
            gid,
            server_obj_ptr,
            Some(register_client_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = REGISTER_CLIENT_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_deregister_client — deregister a specific client process
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_deregister_client`.
///
/// Implement this trait to receive the result of a non-blocking client
/// deregistration. The `on_complete` method receives the `PmixStatus` result.
pub trait DeregisterClientCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending deregister_client callbacks.
static DEREGISTER_CLIENT_REGISTRY: LazyLock<Registry<Box<dyn DeregisterClientCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (deregister_client completion).
///
/// Called by PMIx when the non-blocking client deregistration completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered closure and invoke it with the result status.
pub(crate) extern "C" fn deregister_client_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DEREGISTER_CLIENT_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Deregister a specific client process and purge all data relating to it.
///
/// This function tells the PMIx server library to delete all information
/// for a specific client process. Unlike [`server_deregister_nspace`],
/// which purges ALL data for an entire namespace, this API targets only
/// a single process within a namespace.
///
/// This API is intended solely for use in exception cases — for example,
/// when a specific client must be forcibly removed while other clients
/// in the same namespace continue to operate normally.
///
/// # Parameters
///
/// * `proc` — the process identifier (namespace + rank) of the client to deregister.
/// * `callback` — invoked when deregistration completes. Pass `None` for
///   blocking behavior (the C API accepts a NULL callback for blocking
///   execution, though this is not recommended in async contexts).
///
/// # Returns
///
/// Nothing — the C API returns `void`. If a callback is provided, the
/// result is delivered asynchronously. If no callback is provided, the
/// call executes as a blocking operation.
///
/// # C API
/// ```c
/// void PMIx_server_deregister_client(const pmix_proc_t *proc,
///                                    pmix_op_cbfunc_t cbfunc,
///                                    void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_deregister_client, DeregisterClientCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyDeregisterCallback;
/// impl DeregisterClientCallback for MyDeregisterCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("deregister_client completed: {:?}", status);
///     }
/// }
///
/// let proc = Proc::new("myjob.12345", 3).expect("invalid nspace");
/// // Deregister a specific misbehaving client
/// server_deregister_client(&proc, Some(Box::new(MyDeregisterCallback)));
/// ```
pub fn server_deregister_client(proc: &Proc, callback: Option<Box<dyn DeregisterClientCallback>>) {
    // The proc's nspace must not contain NUL bytes — Proc::new already
    // validates this at construction time, so we can safely access the
    // internal CString here.

    match callback {
        Some(cb) => {
            // Non-blocking mode: register callback and pass bridge to FFI.
            let req_id = DEREGISTER_CLIENT_REGISTRY.next_req_id();
            {
                let mut registry = DEREGISTER_CLIENT_REGISTRY.lock();
                registry.insert(req_id, cb);
            }

            // Encode the request ID as a non-null pointer for cbdata.
            let cbdata = crate::cbdata::encode_req_id(req_id);

            // Get a pointer to the proc's internal pmix_proc_t for FFI.
            let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

            // SAFETY: PMIx_server_deregister_client is a non-blocking server API.
            // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
            //   that remains alive for the duration of this call (PMIx copies it).
            // - The callback bridge has C linkage and properly handles cbdata.
            // - cbdata is an opaque pointer that we control and decode in the bridge.
            // - The C function returns void; no return value to check.
            unsafe {
                ffi::PMIx_server_deregister_client(
                    proc_ptr,
                    Some(deregister_client_callback_bridge),
                    cbdata,
                );
            }
        }
        None => {
            // Blocking mode: pass NULL callback. The C API documents
            // that a NULL cbfunc means the function executes as a
            // blocking operation.
            //
            // SAFETY: proc_ptr is a valid reference to the Proc's internal
            // pmix_proc_t. NULL callback means blocking execution.
            let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;
            unsafe {
                ffi::PMIx_server_deregister_client(proc_ptr, None, std::ptr::null_mut());
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_fork — prepare environment for forked child process
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_setup_fork`.
///
/// Sets up the environment of a child process to be forked by the host
/// so it can correctly interact with the PMIx server. The PMIx client
/// needs setup information to properly connect back to the server. This
/// function sets appropriate environment variables for that purpose, and
/// also provides any environment variables that were specified in the
/// launch command (e.g., via `PMIx_Spawn`) plus other values (e.g.,
/// variables required to properly initialize the client's fabric library).
///
/// # Environment variables set
///
/// Typical variables include (implementation-dependent):
///
/// * `PMIX_NAMESPACE` — the process's namespace
/// * `PMIX_RANK` — the process's rank within the namespace
/// * Listener URI variable(s) — rendezvous information for the client
///   to connect back to the server
/// * `PMIX_SECURITY_MODE` — active security module
/// * `PMIX_BFROP_BUFFER_TYPE` — buffer serialization format
/// * `PMIX_GDS_MODULE` — available GDS modules
/// * `PMIX_HOSTNAME` — agreed hostname
/// * `PMIX_VERSION` — PMIx version string
///
/// # Parameters
/// * `proc` — the process whose environment should be set up
/// * `env` — optional initial environment variables (in `KEY=VALUE` format).
///   If `None`, an empty environment is created. The returned environment
///   includes both the initial variables and those added by PMIx.
///
/// # Returns
/// * `Ok(Vec<String>)` — the populated environment as `KEY=VALUE` strings.
///   Pass this directly to `std::process::Command::env_clear().envs(...)`
///   when forking the child process.
/// * `Err(PmixStatus)` — setup failed (e.g., server not initialized,
///   invalid proc, or internal error).
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_fork(const pmix_proc_t *proc, char ***env);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_init, server_setup_fork, PmixServerModule};
/// use pmix::InfoBuilder;
/// use pmix::Proc;
///
/// let module = PmixServerModule::default();
/// let _handle = server_init(Some(&module), &InfoBuilder::new().build().expect("build info")).expect("server_init failed");
///
/// let proc = Proc::new("myjob.12345", 0).expect("proc creation failed");
/// let env = server_setup_fork(&proc, None).expect("setup_fork failed");
///
/// // Use env to fork/exec the child process
/// // e.g., Command::new("client").env_clear().envs(env).spawn();
/// ```
#[allow(clippy::collapsible_if)]
pub fn server_setup_fork(proc: &Proc, env: Option<Vec<&str>>) -> Result<Vec<String>, PmixStatus> {
    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // Build the initial C environment array from the optional env parameter.
    // The C API expects a char ** that it will modify (append to).
    // We allocate it using libc::calloc/realloc-compatible memory so that
    // pmix_argv_free (which calls free) can safely release it.
    let mut c_env: *mut *mut std::os::raw::c_char = std::ptr::null_mut();

    // If the caller provided initial env vars, convert them to a C array.
    if let Some(initial_env) = env {
        if !initial_env.is_empty() {
            // Allocate array of pointers (null-terminated).
            let arr_len = initial_env.len() + 1; // +1 for NULL terminator
            // SAFETY: calloc returns a zeroed allocation or null on failure.
            // We use std::alloc for a null-terminated array of null pointers.
            let arr_ptr = unsafe {
                libc::calloc(arr_len, std::mem::size_of::<*mut std::os::raw::c_char>())
                    as *mut *mut std::os::raw::c_char
            };
            if arr_ptr.is_null() && !initial_env.is_empty() {
                return Err(PmixStatus::from_raw(-32)); // PMIX_ERR_NOMEM
            }

            for (i, env_str) in initial_env.iter().enumerate() {
                match CString::new(*env_str) {
                    Ok(cs) => {
                        // SAFETY: `strdup` returns storage owned by libc::free.
                        unsafe {
                            *arr_ptr.add(i) = libc::strdup(cs.as_ptr());
                        }
                    }
                    Err(_) => {
                        // NUL byte in env string — clean up and return error.
                        // SAFETY: Free already-stored strings and the array.
                        unsafe {
                            for j in 0..i {
                                let s = *arr_ptr.add(j);
                                if !s.is_null() {
                                    libc::free(s as *mut std::os::raw::c_void);
                                }
                            }
                            libc::free(arr_ptr as *mut std::os::raw::c_void);
                        }
                        return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
                    }
                }
            }
            c_env = arr_ptr;
        }
    }

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_fork is a blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - c_env is either a valid null-terminated char** array (allocated
        //   with calloc, compatible with free) or null (PMIx will allocate).
        // - We pass &mut c_env as the char *** output parameter.
        ffi::PMIx_server_setup_fork(proc_ptr, &mut c_env)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        // On error, free the C environment array we may have allocated.
        // SAFETY: c_env is either null or points to a valid null-terminated
        // array of null-terminated strings allocated with libc.
        unsafe {
            if !c_env.is_null() {
                pmix_argv_free(c_env);
            }
        }
        return Err(pmix_status);
    }

    // On success, read the environment array into a Vec<String>.
    // The array is null-terminated.
    let env_vec: Vec<String> = unsafe {
        let mut result = Vec::new();
        if !c_env.is_null() {
            let mut i = 0;
            loop {
                let entry = *c_env.add(i);
                if entry.is_null() {
                    break; // Null terminator reached
                }
                // Convert C string to Rust String.
                let cstr = CStr::from_ptr(entry);
                if let Ok(s) = cstr.to_str() {
                    result.push(s.to_owned());
                }
                i += 1;
            }
        }
        result
    };

    // Free the C environment array (both the array and individual strings).
    // SAFETY: c_env is a valid null-terminated char** allocated by PMIx
    // or by our calloc above. pmix_argv_free frees both the strings and
    // the array itself. After this, c_env is dangling.
    unsafe {
        pmix_argv_free(c_env);
    }

    Ok(env_vec)
}

/// Free a PMIx-allocated `char **` environment array.
///
/// This mirrors `pmix_argv_free` from the PMIx library: it iterates the
/// null-terminated array, frees each string with `libc::free`, then frees
/// the array itself.
///
/// # Safety
/// The caller must ensure that `env` is either null or points to a valid
/// null-terminated array of null-terminated strings allocated by PMIx
/// (which uses standard `calloc`/`realloc`/`strdup` internally).
/// Do not call this on a Rust-owned or stack-allocated array.
unsafe fn pmix_argv_free(env: *mut *mut std::os::raw::c_char) {
    if env.is_null() {
        return;
    }
    let mut p = env;
    loop {
        let entry = unsafe { *p };
        p = unsafe { p.add(1) };
        if entry.is_null() {
            break;
        }
        unsafe {
            libc::free(entry as *mut std::os::raw::c_void);
        }
    }
    unsafe {
        libc::free(env as *mut std::os::raw::c_void);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_dmodex_request — request modex data for a remote process
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_dmodex_request`.
///
/// Implement this trait to receive the result of a direct modex request.
/// The `on_complete` method receives:
///
/// * `status` — the PMIx status of the request (success or error code).
/// * `blob` — the serialized modex data blob (owned by the caller after
///   the callback returns; the PMIx library frees it upon callback return).
///
/// The `blob` is a serialized byte array containing the modex data for the
/// requested process. The host server is responsible for sending this blob
/// back to the original remote requestor. The PMIx library **frees** the
/// `data` buffer immediately after the callback returns, so the caller must
/// copy the data if it needs to retain it beyond the callback.
pub trait DmodexRequestCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>);
}

/// Global registry mapping request IDs to pending dmodex_request callbacks.
static DMODEX_REQUEST_REGISTRY: LazyLock<Registry<Box<dyn DmodexRequestCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_dmodex_response_fn_t` (dmodex_request completion).
///
/// Called by PMIx when the direct modex request completes. The `data`
/// parameter is a C-allocated buffer containing the serialized modex blob.
/// The PMIx library frees this buffer upon return from this function, so
/// we must copy the data before returning.
///
/// The `cbdata` parameter encodes the request ID as a raw pointer.
/// We look up the registered Rust callback and invoke it with the result.
pub(crate) extern "C" fn dmodex_request_callback_bridge(
    status: ffi::pmix_status_t,
    data: *mut std::os::raw::c_char,
    sz: usize,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    // Copy the data blob before the PMIx library frees it.
    // The PMIx docs state: "The PMIx server will free the data blob
    // upon return from the response fn."
    let blob: Vec<u8> = if !data.is_null() && sz > 0 {
        // SAFETY: data points to a valid buffer of sz bytes allocated by PMIx.
        // We copy the data into a Vec before returning so we own it.
        unsafe { std::slice::from_raw_parts(data as *const u8, sz) }.to_vec()
    } else {
        Vec::new()
    };

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DMODEX_REQUEST_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback with the copied data.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status, blob);
    });
}

/// Request modex data for a specific process (direct modex operation).
///
/// This function is used by the host server to obtain a serialized blob
/// of modex data for a specific process. It is part of the "direct modex"
/// (dmodex) mechanism, where modex data is cached locally on each PMIx
/// server for its own local clients and obtained on-demand for remote
/// requests.
///
/// When a remote server needs modex data for a process managed by this
/// local PMIx server, the host server receives the request and calls
/// `PMIx_server_dmodex_request`. The PMIx library assembles the modex
/// data into a serialized blob and returns it via the callback. The host
/// server is then responsible for sending the blob back to the original
/// remote requestor.
///
/// **Important:** The data buffer passed to the callback is owned by the
/// PMIx library and is freed immediately upon callback return. The safe
/// Rust wrapper copies the data into a `Vec<u8>` so the caller owns it.
///
/// # Parameters
///
/// * `proc` — the process whose modex data is being requested.
/// * `callback` — invoked when the modex data is available (or an error
///   occurs). The callback receives the status and the serialized blob.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid proc,
///   NULL callback, or PMIx not initialized as server). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — `proc` is null or `callback` is null.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_dmodex_request(const pmix_proc_t *proc,
///                                          pmix_dmodex_response_fn_t cbfunc,
///                                          void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_dmodex_request, DmodexRequestCallback};
/// use pmix::{PmixStatus, Proc};
///
/// struct MyDmodexCallback;
/// impl DmodexRequestCallback for MyDmodexCallback {
///     fn on_complete(self: Box<Self>, status: PmixStatus, blob: Vec<u8>) {
///         if status.is_success() {
///             println!("Received modex blob of {} bytes", blob.len());
///             // Send blob to the remote requestor...
///         } else {
///             eprintln!("dmodex request failed: {:?}", status);
///         }
///     }
/// }
///
/// let proc = Proc::new("remote.job.12345", 0).expect("invalid nspace");
/// server_dmodex_request(&proc, Box::new(MyDmodexCallback))
///     .expect("dmodex_request rejected");
/// ```
pub fn server_dmodex_request(
    proc: &Proc,
    callback: Box<dyn DmodexRequestCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = DMODEX_REQUEST_REGISTRY.next_req_id();
    {
        let mut registry = DMODEX_REQUEST_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // encode_req_id casts the request ID directly; Registry never returns 0,
    // so cbdata is never null.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Get a pointer to the proc's internal pmix_proc_t for FFI.
    let proc_ptr = &proc.handle as *const ffi::pmix_proc_t;

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_dmodex_request is a non-blocking server API.
        // - proc_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - The callback bridge has C linkage and properly handles cbdata.
        //   It copies the data blob before the PMIx library frees it.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates proc and cbfunc internally and returns
        //   PMIX_ERR_BAD_PARAM if either is null.
        ffi::PMIx_server_dmodex_request(proc_ptr, Some(dmodex_request_callback_bridge), cbdata)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.is_success() {
        // Request accepted — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = DMODEX_REQUEST_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_application — setup application before launch
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_setup_application`.
///
/// Implement this trait to receive the result of a non-blocking application
/// setup request. The `on_complete` method receives:
///
/// * `status` — the PMIx status of the setup operation (success or error code).
/// * `info` — the resulting info key-value pairs produced by the setup
///   operation. This contains environment variables, resource assignments,
///   security credentials, and other data needed prior to process launch.
///   The values are owned Rust `String`s copied from the C info array.
///
/// # Ownership
///
/// The underlying C info array is owned by the PMIx library until the
/// caller invokes the acknowledgment callback. The safe wrapper copies
/// the info keys/values into owned `Vec<(String, String)>` so the
/// caller owns the data, and automatically invokes the ack callback
/// after copying.
pub trait SetupApplicationCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>);
}

/// Global registry mapping request IDs to pending setup_application callbacks.
static SETUP_APPLICATION_REGISTRY: LazyLock<Registry<Box<dyn SetupApplicationCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_setup_application_cbfunc_t`.
///
/// Called by PMIx when the setup_application operation completes. The
/// `info` array contains the setup results (env vars, resources, etc.).
/// The PMIx library owns this array until we call the provided `cbfunc`
/// acknowledgment callback.
///
/// The `cbdata` parameter encodes the request ID as a raw pointer.
/// We look up the registered Rust callback, copy the info data, invoke
/// the acknowledgment, and then invoke the user's callback.
pub(crate) extern "C" fn setup_application_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    provided_cbdata: *mut c_void,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    if provided_cbdata.is_null() {
        return;
    }

    // SAFETY: provided_cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(provided_cbdata);

    release_server_retained_data(req_id);

    // Copy the info array before the PMIx library frees it.
    // The info array is owned by PMIx until we call the ack callback.
    //
    // pmix_info_t has: key (pmix_key_t = [c_char; 512]), flags, value (pmix_value_t).
    // pmix_value_t has: type_ (pmix_data_type_t = u16), data (union).
    // The union fields are: string, integer, uint, size, int8, int16, int32, int64,
    // uint8, uint16, uint32, uint64, fval, dval, rank, flag, byte, bo (byte_object), etc.
    let copied_info: Vec<(String, String)> = if !info.is_null() && ninfo > 0 {
        // SAFETY: info points to a valid array of ninfo pmix_info_t entries
        // allocated by PMIx. We only read the key and value fields.
        unsafe {
            let mut entries = Vec::with_capacity(ninfo);
            for i in 0..ninfo {
                let entry = *info.add(i);
                let key_slice =
                    std::slice::from_raw_parts(entry.key.as_ptr() as *const u8, entry.key.len());
                let key = CStr::from_bytes_until_nul(key_slice)
                    .map(|key| key.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Extract value as string based on type_.
                // pmix_data_type_t is u16; match against known values.
                let dtype = entry.value.type_;
                let value_str = match dtype as u32 {
                    ffi::PMIX_STRING => {
                        // PMIX_STRING
                        if !entry.value.data.string.is_null() {
                            CStr::from_ptr(entry.value.data.string)
                                .to_string_lossy()
                                .into_owned()
                        } else {
                            String::new()
                        }
                    }
                    ffi::PMIX_INT => format!("{}", entry.value.data.integer), // PMIX_INT
                    ffi::PMIX_UINT => format!("{}", entry.value.data.uint),   // PMIX_UINT
                    ffi::PMIX_SIZE => format!("{}", entry.value.data.size),   // PMIX_SIZE
                    ffi::PMIX_INT8 => format!("{}", entry.value.data.int8),   // PMIX_INT8
                    ffi::PMIX_INT16 => format!("{}", entry.value.data.int16), // PMIX_INT16
                    ffi::PMIX_INT32 => format!("{}", entry.value.data.int32), // PMIX_INT32
                    ffi::PMIX_INT64 => format!("{}", entry.value.data.int64), // PMIX_INT64
                    ffi::PMIX_UINT8 => format!("{}", entry.value.data.uint8), // PMIX_UINT8
                    ffi::PMIX_UINT16 => format!("{}", entry.value.data.uint16), // PMIX_UINT16
                    ffi::PMIX_UINT32 => format!("{}", entry.value.data.uint32), // PMIX_UINT32
                    ffi::PMIX_UINT64 => format!("{}", entry.value.data.uint64), // PMIX_UINT64
                    ffi::PMIX_FLOAT => format!("{}", entry.value.data.fval),  // PMIX_FLOAT
                    ffi::PMIX_DOUBLE => format!("{}", entry.value.data.dval), // PMIX_DOUBLE
                    ffi::PMIX_BOOL => format!("{}", entry.value.data.flag),   // PMIX_BOOL
                    ffi::PMIX_PID => format!("{}", entry.value.data.pid),     // PMIX_PID
                    ffi::PMIX_STATUS => format!("{}", entry.value.data.status), // PMIX_STATUS
                    ffi::PMIX_PROC_RANK => format!("{}", entry.value.data.rank), // PMIX_PROC_RANK (stored as rank in union)
                    _ => format!("[type={}] ", dtype),
                };
                entries.push((key, value_str));
            }
            entries
        }
    } else {
        Vec::new()
    };

    // Call the acknowledgment callback to let PMIx free the info array.
    // This must be done before we return from the bridge function.
    if let Some(ack) = cbfunc {
        // SAFETY: cbfunc and cbdata are provided by PMIx and are valid
        // for the duration of this callback invocation.
        unsafe { ack(status, cbdata) };
    }

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = SETUP_APPLICATION_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback with the copied data.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status, copied_info);
    });
}

/// Request application-specific setup prior to process launch.
///
/// This function asks the PMIx library (and any loaded network/fabric
/// modules) to prepare for the launch of an application identified by
/// the given namespace. It returns setup information such as environment
/// variables, security credentials, and resource assignments via the
/// asynchronous callback.
///
/// The host resource manager calls this function after registering the
/// namespace with [`server_register_nspace`] and before calling
/// [`server_setup_fork`] for individual processes.
///
/// # Parameters
///
/// * `nspace` — the namespace of the application being set up.
/// * `info` — info keys that describe the application (job size, number
///   of nodes, fabric requirements, etc.).
/// * `callback` — invoked when setup completes with the status and
///   resulting info array.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., PMIx not
///   initialized as server, or invalid parameters). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_application(
///     const pmix_nspace_t nspace,
///     pmix_info_t info[], size_t ninfo,
///     pmix_setup_application_cbfunc_t cbfunc,
///     void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_setup_application, SetupApplicationCallback};
/// use pmix::InfoBuilder;
/// use pmix::PmixStatus;
///
/// struct MySetupCallback;
/// impl SetupApplicationCallback for MySetupCallback {
///     fn on_complete(self: Box<Self>, status: PmixStatus, info: Vec<(String, String)>) {
///         if status.is_success() {
///             println!("Setup complete, got {} info entries", info.len());
///             for (key, value) in &info {
///                 println!("  {} = {}", key, value);
///             }
///         } else {
///             eprintln!("Setup failed: {:?}", status);
///         }
///     }
/// }
///
/// // After registering the namespace...
/// server_setup_application("myapp.ns", &InfoBuilder::new().build().expect("build info"), Box::new(MySetupCallback))
///     .expect("setup_application rejected");
/// ```
pub fn server_setup_application(
    nspace: &str,
    info: &Info,
    callback: Box<dyn SetupApplicationCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            // Invoke the callback with an error status immediately.
            callback.on_complete(PmixStatus::from_raw(-1), Vec::new());
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
    };

    // Allocate a unique request ID and register the callback.
    let req_id = SETUP_APPLICATION_REGISTRY.next_req_id();
    {
        let mut registry = SETUP_APPLICATION_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // encode_req_id casts the request ID directly; Registry never returns 0,
    // so cbdata is never null.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Retain the info array while PMIx processes the threadshifted request.
    let (info_ptr, info_len) = retain_server_info(req_id, info);

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_application is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - info_len is the number of entries matching info_ptr.
        // - The callback bridge has C linkage and properly handles all parameters:
        //   copies the info array, calls the ack callback, then invokes the user.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_setup_application(
            nspace_c.as_ptr(),
            info_ptr,
            info_len,
            Some(setup_application_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.to_raw() == ffi::PMIX_OPERATION_SUCCEEDED {
        release_server_retained_data(req_id);
        let mut registry = SETUP_APPLICATION_REGISTRY.lock();
        registry.remove(&req_id);
        Ok(())
    } else if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = SETUP_APPLICATION_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_setup_local_support — setup local support for an nspace
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_setup_local_support` (non-blocking mode).
///
/// Implement this trait to receive the result of a non-blocking local
/// support setup operation. The `on_complete` method receives the
/// `PmixStatus` result — success means the PMIx server has completed
/// any application-specific operations prior to spawning local clients.
pub trait SetupLocalSupportCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending setup_local_support callbacks.
static SETUP_LOCAL_SUPPORT_REGISTRY: LazyLock<Registry<Box<dyn SetupLocalSupportCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (setup_local_support completion).
///
/// Called by PMIx when the non-blocking setup_local_support operation completes.
/// The `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered Rust callback and invoke it with the result status.
pub(crate) extern "C" fn setup_local_support_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    release_server_retained_data(req_id);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Setup local support for a given namespace before spawning local clients.
///
/// This function allows the local PMIx server to perform any application-specific
/// operations prior to spawning local clients of a given application. The host
/// resource manager (RM) calls this to inform the PMIx server about the local
/// processes that will be spawned, allowing it to prepare internal data
/// structures and perform any necessary setup.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`.
///
/// # Parameters
///
/// * `nspace` — the namespace identifier for the job whose local support
///   is being set up.
/// * `info` — optional per-process info keys that describe the local
///   processes (e.g., node info, process counts, resource allocations).
/// * `callback` — invoked when setup completes. The callback receives
///   the status of the operation.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(PmixStatus::OperationSucceeded)` — the request was immediately
///   processed and returned success. The callback will NOT be called.
/// * `Err(status)` — request rejected immediately (e.g., invalid nspace,
///   PMIx not initialized as server). The callback will NOT be called.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_setup_local_support(const pmix_nspace_t nspace,
///                                               pmix_info_t info[], size_t ninfo,
///                                               pmix_op_cbfunc_t cbfunc,
///                                               void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_setup_local_support, SetupLocalSupportCallback};
/// use pmix::{Info, InfoBuilder, PmixStatus};
///
/// struct MySetupLocalCallback;
/// impl SetupLocalSupportCallback for MySetupLocalCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         match status {
///             ok if ok.is_success() => println!("Local support setup complete"),
///             err => eprintln!("Setup failed: {:?}", err),
///         }
///     }
/// }
///
/// // Setup local support for a namespace
/// server_setup_local_support(
///     "myapp.12345",
///     &InfoBuilder::new().build().expect("build info"),
///     Box::new(MySetupLocalCallback),
/// )
/// .expect("setup_local_support rejected");
/// ```
pub fn server_setup_local_support(
    nspace: &str,
    info: &Info,
    callback: Box<dyn SetupLocalSupportCallback>,
) -> Result<(), PmixStatus> {
    // Convert nspace to CString for FFI.
    let nspace_c = match CString::new(nspace) {
        Ok(cs) => cs,
        Err(_) => {
            // NUL byte in nspace — cannot proceed.
            callback.on_complete(PmixStatus::from_raw(-1));
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
    };

    // Allocate a unique request ID and register the callback.
    let req_id = SETUP_LOCAL_SUPPORT_REGISTRY.next_req_id();
    {
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // encode_req_id casts the request ID directly; Registry never returns 0,
    // so cbdata is never null.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Prepare info parameters.
    let (info_ptr, ninfo) = retain_server_info(req_id, info);

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_setup_local_support is a non-blocking server API.
        // - nspace_c.as_ptr() is a valid null-terminated string for the
        //   duration of this call (PMIx copies it internally).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - ninfo is the number of entries matching info_ptr.
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_setup_local_support(
            nspace_c.as_ptr(),
            info_ptr,
            ninfo,
            Some(setup_local_support_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.to_raw() == ffi::PMIX_OPERATION_SUCCEEDED {
        // Immediately processed and succeeded — callback was not called.
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Ok(())
    } else if pmix_status.is_success() {
        // PMIX_SUCCESS — callback will be invoked asynchronously.
        Ok(())
    } else {
        // Immediate failure — remove the registered callback so it
        // will never be invoked.
        let mut registry = SETUP_LOCAL_SUPPORT_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_IOF_deliver — deliver forwarded I/O to local PMIx server
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `PMIx_server_IOF_deliver`.
///
/// Implement this trait to receive the result of an I/O forwarding
/// delivery request. The `on_complete` method receives the `PmixStatus`
/// result — success means the PMIx server has accepted the data for
/// distribution to its clients.
///
/// # Important
///
/// The host RM must retain ownership of the byte object (`bo`) until
/// the callback is executed, or until a non-success status is returned
/// immediately by the function. The safe wrapper handles this by taking
/// a reference that must remain valid until the callback fires.
pub trait IOFDeliverCallback: Send {
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry mapping request IDs to pending IOF_deliver callbacks.
static IOF_DELIVER_REGISTRY: LazyLock<Registry<Box<dyn IOFDeliverCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (IOF_deliver completion).
///
/// Called by PMIx when the I/O forwarding delivery completes. The
/// `cbdata` parameter is a raw pointer encoding the request ID.
/// We look up the registered Rust callback and invoke it with the
/// result status.
pub(crate) extern "C" fn iof_deliver_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    release_server_retained_data(req_id);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = IOF_DELIVER_REGISTRY.lock();
        registry.remove(&req_id)
    };

    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Deliver forwarded I/O data to the local PMIx server for distribution.
///
/// This function allows the host resource manager (RM) to pass I/O data
/// that has been forwarded from a remote source to the local PMIx server
/// for distribution to its clients that have registered for the data.
///
/// The PMIx server is responsible for determining which of its clients
/// have actually registered to receive the provided data and delivering
/// it to them.
///
/// # Parameters
///
/// * `source` — the process that provided the data being forwarded.
///   This identifies the source of the I/O stream.
/// * `channel` — the I/O channel (stdin, stdout, stderr) from which
///   the data originated. Specified as `IOFChannelFlags` bitmask.
/// * `bo` — a byte object containing the raw I/O data to deliver.
///   The data must remain valid until the callback is invoked.
/// * `info` — optional metadata describing the data, including
///   attributes such as `PMIX_IOF_COMPLETE` to indicate that the
///   source channel has been closed (EOF).
/// * `callback` — invoked when the data has been processed by the
///   PMIx server. The host RM must retain the byte object until this
///   callback fires.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately (e.g., invalid
///   source, PMIx not initialized as server). The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — `source` is null, `bo` is null, or
///   `channel` is invalid.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_IOF_deliver(const pmix_proc_t *source,
///                                       pmix_iof_channel_t channel,
///                                       const pmix_byte_object_t *bo,
///                                       const pmix_info_t info[], size_t ninfo,
///                                       pmix_op_cbfunc_t cbfunc, void *cbdata);
/// ```
///
/// # Examples
///
/// ```no_run
/// use pmix::server::{server_iof_deliver, IOFDeliverCallback};
/// use pmix::{Proc, PmixStatus, IOFChannelFlags, InfoBuilder};
/// use pmix::data_serialization::PmixByteObject;
///
/// struct MyIOFCallback;
/// impl IOFDeliverCallback for MyIOFCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         if status.is_success() {
///             println!("I/O data delivered successfully");
///         } else {
///             eprintln!("I/O delivery failed: {:?}", status);
///         }
///     }
/// }
///
/// let source = Proc::new("myapp.12345", 0).expect("invalid nspace");
/// let data = PmixByteObject::from(b"Hello, stdout!".to_vec());
/// let channel = IOFChannelFlags::STDOUT;
///
/// // Note: data must remain alive until the callback fires.
/// // In practice, use Arc or a longer-lived buffer.
/// server_iof_deliver(
///     &source,
///     channel,
///     &data,
///     &InfoBuilder::new().build().expect("build info"),
///     Box::new(MyIOFCallback),
/// ).expect("IOF_deliver rejected");
/// ```
pub fn server_iof_deliver(
    source: &Proc,
    channel: crate::IOFChannelFlags,
    bo: &crate::data_serialization::PmixByteObject,
    info: &Info,
    callback: Box<dyn IOFDeliverCallback>,
) -> Result<(), PmixStatus> {
    // Allocate a unique request ID and register the callback.
    let req_id = IOF_DELIVER_REGISTRY.next_req_id();
    {
        let mut registry = IOF_DELIVER_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    // Encode the request ID as a non-null pointer for cbdata.
    // encode_req_id casts the request ID directly; Registry never returns 0,
    // so cbdata is never null.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    // Retain all inputs PMIx may access after this call returns.
    let (source_ptr, bo_ptr, info_ptr, ninfo) = retain_server_iof(req_id, source, bo, info);

    // Call the FFI function.
    let status = unsafe {
        // SAFETY: PMIx_server_IOF_deliver is a non-blocking server API.
        // - source_ptr is a valid reference to the Proc's internal pmix_proc_t
        //   that remains alive for the duration of this call (PMIx copies it).
        // - channel.0 is the raw pmix_iof_channel_t bitmask.
        // - bo_ptr is obtained from PmixByteObject::as_ptr and is a valid
        //   reference to its internal pmix_byte_object_t. The caller must
        //   ensure bo remains valid until the callback fires (the PMIx spec
        //   requires the host RM to retain the byte object until the callback
        //   is executed).
        // - info_ptr is either a valid array of pmix_info_t (from Info.handle)
        //   or null (PMIx accepts null info with ninfo=0).
        // - The callback bridge has C linkage and properly handles cbdata.
        // - cbdata is an opaque pointer that we control and decode in the bridge.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_BAD_PARAM
        //   if source/bo/channel are invalid, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_IOF_deliver(
            source_ptr,
            channel.0,
            bo_ptr,
            info_ptr,
            ninfo,
            Some(iof_deliver_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.to_raw() == ffi::PMIX_OPERATION_SUCCEEDED {
        release_server_retained_data(req_id);
        let mut registry = IOF_DELIVER_REGISTRY.lock();
        registry.remove(&req_id);
        Ok(())
    } else if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = IOF_DELIVER_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_collect_inventory
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `server_collect_inventory`.
///
/// Implement this trait to receive the result of an asynchronous
/// inventory collection request. The `on_complete` method is called
/// exactly once when the operation finishes, with the status and
/// collected inventory info array.
pub trait CollectInventoryCallback: Send + 'static {
    /// Called when the inventory collection completes.
    ///
    /// - `status`: The result status (success or error).
    /// - `inventory`: The collected inventory info entries (owned, freed on drop).
    fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults);
}

/// Results from a `server_collect_inventory` call.
///
/// Contains the collected inventory as an array of info entries.
/// The underlying C memory is automatically freed by its [`ReleaseCallback`]
/// when this value is dropped.
#[derive(Debug)]
pub struct CollectInventoryResults {
    handle: *mut ffi::pmix_info_t,
    len: usize,
    release_fn: ReleaseCallback,
    release_cbdata: *mut c_void,
    /// Makes this type `!Send` + `!Sync` (owns PMIx/C memory — not free-threaded).
    _not_thread_safe: std::marker::PhantomData<*mut u8>,
}

impl CollectInventoryResults {
    /// Number of info entries in the inventory result set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the inventory result set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for CollectInventoryResults {
    fn drop(&mut self) {
        if let Some(release_fn) = std::mem::take(&mut self.release_fn).into_c_callback() {
            // SAFETY: PMIx supplied this callback and opaque data for this
            // completion; it releases the tracker-owned info array.
            unsafe { release_fn(self.release_cbdata) };
            self.handle = ptr::null_mut();
            self.len = 0;
        }
    }
}

/// Global registry of pending collect-inventory callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static COLLECT_INVENTORY_REGISTRY: LazyLock<Registry<Box<dyn CollectInventoryCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_info_cbfunc_t` (collect inventory completion).
///
/// Called by PMIx when the inventory collection request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered Rust callback and invoke it with the result status
/// and the collected inventory info array.
extern "C" fn collect_inventory_callback_bridge(
    status: ffi::pmix_status_t,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
    release_fn: ffi::pmix_release_cbfunc_t,
    release_cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    release_server_retained_data(req_id);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => {
            // Callback already consumed — release the PMIx tracker.
            if let Some(release_fn) = release_fn {
                unsafe { release_fn(release_cbdata) };
            }
            return;
        }
    };

    let pmix_status = PmixStatus::from_raw(status);
    let inventory = CollectInventoryResults {
        handle: info,
        len: ninfo,
        release_fn: release_fn.into(),
        release_cbdata,
        _not_thread_safe: std::marker::PhantomData,
    };
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status, inventory);
    });
}

/// Collect hardware and software inventory from the local system (non-blocking).
///
/// Request the PMIx server to collect inventory information about the local
/// system (CPU, memory, network, GPU, etc.) from the configured inventory
/// plugins. The `callback` closure is invoked once the collection completes,
/// receiving both the status and the collected inventory as an info array.
///
/// The function returns immediately:
/// - `Ok(())` if the request was accepted for asynchronous processing.
///   The actual result will be delivered via `callback`.
/// - `Err(status)` if the request was rejected immediately (e.g., invalid
///   parameters or PMIx not initialized as server). The callback will
///   NOT be called.
///
/// # Parameters
///
/// * `directives` — optional info entries that specify collection directives
///   (e.g., which plugins to query, filtering criteria). Pass an empty slice
///   to use default collection behavior.
/// * `callback` — invoked when inventory collection completes, receiving
///   the status and the collected inventory info array.
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback`.
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_collect_inventory(pmix_info_t directives[],
///                                             size_t ndirs,
///                                             pmix_info_cbfunc_t cbfunc,
///                                             void *cbdata);
/// ```
///
/// # Example
///
/// ```no_run
/// use pmix::server::{server_collect_inventory, CollectInventoryCallback, CollectInventoryResults};
/// use pmix::{Info, InfoBuilder, PmixStatus};
///
/// struct MyInventoryCallback;
/// impl CollectInventoryCallback for MyInventoryCallback {
///     fn on_complete(&self, status: PmixStatus, inventory: CollectInventoryResults) {
///         if status.is_success() {
///             println!("Collected {} inventory items", inventory.len());
///         } else {
///             eprintln!("Inventory collection failed: {:?}", status);
///         }
///     }
/// }
///
/// let directives = InfoBuilder::new().build().expect("build info");
/// server_collect_inventory(
///     &directives,
///     Box::new(MyInventoryCallback),
/// ).expect("collect_inventory rejected");
/// ```
pub fn server_collect_inventory(
    directives: &Info,
    callback: Box<dyn CollectInventoryCallback>,
) -> Result<(), PmixStatus> {
    // Assign a unique request ID and register the callback.
    let req_id = COLLECT_INVENTORY_REGISTRY.next_req_id();

    // SAFETY: encode_req_id casts the request ID directly; Registry never
    // returns 0, so cbdata is never null.
    let cbdata = crate::cbdata::encode_req_id(req_id);

    {
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock();
        registry.insert(req_id, callback);
    }

    let (directives_ptr, ndirs) = retain_server_info(req_id, directives);

    let status = unsafe {
        // SAFETY:
        // - directives_ptr is either null or points to a valid array of
        //   pmix_info_t objects from the Info handle that remains alive
        //   for the duration of this call (PMIx copies the pointer).
        // - ndirs matches the length of the directives array.
        // - collect_inventory_callback_bridge is a valid extern "C" function
        //   matching the pmix_info_cbfunc_t signature.
        // - cbdata encodes the request ID and is guaranteed non-null.
        // - The callback registered in COLLECT_INVENTORY_REGISTRY outlives
        //   this call and will be removed when the callback fires.
        // - The PMIx library validates parameters internally and returns
        //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
        ffi::PMIx_server_collect_inventory(
            directives_ptr,
            ndirs,
            Some(collect_inventory_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(status);

    if pmix_status.to_raw() == ffi::PMIX_OPERATION_SUCCEEDED {
        release_server_retained_data(req_id);
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock();
        registry.remove(&req_id);
        Ok(())
    } else if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = COLLECT_INVENTORY_REGISTRY.lock();
        registry.remove(&req_id);
        release_server_retained_data(req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_deliver_inventory
// ─────────────────────────────────────────────────────────────────────────────

/// Callback trait for `server_deliver_inventory` completion.
///
/// Implement this trait to receive the result of a non-blocking
/// inventory delivery request. The `on_complete` method is invoked
/// asynchronously by the PMIx library when the delivery completes.
///
/// # Example
///
/// ```no_run
/// use pmix::PmixStatus;
/// use pmix::server::DeliverInventoryCallback;
///
/// struct MyDeliverCallback;
/// impl DeliverInventoryCallback for MyDeliverCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         if status.is_success() {
///             println!("Inventory delivered successfully");
///         } else {
///             eprintln!("Inventory delivery failed: {:?}", status);
///         }
///     }
/// }
/// ```
pub trait DeliverInventoryCallback: Send + 'static {
    /// Called when the inventory delivery request completes.
    ///
    /// - `status`: The result status (success or error).
    fn on_complete(self: Box<Self>, status: PmixStatus);
}

/// Global registry of pending deliver-inventory callbacks.
///
/// Maps request ID -> callback. Entries are removed when the callback fires.
static DELIVER_INVENTORY_REGISTRY: LazyLock<Registry<Box<dyn DeliverInventoryCallback>>> =
    LazyLock::new(Registry::new);

/// C bridge for `pmix_op_cbfunc_t` (deliver inventory completion).
///
/// Called by PMIx when the inventory delivery request completes.
/// The `cbdata` parameter encodes the request ID. We look up the
/// registered Rust callback and invoke it with the result status.
pub(crate) extern "C" fn deliver_inventory_callback_bridge(
    status: ffi::pmix_status_t,
    cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }

    // SAFETY: cbdata is the request ID we passed as a pointer cast.
    // We reconstruct the usize from the pointer address.
    let req_id = crate::cbdata::decode_req_id(cbdata);

    release_server_retained_data(req_id);

    // Look up and remove the callback from the registry.
    let cb = {
        let mut registry = DELIVER_INVENTORY_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let cb = match cb {
        Some(cb) => cb,
        None => return, // Callback already consumed or never registered.
    };

    // Invoke the user's Rust callback.
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("server", move || {
        cb.on_complete(pmix_status);
    });
}

/// Deliver collected inventory information to the PMIx server library.
///
/// Pass collected inventory data (e.g., from hardware discovery or
/// inventory plugins) to the PMIx server for storage and subsequent
/// access by clients. The inventory is provided as an array of
/// `pmix_info_t` key-value pairs describing hardware or software
/// attributes.
///
/// This is a non-blocking call — the result is delivered asynchronously
/// via the provided `callback`. If you need blocking behavior, pass
/// `None` for the callback (the C API accepts a NULL callback for
/// blocking execution).
///
/// # Parameters
///
/// * `inventory` — info entries containing the inventory data to deliver.
///   Each entry describes a hardware or software attribute (e.g., CPU model,
///   GPU count, memory capacity).
/// * `directives` — optional info entries that direct the delivery
///   (e.g., filtering, storage options). Pass an empty slice for defaults.
/// * `callback` — invoked when delivery completes. Pass `None` for
///   blocking behavior (not recommended in async contexts).
///
/// # Returns
///
/// * `Ok(())` — request accepted for asynchronous processing.
///   The actual result arrives via `callback` (if provided).
/// * `Err(status)` — request rejected immediately. The callback
///   will NOT be called.
///
/// # Error conditions
///
/// * `PMIX_ERR_INIT` — PMIx server library has not been initialized.
/// * `PMIX_ERR_BAD_PARAM` — invalid parameters (e.g., NULL info with ninfo > 0).
/// * `PMIX_ERR_NOMEM` — insufficient memory to process the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_server_deliver_inventory(
///     const pmix_info_t info[],
///     size_t ninfo,
///     const pmix_info_t directives[],
///     size_t ndirs,
///     pmix_op_cbfunc_t cbfunc,
///     void *cbdata);
/// ```
///
/// # Example
///
/// ```no_run
/// use pmix::server::{server_deliver_inventory, DeliverInventoryCallback};
/// use pmix::{Info, InfoBuilder};
///
/// struct MyDeliverCallback;
/// impl DeliverInventoryCallback for MyDeliverCallback {
///     fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
///         println!("Delivery result: {:?}", status);
///     }
/// }
///
/// let inventory = InfoBuilder::new().build().expect("build info");
/// let directives = InfoBuilder::new().build().expect("build info");
/// server_deliver_inventory(
///     &inventory,
///     &directives,
///     Some(Box::new(MyDeliverCallback)),
/// ).expect("deliver_inventory rejected");
/// ```
pub fn server_deliver_inventory(
    inventory: &Info,
    directives: &Info,
    callback: Option<Box<dyn DeliverInventoryCallback>>,
) -> Result<(), PmixStatus> {
    // If a callback is provided, register it for async completion.
    if let Some(cb) = callback {
        let req_id = DELIVER_INVENTORY_REGISTRY.next_req_id();

        // SAFETY: encode_req_id casts the request ID directly; Registry never
        // returns 0, so cbdata is never null.
        let cbdata = crate::cbdata::encode_req_id(req_id);

        {
            let mut registry = DELIVER_INVENTORY_REGISTRY.lock();
            registry.insert(req_id, cb);
        }

        // Retain both arrays while PMIx processes this threadshifted request.
        let retained_inventory = retained_info(inventory);
        let retained_directives = retained_info(directives);
        let ninfo = retained_inventory.len();
        let ndirs = retained_directives.len();
        let info_ptr = if ninfo == 0 {
            ptr::null_mut()
        } else {
            retained_inventory.as_ptr() as *mut _
        };
        let directives_ptr = if ndirs == 0 {
            ptr::null_mut()
        } else {
            retained_directives.as_ptr() as *mut _
        };
        SERVER_RETAINED_DATA
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                req_id,
                RetainedServerData {
                    infos: retained_inventory,
                    directives: retained_directives,
                    source: None,
                    bo: None,
                    bo_bytes: None,
                },
            );

        let status = unsafe {
            // SAFETY:
            // - info_ptr is either null or points to a valid array of
            //   pmix_info_t objects from the inventory Info handle that
            //   remains alive for the duration of this call.
            // - directives_ptr is either null or points to a valid array
            //   of pmix_info_t objects from the directives Info handle.
            // - ninfo and ndirs match the lengths of their respective arrays.
            // - deliver_inventory_callback_bridge is a valid extern "C" function
            //   matching the pmix_op_cbfunc_t signature.
            // - cbdata encodes the request ID and is guaranteed non-null.
            // - The callback registered in DELIVER_INVENTORY_REGISTRY outlives
            //   this call and will be removed when the callback fires.
            // - The PMIx library validates parameters internally and returns
            //   PMIX_ERR_INIT if not initialized as server, PMIX_ERR_NOMEM on OOM.
            ffi::PMIx_server_deliver_inventory(
                info_ptr,
                ninfo,
                directives_ptr,
                ndirs,
                Some(deliver_inventory_callback_bridge),
                cbdata,
            )
        };

        let pmix_status = PmixStatus::from_raw(status);

        if pmix_status.to_raw() == ffi::PMIX_OPERATION_SUCCEEDED {
            release_server_retained_data(req_id);
            let mut registry = DELIVER_INVENTORY_REGISTRY.lock();
            registry.remove(&req_id);
            Ok(())
        } else if pmix_status.is_success() {
            Ok(())
        } else {
            let mut registry = DELIVER_INVENTORY_REGISTRY.lock();
            registry.remove(&req_id);
            release_server_retained_data(req_id);
            Err(pmix_status)
        }
    } else {
        // Blocking mode: no callback provided.
        // The C API accepts NULL for cbfunc to execute synchronously.

        // Convert inventory Info slice to C pointers.
        let (info_ptr, ninfo) = if inventory.len > 0 {
            (inventory.handle, inventory.len)
        } else {
            (ptr::null_mut(), 0)
        };

        // Convert directives Info slice to C pointers.
        let (directives_ptr, ndirs) = if directives.len > 0 {
            (directives.handle, directives.len)
        } else {
            (ptr::null_mut(), 0)
        };

        let status = unsafe {
            // SAFETY:
            // - info_ptr is either null or points to a valid array of
            //   pmix_info_t objects from the inventory Info handle.
            // - directives_ptr is either null or points to a valid array
            //   of pmix_info_t objects from the directives Info handle.
            // - ninfo and ndirs match the lengths of their respective arrays.
            // - Passing None for cbfunc is the documented blocking mode.
            // - The PMIx library validates parameters internally.
            ffi::PMIx_server_deliver_inventory(
                info_ptr,
                ninfo,
                directives_ptr,
                ndirs,
                None,
                ptr::null_mut(),
            )
        };

        let pmix_status = PmixStatus::from_raw(status);

        if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_generate_locality_string
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_generate_locality_string`.
///
/// Generate a PMIx locality string from a given CPU set bitmap.
///
/// The returned locality string encodes the hardware topology location
/// (NUMA node, L3/L2/L1 cache domain, socket, core, etc.) of the CPUs
/// in the provided cpuset. The string can be passed to
/// `PMIx_Get_relative_locality` to determine the relative locality of
/// two processes.
///
/// This function shall only be called for local client processes, with
/// the returned locality included in the job-level information (via the
/// `PMIX_LOCALITY_STRING` attribute) provided to local clients.
///
/// # Parameters
/// * `cpuset` — CPU set bitmap from which to generate the locality string.
///
/// # Returns
/// * `Ok(String)` — the locality string on success.
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_NOT_SUPPORTED` if
///   hwloc is not available or the cpuset is invalid.
///
/// # C API
/// `pmix_status_t PMIx_server_generate_locality_string(const pmix_cpuset_t *cpuset, char **locality)`
pub fn server_generate_locality_string(
    cpuset: &mut crate::fabric::PmixCpuset,
) -> Result<String, PmixStatus> {
    let cpuset_ptr = cpuset.as_mut_ptr();

    let mut locality_ptr: *mut std::os::raw::c_char = ptr::null_mut();

    let status = unsafe {
        // SAFETY:
        // - cpuset_ptr points to a valid, constructed pmix_cpuset_t that
        //   remains alive for the duration of this call (PMIx copies its
        //   contents to build the locality string).
        // - locality_ptr is a valid output pointer (&mut of a stack variable).
        // - The PMIx library allocates the returned string internally.
        ffi::PMIx_server_generate_locality_string(cpuset_ptr, &mut locality_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // On success, PMIx has allocated a null-terminated string.
    // Read it and take ownership, then free the C allocation.
    let locality = unsafe {
        if locality_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        let s = CStr::from_ptr(locality_ptr).to_string_lossy().into_owned();
        // PMIx_server_generate_locality_string allocates with strdup/calloc;
        // free with libc::free.
        libc::free(locality_ptr as *mut std::os::raw::c_void);
        s
    };

    Ok(locality)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_server_generate_cpuset_string
// ─────────────────────────────────────────────────────────────────────────────

/// Safe wrapper for `PMIx_server_generate_cpuset_string`.
///
/// Generate a PMIx string representation of the provided CPU set bitmap.
///
/// The returned string is prefixed by the source field of the provided cpuset
/// followed by a colon (e.g., `hwloc:0-3,8-11`). The remainder of the string
/// represents the PUs to which the process is bound as expressed by the
/// underlying implementation (e.g., hwloc bitmap list notation).
///
/// This function shall only be called for local client processes, with the
/// returned string included in the job-level information (via the
/// `PMIX_CPUSET` attribute) provided to local clients. Local clients can use
/// these strings as input to obtain their PU bindings via the
/// `PMIx_Parse_cpuset_string` API.
///
/// # Parameters
/// * `cpuset` — CPU set bitmap from which to generate the string representation.
///
/// # Returns
/// * `Ok(String)` — the cpuset string on success (e.g., `"hwloc:0-3,8-11"`).
/// * `Err(PmixStatus)` — error code, e.g. `PMIX_ERR_BAD_PARAM` if the cpuset
///   or its bitmap is null, `PMIX_ERR_TAKE_NEXT_OPTION` if the cpuset source
///   is not hwloc, or other PMIx error constants.
///
/// # C API
/// `pmix_status_t PMIx_server_generate_cpuset_string(const pmix_cpuset_t *cpuset, char **cpuset_string)`
pub fn server_generate_cpuset_string(
    cpuset: &mut crate::fabric::PmixCpuset,
) -> Result<String, PmixStatus> {
    let cpuset_ptr = cpuset.as_mut_ptr();

    let mut cpuset_string_ptr: *mut std::os::raw::c_char = ptr::null_mut();

    let status = unsafe {
        // SAFETY:
        // - cpuset_ptr points to a valid, constructed pmix_cpuset_t that
        //   remains alive for the duration of this call (PMIx copies its
        //   contents to build the cpuset string).
        // - cpuset_string_ptr is a valid output pointer (&mut of a stack variable).
        // - The PMIx library allocates the returned string internally.
        ffi::PMIx_server_generate_cpuset_string(cpuset_ptr, &mut cpuset_string_ptr)
    };

    let pmix_status = PmixStatus::from_raw(status);

    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // On success, PMIx has allocated a null-terminated string.
    // Read it and take ownership, then free the C allocation.
    let cpuset_string = unsafe {
        if cpuset_string_ptr.is_null() {
            return Err(PmixStatus::from_raw(-1)); // PMIX_ERROR
        }
        let s = CStr::from_ptr(cpuset_string_ptr)
            .to_string_lossy()
            .into_owned();
        // PMIx_server_generate_cpuset_string allocates with asprintf/strdup;
        // free with libc::free.
        libc::free(cpuset_string_ptr as *mut std::os::raw::c_void);
        s
    };

    Ok(cpuset_string)
}

/// 6.x-only: `PMIx_server_generate_cpuset` does not exist in OpenPMIx 5.0.
#[cfg(pmix6)]
pub fn server_generate_cpuset(
    cpuset_string: &str,
    cpuset: &mut crate::fabric::PmixCpuset,
) -> Result<(), PmixStatus> {
    let s =
        CString::new(cpuset_string).map_err(|_| PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM))?;
    let raw = crate::pmix_ffi_or_mock!(
        mock = unsafe { mock_ffi::mock_server_generate_cpuset(s.as_ptr(), cpuset.as_mut_ptr()) },
        real = unsafe { ffi::PMIx_server_generate_cpuset(s.as_ptr(), cpuset.as_mut_ptr()) }
    );
    if raw == ffi::PMIX_SUCCESS as i32 {
        Ok(())
    } else {
        Err(PmixStatus::from_raw(raw))
    }
}
/// 6.x-only: `PMIx_server_collect_job_info` does not exist in OpenPMIx 5.0.
#[cfg(pmix6)]
pub fn server_collect_job_info(
    procs: &[Proc],
    dbuf: &mut crate::data_serialization::PmixDataBuffer,
) -> Result<(), PmixStatus> {
    let mut raw = procs.iter().map(|p| p.handle).collect::<Vec<_>>();
    let s = crate::pmix_ffi_or_mock!(
        mock = unsafe {
            mock_ffi::mock_server_collect_job_info(raw.as_mut_ptr(), raw.len(), dbuf.as_mut_ptr())
        },
        real = unsafe {
            ffi::PMIx_server_collect_job_info(raw.as_mut_ptr(), raw.len(), dbuf.as_mut_ptr())
        }
    );
    if s == ffi::PMIX_SUCCESS as i32 {
        Ok(())
    } else {
        Err(PmixStatus::from_raw(s))
    }
}

#[cfg(test)]
mod misc_wrapper_tests {
    use super::*;

    #[test]
    fn op_callback_wrapper_construction() {
        fn assert_send<T: Send>() {}

        assert_send::<OpCallbackWrapper>();
        let callback = OpCallbackWrapper::new(|_| {});
        callback.call(PmixStatus::from_raw(0));
    }

    #[test]
    fn op_callback_wrapper_receives_status() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let callback = OpCallbackWrapper::new(move |status| {
            sender.send(status.to_raw()).unwrap();
        });

        std::thread::spawn(move || {
            callback.call(PmixStatus::from_raw(-1));
        })
        .join()
        .unwrap();

        assert_eq!(receiver.recv().unwrap(), -1);
    }

    #[cfg(pmix6)]
    #[test]
    fn server_cpuset_and_collect_job_info_wrappers() {
        let _guard = crate::mock_ffi::MockGuard::new();
        let mut cpuset = crate::fabric::PmixCpuset::new();
        assert!(server_generate_cpuset("mock:0", &mut cpuset).is_ok());
        let proc = crate::Proc::new("test", 0).unwrap();
        let mut buffer = crate::data_serialization::data_buffer_create().unwrap();
        assert!(server_collect_job_info(&[proc], &mut buffer).is_ok());
    }
}
