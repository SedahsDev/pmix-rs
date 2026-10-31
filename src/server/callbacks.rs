use std::os::raw::c_void;

/// Wrapper for PMIx operation callback functions.
///
/// Stores a user-provided closure and exposes it through the FFI callback
/// signature. The wrapper is `Send` but not `Sync`, allowing it to be moved
/// across threads but not shared concurrently.
pub struct OpCallbackWrapper {
    cb: Option<Box<dyn FnMut(crate::PmixStatus, *mut c_void) + Send + 'static>>,
    c_callback: unsafe extern "C" fn(crate::ffi::pmix_status_t, *mut std::os::raw::c_void),
}

impl OpCallbackWrapper {
    /// Create a new OpCallbackWrapper with the given closure.
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(crate::PmixStatus, *mut c_void) + Send + 'static,
    {
        Self {
            cb: Some(Box::new(f)),
            c_callback: op_callback_bridge,
        }
    }

    /// Get the C callback function pointer for FFI calls.
    pub fn as_c_callback(
        &self,
    ) -> Option<unsafe extern "C" fn(crate::ffi::pmix_status_t, *mut std::os::raw::c_void)> {
        Some(self.c_callback)
    }

    /// Invoke the stored closure with the given status and callback data.
    pub fn invoke(&mut self, status: crate::PmixStatus, cbdata: *mut c_void) {
        if let Some(ref mut cb) = self.cb {
            cb(status, cbdata);
        }
    }
}

impl Default for OpCallbackWrapper {
    fn default() -> Self {
        Self {
            cb: None,
            c_callback: op_callback_bridge,
        }
    }
}

unsafe extern "C" fn op_callback_bridge(status: crate::ffi::pmix_status_t, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }
    let wrapper = unsafe { &mut *(cbdata as *mut OpCallbackWrapper) };
    if let Some(ref mut cb) = wrapper.cb {
        let status = crate::PmixStatus::from_raw(status);
        cb(status, cbdata);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_op_callback_wrapper_new() {
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = std::sync::Arc::clone(&called);
        let wrapper = OpCallbackWrapper::new(move |_status, _cbdata| {
            called_clone.store(true, Ordering::SeqCst);
        });
        assert_eq!(
            wrapper.as_c_callback(),
            Some(op_callback_bridge as unsafe extern "C" fn(_, _))
        );
    }

    #[test]
    fn test_op_callback_wrapper_invoke() {
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = std::sync::Arc::clone(&called);
        let mut wrapper = OpCallbackWrapper::new(move |_status, _cbdata| {
            called_clone.store(true, Ordering::SeqCst);
        });
        wrapper.invoke(
            crate::PmixStatus::Known(crate::PmixError::Success),
            std::ptr::null_mut(),
        );
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_op_callback_wrapper_default() {
        let wrapper = OpCallbackWrapper::default();
        assert!(wrapper.cb.is_none());
        assert_eq!(
            wrapper.as_c_callback(),
            Some(op_callback_bridge as unsafe extern "C" fn(_, _))
        );
    }

    #[test]
    fn test_op_callback_wrapper_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OpCallbackWrapper>();
    }
}
