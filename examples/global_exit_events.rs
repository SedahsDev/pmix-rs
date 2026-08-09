//! Global-exit event handling with blocking and non-blocking registration.
//!
//! This example demonstrates event-handler registration, `notify_event`,
//! deregistration, and abort-from-handler for the global-exit pattern used by
//! osss-ucx (`src/shmemc/ucx/pmix_client.c`) and Open MPI (`ompi_rte.c`). The
//! blocking handler runs on PMIx's progress thread, hops to an application
//! thread, completes the event chain, and then calls `PMIx_Abort` through the
//! safe Rust wrapper. A second handler demonstrates non-blocking registration.
//!
//! Run standalone with `cargo run --example global_exit_events`; without a DVM
//! the connection failure is reported and the example exits gracefully. With
//! a DVM, run `prterun -n 2 ./target/debug/examples/global_exit_events`.

use std::os::raw::c_void;
use std::ptr;
use std::sync::mpsc;
use std::time::Duration;

fn main() {
    println!("pmix-rs: global-exit event example");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };
    let proc = client.require_proc();
    println!("connected: rank {}", proc.get_rank());

    let abort_event = pmix::PmixStatus::Known(pmix::PmixError::ErrProcRequestedAbort);
    let empty = pmix::info::empty();
    let abort_ref = match pmix::events::register_event_handler(
        &[abort_event],
        &empty,
        Some(on_global_exit),
        None,
    ) {
        Ok(reference) => {
            println!("registered blocking global-exit handler {reference}");
            Some(reference)
        }
        Err(error) => {
            println!("blocking registration failed: {error:?}");
            None
        }
    };

    let ready_event = pmix::PmixStatus::Known(pmix::PmixError::ReadyForDebug);
    let (registration_tx, registration_rx) = mpsc::channel::<(i32, usize)>();
    let registration_data = Box::new(registration_tx);
    let registration_data = Box::into_raw(registration_data).cast::<c_void>();
    let ready_result = pmix::events::register_event_handler_nb(
        &[ready_event],
        &empty,
        Some(on_ready_for_debug),
        Some(on_registration),
        registration_data,
    );
    match ready_result {
        Ok(()) => println!("submitted non-blocking ready-for-debug registration"),
        Err(error) => {
            println!("non-blocking registration failed: {error:?}");
            // The callback cannot run after a synchronous failure, so reclaim
            // the sender ownership transferred to the registration request.
            // SAFETY: `register_event_handler_nb` returned an error and thus
            // did not transfer ownership of this boxed sender to PMIx.
            unsafe {
                drop(Box::from_raw(
                    registration_data.cast::<mpsc::Sender<(i32, usize)>>(),
                ));
            }
        }
    }

    let ready_ref = match registration_rx.recv_timeout(Duration::from_secs(2)) {
        Ok((status, reference)) => {
            println!("non-blocking registration callback: status={status}, ref={reference}");
            (status == 0).then_some(reference)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            println!("no non-blocking registration callback within 2s");
            None
        }
        Err(error) => {
            println!("non-blocking registration callback channel closed: {error}");
            None
        }
    };

    std::thread::sleep(Duration::from_secs(1));
    if proc.get_rank() == 0 {
        let mut builder = pmix::InfoBuilder::new();
        if let Err(error) = builder.add_int_key("pmix.exit.code", 17) {
            println!("could not build exit-code info: {error}");
        } else {
            match builder.build() {
                Ok(exit_info) => match pmix::events::notify_event(
                    abort_event,
                    &proc,
                    pmix::PmixDataRange::Namespace,
                    &exit_info,
                ) {
                    Ok(()) => println!("rank 0 notified global exit"),
                    Err(error) => println!("notify_event failed: {error:?}"),
                },
                Err(error) => println!("building exit-code info failed: {error:?}"),
            }
        }
    } else {
        println!(
            "rank {} waiting for global-exit notification",
            proc.get_rank()
        );
        std::thread::sleep(Duration::from_secs(3));
    }

    for reference in [abort_ref, ready_ref].into_iter().flatten() {
        match pmix::events::deregister_event_handler(reference, None) {
            Ok(()) => println!("deregistered event handler {reference}"),
            Err(error) => println!("deregister handler {reference} failed: {error:?}"),
        }
    }
    match client.disconnect(None) {
        Ok(()) => println!("disconnected"),
        Err(error) => println!("disconnect failed: {error:?}"),
    }
    println!("global_exit_events example done");
}

/// Handles global-exit notifications on PMIx's progress thread.
unsafe fn on_global_exit(
    _id: pmix::events::EventHandlerRef,
    status: i32,
    _source: *const c_void,
    _info: *mut c_void,
    _ninfo: usize,
    _results: *mut c_void,
    _nresults: usize,
    cbfunc: pmix::events::pmix_event_notification_cbfunc_fn_t,
    cbdata: *mut c_void,
) {
    let _progress = pmix::threading::ProgressContext;
    let chain_addr = cbdata as usize;
    let _ = pmix::threading::spawn_from_callback(move || {
        println!("[app thread] global-exit status={status}; exit code info was delivered");
        if let Some(cbfunc) = cbfunc {
            // SAFETY: PMIx supplied this chain callback and cbdata for this
            // notification; completing it once is required by the API.
            unsafe {
                cbfunc(
                    status,
                    ptr::null_mut(),
                    0,
                    None,
                    ptr::null_mut(),
                    chain_addr as *mut c_void,
                );
            }
        }
        match pmix::process_mgmt::abort(
            pmix::PmixStatus::Known(pmix::PmixError::Error),
            Some("global_exit"),
            None,
        ) {
            Ok(()) => println!("[app thread] abort accepted"),
            Err(error) => println!("[app thread] abort failed: {error:?}"),
        }
    });
}

/// Completes a ready-for-debug notification without blocking progress.
unsafe fn on_ready_for_debug(
    _id: pmix::events::EventHandlerRef,
    status: i32,
    _source: *const c_void,
    _info: *mut c_void,
    _ninfo: usize,
    _results: *mut c_void,
    _nresults: usize,
    cbfunc: pmix::events::pmix_event_notification_cbfunc_fn_t,
    cbdata: *mut c_void,
) {
    let _progress = pmix::threading::ProgressContext;
    let chain_addr = cbdata as usize;
    let _ = pmix::threading::spawn_from_callback(move || {
        println!("[app thread] ready-for-debug status={status}");
        if let Some(cbfunc) = cbfunc {
            // SAFETY: PMIx supplied the callback and chain data for this event.
            unsafe {
                cbfunc(
                    status,
                    ptr::null_mut(),
                    0,
                    None,
                    ptr::null_mut(),
                    chain_addr as *mut c_void,
                );
            }
        }
    });
}

unsafe extern "C" fn on_registration(status: i32, reference: usize, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    // SAFETY: the registration request transfers ownership of this boxed
    // sender to the completion callback exactly once.
    let sender = unsafe { Box::from_raw(data.cast::<mpsc::Sender<(i32, usize)>>()) };
    let _ = sender.send((status, reference));
}
