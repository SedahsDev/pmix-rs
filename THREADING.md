# Threading Model & Send/Sync Inventory

**Date:** 2026-07-27  
**Issues:** [#64](https://github.com/SedahsDev/pmix-rs/issues/64), [#45](https://github.com/SedahsDev/pmix-rs/issues/45), [#50](https://github.com/SedahsDev/pmix-rs/issues/50)  
**Related:** [#51](https://github.com/SedahsDev/pmix-rs/issues/51)–[#52](https://github.com/SedahsDev/pmix-rs/issues/52), [#54](https://github.com/SedahsDev/pmix-rs/issues/54), [#66](https://github.com/SedahsDev/pmix-rs/issues/66)–[#67](https://github.com/SedahsDev/pmix-rs/issues/67)

Crate-root docs in `src/lib.rs` (`//! # Concurrency model`) match this document.  
Compile-time matrix: `src/threading_assert.rs`.  
Callback hop-off helpers + bridge policy: `src/threading.rs`.

---

## 1. Strategy (one-liner)

| Layer | Policy |
|--------|--------|
| **Session** | Process-wide `OnceLock<Arc<Inner>>`. Handle is **`Clone + Send + Sync`**. **No** `PhantomData<*mut u8>` on the session Inner. |
| **Client API** | **`PmixClient` only** — no legacy `Context` / `init()`. Explicit `connect` / `disconnect`. **Drop never finalizes.** |
| **C API entry** | Trust OpenPMIx **≥ 6.1** threadshift. No global op mutex by default. |
| **C-owned handles** | `Info`, buffers, fabric, results → **`!Send + !Sync`** (`PhantomData<*mut u8>`). Prefer build-per-call. |
| **Data ops** | Free functions (`put_value`, `get_value`, `commit`, `fence`, `data_ops::*`, …). |
| **Callbacks / upcalls** | PMIx **progress thread**. No blocking PMIx in-handler (#51, #52, #67). |

**Anti-pattern:** `PhantomData<*mut u8>` on `PmixClientInner` while advertising multi-thread clones.

---

## 2. OpenPMIx ≥ 6.1

Threadshift on C entry. Progress must run (internal thread or `external_progress` + `pmix::progress()`). Pre-6.1 needs external sync (unsupported default).

---

## 3. Sessions

| Type | Auto-traits | Drop finalize? |
|------|-------------|----------------|
| `PmixClient` | `Clone + Send + Sync` | No |
| `PmixServer` | `Clone + Send + Sync` | No |
| `PmixTool` | `Clone + Send + Sync` | No |
| `Proc` | `Clone + Send + Sync` (POD) | n/a |

```rust
let client = pmix::PmixClient::connect_new(None)?;
let w = client.clone();
std::thread::spawn(move || { let _ = w.rank(); });
client.disconnect(None)?;
```

`PmixToolHandle` / `tool::PmixServerHandle` are **identity tokens** (nspace+rank), not process sessions.

---

## 4. Progress & pinning

| Goal | How |
|------|-----|
| Pin progress CPUs | `InitOptions::bind_progress_thread` |
| Host progress | `InitOptions::external_progress(true)` + `progress()` |
| Stop progress thread | `progress_thread_stop()` |

Deadlocks: external progress without a loop; mutex held across `progress()` + callback; blocking PMIx inside a callback; `progress()` after `progress_thread_stop()`.

---

## 5. Type inventory

### 5.1 Sessions / POD — `Send + Sync` (enforced in `threading_assert.rs`)

`PmixClient`, `PmixClientState`, `PmixServer`, `PmixServerState`, `PmixTool`, `PmixToolState`, `Proc`

### 5.2 C-owned — `!Send + !Sync` (enforced)

| Type | Module |
|------|--------|
| `Info`, `InfoBuilder`, `PmixOwnedValue` | `lib` |
| `PmixDataBuffer`, `PmixByteObject` | `data_serialization` |
| `PmixFabric`, `PmixTopology`, `PmixCpuset`, `DeviceDistances` | `fabric` |
| `QueryResults`, `PmixQuery` | `query_log` |
| `AllocationResults`, `JobControlResults`, `SessionControlResults` | `allocation` |
| `MonitorResults` | `monitoring` |
| `ValidationResults` | `security` |
| `CollectInventoryResults` | `server` |

**Share model:** build ephemeral handles **per call** on the calling thread. Optional app-side `Arc<Mutex<T>>` if you must share (no library `into_shared` helper — YAGNI).

### 5.3 Pure Rust / remaining

`PmixCredential` holds a Rust `Vec<u8>` (opaque bytes) — stays `Send + Sync`. Enums and pure builders are `Send + Sync`.

---

## 6. Caller rules

1. Use session types (`PmixClient` / `PmixServer` / `PmixTool`) — clone for workers; `disconnect` once.  
2. Build `Info` and other C-owned handles on the calling thread.  
3. Callbacks = progress thread — hop before blocking PMIx. Use
   `threading::spawn_from_callback` (fire-and-forget thread) or
   `threading::CallbackChannel` (app-thread receiver) — see
   `examples/callback_hop.rs`. Never join/wait in-handler; convert C-owned
   (`!Send`) values to Rust-owned data (e.g. `bytes_copy()`) before hopping.  
4. One connect/disconnect cycle per process (per role).  
5. Progress must run.  
6. cbdata: `crate::cbdata::encode_req_id` / `decode_req_id`.  
7. Bridges never hold a registry `Mutex` across user callback execution
   (regression-tested in `events`).

### 6.1 Forbidden on the progress thread (concrete)

```rust,ignore
// ❌ NEVER — blocks progress on a PMIx round-trip
let _ = pmix::data_ops::get(&proc, "pmix.job.size", None);

// ❌ NEVER — waits for a condition only progress can satisfy
while !ready.load(Ordering::SeqCst) { /* spin / park */ }

// ❌ NEVER — holds a registry lock across application callback code
let mut guard = EVENT_HANDLERS.lock().unwrap();
if let Some(cb) = guard.get_mut(&id) { cb(); } // user code under lock
```

Hop first (`spawn_from_callback` / `CallbackChannel`), then do the blocking
work on an application thread. Full policy: `src/threading.rs`.

---

## 7. Examples

`examples/client_minimal.rs`, `simple_put_get.rs`, `simple_fence.rs`,
`server_minimal.rs`, `tool_attach.rs`, `callback_hop.rs` (get_nb + events
callback hop-off via `threading` helpers).

---

## 8. Roadmap

| Item | Issue | Status |
|------|------:|--------|
| Inventory + ≥6.1 | #45 | Done |
| InitOptions / progress stop | #46, #47 | Done |
| PmixClient session | #48 | Done |
| Remove Context/init | #69 | Done |
| Server/tool sessions | #49 | Done |
| C-owned !Send + assert matrix | #50 | Done (`threading_assert.rs`) |
| Callback hop / audit | #51, #67 | #51 Done (`threading` module + events registry); #67 Open |
| Server upcall example | #52 | Open |
| Global FFI mutex | #53 | Deferred |
| MT integration tests | #54 | Open |
| Extra static_assertions | #66 | Mostly superseded by #50 |
