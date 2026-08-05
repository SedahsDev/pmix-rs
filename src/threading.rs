//! # Progress-thread callbacks: hop-off helpers and bridge policy
//!
//! OpenPMIx delivers `_nb` completions and events on the **progress thread**
//! (the thread that runs the PMIx event engine), never on the thread that
//! issued the call. That means every callback this crate hands to PMIx —
//! `data_ops` non-blocking completions, event notifications, and server
//! upcalls — runs on a thread the application does not control.
//!
//! This module provides the shared helpers for **hopping off** the progress
//! thread before doing anything that could block, plus the **bridge policy**
//! that every callback bridge in this crate follows.
//!
//! # Bridge policy
//!
//! 1. **Never call blocking PMIx APIs from the handler.** A blocking call
//!    (`get`, `fence`, `publish`, `lookup`, `unpublish`, `spawn`,
//!    `register_event_handler`, `notify_event`, `disconnect`, …) from inside
//!    a callback waits for the progress thread — which is the thread the
//!    callback is running on. That is a self-deadlock. Hop to an application
//!    thread first (see the template below).
//! 2. **Non-blocking calls from the handler are allowed only if you do not
//!    wait for their completion in-handler.** Issuing an `_nb` call and
//!    returning is fine; blocking on its callback is not.
//! 3. **Bridges stay minimal and never hold a registry `Mutex` across user
//!    callback execution.** Registry lookups are scoped to the shortest
//!    possible critical section; the user's code runs after the lock is
//!    dropped. (Regression-tested in `events`.)
//! 4. **cbdata** is an opaque request ID encoded with
//!    [`crate::cbdata::encode_req_id`] / [`crate::cbdata::decode_req_id`],
//!    never a raw pointer to a callback.
//!
//! # Hop-off template
//!
//! The two primitives cover the two ways a handler can get work off the
//! progress thread:
//!
//! * [`spawn_from_callback`] — fire-and-forget: start a fresh application
//!   thread for blocking work and return immediately.
//! * [`CallbackChannel`] — request/response: the application thread owns the
//!   channel (receiver side) and processes messages the handler sends.
//!
//! ```no_run
//! use pmix::threading::{CallbackChannel, ProgressContext, spawn_from_callback};
//!
//! // App thread: create the hop channel and hand a clone-able sender to the
//! // callback. The receiver stays on the app thread.
//! let hop = CallbackChannel::<(i32, Vec<u8>)>::new();
//! let tx = hop.sender();
//!
//! // Inside a PMIx callback (progress thread):
//! let _ctx = ProgressContext;                 // documents: we are on progress
//! let _ = tx.send((0, b"payload".to_vec()));  // cheap, non-blocking
//! let _ = spawn_from_callback(move || {       // or: fresh thread for blocking work
//!     // blocking PMIx calls are legal here
//! });
//!
//! // Back on the app thread:
//! let (status, data) = hop.recv().expect("callback never fired");
//! # let _ = (status, data);
//! ```
//!
//! **C-owned handles are `!Send`.** A value like [`PmixOwnedValue`]
//! (crate::PmixOwnedValue) cannot cross the channel; convert it to
//! Rust-owned data first (e.g. [`bytes_copy`](crate::PmixOwnedValue::bytes_copy))
//! and send the copy.
//!
//! See also `examples/callback_hop.rs` for a complete `get_nb` + events
//! demonstration, and [THREADING.md](../THREADING.md).

use std::sync::mpsc::{self, Receiver, RecvError, RecvTimeoutError, Sender, TryRecvError};

/// Zero-sized marker for code that runs on the PMIx **progress thread**.
///
/// `ProgressContext` is a documentation marker: it carries no data and has no
/// runtime effect. A callback implementation can take one (or name the type in
/// a comment) so that the forbidden-API list lives next to the code that must
/// respect it.
///
/// # Forbidden while on the progress thread
///
/// Anything that blocks on PMIx progress — in this crate:
///
/// * Blocking data ops: [`get`](crate::data_ops::get),
///   [`lookup`](crate::data_ops::lookup), [`publish`](crate::data_ops::publish),
///   [`unpublish`](crate::data_ops::unpublish),
///   [`fence`](crate::fence).
/// * Blocking events: [`register_event_handler`](crate::events::register_event_handler),
///   [`notify_event`](crate::events::notify_event).
/// * Session lifecycle: [`disconnect`](crate::PmixClient::disconnect),
///   [`finalize`](crate::finalize).
/// * [`progress`](crate::progress) (re-entering the event engine).
/// * Waiting on another thread (joining a handle, blocking on a lock that an
///   application thread holds while it calls PMIx).
/// * Holding a crate registry `Mutex` across user code (bridges must drop the
///   lock before invoking application callbacks).
///
/// ```rust,ignore
/// // ❌ NEVER — blocks the progress thread on a PMIx round-trip
/// let _ = pmix::data_ops::get(&proc, "pmix.job.size", None);
///
/// // ❌ NEVER — waits for a condition that only progress can satisfy
/// while !ready.load(Ordering::SeqCst) { /* spin / park */ }
///
/// // ❌ NEVER — holds a registry lock across application callback code
/// let mut guard = EVENT_HANDLERS.lock().unwrap();
/// if let Some(cb) = guard.get_mut(&id) { cb(); } // user code under lock
/// ```
///
/// Use [`spawn_from_callback`] or a [`CallbackChannel`] to hop off first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ProgressContext;

/// Spawn an application thread from inside a PMIx callback.
///
/// The callback runs on the PMIx progress thread; any work that could block
/// (including blocking PMIx calls) must be moved to an application thread.
/// This helper spawns a fresh, named thread to run `f` and returns a
/// [`JoinHandle`](std::thread::JoinHandle) the caller may store or detach.
///
/// # Rules
///
/// * **Do not join or otherwise wait on the returned handle from inside the
///   callback.** The spawned thread may need progress to complete (e.g. a
///   blocking PMIx call); waiting for it would deadlock the progress thread.
///   Return from the handler and join from an application thread instead.
/// * The callback must not rely on any PMIx C-owned handle outliving the
///   callback frame; move Rust-owned data (or clones) into the closure.
/// * If spawning fails (thread limits / resource exhaustion), the error is
///   returned — log it and continue; never panic from a PMIx callback.
/// * A panic inside `f` is confined to the hop thread under the default
///   `panic = "unwind"` profile: it does **not** abort the process by itself
///   (the default panic hook does not call `exit()` for non-main threads).
///   Under `panic = "abort"` the process aborts regardless of this helper.
///   This helper logs a branded diagnostic on stderr for fire-and-forget
///   callers, then resumes unwinding so a joined handle still reports
///   [`JoinHandle::join`](std::thread::JoinHandle::join) `Err`. Prefer not
///   panicking in hop work in long-running HPC jobs — treat panics as bugs.
///
/// # Example
///
/// ```no_run
/// use pmix::threading::spawn_from_callback;
///
/// let _ = spawn_from_callback(move || {
///     // Blocking work — safe on this application thread.
///     std::thread::sleep(std::time::Duration::from_millis(10));
/// });
/// ```
pub fn spawn_from_callback<F>(f: F) -> std::io::Result<std::thread::JoinHandle<()>>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .name("pmix-callback-hop".to_string())
        .spawn(move || {
            // Log a clear diagnostic for operators (fire-and-forget callers
            // never join), then resume so `JoinHandle::join` still surfaces
            // the payload instead of a silent `Ok(())`. This is not true
            // isolation under `panic = "abort"`, and resume_unwind still
            // panics the hop thread (by design, for joiners).
            if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
                eprintln!(
                    "pmix: thread 'pmix-callback-hop' panicked while running \
                     work hopped off the PMIx progress thread"
                );
                std::panic::resume_unwind(payload);
            }
        })
}

/// Channel pair for hopping callback payloads off the progress thread.
///
/// Wraps a [`std::sync::mpsc`] channel with the intended usage spelled out:
/// the **application thread** creates the channel and keeps the receiver;
/// the **callback** (progress thread) gets a cloned [`Sender`] via
/// [`sender`](CallbackChannel::sender) and pushes Rust-owned payloads into it
/// before returning. The application thread then does the blocking work.
///
/// The sender is `Send + Sync + Clone`, so it can be moved into any of this
/// crate's `Send` callback traits ([`GetValueCallback`](crate::data_ops::GetValueCallback),
/// [`FenceCallback`](crate::data_ops::FenceCallback), …) or into a
/// [`NotificationFn`](crate::events::NotificationFn) closure.
///
/// Payloads must be `Send` (and typically Rust-owned — see the module docs
/// on C-owned handles).
///
/// # Example
///
/// ```no_run
/// use pmix::threading::CallbackChannel;
///
/// let hop = CallbackChannel::<u64>::new();
/// let tx = hop.sender();
/// // hand `tx` to a callback running on the progress thread…
/// std::thread::spawn(move || { let _ = tx.send(7); });
/// // …and process on the application thread:
/// assert_eq!(hop.recv().unwrap(), 7);
/// ```
pub struct CallbackChannel<T> {
    tx: Sender<T>,
    rx: Receiver<T>,
}

impl<T> CallbackChannel<T> {
    /// Create a new hop channel. Keep the returned value on the application
    /// thread and distribute [`sender`](CallbackChannel::sender) clones to
    /// callbacks.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }

    /// A clone-able sender to move into a callback (runs on the progress
    /// thread). `send` on an unbounded channel never blocks the handler.
    pub fn sender(&self) -> Sender<T> {
        self.tx.clone()
    }

    /// Block the application thread until a payload arrives.
    pub fn recv(&self) -> Result<T, RecvError> {
        self.rx.recv()
    }

    /// Block up to `timeout` for a payload (prefer this in examples and tests
    /// so a never-firing callback cannot hang the process forever).
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }

    /// Non-blocking poll for a payload.
    ///
    /// Returns [`TryRecvError::Empty`] when no message is ready yet, and
    /// [`TryRecvError::Disconnected`] when every sender has been dropped
    /// (no further messages will arrive). Callers should treat both as
    /// non-fatal — never unwrap this on the application thread without
    /// deciding how end-of-stream is handled.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.rx.try_recv()
    }

    /// Iterate over incoming payloads (blocks the application thread).
    pub fn iter(&self) -> mpsc::Iter<'_, T> {
        self.rx.iter()
    }

    /// Consume the channel and take ownership of the receiver (for moving it
    /// into a dedicated worker thread).
    pub fn into_receiver(self) -> Receiver<T> {
        self.rx
    }
}

impl<T> Default for CallbackChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn progress_context_is_zero_sized() {
        assert_eq!(std::mem::size_of::<ProgressContext>(), 0);
        let _ctx = ProgressContext; // constructible without ceremony
    }

    #[test]
    fn spawn_from_callback_runs_on_a_different_thread() {
        let caller = std::thread::current().id();
        let (tx, rx) = mpsc::channel();
        let handle = spawn_from_callback(move || {
            let _ = tx.send(std::thread::current().id());
        })
        .expect("spawn should succeed");
        let spawned = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("callback ran");
        assert_ne!(
            caller, spawned,
            "closure must run on a fresh application thread"
        );
        handle.join().expect("join");
    }

    #[test]
    fn spawn_from_callback_failure_returns_error() {
        // Spawning with an exhausted OS limit is not reliably triggerable in
        // tests; verify only that the signature reports io::Result.
        let _: std::io::Result<std::thread::JoinHandle<()>> = spawn_from_callback(|| {});
    }

    #[test]
    fn spawn_from_callback_panic_is_surfaced_on_join() {
        // Hop-thread panics must not be swallowed into a silent Ok(()) —
        // callers that join still observe the failure.
        let handle = spawn_from_callback(|| panic!("hop-work boom")).expect("spawn");
        let join_err = handle
            .join()
            .expect_err("panic in hop work must surface on JoinHandle::join");
        let msg = join_err
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| join_err.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        assert!(
            msg.contains("hop-work boom"),
            "unexpected panic payload: {msg}"
        );
    }

    #[test]
    fn callback_channel_send_recv_across_threads() {
        let hop = CallbackChannel::<String>::new();
        let tx = hop.sender();
        std::thread::spawn(move || {
            let _ = tx.send("hopped off the progress thread".to_string());
        });
        let got = hop
            .recv_timeout(Duration::from_secs(5))
            .expect("payload should arrive");
        assert_eq!(got, "hopped off the progress thread");
    }

    #[test]
    fn callback_channel_try_recv_empty() {
        let hop = CallbackChannel::<u8>::new();
        assert!(matches!(hop.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn callback_channel_try_recv_disconnected_when_senders_dropped() {
        // Keep the Result API (Empty vs Disconnected) so callers can tell a
        // quiet progress thread from end-of-stream after every sender dies.
        let hop = CallbackChannel::<u8>::new();
        let tx = hop.sender();
        let rx = hop.into_receiver(); // drops the channel's owned sender
        drop(tx); // last sender gone → Disconnected, not Empty
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    }

    #[test]
    fn callback_channel_sender_is_cloneable_and_send_sync() {
        let hop = CallbackChannel::<u8>::new();
        let tx1 = hop.sender();
        let tx2 = hop.sender();
        std::thread::spawn(move || {
            let _ = tx1.send(1);
            let _ = tx2.send(2);
        });
        let mut got = Vec::new();
        while got.len() < 2 {
            match hop.recv_timeout(Duration::from_secs(5)) {
                Ok(v) => got.push(v),
                Err(_) => break,
            }
        }
        got.sort_unstable();
        assert_eq!(got, vec![1, 2]);
    }

    #[test]
    fn callback_channel_into_receiver_moves_to_worker() {
        let hop = CallbackChannel::<u8>::new();
        let tx = hop.sender();
        let rx = hop.into_receiver();
        let worker = std::thread::spawn(move || rx.recv_timeout(Duration::from_secs(5)));
        let _ = tx.send(42);
        assert_eq!(worker.join().expect("worker").expect("value"), 42);
    }

    /// End-to-end hop template: a "progress thread" (a spawned thread standing
    /// in for the PMIx event engine) pushes a payload through the channel and
    /// spawns blocking work; the application thread receives both and joins.
    #[test]
    fn hop_template_end_to_end() {
        let hop = CallbackChannel::<u32>::new();
        let tx = hop.sender();
        let blocking_done = std::sync::Arc::new(AtomicUsize::new(0));
        let blocking_done2 = std::sync::Arc::clone(&blocking_done);

        // The callback side — must return quickly, never block.
        let callback_side = std::thread::spawn(move || {
            let _ctx = ProgressContext; // runs "on the progress thread"
            let _ = tx.send(99);
            let _ = spawn_from_callback(move || {
                blocking_done2.fetch_add(1, Ordering::SeqCst);
            });
        });

        // The application side — receives and then does blocking work.
        assert_eq!(
            hop.recv_timeout(Duration::from_secs(5)).expect("payload"),
            99
        );
        callback_side.join().expect("callback returns promptly");
        // The spawned blocking work completes on its own thread.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while blocking_done.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(blocking_done.load(Ordering::SeqCst), 1);
    }
}
