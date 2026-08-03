//! Callback hop-off-progress example (issue #51).
//!
//! Demonstrates the two shared helpers from [`pmix::threading`] with **one
//! `data_ops` non-blocking path** (`get_nb`) and **one events path**
//! (`register_event_handler` + `notify_event`):
//!
//! * [`CallbackChannel`](pmix::threading::CallbackChannel) — the application
//!   thread keeps the receiver; the `get_nb` completion callback (which PMIx
//!   invokes on the **progress thread**) pushes Rust-owned data through a
//!   cloned sender and returns immediately.
//! * [`spawn_from_callback`](pmix::threading::spawn_from_callback) — the event
//!   handler (also on the progress thread) hops blocking work onto a fresh
//!   application thread.
//! * [`ProgressContext`](pmix::threading::ProgressContext) — a zero-sized
//!   marker that documents "we are on the progress thread" and the forbidden
//!   APIs at the point of use.
//!
//! # Running
//!
//! The example compiles and runs standalone (it exits gracefully when no DVM
//! is present). Under a process manager it exercises real callbacks:
//!
//! ```text
//! prterun --np 2 ./target/debug/examples/callback_hop
//! ```

use std::os::raw::c_void;
use std::sync::mpsc;

fn main() {
    println!("pmix-rs: callback hop-off-progress example (issue #51)");
    println!("Run under a DVM to see real callbacks, e.g.");
    println!("  prterun --np 2 ./target/debug/examples/callback_hop");

    // Without a DVM there is no PMIx server to connect to — exit gracefully
    // like the other role examples in this crate.
    let client = match pmix::PmixClient::connect_new(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("connect_new failed (need prterun/DVM?): {e:?}");
            return;
        }
    };
    let proc = client.require_proc();
    println!("connected: rank {}", proc.get_rank());

    // ── 1. Events path ─────────────────────────────────────────────────────
    // Empty `codes` = handle all events. The handler runs on the PMIx
    // progress thread, so it hops off before doing any work.
    let info = pmix::info::empty();
    let handler_ref = match pmix::events::register_event_handler(&[], &info, Some(on_event), None) {
        Ok(r) => r,
        Err(e) => {
            println!("register_event_handler failed (no DVM?): {e:?}");
            0
        }
    };

    // ── 2. data_ops non-blocking path ──────────────────────────────────────
    // Application thread owns the channel; the callback gets a cloned sender.
    let hop = pmix::threading::CallbackChannel::<(i32, Option<Vec<u8>>)>::new();
    let tx = hop.sender();
    match pmix::data_ops::get_nb(&proc, "pmix.job.size", None, Box::new(HopCallback { tx })) {
        Ok(()) => println!("get_nb accepted — completion will arrive on the progress thread"),
        Err(e) => println!("get_nb submit failed (no DVM?): {e:?}"),
    }

    // ── 3. Deliver an event so the handler fires (needs a DVM) ─────────────
    let event = pmix::PmixStatus::Known(pmix::PmixError::EventJobEnd);
    match pmix::events::notify_event(event, &proc, pmix::PmixDataRange::Session, &info) {
        Ok(()) => println!("notify_event accepted — handler will fire on the progress thread"),
        Err(e) => println!("notify_event failed (no DVM?): {e:?}"),
    }

    // ── 4. Application thread: drain the hop channel ───────────────────────
    // This thread is where blocking PMIx calls are legal (the callback that
    // pushed into the channel must never block on progress).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match hop.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok((status, payload)) => {
                let text = payload
                    .as_deref()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_else(|| "<no value>".to_string());
                println!("[app thread] get_nb completed: status={status} value={text:?}");
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) if std::time::Instant::now() < deadline => {
                // Standalone runs never produce the completion; keep waiting
                // briefly so a real DVM run has time to deliver it.
            }
            Err(_) => {
                println!("[app thread] no get_nb completion within 5s (standalone run?)");
                break;
            }
        }
    }

    // ── 5. Cleanup ─────────────────────────────────────────────────────────
    if handler_ref != 0 {
        match pmix::events::deregister_event_handler(handler_ref, None) {
            Ok(()) => println!("deregistered event handler {handler_ref}"),
            Err(e) => println!("deregister_event_handler: {e:?}"),
        }
    }
    let _ = client.disconnect(None);
    println!("callback_hop example done");
}

/// Event handler — runs on the PMIx **progress thread**.
///
/// Note the [`ProgressContext`](pmix::threading::ProgressContext) marker: it
/// documents that blocking PMIx calls are forbidden here, and the work is
/// hopped onto a fresh application thread via
/// [`spawn_from_callback`](pmix::threading::spawn_from_callback).
unsafe extern "C" fn on_event(
    id: pmix::events::EventHandlerRef,
    status: i32,
    _source: *const c_void,
    _info: *mut c_void,
    _ninfo: usize,
    _results: *mut c_void,
    _nresults: usize,
    _cbfunc: pmix::events::pmix_event_notification_cbfunc_fn_t,
    _cbdata: *mut c_void,
) {
    // We are on the progress thread — this marker is documentation, and any
    // blocking PMIx call here would deadlock progress.
    let _ctx = pmix::threading::ProgressContext;

    // Hop off: spawn an application thread for the actual handling. We never
    // join it here (that could block progress and deadlock the process).
    let _ = pmix::threading::spawn_from_callback(move || {
        println!("[app thread] event {id} fired with status {status} — blocking work is safe here");
    });
}

/// `get_nb` completion callback — also invoked on the PMIx progress thread.
///
/// The [`PmixOwnedValue`](pmix::PmixOwnedValue) handed to us is a C-owned
/// handle (`!Send`), so it is converted to Rust-owned bytes with
/// [`bytes_copy`](pmix::PmixOwnedValue::bytes_copy) **before** crossing the
/// channel. Only the cheap, non-blocking `send` happens on the progress
/// thread; the application thread does any blocking work.
struct HopCallback {
    tx: mpsc::Sender<(i32, Option<Vec<u8>>)>,
}

impl pmix::data_ops::GetValueCallback for HopCallback {
    fn on_result(self: Box<Self>, status: pmix::PmixStatus, value: Option<pmix::PmixOwnedValue>) {
        let _ctx = pmix::threading::ProgressContext;

        // Convert the C-owned (!Send) value into Rust-owned bytes so it can
        // cross the channel, then hop — never block here.
        let payload = value.map(|v| v.bytes_copy());
        let _ = self.tx.send((status.to_raw(), payload));
    }
}
