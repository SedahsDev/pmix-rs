//! Callback wrappers for PMIx operations
//!
//! This module provides native Rust wrappers around PMIx callback types,
//! preventing FFI types from leaking into public APIs.

use crate::PmixStatus;
use crate::ffi::{pmix_data_range_t, pmix_info_t, pmix_op_cbfunc_t, pmix_pdata_t, pmix_proc_t};
use std::os::raw::c_void;

macro_rules! native_server_callback {
    (
        $(#[$meta:meta])*
        $name:ident, $free:ident,
        ($($arg:ident: $arg_type:ty),* $(,)?) $(-> $return_type:ty)?
    ) => {
        $(#[$meta])*
        pub struct $name {
            inner: Box<dyn Fn($($arg_type),*) $(-> $return_type)? + Send + 'static>,
        }

        impl $name {
            /// Create a callback wrapper from a closure.
            ///
            /// The closure must not panic when invoked from C.
            pub fn new<F>(f: F) -> Self
            where
                F: Fn($($arg_type),*) $(-> $return_type)? + Send + 'static,
            {
                Self { inner: Box::new(f) }
            }

            /// Transfer ownership to a one-shot C callback and its cbdata.
            ///
            /// The returned callback consumes and frees cbdata when invoked.
            /// Invoke it exactly once, or release unused cbdata with the
            /// corresponding free function. Never do both, reuse cbdata, or
            /// access it concurrently with either operation.
            pub fn into_c_callback(
                self,
            ) -> (
                Option<unsafe extern "C" fn($($arg_type,)* *mut c_void) $(-> $return_type)?>,
                *mut c_void,
            ) {
                unsafe extern "C" fn c_callback(
                    $($arg: $arg_type,)*
                    cbdata: *mut c_void,
                ) $(-> $return_type)? {
                    // SAFETY: the caller must supply the live pointer returned
                    // by this wrapper's into_c_callback, exactly once.
                    let wrapper = unsafe { Box::from_raw(cbdata.cast::<$name>()) };
                    (wrapper.inner)($($arg),*)
                }

                let cbdata = Box::into_raw(Box::new(self)).cast::<c_void>();
                (Some(c_callback), cbdata)
            }
        }

        /// Release unused cbdata without invoking its callback.
        ///
        /// # Safety
        ///
        /// A non-null pointer must come from this wrapper's into_c_callback
        /// and must not have been consumed by the callback or previously freed.
        /// No other thread may access it during or after this call.
        pub unsafe fn $free(cbdata: *mut c_void) {
            if !cbdata.is_null() {
                // SAFETY: the caller guarantees ownership of this live Box.
                unsafe {
                    drop(Box::from_raw(cbdata.cast::<$name>()));
                }
            }
        }
    };
}

native_server_callback!(
    /// A one-shot Rust callback for client connections.
    NativeClientConnectedCallback, free_client_connected_cbdata,
    (proc: pmix_proc_t, info: pmix_info_t, ninfo: usize) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for client connections with additional info.
    NativeClientConnected2Callback, free_client_connected2_cbdata,
    (
        proc: pmix_proc_t,
        info: pmix_info_t,
        ninfo: usize,
        additional_info: pmix_info_t,
        nadditional_info: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for client finalization.
    NativeClientFinalizedCallback, free_client_finalized_cbdata,
    (proc: pmix_proc_t)
);

native_server_callback!(
    /// A one-shot Rust callback for tool connections.
    NativeToolConnectedCallback, free_tool_connected_cbdata,
    (proc: pmix_proc_t, info: pmix_info_t, ninfo: usize) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for tool connections with additional info.
    NativeToolConnected2Callback, free_tool_connected2_cbdata,
    (
        proc: pmix_proc_t,
        info: pmix_info_t,
        ninfo: usize,
        additional_info: pmix_info_t,
        nadditional_info: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for publishing data.
    NativePublishCallback, free_publish_cbdata,
    (
        proc: pmix_proc_t,
        info: pmix_info_t,
        ninfo: usize,
        directives: pmix_info_t,
        ndirectives: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for looking up published data.
    NativeLookupCallback, free_lookup_cbdata,
    (
        proc: pmix_proc_t,
        pdata: pmix_pdata_t,
        npdata: usize,
        info: pmix_info_t,
        ninfo: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for unpublishing data.
    NativeUnpublishCallback, free_unpublish_cbdata,
    (
        proc: pmix_proc_t,
        info: pmix_info_t,
        ninfo: usize,
        directives: pmix_info_t,
        ndirectives: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for registering events.
    NativeRegisterEventsCallback, free_register_events_cbdata,
    (
        proc: pmix_proc_t,
        info: pmix_info_t,
        ninfo: usize,
        directives: pmix_info_t,
        ndirectives: usize,
    ) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for deregistering events.
    NativeDeregisterEventsCallback, free_deregister_events_cbdata,
    (proc: pmix_proc_t, info: pmix_info_t, ninfo: usize) -> pmix_status_t
);

native_server_callback!(
    /// A one-shot Rust callback for event notifications.
    NativeNotifyEventCallback, free_notify_event_cbdata,
    (
        status: pmix_status_t,
        proc: pmix_proc_t,
        range: pmix_data_range_t,
        info: pmix_info_t,
        ninfo: usize,
        completion: pmix_op_cbfunc_t,
        completion_cbdata: *mut c_void,
    ) -> pmix_status_t
);
// PMIx status type alias for FFI compatibility
#[allow(non_camel_case_types)]
type pmix_status_t = i32;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropCounter(Arc<AtomicUsize>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    macro_rules! test_native_server_callback {
        (
            $test:ident, $wrapper:ident, $free:ident,
            ($($arg:ident = $value:expr),* $(,)?),
            $body:block, $expected:expr
        ) => {
            #[test]
            fn $test() {
                for invoke in [true, false] {
                    let calls = Arc::new(AtomicUsize::new(0));
                    let drops = Arc::new(AtomicUsize::new(0));
                    let callback_calls = Arc::clone(&calls);
                    let drop_counter = DropCounter(Arc::clone(&drops));

                    let wrapper = $wrapper::new(move |$($arg),*| {
                        let _keep_alive = &drop_counter;
                        callback_calls.fetch_add(1, Ordering::SeqCst);
                        $body
                    });
                    let (callback, cbdata) = wrapper.into_c_callback();
                    assert!(callback.is_some());
                    assert!(!cbdata.is_null());
                    assert_eq!(drops.load(Ordering::SeqCst), 0);

                    if invoke {
                        // SAFETY: cbdata is live and consumed exactly once.
                        // Zeroed PMIx C structs are passed only as opaque values;
                        // their embedded pointers are never dereferenced.
                        let result = unsafe {
                            callback.unwrap()($($value,)* cbdata)
                        };
                        assert_eq!(result, $expected);
                    } else {
                        // SAFETY: cbdata has not been invoked or freed.
                        unsafe {
                            $free(cbdata);
                        }
                    }

                    assert_eq!(calls.load(Ordering::SeqCst), usize::from(invoke));
                    assert_eq!(drops.load(Ordering::SeqCst), 1);

                    // SAFETY: the free functions explicitly accept null.
                    unsafe {
                        $free(std::ptr::null_mut());
                    }
                }
            }
        };
    }

    test_native_server_callback!(
        client_connected,
        NativeClientConnectedCallback,
        free_client_connected_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
        ),
        {
            assert_eq!(ninfo, 3);
            -17
        },
        -17
    );

    test_native_server_callback!(
        client_connected2,
        NativeClientConnected2Callback,
        free_client_connected2_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            _additional_info = std::mem::zeroed::<pmix_info_t>(),
            nadditional_info = 7,
        ),
        {
            assert_eq!((ninfo, nadditional_info), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        client_finalized,
        NativeClientFinalizedCallback,
        free_client_finalized_cbdata,
        (_proc = std::mem::zeroed::<pmix_proc_t>()),
        {},
        ()
    );

    test_native_server_callback!(
        tool_connected,
        NativeToolConnectedCallback,
        free_tool_connected_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
        ),
        {
            assert_eq!(ninfo, 3);
            -17
        },
        -17
    );

    test_native_server_callback!(
        tool_connected2,
        NativeToolConnected2Callback,
        free_tool_connected2_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            _additional_info = std::mem::zeroed::<pmix_info_t>(),
            nadditional_info = 7,
        ),
        {
            assert_eq!((ninfo, nadditional_info), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        publish,
        NativePublishCallback,
        free_publish_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            _directives = std::mem::zeroed::<pmix_info_t>(),
            ndirectives = 7,
        ),
        {
            assert_eq!((ninfo, ndirectives), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        lookup,
        NativeLookupCallback,
        free_lookup_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _pdata = std::mem::zeroed::<pmix_pdata_t>(),
            npdata = 3,
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 7,
        ),
        {
            assert_eq!((npdata, ninfo), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        unpublish,
        NativeUnpublishCallback,
        free_unpublish_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            _directives = std::mem::zeroed::<pmix_info_t>(),
            ndirectives = 7,
        ),
        {
            assert_eq!((ninfo, ndirectives), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        register_events,
        NativeRegisterEventsCallback,
        free_register_events_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            _directives = std::mem::zeroed::<pmix_info_t>(),
            ndirectives = 7,
        ),
        {
            assert_eq!((ninfo, ndirectives), (3, 7));
            -17
        },
        -17
    );

    test_native_server_callback!(
        deregister_events,
        NativeDeregisterEventsCallback,
        free_deregister_events_cbdata,
        (
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
        ),
        {
            assert_eq!(ninfo, 3);
            -17
        },
        -17
    );

    unsafe extern "C" fn notify_completion(status: pmix_status_t, cbdata: *mut c_void) {
        assert_eq!(status, 17);
        assert_eq!(
            cbdata,
            std::ptr::NonNull::<u8>::dangling()
                .as_ptr()
                .cast::<c_void>()
        );
    }

    test_native_server_callback!(
        notify_event,
        NativeNotifyEventCallback,
        free_notify_event_cbdata,
        (
            status = -23,
            _proc = std::mem::zeroed::<pmix_proc_t>(),
            _range = std::mem::zeroed::<pmix_data_range_t>(),
            _info = std::mem::zeroed::<pmix_info_t>(),
            ninfo = 3,
            completion = Some(notify_completion),
            completion_cbdata = std::ptr::NonNull::<u8>::dangling()
                .as_ptr()
                .cast::<c_void>(),
        ),
        {
            assert_eq!(status, -23);
            assert_eq!(ninfo, 3);
            // SAFETY: the test supplies notify_completion, which checks but
            // never dereferences its cbdata.
            unsafe {
                completion.expect("completion callback")(17, completion_cbdata);
            }
            -17
        },
        -17
    );
}
