//! Process-wide `PmixServer` session — `Clone + Send + Sync` (mirrors [`crate::PmixClient`]).
//!
//! # Drop / finalize
//!
//! **Drop does not finalize.** Call [`PmixServer::disconnect`] or [`super::server_finalize`].

use crate::{Info, PmixStatus, ffi};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::ptr;

#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

use super::PmixServerModule;

/// Lifecycle state of the process-wide PMIx **server** session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PmixServerState {
    Uninitialized = 0,
    Live = 1,
    Finalizing = 2,
    Dead = 3,
}

impl PmixServerState {
    fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::Uninitialized,
            1 => Self::Live,
            2 => Self::Finalizing,
            _ => Self::Dead,
        }
    }
}

struct PmixServerInner {
    /// Serializes connect/disconnect transitions.
    gate: Mutex<()>,
    state: AtomicU8,
}

impl PmixServerInner {
    fn state(&self) -> PmixServerState {
        PmixServerState::from_raw(self.state.load(Ordering::Acquire))
    }
}

static SERVER_SESSION: OnceLock<Arc<PmixServerInner>> = OnceLock::new();

fn server_session() -> Arc<PmixServerInner> {
    SERVER_SESSION
        .get_or_init(|| {
            Arc::new(PmixServerInner {
                gate: Mutex::new(()),
                state: AtomicU8::new(PmixServerState::Uninitialized as u8),
            })
        })
        .clone()
}

/// Thread-shareable handle to the process-wide PMIx server session.
///
/// Cloning is an `Arc` clone. **Drop does not** call `PMIx_server_finalize`.
#[derive(Clone)]
pub struct PmixServer {
    inner: Arc<PmixServerInner>,
}

impl std::fmt::Debug for PmixServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmixServer")
            .field("state", &self.state())
            .finish()
    }
}

impl Default for PmixServer {
    fn default() -> Self {
        Self::new()
    }
}

impl PmixServer {
    /// Attach to the process-wide server session (no `PMIx_server_init`).
    pub fn new() -> Self {
        Self {
            inner: server_session(),
        }
    }

    /// `new()` + [`connect`](Self::connect).
    pub fn connect_new(
        module: Option<&PmixServerModule>,
        info: &Info,
    ) -> Result<Self, PmixStatus> {
        let s = Self::new();
        s.connect(module, info)?;
        Ok(s)
    }

    /// Minimal connect (no info keys).
    pub fn connect_new_minimal(module: Option<&PmixServerModule>) -> Result<Self, PmixStatus> {
        let empty = Info {
            handle: ptr::null_mut(),
            len: 0,
            _not_thread_safe: std::marker::PhantomData,
        };
        Self::connect_new(module, &empty)
    }

    pub fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn state(&self) -> PmixServerState {
        self.inner.state()
    }

    pub fn is_live(&self) -> bool {
        self.state() == PmixServerState::Live
    }

    pub fn check_live(&self) -> Result<(), PmixStatus> {
        if self.is_live() {
            Ok(())
        } else {
            Err(PmixStatus::Known(crate::PmixError::ErrInit))
        }
    }

    /// `PMIx_server_init` — `Uninitialized` → `Live`.
    pub fn connect(
        &self,
        module: Option<&PmixServerModule>,
        info: &Info,
    ) -> Result<(), PmixStatus> {
        let _gate = self
            .inner
            .gate
            .lock()
            .expect("pmix: server session mutex poisoned");

        match self.inner.state() {
            PmixServerState::Uninitialized => {}
            PmixServerState::Live => {
                return Err(PmixStatus::Known(crate::PmixError::ErrExists));
            }
            PmixServerState::Finalizing | PmixServerState::Dead => {
                return Err(PmixStatus::Known(crate::PmixError::ErrInit));
            }
        }

        let module_ptr = match module {
            Some(m) => m.as_c_ptr() as *mut ffi::pmix_server_module_t,
            None => ptr::null_mut(),
        };
        let info_ptr = if info.len > 0 {
            info.handle
        } else {
            ptr::null_mut()
        };
        let info_len = info.len;

        let status = crate::pmix_ffi_or_mock!(
            mock = unsafe {
                mock_ffi::mock_server_init(
                    module_ptr as *mut std::ffi::c_void,
                    info_ptr as *mut std::ffi::c_void,
                    info_len,
                )
            },
            real = unsafe { ffi::PMIx_server_init(module_ptr, info_ptr, info_len) },
        );

        let pmix_status = PmixStatus::from_raw(status);
        if pmix_status.is_success() {
            self.inner
                .state
                .store(PmixServerState::Live as u8, Ordering::Release);
            Ok(())
        } else {
            Err(pmix_status)
        }
    }

    /// `PMIx_server_finalize` — `Live` → `Dead`. No-op if not live.
    pub fn disconnect(&self) -> Result<(), PmixStatus> {
        let _gate = self
            .inner
            .gate
            .lock()
            .expect("pmix: server session mutex poisoned");

        if self.inner.state() != PmixServerState::Live {
            return Ok(());
        }

        self.inner
            .state
            .store(PmixServerState::Finalizing as u8, Ordering::Release);

        let status = crate::pmix_ffi_or_mock!(
            mock = mock_ffi::mock_server_finalize(),
            real = unsafe { ffi::PMIx_server_finalize() },
        );

        // Drop parked event handlers that were never deregistered.
        crate::events::clear_handler_registry();

        self.inner
            .state
            .store(PmixServerState::Dead as u8, Ordering::Release);

        let pmix_status = PmixStatus::from_raw(status);
        if pmix_status.is_success() {
            Ok(())
        } else {
            Err(pmix_status)
        }
    }
}

/// Alias kept for existing call sites — same type as [`PmixServer`].
pub type PmixServerHandle = PmixServer;

#[cfg(test)]
mod session_tests {
    use super::*;
    use static_assertions::assert_impl_all;

    assert_impl_all!(PmixServer: Clone, Send, Sync);
    assert_impl_all!(PmixServerState: Send, Sync);

    #[test]
    fn test_server_session_identity() {
        let a = PmixServer::new();
        let b = PmixServer::new();
        assert!(a.same_session(&b));
        let c = a.clone();
        assert!(a.same_session(&c));
    }

    #[test]
    fn test_server_clone_moves_to_thread() {
        let s = PmixServer::new();
        let state = s.state();
        let w = s.clone();
        let h = std::thread::spawn(move || w.state());
        assert_eq!(h.join().unwrap(), state);
    }

    #[test]
    fn test_server_disconnect_noop_when_not_live() {
        let s = PmixServer::new();
        if s.is_live() {
            return;
        }
        assert!(s.disconnect().is_ok());
        assert!(!s.is_live());
    }
}
