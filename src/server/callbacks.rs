//! Callback wrappers for PMIx operations
//!
//! This module provides native Rust wrappers around PMIx callback types,
//! preventing FFI types from leaking into public APIs.
//!
//! NOTE: this file is **not** an orphan — `src/server/mod.rs` includes it
//! directly via `#[path = "callbacks.rs"] mod native_callbacks`, and
//! re-exports its items both as `pmix::server::callbacks::*` and
//! `pmix::server::native_callbacks::*`. The `PmixServerModule` host-upcall
//! surface itself (the `Server*Callback` type aliases and the C-ABI slot
//! trampolines) lives in `src/server/mod.rs` and is NOT defined by the
//! macros below.
//!
//! What lives here:
//!
//! * `ServerFenceNbCallback` — a registerable native fence upcall that owns
//!   a private `pmix_server_module_t` (the `module` form of
//!   `native_server_callback!`), for hosts that install a single fence
//!   upcall outside `PmixServerModule`.
//! * The `Native*Callback` one-shot adapters (the closure and
//!   function-pointer forms of `native_server_callback!`) and the
//!   `native_rust_server_callback!` context wrappers. Unlike one-shot
//!   completion callbacks, these registerable callbacks receive
//!   PMIx-owned completion data, not wrapper-owned context; registration
//!   cannot adapt an arbitrary capturing closure.
//! * The `into_c_callback()` trailing ownership argument is for one-shot
//!   use only and must not be confused with the native callback's cbdata.
//! Release an unused adapter's data with its `free_*_cbdata` function.

use crate::allocation::{JobControlResults, ResourceBlockDirective, SessionControlResults};
use crate::data_serialization::PmixDataBuffer;
use crate::ffi::pmix_iof_channel_t as IOFChannel;
use crate::ffi::{pmix_app_t, pmix_data_buffer_t, pmix_query_t};
use crate::ffi::{
    pmix_data_range_t, pmix_info_t, pmix_modex_cbfunc_t, pmix_nspace_t, pmix_op_cbfunc_t,
    pmix_pdata_t, pmix_proc_t, pmix_server_fencenb_fn_t, pmix_server_module_t,
};
use crate::security::PmixCredential;
use crate::server::OpCallbackWrapper;
use crate::{Info, PmixStatus};
use std::os::raw::{c_char, c_int, c_void};

macro_rules! native_server_callback {
    (
        $(#[$meta:meta])*
        pointer: $name:ident,
        ($($arg:ident: $arg_type:ty),* $(,)?) -> $return_type:ty
    ) => {
        $(#[$meta])*
        #[derive(Clone)]
        #[allow(clippy::type_complexity)]
        pub struct $name {
            cb: Option<extern "C" fn($($arg_type),*) -> $return_type>,
        }

        #[allow(clippy::type_complexity)]
        impl $name {
            /// Wrap a native function pointer without invoking it.
            ///
            /// Accepts a plain Rust function pointer; non-capturing closures
            /// coerce to it.
            pub fn new(cb: fn($($arg_type),*) -> $return_type) -> Self {
                // SAFETY: On every platform this crate supports, a Rust
                // function pointer and an `extern "C"` function pointer with
                // the same non-variadic signature share the same
                // representation; `PmixServerModule::as_c_ptr` already erases
                // fn-pointer ABIs the same way.
                Self {
                    cb: Some(unsafe {
                        std::mem::transmute::<
                            fn($($arg_type),*) -> $return_type,
                            extern "C" fn($($arg_type),*) -> $return_type,
                        >(cb)
                    }),
                }
            }

            /// Return the wrapped native function pointer.
            pub fn as_c_ptr(&self) -> Option<extern "C" fn($($arg_type),*) -> $return_type> {
                self.cb
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($name), "({})"),
                    if self.cb.is_some() { "Some(..)" } else { "None" }
                )
            }
        }

        // SAFETY: The wrapper contains only a Send function pointer.
        unsafe impl Send for $name {}

        // SAFETY: The wrapper contains only a Sync function pointer.
        unsafe impl Sync for $name {}
    };
    (
        $(#[$meta:meta])*
        $name:ident, $free:ident,
        module: $module:ident, $field:ident: $callback_type:ty,
        ($($arg:ident: $arg_type:ty),* $(,)?) $(-> $return_type:ty)?
    ) => {
        $(#[$meta])*
        pub struct $name {
            module: Box<$module>,
        }

        impl $name {
            /// Wrap a native function pointer without invoking it.
            ///
            /// Capturing closures cannot be registered through this interface:
            /// the native callback has no wrapper-owned context argument.
            /// Represent an absent callback with `Option<Self>::None`.
            pub fn new(
                callback: unsafe extern "C" fn($($arg_type),*) $(-> $return_type)?,
            ) -> Self {
                let callback: $callback_type = Some(callback);
                // SAFETY: PMIx server module fields are nullable function
                // pointers, for which an all-zero representation means None.
                let mut module = Box::new(unsafe { std::mem::zeroed::<$module>() });
                module.$field = callback;
                Self { module }
            }

            /// Borrow a native module containing only this callback.
            ///
            /// All other module fields are null. The allocation remains stable
            /// when this wrapper moves, but is freed when the wrapper is dropped.
            /// Any C code retaining this pointer must finish using it before
            /// then and must not modify or free the borrowed module.
            pub fn as_c_ptr(&self) -> *const $module {
                self.module.as_ref()
            }

            /// Transfer ownership to a one-shot callback and its wrapper data.
            ///
            /// This adapter has an additional trailing ownership argument and
            /// is not the native module callback. Invoke it exactly once, or
            /// release unused wrapper data with the corresponding free function.
            /// Never do both or access the wrapper data concurrently.
            pub fn into_c_callback(
                self,
            ) -> (
                Option<unsafe extern "C" fn($($arg_type,)* *mut c_void) $(-> $return_type)?>,
                *mut c_void,
            ) {
                unsafe extern "C" fn c_callback(
                    $($arg: $arg_type,)*
                    wrapper_cbdata: *mut c_void,
                ) $(-> $return_type)? {
                    // SAFETY: the caller supplies the live pointer returned
                    // by into_c_callback, exactly once.
                    let wrapper = unsafe { Box::from_raw(wrapper_cbdata.cast::<$name>()) };
                    let callback = wrapper.module.$field.expect("callback is present");
                    // SAFETY: the caller must satisfy the native callback's
                    // argument requirements in addition to owning wrapper_cbdata.
                    unsafe { callback($($arg),*) }
                }

                let cbdata = Box::into_raw(Box::new(self)).cast::<c_void>();
                (Some(c_callback), cbdata)
            }
        }

        /// Release unused wrapper data without invoking its callback.
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

macro_rules! native_rust_server_callback {
    (
        $(#[$meta:meta])*
        $name:ident, $test:ident,
        ($($arg:ident: $arg_type:ty),* $(,)?) $(-> $return_type:ty)?
    ) => {
        $(#[$meta])*
        pub struct $name {
            inner: Box<dyn FnMut($($arg_type),*) $(-> $return_type)? + Send + 'static>,
        }

        impl $name {
            /// Create a callback wrapper from a mutable closure.
            ///
            /// The closure must not panic when invoked from C.
            pub fn new<F>(callback: F) -> Self
            where
                F: FnMut($($arg_type),*) $(-> $return_type)? + Send + 'static,
            {
                Self {
                    inner: Box::new(callback),
                }
            }

            /// Invoke the wrapped mutable callback.
            pub fn call(&mut self, $($arg: $arg_type),*) $(-> $return_type)? {
                (self.inner)($($arg),*)
            }

            /// Borrow the boxed closure as opaque callback data.
            ///
            /// This is a context pointer, not a callable C function pointer.
            /// A matching trampoline must cast it back to the original boxed
            /// closure type before invocation. Ownership remains with this
            /// wrapper: C must not free the pointer or retain it after the
            /// wrapper moves or is dropped. Access must be exclusive, including
            /// against Rust access to this wrapper, and must not be reentrant.
            pub fn as_c_ptr(&mut self) -> *mut c_void {
                std::ptr::from_mut(&mut self.inner).cast::<c_void>()
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("inner", &format_args!("{:p}", self.inner.as_ref()))
                    .finish()
            }
        }

        #[cfg(test)]
        #[test]
        fn $test() {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicUsize, Ordering};

            struct DropProbe(Arc<AtomicUsize>);

            impl Drop for DropProbe {
                fn drop(&mut self) {
                    self.0.fetch_add(1, Ordering::SeqCst);
                }
            }

            type Callback =
                Box<dyn FnMut($($arg_type),*) $(-> $return_type)? + Send + 'static>;

            let drops = Arc::new(AtomicUsize::new(0));
            let mut probe = Some(DropProbe(Arc::clone(&drops)));
            let mut wrapper = $name::new(
                move |$($arg: $arg_type),*| $(-> $return_type)? {
                    let _ = ($($arg,)*);
                    let _ = probe.as_mut();
                    panic!("the context round-trip test must not invoke the callback");
                },
            );
            let original = wrapper.inner.as_ref() as *const _ as *const ();
            let cbdata = wrapper.as_c_ptr();
            assert!(!cbdata.is_null());

            // SAFETY: cbdata borrows this live, unmoved wrapper's boxed
            // closure. Its exact type is Callback, and there is no concurrent
            // or reentrant access to the wrapper.
            let recovered = unsafe { &mut *cbdata.cast::<Callback>() };
            let round_tripped = recovered.as_ref() as *const _ as *const ();
            assert_eq!(original, round_tripped);
            assert_eq!(drops.load(Ordering::SeqCst), 0);

            let original_callback = std::mem::replace(
                recovered,
                Box::new(|$($arg: $arg_type),*| $(-> $return_type)? {
                    let _ = ($($arg,)*);
                    panic!("the replacement callback must not be invoked");
                }),
            );
            drop(original_callback);
            assert_eq!(drops.load(Ordering::SeqCst), 1);

            drop(wrapper);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
    };
}

native_server_callback!(
    /// A native server fence callback with an owned registration module.
    ServerFenceNbCallback, free_fence_nb_cbdata,
    module: pmix_server_module_t, fence_nb: pmix_server_fencenb_fn_t,
    (
        procs: *const pmix_proc_t,
        nprocs: usize,
        info: *const pmix_info_t,
        ninfo: usize,
        data: *mut c_char,
        ndata: usize,
        cbfunc: pmix_modex_cbfunc_t,
        cbdata: *mut c_void,
    ) -> pmix_status_t
);

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

native_server_callback!(
    /// Native `dmodex_cbfunc_t` data-plane upcall wrapper.
    pointer: NativeDirectModexCallback,
    (
        nspace: *const c_char,
        rank: c_int,
        data: *mut c_void,
        cbfunc: Option<extern "C" fn(
            pmix_status_t,
            *mut c_void,
            *const pmix_data_buffer_t,
            *mut c_void,
        )>,
        cbdata: *mut c_void,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `connect_cbfunc_t` job upcall wrapper.
    pointer: NativeConnectCallback,
    (
        nspace: *const c_char,
        info: *const pmix_info_t,
        ninfo: usize,
        results: *mut pmix_pdata_t,
        nresults: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `op_cbfunc_t` disconnect upcall wrapper.
    pointer: NativeDisconnectCallback,
    (
        nspace: *const c_char,
        info: *const pmix_info_t,
        ninfo: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `spawn_cbfunc_t` job upcall wrapper.
    pointer: NativeSpawnCallback,
    (
        info: *const pmix_info_t,
        ninfo: usize,
        apps: *const pmix_app_t,
        napps: usize,
        jobid: *mut pmix_proc_t,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `op_cbfunc_t` abort upcall wrapper.
    pointer: NativeAbortCallback,
    (
        nspace: *const c_char,
        rank: c_int,
        error_code: c_int,
        errmsg: *const c_char,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `pmix_server_log_fn_t` upcall wrapper.
    pointer: NativeLogCallback,
    (
        info: *const pmix_info_t,
        ninfo: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
        cbdata: *mut c_void,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `pmix_server_log2_fn_t` upcall wrapper.
    pointer: NativeLog2Callback,
    (
        proc: *const pmix_proc_t,
        info: *const pmix_info_t,
        ninfo: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
        cbdata: *mut c_void,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native `pmix_query_cbfunc_t` upcall wrapper.
    pointer: NativeQueryCallback,
    (
        nspace: *const c_char,
        rank: c_int,
        queries: *const pmix_query_t,
        nqueries: usize,
        cbfunc: Option<extern "C" fn(
            pmix_status_t,
            *mut c_void,
            *const pmix_data_buffer_t,
            *mut c_void,
        )>,
        cbdata: *mut c_void,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native event listener upcall wrapper.
    pointer: NativeListenerCallback,
    (
        nspace: *const c_char,
        rank: c_int,
        evhdlr_registration_id: c_int,
        status: pmix_status_t,
        source: *const pmix_proc_t,
        info: *const pmix_info_t,
        ninfo: usize,
        results: *mut pmix_pdata_t,
        nresults: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_server_callback!(
    /// Native event monitor upcall wrapper.
    pointer: NativeMonitorCallback,
    (
        status: pmix_status_t,
        source: *const pmix_proc_t,
        info: *const pmix_info_t,
        ninfo: usize,
        cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)>,
    ) -> pmix_status_t
);

native_rust_server_callback!(
    /// A native Rust allocation callback.
    NativeAllocateCallback, native_allocate_context_round_trip,
    (
        directives: &[Info],
        info: &[Info],
        buffer: &mut PmixDataBuffer,
    ) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust job-control completion callback.
    NativeJobControlCallback, native_job_control_context_round_trip,
    (status: PmixStatus, results: JobControlResults)
);

native_rust_server_callback!(
    /// A native Rust session-control completion callback.
    NativeSessionControlCallback, native_session_control_context_round_trip,
    (status: PmixStatus, results: SessionControlResults)
);

native_rust_server_callback!(
    /// A native Rust resource-block callback.
    NativeResourceBlockCallback, native_resource_block_context_round_trip,
    (
        directive: ResourceBlockDirective,
        info: &mut [Info],
    ) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust credential-generation callback.
    NativeGetCredentialCallback, native_get_credential_context_round_trip,
    (info: &[Info], credential: &mut PmixCredential) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust credential-validation callback.
    NativeValidateCredentialCallback, native_validate_credential_context_round_trip,
    (credential: &PmixCredential, info: &[Info]) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust I/O-forwarding pull callback.
    NativeIOFPullCallback, native_iof_pull_context_round_trip,
    (channel: IOFChannel, data: &mut [u8]) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust standard-input callback.
    NativePushStdinCallback, native_push_stdin_context_round_trip,
    (data: &[u8]) -> PmixStatus
);

native_rust_server_callback!(
    /// A native Rust group-operation callback.
    NativeGroupCallback, native_group_context_round_trip,
    (status: PmixStatus, results: Vec<Info>)
);

native_rust_server_callback!(
    /// A native Rust fabric-operation callback.
    NativeFabricCallback, native_fabric_context_round_trip,
    (status: PmixStatus)
);

// PMIx status type alias for FFI compatibility
#[allow(non_camel_case_types)]
type pmix_status_t = i32;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    macro_rules! test_native_function_pointer_callback {
        (
            $callback:ident, $wrapper:ident,
            $new:ident, $as_c_ptr:ident, $roundtrip:ident,
            ($($arg:ident: $arg_type:ty = $value:expr),* $(,)?)
        ) => {
            extern "Rust" fn $callback($($arg: $arg_type),*) -> pmix_status_t {
                -17
            }

            #[test]
            fn $new() {
                fn assert_traits<T: Clone + Send + Sync>() {}
                assert_traits::<$wrapper>();

                let wrapper = $wrapper::new($callback);
                assert!(wrapper.cb.is_some());
            }

            #[test]
            fn $as_c_ptr() {
                let wrapper = $wrapper::new($callback);
                assert!(wrapper.as_c_ptr().is_some());

                let empty = $wrapper { cb: None };
                assert!(empty.as_c_ptr().is_none());

                let absent: Option<$wrapper> = None;
                assert!(absent.as_ref().and_then($wrapper::as_c_ptr).is_none());
            }

            #[test]
            fn $roundtrip() {
                let callback = {
                    let wrapper = $wrapper::new($callback);
                    let cloned = wrapper.clone();
                    cloned.as_c_ptr().expect("native callback")
                };
                assert_eq!(callback($($value),*), -17);
            }
        };
    }

    unsafe extern "C" fn fence_nb_callback(
        procs: *const pmix_proc_t,
        nprocs: usize,
        info: *const pmix_info_t,
        ninfo: usize,
        data: *mut c_char,
        ndata: usize,
        cbfunc: pmix_modex_cbfunc_t,
        cbdata: *mut c_void,
    ) -> pmix_status_t {
        assert!(procs.is_null());
        assert_eq!(nprocs, 3);
        assert!(info.is_null());
        assert_eq!(ninfo, 7);
        assert!(data.is_null());
        assert_eq!(ndata, 11);
        assert!(cbfunc.is_none());
        assert!(cbdata.is_null());
        -17
    }

    #[test]
    fn fence_nb() {
        let absent: Option<ServerFenceNbCallback> = None;
        assert!(
            absent
                .as_ref()
                .map_or(std::ptr::null(), ServerFenceNbCallback::as_c_ptr)
                .is_null()
        );

        for invoke in [true, false] {
            let wrapper = ServerFenceNbCallback::new(fence_nb_callback);
            let module = wrapper.as_c_ptr();
            assert!(!module.is_null());
            assert_eq!(module, wrapper.as_c_ptr());

            // SAFETY: wrapper owns the live module. The test callback only
            // checks its arguments and does not dereference any pointers.
            let result = unsafe {
                ((*module).fence_nb.expect("fence callback"))(
                    std::ptr::null(),
                    3,
                    std::ptr::null(),
                    7,
                    std::ptr::null_mut(),
                    11,
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(result, -17);

            let (callback, cbdata) = wrapper.into_c_callback();
            assert!(callback.is_some());
            assert!(!cbdata.is_null());

            if invoke {
                // SAFETY: cbdata is live and consumed exactly once. The
                // native callback checks but never dereferences its arguments.
                let result = unsafe {
                    callback.unwrap()(
                        std::ptr::null(),
                        3,
                        std::ptr::null(),
                        7,
                        std::ptr::null_mut(),
                        11,
                        None,
                        std::ptr::null_mut(),
                        cbdata,
                    )
                };
                assert_eq!(result, -17);
            } else {
                // SAFETY: cbdata has not been invoked or freed.
                unsafe {
                    free_fence_nb_cbdata(cbdata);
                }
            }
        }

        // SAFETY: the free function explicitly accepts null.
        unsafe {
            free_fence_nb_cbdata(std::ptr::null_mut());
        }
    }

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

    test_native_function_pointer_callback!(
        direct_modex_callback, NativeDirectModexCallback,
        test_native_direct_modex_callback_new,
        test_native_direct_modex_callback_as_c_ptr,
        test_native_direct_modex_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _rank: c_int = 3,
            _data: *mut c_void = std::ptr::null_mut(),
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void, *const pmix_data_buffer_t, *mut c_void)> = None,
            _cbdata: *mut c_void = std::ptr::null_mut(),
        )
    );

    test_native_function_pointer_callback!(
        connect_callback, NativeConnectCallback,
        test_native_connect_callback_new,
        test_native_connect_callback_as_c_ptr,
        test_native_connect_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 3,
            _results: *mut pmix_pdata_t = std::ptr::null_mut(),
            _nresults: usize = 7,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );

    test_native_function_pointer_callback!(
        disconnect_callback, NativeDisconnectCallback,
        test_native_disconnect_callback_new,
        test_native_disconnect_callback_as_c_ptr,
        test_native_disconnect_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 3,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );

    test_native_function_pointer_callback!(
        spawn_callback, NativeSpawnCallback,
        test_native_spawn_callback_new,
        test_native_spawn_callback_as_c_ptr,
        test_native_spawn_callback_roundtrip,
        (
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 3,
            _apps: *const pmix_app_t = std::ptr::null(),
            _napps: usize = 7,
            _jobid: *mut pmix_proc_t = std::ptr::null_mut(),
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );

    test_native_function_pointer_callback!(
        abort_callback, NativeAbortCallback,
        test_native_abort_callback_new,
        test_native_abort_callback_as_c_ptr,
        test_native_abort_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _rank: c_int = 3,
            _error_code: c_int = -23,
            _errmsg: *const c_char = std::ptr::null(),
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );

    test_native_function_pointer_callback!(
        log_callback, NativeLogCallback,
        test_native_log_callback_new,
        test_native_log_callback_as_c_ptr,
        test_native_log_callback_roundtrip,
        (
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 3,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
            _cbdata: *mut c_void = std::ptr::null_mut(),
        )
    );

    test_native_function_pointer_callback!(
        log2_callback, NativeLog2Callback,
        test_native_log2_callback_new,
        test_native_log2_callback_as_c_ptr,
        test_native_log2_callback_roundtrip,
        (
            _proc: *const pmix_proc_t = std::ptr::null(),
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 7,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
            _cbdata: *mut c_void = std::ptr::null_mut(),
        )
    );

    test_native_function_pointer_callback!(
        query_callback, NativeQueryCallback,
        test_native_query_callback_new,
        test_native_query_callback_as_c_ptr,
        test_native_query_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _rank: c_int = 3,
            _queries: *const pmix_query_t = std::ptr::null(),
            _nqueries: usize = 7,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void, *const pmix_data_buffer_t, *mut c_void)> = None,
            _cbdata: *mut c_void = std::ptr::null_mut(),
        )
    );

    test_native_function_pointer_callback!(
        listener_callback, NativeListenerCallback,
        test_native_listener_callback_new,
        test_native_listener_callback_as_c_ptr,
        test_native_listener_callback_roundtrip,
        (
            _nspace: *const c_char = std::ptr::null(),
            _rank: c_int = 3,
            _evhdlr_registration_id: c_int = 7,
            _status: pmix_status_t = -23,
            _source: *const pmix_proc_t = std::ptr::null(),
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 11,
            _results: *mut pmix_pdata_t = std::ptr::null_mut(),
            _nresults: usize = 13,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );

    test_native_function_pointer_callback!(
        monitor_callback, NativeMonitorCallback,
        test_native_monitor_callback_new,
        test_native_monitor_callback_as_c_ptr,
        test_native_monitor_callback_roundtrip,
        (
            _status: pmix_status_t = -23,
            _source: *const pmix_proc_t = std::ptr::null(),
            _info: *const pmix_info_t = std::ptr::null(),
            _ninfo: usize = 3,
            _cbfunc: Option<extern "C" fn(pmix_status_t, *mut c_void)> = None,
        )
    );
}
