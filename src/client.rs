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
// PmixClient — process-wide Arc session (Send + Sync)
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// Lifecycle state of the process-wide PMIx client session.
///
/// Transitions: `Uninitialized` → `Live` → `Finalizing` → `Dead`.
///
/// Re-init after `Dead` is not supported (matches typical OpenPMIx process
/// lifetime). Use one logical connect/disconnect cycle per process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PmixClientState {
    /// No successful `PMIx_Init` for this process session yet.
    Uninitialized = 0,
    /// Session is active; ops may call into OpenPMIx.
    Live = 1,
    /// `disconnect` / finalize is in progress.
    Finalizing = 2,
    /// Session torn down; further `connect` fails.
    Dead = 3,
}

impl PmixClientState {
    fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::Uninitialized,
            1 => Self::Live,
            2 => Self::Finalizing,
            _ => Self::Dead,
        }
    }
}

/// Process-wide inner session. Fields are `Send + Sync` so `Arc<Self>` (and
/// thus [`PmixClient`]) can move across threads. Do **not** add
/// `PhantomData<*mut u8>` here — that would make the session `!Send` and defeat
/// multi-threaded cloning.
pub(crate) struct PmixClientInner {
    /// Cached proc from `PMIx_Init`; only meaningful while state is `Live`.
    pub(crate) proc: RwLock<Option<Proc>>,
    pub(crate) state: AtomicU8,
}

impl std::fmt::Debug for PmixClientInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixClientInner")
            .field("state", &self.state())
            .field("proc", &"RwLock<Option<Proc>>")
            .finish()
    }
}

impl PmixClientInner {
    fn state(&self) -> PmixClientState {
        PmixClientState::from_raw(self.state.load(Ordering::Acquire))
    }
}

/// Process-wide client session storage. Created on first [`PmixClient::new`].
static CLIENT_SESSION: OnceLock<Arc<PmixClientInner>> = OnceLock::new();

fn client_session() -> Arc<PmixClientInner> {
    CLIENT_SESSION
        .get_or_init(|| {
            Arc::new(PmixClientInner {
                proc: RwLock::new(None),
                state: AtomicU8::new(PmixClientState::Uninitialized as u8),
            })
        })
        .clone()
}

/// Thread-shareable handle to the process-wide PMIx client session.
///
/// Cloning is an `Arc` clone: every handle refers to the **same** process
/// session and state machine. OpenPMIx ≥ 6.1 threadshifts C API entry; this type
/// is `Send + Sync` so applications can move clones onto worker threads.
///
/// # Lifecycle
///
/// 1. [`PmixClient::new`] / [`Default`] — attach to the process session (`Uninitialized`
///    until connect).
/// 2. [`connect`](Self::connect) — `PMIx_Init`, `Uninitialized` → `Live`.
/// 3. Clone across threads; use free functions ([`put_value`], [`get_value`],
///    [`commit`], [`fence`], …) with [`proc`](Self::proc) / [`get_proc`](Self::get_proc).
/// 4. [`disconnect`](Self::disconnect) — `PMIx_Finalize`, `Live` → `Dead`.
///
/// # Drop / finalize rules
///
/// - **Drop does not finalize.** Clones must not each run `PMIx_Finalize`.
/// - Call [`disconnect`](Self::disconnect) once (or free-function [`finalize`]).
/// - Further disconnect/finalize calls are no-ops once not `Live`.
///
/// # Example
///
/// ```no_run
/// use pmix::PmixClient;
///
/// let client = PmixClient::connect_new(None).expect("connect");
///
/// let worker = client.clone();
/// std::thread::spawn(move || {
///     let _ = worker.rank();
///     // put_value / get_value / … with worker.proc()
/// });
///
/// client.disconnect(None).expect("disconnect");
/// ```
#[derive(Debug, Clone)]
pub struct PmixClient {
    pub(crate) inner: Arc<PmixClientInner>,
}

impl Default for PmixClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PmixClient {
    /// Attach to the process-wide client session (creates it on first call).
    ///
    /// Does not call `PMIx_Init`. Use [`connect`](Self::connect) or
    /// [`connect_new`](Self::connect_new).
    pub fn new() -> Self {
        Self {
            inner: client_session(),
        }
    }

    /// `new()` + [`connect`](Self::connect).
    pub fn connect_new(info: Option<Info>) -> Result<Self, PmixError> {
        let client = Self::new();
        client.connect(info)?;
        Ok(client)
    }

    /// Whether `self` and `other` refer to the same process session (`Arc` identity).
    ///
    /// All `PmixClient` values in a process share one session today, so this is
    /// true for any pair created via [`new`](Self::new).
    pub fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Current lifecycle state.
    pub fn state(&self) -> PmixClientState {
        self.inner.state()
    }

    /// `true` when state is [`PmixClientState::Live`].
    pub fn is_live(&self) -> bool {
        self.state() == PmixClientState::Live
    }

    /// Initialize PMIx for this process (`PMIx_Init`).
    ///
    /// Transitions `Uninitialized` → `Live`. Fails if already `Live`
    /// ([`PmixError::ErrExists`]) or after finalize ([`PmixError::ErrInit`]).
    ///
    /// Serialized on the session lock so concurrent `connect` calls cannot
    /// double-init.
    pub fn connect(&self, info: Option<Info>) -> Result<(), PmixError> {
        let mut proc_guard = self
            .inner
            .proc
            .write()
            .expect("pmix: client proc lock poisoned");

        match self.inner.state() {
            PmixClientState::Uninitialized => {}
            PmixClientState::Live => return Err(PmixError::ErrExists),
            PmixClientState::Finalizing | PmixClientState::Dead => {
                return Err(PmixError::ErrInit);
            }
        }

        let mut uninit_proc = mem::MaybeUninit::<pmix_proc_t>::uninit();
        let status = match info {
            Some(ref info) => unsafe { PMIx_Init(uninit_proc.as_mut_ptr(), info.handle, info.len) },
            None => unsafe { PMIx_Init(uninit_proc.as_mut_ptr(), ptr::null_mut(), 0) },
        };

        let pmix_status = PmixStatus::from_raw(status);
        if pmix_status.is_success() {
            let proc = unsafe { uninit_proc.assume_init() };
            *proc_guard = Some(Proc {
                handle: proc,
                len: 1,
            });
            self.inner
                .state
                .store(PmixClientState::Live as u8, Ordering::Release);
            Ok(())
        } else if let Some(known) = pmix_status.known() {
            Err(known)
        } else {
            Err(PmixError::Error)
        }
    }

    /// Finalize PMIx for this process (`PMIx_Finalize`).
    ///
    /// Transitions `Live` → `Finalizing` → `Dead`. No-op if not `Live`
    /// (already finalized or never connected).
    pub fn disconnect(&self, info: Option<Info>) -> Result<(), pmix_status_t> {
        let mut proc_guard = self
            .inner
            .proc
            .write()
            .expect("pmix: client proc lock poisoned");

        if self.inner.state() != PmixClientState::Live {
            return Ok(());
        }

        self.inner
            .state
            .store(PmixClientState::Finalizing as u8, Ordering::Release);
        *proc_guard = None;

        let status = match info {
            Some(ref x) => unsafe { PMIx_Finalize(x.handle, x.len) },
            None => unsafe { PMIx_Finalize(ptr::null_mut(), 0) },
        };

        // Drop parked event handlers that were never deregistered. After
        // PMIx_Finalize, OpenPMIx will not deliver further notifications for
        // this session (crate no-reinit policy).
        events::clear_handler_registry();

        self.inner
            .state
            .store(PmixClientState::Dead as u8, Ordering::Release);

        if status as u32 == PMIX_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }

    /// Copy of the process handle when live.
    pub fn proc(&self) -> Option<Proc> {
        if self.inner.state() != PmixClientState::Live {
            return None;
        }
        self.inner
            .proc
            .read()
            .expect("pmix: client proc lock poisoned")
            .clone()
    }

    /// Alias for [`proc`](Self::proc).
    pub fn get_proc(&self) -> Option<Proc> {
        self.proc()
    }

    /// Process handle, or panic if not live (tests / infallible paths).
    pub fn require_proc(&self) -> Proc {
        self.proc()
            .expect("pmix: PmixClient must be connected (Live) to use require_proc")
    }

    /// Rank from the cached process handle when live.
    pub fn rank(&self) -> Option<u32> {
        if self.inner.state() != PmixClientState::Live {
            return None;
        }
        self.inner
            .proc
            .read()
            .expect("pmix: client proc lock poisoned")
            .as_ref()
            .map(|p| p.get_rank())
    }

    /// Alias for [`rank`](Self::rank).
    pub fn get_rank(&self) -> Option<u32> {
        self.rank()
    }

    /// Rank, or panic if not live.
    pub fn require_rank(&self) -> u32 {
        self.rank()
            .expect("pmix: PmixClient must be connected (Live) to use require_rank")
    }

    /// `Proc` for `rank` in this client's namespace.
    ///
    /// Returns [`PmixError::ErrInit`] if not connected.
    pub fn proc_with_nspace(&self, rank: u32) -> Result<Proc, PmixError> {
        let guard = self
            .inner
            .proc
            .read()
            .expect("pmix: client proc lock poisoned");
        let my = guard.as_ref().ok_or(PmixError::ErrInit)?;
        my.new_with_nspace(rank).map_err(|_| PmixError::ErrBadParam)
    }

    /// Error if the session is not [`PmixClientState::Live`].
    pub fn check_live(&self) -> Result<(), PmixError> {
        if self.is_live() {
            Ok(())
        } else {
            Err(PmixError::ErrInit)
        }
    }
}

pub fn get_value(proc: &Proc, key: &[u8], info: Option<Info>) -> Result<PmixOwnedValue, PmixError> {
    let mut value: *mut pmix_value_t = null_mut();
    let info_handle: *const pmix_info_t;
    let ninfos: usize;

    match info {
        Some(info) => {
            info_handle = info.handle;
            if info_handle.is_null() {
                ninfos = 0;
            } else {
                ninfos = info.len;
            }
        }
        None => {
            info_handle = null();
            ninfos = 0;
        }
    }

    let key_ptr = match CStr::from_bytes_with_nul(key) {
        Ok(c) => c.as_ptr(),
        Err(_) => return Err(PmixError::ErrBadParam),
    };
    let status = PmixStatus::from_raw(crate::pmix_ffi_or_mock!(
        mock = unsafe {
            crate::mock_ffi::mock_get(
                &proc.handle as *const _ as *const std::ffi::c_void,
                key_ptr,
                info_handle as *const std::ffi::c_void,
                ninfos,
                &mut value,
            )
        },
        real = unsafe { PMIx_Get(&proc.handle, key_ptr, info_handle, ninfos, &mut value,) },
    ));

    if status.is_success() && !value.is_null() {
        // SAFETY: PMIx returned a valid heap-allocated value on success. The
        let owned = unsafe { ptr::read(value) };
        // The nested payloads moved into `owned`; release only the now-empty
        // PMIx struct allocation.
        unsafe { ptr::write_bytes(value, 0, 1) };
        // SAFETY: `value` is the PMIx allocation returned by PMIx_Get.
        unsafe { release_pmix_value(value) };
        Ok(PmixOwnedValue {
            inner: owned,
            pmix_owned: true,
            _not_thread_safe: std::marker::PhantomData,
        })
    } else {
        if let Some(known) = status.known() {
            Err(known)
        } else {
            Err(PmixError::Error)
        }
    }
}

/// Get a value with `PMIX_GET_REFRESH_CACHE` set to bypass the local cache.
///
/// This is useful when you need the most up-to-date value from a peer,
/// especially in scenarios where multiple rounds of data exchange occur
/// (e.g., allgather patterns). Without this, `PMIx_Get` may return a cached
/// value from a previous round.
///
/// # Example
/// ```no_run
/// use pmix::{Proc, get_value_refresh_cache};
///
/// let proc = Proc::new("myapp", 0).expect("proc");
/// let _value = get_value_refresh_cache(&proc, b"my.key\0").expect("fresh get");
/// ```
pub fn get_value_refresh_cache(proc: &Proc, key: &[u8]) -> Result<PmixOwnedValue, PmixError> {
    let mut info = InfoBuilder::new();
    info.refresh_cache(true);
    let info = match info.build() {
        Ok(i) => i,
        Err(_) => return Err(PmixError::Error),
    };
    get_value(proc, key, Some(info))
}

pub fn put_value(
    scope: pmix_scope_t,
    key: &CStr,
    value: &mut PmixOwnedValue,
) -> Result<(), pmix_status_t> {
    let status: pmix_status_t = crate::pmix_ffi_or_mock!(
        mock = unsafe { crate::mock_ffi::mock_put(scope as u8, key.as_ptr(), &mut value.inner) },
        real = unsafe { PMIx_Put(scope, key.as_ptr(), &mut value.inner) },
    );
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn commit() -> Result<(), pmix_status_t> {
    let status: pmix_status_t = crate::pmix_ffi_or_mock!(
        mock = crate::mock_ffi::mock_commit(),
        real = unsafe { PMIx_Commit() },
    );
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn fence(proc: &Proc, info: Option<Info>) -> Result<(), pmix_status_t> {
    let proc_handle: *const pmix_proc_t = &proc.handle;
    let nprocs = if proc_handle.is_null() { 0 } else { proc.len };
    let (info_handle, ninfos) = match info {
        Some(info) => {
            let ih = info.handle;
            let ni = if proc_handle.is_null() { 0 } else { info.len };
            (ih, ni)
        }
        None => (ptr::null_mut(), 0),
    };

    let status = crate::pmix_ffi_or_mock!(
        mock = unsafe {
            crate::mock_ffi::mock_fence(
                proc_handle as *const std::ffi::c_void,
                nprocs,
                info_handle as *const std::ffi::c_void,
                ninfos,
            )
        },
        real = unsafe { PMIx_Fence(proc_handle, nprocs, info_handle, ninfos) },
    );
    if status as u32 == PMIX_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn get_version() -> Result<&'static str, std::str::Utf8Error> {
    // SAFETY: PMIx_Get_version() returns a pointer to a NUL-terminated,
    // process-lifetime static string.
    let version = unsafe { CStr::from_ptr(PMIx_Get_version()) };
    version.to_str()
}
/// Manually drive the PMIx event progress loop.
///
/// Call this periodically from your own event loop when running in
/// **external progress mode** (`InitOptions::external_progress(true)`).
/// In internal progress mode (the default), PMIx manages its own thread
/// and you should **not** call this.
///
/// # Interaction with [`progress_thread_stop`]
///
/// Do **not** call `progress()` after [`progress_thread_stop`] — the
/// event base has been torn down and further calls have undefined behavior.
pub fn progress() {
    unsafe {
        PMIx_Progress();
    }
}

/// Stop the internal PMIx progress thread.
///
/// Use this when you initialized with `external_progress(false)` (the
/// default) and want to cleanly shut down the internal progress thread
/// before calling [`finalize`] or exiting.
///
/// # When to call
///
/// - **External progress mode (`external_progress(true)`):** Do **not**
///   call this — there is no internal progress thread to stop.
/// - **Internal progress mode (default):** Call this before [`finalize`]
///   for a clean shutdown. The progress thread will drain pending events
///   and exit gracefully.
/// - **Tests:** If your test calls [`init`] and then exits without
///   calling [`finalize`], call this first to avoid the progress thread
///   running after the test process terminates.
///
/// # Interaction with [`progress()`]
///
/// After calling `progress_thread_stop()`, do **not** call [`progress()`]
/// anymore — the event base has been torn down and further calls have
/// undefined behavior.
///
/// # C API
/// `PMIx_Progress_thread_stop(info, ninfo)` — takes an optional Info
/// array for future extension (pass null/0 for now).
///
/// 6.x-only: OpenPMIx 5.0 has no explicit progress-thread stop; the
/// internal thread is torn down by `PMIx_Finalize`, so this is a no-op there.
#[cfg(pmix6)]
pub fn progress_thread_stop() {
    unsafe {
        PMIx_Progress_thread_stop(std::ptr::null(), 0);
    }
}

/// 5.0 fallback — see [`progress_thread_stop`].
#[cfg(not(pmix6))]
pub fn progress_thread_stop() {}

/// Finalize the process-wide PMIx client session (`PMIx_Finalize`).
///
/// Equivalent to [`PmixClient::disconnect`] on the process session.
/// No-op if the session is not live. Prefer calling
/// [`disconnect`](PmixClient::disconnect) on your client handle when you have one.
pub fn finalize(info: Option<Info>) -> Result<(), pmix_status_t> {
    PmixClient::new().disconnect(info)
}
