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
use crate::views::{
    AppView, InfoView, NspaceView, PdataView, ProcView, QueryView, view_slice, view_slice_mut,
};
use crate::{Info, PmixError, PmixOwnedValue, PmixStatus, Proc, ffi};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{LazyLock, Mutex};

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
    pub struct ModexCallback(Box<ModexClosure>, RawModexCallback);

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
            Self(Box::new(callback), None)
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
        /// Returns `None` for a Rust closure created with [`Self::new`].
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
            self.1
        }
    }

    impl From<ffi::pmix_modex_cbfunc_t> for ModexCallback {
        fn from(callback: ffi::pmix_modex_cbfunc_t) -> Self {
            Self(
                Box::new(move |status, data, len, cbdata, release, release_cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                data,
                                len,
                                cbdata,
                                release.into_c_callback(),
                                release_cbdata,
                            );
                        }
                    }
                }),
                callback,
            )
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

    impl DmodexResponseCallback {
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_dmodex_response_fn_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
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
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_lookup_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_lookup_cbfunc_t> for LookupCallback {
        fn from(callback: ffi::pmix_lookup_cbfunc_t) -> Self {
            Self(
                Box::new(move |status, data, cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                data.as_ptr().cast::<ffi::pmix_pdata_t>().cast_mut(),
                                data.len(),
                                cbdata,
                            )
                        };
                    }
                }),
                callback,
            )
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
    );

    impl SpawnCallback {
        /// Wrap a Rust completion callback (native surface).
        pub fn new(
            callback: impl FnOnce(PmixStatus, &crate::views::NspaceView, *mut c_void) + Send + 'static,
        ) -> Self {
            Self(Box::new(callback), None)
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
        /// Returns `None` for a Rust closure created with [`Self::new`].
        pub fn into_c_callback(
            self,
        ) -> Option<unsafe extern "C" fn(i32, *mut c_char, *mut c_void)> {
            self.1
        }
    }

    impl SpawnCallback {
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_spawn_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_spawn_cbfunc_t> for SpawnCallback {
        fn from(callback: ffi::pmix_spawn_cbfunc_t) -> Self {
            Self(
                Box::new(move |status, nspace, cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(status.to_raw(), nspace.raw_copy().as_mut_ptr(), cbdata)
                        };
                    }
                }),
                callback,
            )
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
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_info_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_info_cbfunc_t> for InfoCallback {
        fn from(callback: ffi::pmix_info_cbfunc_t) -> Self {
            Self(
                Box::new(move |status, info, cbdata, release, release_cbdata| {
                    if let Some(callback) = callback {
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                                cbdata,
                                release.into_c_callback(),
                                release_cbdata,
                            )
                        };
                    }
                }),
                callback,
            )
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
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_credential_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_credential_cbfunc_t> for CredentialCallback {
        fn from(callback: ffi::pmix_credential_cbfunc_t) -> Self {
            Self(
                Box::new(move |status, credential, info, cbdata| {
                    if let Some(callback) = callback {
                        // A stack byte object mirrors the credential for the
                        // duration of the synchronous one-shot call; PMIx
                        // copies it before returning.
                        let bo = credential.map(|bytes| ffi::pmix_byte_object_t {
                            bytes: if bytes.is_empty() {
                                std::ptr::null_mut()
                            } else {
                                bytes.as_ptr() as *mut c_char
                            },
                            size: bytes.len(),
                        });
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                status.to_raw(),
                                bo.as_ref()
                                    .map_or(std::ptr::null_mut(), |bo| bo as *const _ as *mut _),
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
                                cbdata,
                            )
                        };
                    }
                }),
                callback,
            )
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

    impl IofCallback {
        /// Wrap the optional C completion callback supplied by PMIx.
        pub(crate) fn from_raw(callback: ffi::pmix_iof_cbfunc_t) -> Option<Self> {
            callback.map(|callback| Self::from(Some(callback)))
        }
    }

    impl From<ffi::pmix_iof_cbfunc_t> for IofCallback {
        fn from(callback: ffi::pmix_iof_cbfunc_t) -> Self {
            Self(
                Box::new(move |iofhdlr, channel, source, payload, info| {
                    if let Some(callback) = callback {
                        // Stack byte object and info pointer mirror the payload
                        // for the synchronous one-shot call; PMIx copies them.
                        let bo = ffi::pmix_byte_object_t {
                            bytes: if payload.is_empty() {
                                std::ptr::null_mut()
                            } else {
                                payload.as_ptr() as *mut c_char
                            },
                            size: payload.len(),
                        };
                        // SAFETY: call requires the original callback contract.
                        unsafe {
                            callback(
                                iofhdlr,
                                channel as ffi::pmix_iof_channel_t,
                                source.raw_ptr().cast_mut(),
                                &bo as *const _ as *mut _,
                                info.as_ptr().cast::<ffi::pmix_info_t>().cast_mut(),
                                info.len(),
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
    unsafe fn from_raw(callback: ffi::pmix_op_cbfunc_t, cbdata: *mut c_void) -> Option<Self> {
        callback.map(|callback| {
            let cbdata = cbdata as usize;
            Self::new(move |status| unsafe {
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

/// Host operation upcall with a process and an array of entries.
///
/// `T` is the borrowed view type of the entries: `InfoView` for info
/// arrays, `PdataView` for pdata arrays, `NspaceView` for nspace arrays.
pub type ServerOpCallback<T> = unsafe fn(&ProcView, &[T], Option<OpCallbackWrapper>, *mut c_void);

/// Host operation upcall with two request arguments.
pub type ServerOpDataCallback<A, B> = unsafe fn(A, B, Option<OpCallbackWrapper>, *mut c_void);

/// Host `resource_block` upcall (6.x-only).
pub type ServerResourceBlockCallback =
    unsafe fn(&[InfoView], (), Option<OpCallbackWrapper>, *mut c_void);

/// Signature of the `push_stdin` upcall. Kept crate-internal so the bindgen
/// `pmix_byte_object_t` parameter does not appear in a public API position (#522a).
pub(crate) type ServerPushStdinCallback =
    Option<ServerOpDataCallback<*const ffi::pmix_proc_t, *const ffi::pmix_byte_object_t>>;

/// Host `client_connected` upcall.
pub type ServerClientConnectedCallback = unsafe fn(&ProcView, &[InfoView], *mut c_void);

/// Host `client_finalized` upcall.
pub type ServerClientFinalizedCallback = unsafe fn(&ProcView, *mut c_void);

/// Host `direct_modex` upcall completed with a one-shot
/// [`DmodexResponseCallback`](callbacks::DmodexResponseCallback).
pub type ServerDirectModexCallback = unsafe fn(
    &ProcView,
    &NspaceView,
    &[InfoView],
    Option<callbacks::DmodexResponseCallback>,
    *mut c_void,
);

/// Host `lookup` upcall completed with a one-shot
/// [`LookupCallback`](callbacks::LookupCallback).
pub type ServerLookupCallback =
    unsafe fn(&ProcView, &[PdataView], &[InfoView], Option<callbacks::LookupCallback>, *mut c_void);

/// Host `spawn` upcall completed with a one-shot
/// [`SpawnCallback`](callbacks::SpawnCallback).
pub type ServerSpawnCallback =
    unsafe fn(&ProcView, &[AppView], Option<callbacks::SpawnCallback>, *mut c_void);

/// Host `connect` upcall completed with a one-shot
/// [`ConnectionCallback`](callbacks::ConnectionCallback).
pub type ServerConnectCallback = unsafe fn(
    &ProcView,
    &[NspaceView],
    &[InfoView],
    Option<callbacks::ConnectionCallback>,
    *mut c_void,
);

/// Host `query` upcall completed with a one-shot
/// [`InfoCallback`](callbacks::InfoCallback).
pub type ServerQueryCallback =
    unsafe fn(&ProcView, &[QueryView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host allocate-style upcall (`allocate`, `job_control`, `monitor`)
/// completed with a one-shot [`InfoCallback`](callbacks::InfoCallback).
pub type ServerAllocateCallback =
    unsafe fn(&ProcView, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `fabric` upcall completed with a one-shot
/// [`InfoCallback`](callbacks::InfoCallback).
pub type ServerFabricCallback =
    unsafe fn(&ProcView, u32, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host `get_credential` / `validate_credential` upcalls completed with a
/// one-shot [`CredentialCallback`](callbacks::CredentialCallback).
pub type ServerCredentialCallback =
    unsafe fn(&ProcView, &[InfoView], Option<callbacks::CredentialCallback>, *mut c_void);

/// Host `iof_pull` upcall completed with a one-shot
/// [`IofCallback`](callbacks::IofCallback).
pub type ServerIofPullCallback =
    unsafe fn(&ProcView, u32, &[InfoView], Option<callbacks::IofCallback>, *mut c_void);

/// Host `group` upcall completed with a one-shot
/// [`InfoCallback`](callbacks::InfoCallback).
pub type ServerGroupCallback = unsafe fn(
    &ProcView,
    u32,
    &[NspaceView],
    &[InfoView],
    Option<callbacks::InfoCallback>,
    *mut c_void,
);

/// Host `session_control` upcall completed with a one-shot
/// [`InfoCallback`](callbacks::InfoCallback).
pub type ServerSessionControlCallback =
    unsafe fn(i32, &[InfoView], Option<callbacks::InfoCallback>, *mut c_void);

/// Host fence upcall completed with a one-shot [`ModexCallback`].
///
/// The `results` array is mutable: the host may update the returned info
/// entries before handing them back through the completion callback.
pub type ServerModexCallback = unsafe fn(
    &ProcView,
    &[NspaceView],
    &[InfoView],
    &mut [InfoView],
    Option<ModexCallback>,
    *mut c_void,
);

/// Host abort upcall.
pub type ServerAbortCallback =
    unsafe fn(&ProcView, i32, &[InfoView], *mut c_void, Option<OpCallbackWrapper>, *mut c_void);

/// Host event notification upcall.
///
/// `results` is the single `pmix_info_t` result PMIx passes by value; the
/// `nresults` length describes the accompanying results array.
pub type ServerNotifyEventCallback = unsafe fn(
    u32,
    &ProcView,
    u32,
    &[InfoView],
    InfoView,
    usize,
    Option<OpCallbackWrapper>,
    *mut c_void,
);

const SERVER_MODULE_CALLBACK_COUNT: usize = std::mem::size_of::<ffi::pmix_server_module_t>()
    / std::mem::size_of::<Option<unsafe extern "C" fn()>>();

#[repr(C)]
#[derive(Debug)]
struct RawPmixServerModule([Option<unsafe extern "C" fn()>; SERVER_MODULE_CALLBACK_COUNT]);

const _: () = {
    assert!(
        std::mem::size_of::<RawPmixServerModule>()
            == std::mem::size_of::<ffi::pmix_server_module_t>()
    );
    assert!(
        std::mem::align_of::<RawPmixServerModule>()
            == std::mem::align_of::<ffi::pmix_server_module_t>()
    );
};

/// Zero-sized marker for shared storage of converted C callback tables.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct ServerModuleCallbackState;

// Keep returned C table addresses stable when the vector grows.
static SERVER_MODULE_CALLBACK_TABLES: Mutex<Vec<(usize, std::pin::Pin<Box<RawPmixServerModule>>)>> =
    Mutex::new(Vec::new());

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

    /// Called when a client process finalizes its connection.
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    pub client_finalized: Option<ServerClientFinalizedCallback>,

    /// Called when a client requests an abort.
    ///
    /// Runs in **progress context** — see [struct-level progress rules](PmixServerModule#progress-thread-rules-read-before-implementing-any-field).
    /// Complete via the provided `pmix_op_cbfunc_t` after hopping if work blocks.
    pub abort: Option<ServerAbortCallback>,

    /// Non-blocking fence / collective upcall (`PMIx_Fence` / `PMIx_Fence_nb`).
    ///
    /// OpenPMIx calls this once local participants have contributed. The host
    /// must share any provided modex blob with peer daemons and complete via
    /// the provided [`ModexCallback`] — typically **after** hopping off progress
    /// (network / collective work must not block the upcall).
    ///
    /// **Not a CPU-pin target** — progress pinning is separate ([THREADING.md](../../THREADING.md)).
    /// Pattern: `examples/server_upcall_hop.rs`.
    pub fence_nb: Option<ServerModexCallback>,

    /// Direct modex request — fetch a remote proc's modex blob via the host.
    ///
    /// Same progress-thread rules as [`Self::fence_nb`]: schedule the remote
    /// fetch off-progress and invoke the provided callback when the blob is
    /// ready (or on error). See `examples/server_upcall_hop.rs`.
    pub direct_modex: Option<ServerDirectModexCallback>,

    /// Publish callback — client requests to publish data.
    pub publish: Option<ServerOpCallback<InfoView>>,

    /// Lookup callback — client requests to lookup data.
    pub lookup: Option<ServerLookupCallback>,

    /// Unpublish callback — client requests to remove published data.
    pub unpublish: Option<ServerOpCallback<PdataView>>,

    /// Spawn callback — client requests to spawn new processes.
    pub spawn: Option<ServerSpawnCallback>,

    /// Connect callback — client requests to establish a connection.
    pub connect: Option<ServerConnectCallback>,

    /// Disconnect callback — client requests to disconnect.
    pub disconnect: Option<ServerOpCallback<NspaceView>>,

    /// Register events callback.
    pub register_events: Option<ServerOpCallback<InfoView>>,

    /// Deregister events callback.
    pub deregister_events: Option<ServerOpCallback<InfoView>>,

    /// Listener callback — for server-to-server communication.
    pub listener: Option<ServerOpCallback<InfoView>>,

    /// Notify event callback — deliver notifications to the server.
    pub notify_event: Option<ServerNotifyEventCallback>,

    /// Query callback — client requests server-side query.
    pub query: Option<ServerQueryCallback>,

    /// Tool connection callback — accepts tool connections.
    pub tool_connected: Option<ServerOpCallback<InfoView>>,

    /// Log callback — client requests logging.
    pub log: Option<ServerOpCallback<InfoView>>,

    /// Allocation callback — client requests resource allocation.
    pub allocate: Option<ServerAllocateCallback>,

    /// Job control callback.
    pub job_control: Option<ServerAllocateCallback>,

    /// Monitoring callback — client requests monitoring.
    pub monitor: Option<ServerAllocateCallback>,

    /// Get credential callback.
    pub get_credential: Option<ServerCredentialCallback>,

    /// Validate credential callback.
    pub validate_credential: Option<ServerCredentialCallback>,

    /// I/O forwarding pull callback.
    pub iof_pull: Option<ServerIofPullCallback>,

    /// Push stdin callback.
    pub push_stdin: ServerPushStdinCallback,

    /// Group operations callback.
    pub group: Option<ServerGroupCallback>,

    /// Fabric operations callback.
    pub fabric: Option<ServerFabricCallback>,

    /// Client connected v2 callback.
    pub client_connected2: Option<ServerOpCallback<InfoView>>,

    /// Tool connection v2 callback.
    ///
    /// 6.x-only: absent from the OpenPMIx 5.0 server module.
    #[cfg(pmix6)]
    pub tool_connected2: Option<ServerOpCallback<InfoView>>,

    /// Log v2 callback.
    ///
    /// 6.x-only: absent from the OpenPMIx 5.0 server module.
    #[cfg(pmix6)]
    pub log2: Option<ServerOpCallback<InfoView>>,

    /// Session control callback (PMIx 5.x).
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
    unpublish: Option<ServerOpCallback<PdataView>>;
    spawn: Option<ServerSpawnCallback>;
    connect: Option<ServerConnectCallback>;
    disconnect: Option<ServerOpCallback<NspaceView>>;
    register_events: Option<ServerOpCallback<InfoView>>;
    deregister_events: Option<ServerOpCallback<InfoView>>;
    listener: Option<ServerOpCallback<InfoView>>;
    notify_event: Option<ServerNotifyEventCallback>;
    query: Option<ServerQueryCallback>;
    tool_connected: Option<ServerOpCallback<InfoView>>;
    log: Option<ServerOpCallback<InfoView>>;
    allocate: Option<ServerAllocateCallback>;
    job_control: Option<ServerAllocateCallback>;
    monitor: Option<ServerAllocateCallback>;
    get_credential: Option<ServerCredentialCallback>;
    validate_credential: Option<ServerCredentialCallback>;
    iof_pull: Option<ServerIofPullCallback>;
    push_stdin: ServerPushStdinCallback;
    group: Option<ServerGroupCallback>;
    fabric: Option<ServerFabricCallback>;
    client_connected2: Option<ServerOpCallback<InfoView>>;
    #[cfg(pmix6)]
    tool_connected2: Option<ServerOpCallback<InfoView>>;
    #[cfg(pmix6)]
    log2: Option<ServerOpCallback<InfoView>>;
    session_control: Option<ServerSessionControlCallback>;
    #[cfg(pmix6)]
    resource_block: Option<ServerResourceBlockCallback>;
}

// ─────────────────────────────────────────────────────────────────────────────
// C-ABI trampolines. Each keeps the raw C signature of its
// `pmix_server_module_t` slot (they are transmuted into that struct by
// `PmixServerModule::as_c_ptr`), converts raw pointers/lengths into native
// view types, wraps the completion callback, and forwards to the user
// callback registered in `SERVER_OP_UPCALLS`.
// ─────────────────────────────────────────────────────────────────────────────

unsafe extern "C" fn client_connected(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    server_object: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_connected;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies the connecting client's proc and info array
        // for the duration of this upcall; null + 0 maps to an empty slice.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, server_object)
        });
    }
}

unsafe extern "C" fn client_finalized(proc: *const ffi::pmix_proc_t, server_object: *mut c_void) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_finalized;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies the finalizing client's proc for this upcall.
        let proc = unsafe { ProcView::from_raw_ptr(proc) };
        let _ = invoke_user_callback("server", move || unsafe { callback(proc, server_object) });
    }
}

unsafe extern "C" fn abort(
    proc: *const ffi::pmix_proc_t,
    status: i32,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    server_object: *mut c_void,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .abort;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall; null
        // + 0 maps to an empty slice.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, status, info, server_object, completion, cbdata)
        });
    }
}

unsafe extern "C" fn direct_modex(
    proc: *const ffi::pmix_proc_t,
    nspace: *const ffi::pmix_nspace_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_dmodex_response_fn_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .direct_modex;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/nspace/info for this upcall; null
        // + 0 maps to an empty slice.
        let (proc, nspace, info) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                NspaceView::from_raw_ptr(nspace),
                view_slice(info.cast(), ninfo),
            )
        };
        let completion = callbacks::DmodexResponseCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, nspace, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn publish(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .publish;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn lookup(
    proc: *const ffi::pmix_proc_t,
    pdata: *const ffi::pmix_pdata_t,
    npdata: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_lookup_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .lookup;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/pdata/info arrays for this upcall.
        let (proc, pdata, info) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(pdata.cast(), npdata),
                view_slice(info.cast(), ninfo),
            )
        };
        let completion = callbacks::LookupCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, pdata, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn unpublish(
    proc: *const ffi::pmix_proc_t,
    data: *const ffi::pmix_pdata_t,
    ndata: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .unpublish;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/pdata arrays for this upcall.
        let (proc, data) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(data.cast(), ndata)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, data, completion, cbdata)
        });
    }
}

unsafe extern "C" fn spawn(
    proc: *const ffi::pmix_proc_t,
    apps: *const ffi::pmix_app_t,
    napps: usize,
    cbfunc: ffi::pmix_spawn_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .spawn;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/app arrays for this upcall.
        let (proc, apps) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(apps.cast(), napps)) };
        let completion = callbacks::SpawnCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, apps, completion, cbdata)
        });
    }
}

unsafe extern "C" fn connect(
    proc: *const ffi::pmix_proc_t,
    procs: *const ffi::pmix_proc_t,
    nprocs: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_connection_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .connect;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/proc/info arrays for this upcall.
        let (proc, procs, info) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(procs.cast(), nprocs),
                view_slice(info.cast(), ninfo),
            )
        };
        let completion = callbacks::ConnectionCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, procs, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn disconnect(
    proc: *const ffi::pmix_proc_t,
    nspace: *const ffi::pmix_nspace_t,
    count: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .disconnect;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/nspace arrays for this upcall.
        let (proc, nspace) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(nspace.cast(), count),
            )
        };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, nspace, completion, cbdata)
        });
    }
}

unsafe extern "C" fn register_events(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .register_events;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn deregister_events(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .deregister_events;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn listener(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .listener;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn notify_event(
    status: u32,
    proc: *const ffi::pmix_proc_t,
    range: u32,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    results: ffi::pmix_info_t,
    nresults: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .notify_event;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info for this upcall; `results`
        // is a shallow by-value copy of the POD (borrow-free).
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let results = InfoView::from_value(results);
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(
                status, proc, range, info, results, nresults, completion, cbdata,
            )
        });
    }
}

unsafe extern "C" fn query(
    proc: *const ffi::pmix_proc_t,
    queries: *const ffi::pmix_query_t,
    nqueries: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .query;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/query arrays for this upcall.
        let (proc, queries) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(queries.cast(), nqueries),
            )
        };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, queries, completion, cbdata)
        });
    }
}

unsafe extern "C" fn tool_connected(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .tool_connected;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn log(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .log;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn allocate(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .allocate;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn job_control(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .job_control;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn monitor(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .monitor;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn get_credential(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_credential_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_credential;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::CredentialCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn validate_credential(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_credential_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .validate_credential;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::CredentialCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn iof_pull(
    proc: *const ffi::pmix_proc_t,
    channel: u32,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_iof_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iof_pull;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::IofCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, channel, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn push_stdin(
    proc: *const ffi::pmix_proc_t,
    data: *const ffi::pmix_byte_object_t,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push_stdin;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            // SAFETY: Arguments are forwarded unchanged from PMIx; the
            // push_stdin callback is crate-internal (#522a).
            callback(proc, data, completion, cbdata)
        });
    }
}

unsafe extern "C" fn group(
    proc: *const ffi::pmix_proc_t,
    op: u32,
    nspace: *const ffi::pmix_nspace_t,
    nspaces: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .group;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/nspace/info arrays for this upcall.
        let (proc, nspace, info) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(nspace.cast(), nspaces),
                view_slice(info.cast(), ninfo),
            )
        };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, op, nspace, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn fabric(
    proc: *const ffi::pmix_proc_t,
    version: u32,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .fabric;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, version, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn client_connected2(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .client_connected2;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

#[cfg(pmix6)]
unsafe extern "C" fn tool_connected2(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .tool_connected2;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

#[cfg(pmix6)]
unsafe extern "C" fn log2(
    proc: *const ffi::pmix_proc_t,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .log2;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/info arrays for this upcall.
        let (proc, info) =
            unsafe { (ProcView::from_raw_ptr(proc), view_slice(info.cast(), ninfo)) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(proc, info, completion, cbdata)
        });
    }
}

unsafe extern "C" fn session_control(
    session: i32,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_info_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .session_control;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies a valid info array for this upcall.
        let info = unsafe { view_slice(info.cast::<InfoView>(), ninfo) };
        let completion = callbacks::InfoCallback::from_raw(cbfunc);
        let _ = invoke_user_callback("server", move || unsafe {
            callback(session, info, completion, cbdata)
        });
    }
}

#[cfg(pmix6)]
unsafe extern "C" fn resource_block(
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    cbfunc: ffi::pmix_op_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .resource_block;
    if let Some(callback) = callback {
        // SAFETY: PMIx supplies a valid info array for this upcall.
        let info = unsafe { view_slice(info.cast::<InfoView>(), ninfo) };
        // SAFETY: PMIx supplies the completion callback and its opaque data.
        let completion = unsafe { OpCallbackWrapper::from_raw(cbfunc, cbdata) };
        let _ = invoke_user_callback("server", move || unsafe {
            callback(info, (), completion, cbdata)
        });
    }
}

unsafe extern "C" fn fence_nb(
    proc: *const ffi::pmix_proc_t,
    nspace: *const ffi::pmix_nspace_t,
    count: usize,
    info: *const ffi::pmix_info_t,
    ninfo: usize,
    results: *mut ffi::pmix_info_t,
    nresults: usize,
    cbfunc: ffi::pmix_modex_cbfunc_t,
    cbdata: *mut c_void,
) {
    let callback = SERVER_OP_UPCALLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .fence_nb;

    if let Some(callback) = callback {
        // SAFETY: PMIx supplies valid proc/nspace/info/results arrays for
        // this upcall; null + 0 maps to an empty slice.
        let (proc, nspace, info, results) = unsafe {
            (
                ProcView::from_raw_ptr(proc),
                view_slice(nspace.cast::<NspaceView>(), count),
                view_slice(info.cast::<InfoView>(), ninfo),
                view_slice_mut(results.cast::<InfoView>(), nresults),
            )
        };
        let completion = cbfunc.map(|callback| ModexCallback::from(Some(callback)));
        let _ = invoke_user_callback("server", move || unsafe {
            // SAFETY: raw arguments are converted to native views at the
            // FFI boundary; the completion callback is wrapped.
            callback(proc, nspace, info, results, completion, cbdata);
        });
    }
}

impl PmixServerModule {
    /// Convert this safe module into the raw C server module pointer.
    ///
    /// The returned pointer points to shared callback-table storage valid for
    /// the duration of the `PMIx_server_init` call. Do not store the
    /// pointer beyond the call — the PMIx library copies the struct
    /// internally during initialization.
    ///
    /// # Safety
    /// The caller must ensure that any callback functions stored in
    /// this module remain valid for the lifetime of the PMIx server
    /// library (i.e., until `PMIx_server_finalize` is called).
    /// Because the PMIx server is process-wide, do not select a different
    /// module while the server is active.
    pub fn as_c_ptr(&self) -> *const c_void {
        macro_rules! raw_upcall {
            ($field:ident) => {
                self.$field.map(|callback| {
                    // SAFETY: Only the pointer representation is erased.
                    // PMIx invokes this slot with its original C signature.
                    unsafe {
                        std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                            callback as *const (),
                        )
                    }
                })
            };
            ($field:ident => $bridge:ident) => {
                self.$field.map(|_| {
                    // SAFETY: The bridge has this slot's original C signature.
                    unsafe {
                        std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                            $bridge as *const (),
                        )
                    }
                })
            };
        }

        // Every field now bridges through a C-ABI trampoline that converts
        // raw arguments into native view types at the FFI boundary (#520).
        let callbacks = [
            raw_upcall!(client_connected => client_connected),
            raw_upcall!(client_finalized => client_finalized),
            raw_upcall!(abort => abort),
            raw_upcall!(fence_nb => fence_nb),
            raw_upcall!(direct_modex => direct_modex),
            raw_upcall!(publish => publish),
            raw_upcall!(lookup => lookup),
            raw_upcall!(unpublish => unpublish),
            raw_upcall!(spawn => spawn),
            raw_upcall!(connect => connect),
            raw_upcall!(disconnect => disconnect),
            raw_upcall!(register_events => register_events),
            raw_upcall!(deregister_events => deregister_events),
            raw_upcall!(listener => listener),
            raw_upcall!(notify_event => notify_event),
            raw_upcall!(query => query),
            raw_upcall!(tool_connected => tool_connected),
            raw_upcall!(log => log),
            raw_upcall!(allocate => allocate),
            raw_upcall!(job_control => job_control),
            raw_upcall!(monitor => monitor),
            raw_upcall!(get_credential => get_credential),
            raw_upcall!(validate_credential => validate_credential),
            raw_upcall!(iof_pull => iof_pull),
            raw_upcall!(push_stdin => push_stdin),
            raw_upcall!(group => group),
            raw_upcall!(fabric => fabric),
            raw_upcall!(client_connected2 => client_connected2),
            #[cfg(pmix6)]
            raw_upcall!(tool_connected2 => tool_connected2),
            #[cfg(pmix6)]
            raw_upcall!(log2 => log2),
            raw_upcall!(session_control => session_control),
            #[cfg(pmix6)]
            raw_upcall!(resource_block => resource_block),
        ];

        *SERVER_OP_UPCALLS
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = ServerOpUpcalls::from_module(self);

        let mut modules = SERVER_MODULE_CALLBACK_TABLES
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        let module = modules.iter().find(|(owner, module)| {
            *owner == self as *const Self as usize
                && module.0.iter().zip(&callbacks).all(|(left, right)| {
                    left.map(|callback| callback as *const ())
                        == right.map(|callback| callback as *const ())
                })
        });
        if let Some((_, module)) = module {
            return module.as_ref().get_ref() as *const RawPmixServerModule as *const c_void;
        }

        let module = Box::pin(RawPmixServerModule(callbacks));
        let pointer = module.as_ref().get_ref() as *const RawPmixServerModule as *const c_void;
        modules.push((self as *const Self as usize, module));
        pointer
    }
}

#[cfg(test)]
#[test]
fn test_server_module_field_count() {
    let module = PmixServerModule::default();
    let raw = module.as_c_ptr().cast::<RawPmixServerModule>();
    // SAFETY: module remains alive while its retained callback table is inspected.
    let fields = unsafe { &(*raw).0 };
    assert_eq!(fields.len(), SERVER_MODULE_CALLBACK_COUNT);
    assert!(fields.iter().all(Option::is_none));
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
