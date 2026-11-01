//! Callback wrappers for PMIx operations
//!
//! This module provides native Rust wrappers around PMIx callback types,
//! preventing FFI types from leaking into public APIs.

use crate::PmixStatus;
use std::os::raw::c_void;

/// A wrapper for PMIx operation callbacks (`pmix_op_cbfunc_t`).
///
/// This wrapper provides a safe, idiomatic Rust interface for operation
/// completion callbacks. It stores a closure that receives the operation
/// status.
///
/// # Example
///
/// ```ignore
/// let wrapper = OpCallbackWrapper::new(|status| {
///     if status.is_success() {
///         println!("Operation completed successfully");
///     }
/// });
/// ```
pub struct OpCallbackWrapper {
    inner: Box<dyn FnOnce(PmixStatus) + Send + 'static>,
}

impl OpCallbackWrapper {
    /// Create a new operation callback wrapper from a closure.
    ///
    /// The closure must be `Send` to allow cross-thread invocation.
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(PmixStatus) + Send + 'static,
    {
        Self { inner: Box::new(f) }
    }

    /// Invoke the callback with the given status.
    ///
    /// This consumes the wrapper since the callback is `FnOnce`.
    pub fn invoke(self, status: PmixStatus) {
        (self.inner)(status);
    }

    /// Convert to a raw C callback function pointer and associated cbdata.
    ///
    /// This is an unsafe operation that prepares the wrapper for FFI.
    /// The caller must ensure the returned pointer is only used within
    /// the lifetime of this wrapper and that cbdata is properly managed.
    ///
    /// # Safety
    ///
    /// The returned `cbdata` pointer must be freed by calling
    /// `free_cbdata` after the callback is invoked or if the callback
    /// is no longer needed.
    pub fn into_c_callback(
        self,
    ) -> (
        Option<unsafe extern "C" fn(pmix_status_t, *mut c_void)>,
        *mut c_void,
    ) {
        // Create a boxed closure that will be passed as cbdata
        let box_closure = Box::new(self);
        let cbdata = Box::into_raw(box_closure) as *mut c_void;

        extern "C" fn c_callback(status: pmix_status_t, cbdata: *mut c_void) {
            if cbdata.is_null() {
                return;
            }
            // SAFETY: cbdata was created from Box::into_raw above
            unsafe {
                let wrapper = Box::from_raw(cbdata as *mut OpCallbackWrapper);
                wrapper.invoke(PmixStatus::from_raw(status));
            }
        }

        (Some(c_callback), cbdata)
    }
}

/// Free the cbdata associated with a callback.
///
/// Call this after the callback has been invoked or if the callback
/// is no longer needed to prevent memory leaks.
pub fn free_op_callback_cbdata(cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }
    // SAFETY: cbdata was created from Box::into_raw in into_c_callback
    unsafe {
        let _ = Box::from_raw(cbdata as *mut OpCallbackWrapper);
    }
}

// PMIx status type alias for FFI compatibility
type pmix_status_t = i32;
