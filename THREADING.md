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

### 4.1 Server module upcalls (not pin targets)

`PmixServerModule` host callbacks (`fence_nb`, `direct_modex`, `publish`, …)
run in **progress context**. They are **not** CPU-pin targets — pin the
progress engine (table above), not which core runs a given upcall body.

Rules (same deadlock class as client `_nb` / events — issue #51 helpers):

1. Return quickly; never call blocking PMIx from the upcall.
2. Hop with `threading::spawn_from_callback` or `CallbackChannel`. The former
   uses a process-wide bounded worker pool (available parallelism workers and
   four queue slots per worker); a full queue falls back to a detached,
   dedicated `pmix-callback-hop` thread so callback work is never dropped or
   blocked. Pool workers are detached and require no finalize-time shutdown.
3. Invoke the provided `cbfunc` **later** when RM / network work finishes.
4. Copy C buffers before hopping; do not join hop work in-handler.

Docs live next to the type: `PmixServerModule` in `src/server/mod.rs`.  
Worked example: `examples/server_upcall_hop.rs` (issue #52).

---

## 5. Type inventory

### 5.1 Sessions, identities, POD, and Rust-owned values — `Send + Sync`

The following concrete public values and handles contain process-safe session
state, copied process identity, scalar/POD data, or data copied into Rust-owned
allocations. They are enforced in `src/threading_assert.rs`:

- Sessions and lifecycle state: `PmixClient`, `PmixClientState`, `PmixServer`,
  `PmixServerState`, `server::PmixServerHandle` (an alias for `PmixServer`),
  `PmixTool`, and `PmixToolState`.
- Identity tokens: `Proc`, `PmixToolHandle`, and `tool::PmixServerHandle`.
  The two `PmixServerHandle` names are intentionally different: the server
  module's name is a session alias, while the tool module's name is an attached
  server identity (`nspace + rank`).
- Scalar/POD values: status/error enums, PMIx enum mirrors, `IOFChannelFlags`,
  `InfoFlags`, `PmixTimeval`, `PmixEnvar`, `PmixBindEnvelope`, and
  `PmixLocality`.
- Rust-owned wrappers/builders: `InitOptions`, `PmixCredential`,
  `utility::PmixByteObject`, `data_serialization::PmixPrintOutput`,
  `fabric::PmixDeviceDistance`, `process_mgmt::PmixApp`,
  `process_mgmt::PmixAppBuilder`, `PmixProcRef`, and the zero-sized
  `threading::ProgressContext` marker.
- Function-pointer aliases are `Send + Sync`; `PmixServerModule` is deliberately
  not included in this matrix because its `as_c_ptr()` method currently casts
  the Rust struct directly to the generated C struct. That ABI representation
  must be made explicit before its threading contract is frozen.

### 5.2 C-owned or raw-pointer-bearing values — `!Send + !Sync`

These values either own PMIx allocations, release C memory in `Drop`, retain an
opaque C pointer, or contain a raw `pmix_value_t` union whose active arm controls
ownership. The complete concrete value/handle matrix is centralized in
`src/threading_assert.rs`.

| Type | Module | Why |
|------|--------|-----|
| `Info`, `InfoBuilder`, `PmixOwnedValue` | `lib` | PMIx allocation / raw value union |
| `PmixPayload`, `PmixValueBuilder` | `lib` | Deliberate raw-pointer payload variant and raw C values |
| `PmixPdata` | `data_ops` | Contains `PmixOwnedValue` |
| `PmixDataBuffer`, `data_serialization::PmixByteObject` | `data_serialization` | PMIx-managed buffer/byte-object lifetime |
| `PmixFabric`, `PmixTopology`, `PmixCpuset`, `DeviceDistances` | `fabric` | PMIx/C-owned state and cleanup pointers |
| `PmixQuery`, `QueryResults` | `query_log` | Query/result allocations released through PMIx |
| `AllocationResults`, `JobControlResults`, `SessionControlResults` | `allocation` | Returned `pmix_info_t` arrays |
| `MonitorResults` | `monitoring` | Returned `pmix_info_t` array |
| `CredentialResults`, `ValidationResults` | `security` | Results contain PMIx info allocations |
| `CollectInventoryResults` | `server` | Returned `pmix_info_t` array |

The `utility::PmixByteObject` type is intentionally different: it owns a
`Vec<u8>` and is `Send + Sync`; do not confuse it with the PMIx-backed
`data_serialization::PmixByteObject`.

**Share model:** build ephemeral handles **per call** on the calling thread.
`Arc<Mutex<T>>` does not make a `!Send` PMIx wrapper transferable: Rust still
requires `T: Send` for the mutex to cross threads. If work must leave the
creating thread, send a Rust-owned snapshot (for example, copied bytes or
materialized fields) through a channel, or keep the C-owned wrapper and all
operations on its owner thread. A library `into_shared` helper is intentionally
not provided (YAGNI).

### 5.3 Callback carriers

The public callback wrapper structs are movable (`Send`) because their trait
objects require `Send`, but they are not concurrently shareable (`!Sync`) unless
the API explicitly adds a `Sync` bound. Function-pointer aliases are
`Send + Sync`. `threading::CallbackChannel<T>` is movable when `T: Send`, but
its `Receiver` makes the channel itself `!Sync`.

Callback trait definitions are contracts rather than concrete values in this
matrix: each public callback trait declares its required `Send` bound at its
definition site. The concrete wrapper carriers are asserted above. A callback
trait object can be moved only when its object type is `Send`, and it is not
concurrently shareable unless `Sync` is also part of the bound.

Callback results must be converted to Rust-owned data before hopping off the
PMIx progress thread; do not send a C-owned result wrapper through a callback
channel.

---

## 6. Caller rules

1. Use session types (`PmixClient` / `PmixServer` / `PmixTool`) — clone for workers; `disconnect` once.  
2. Build `Info` and other C-owned handles on the calling thread.  
3. Callbacks **and server module upcalls** = progress context — hop before
   blocking PMIx. Use `threading::spawn_from_callback` (fire-and-forget
   worker pool, with a dedicated-thread fallback) or
   `threading::CallbackChannel` (app-thread receiver) — see
   `examples/callback_hop.rs` (client `_nb` / events) and
   `examples/server_upcall_hop.rs` (fence_nb / direct_modex + delayed
   `cbfunc`). Never join/wait in-handler; convert C-owned (`!Send`) values
   to Rust-owned data (e.g. `bytes_copy()`) before hopping. Server upcalls
   are **not** CPU-pin targets (§4.1).  

4. One connect/disconnect cycle per process (per role).  
5. Progress must run.  
6. cbdata: `crate::cbdata::encode_req_id` / `decode_req_id`.  
7. Bridges never hold a registry `Mutex` across user callback execution
   (regression-tested in `events` / `groups`; full inventory in §9).
   Prefer `threading::invoke_user_callback` so user panics cannot unwind
   into OpenPMIx.

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
callback hop-off via `threading` helpers), `server_upcall_hop.rs`
(server module fence/modex upcall hop + delayed `cbfunc`).

### 7.1 Multi-thread + external-progress integration tests (#54)

| Goal | Test (`tests/threading_mt_via_prterun.rs`) |
|------|---------------------------------------------|
| (1) N threads: clone `PmixClient`, concurrent put + fence | `mt_concurrent_put_and_fence` |
| (2) Concurrent `_nb` completions | `mt_concurrent_fence_nb_completions` |
| (3) `external_progress` + host `progress()` | `mt_external_progress_host_thread` (own process) |
| (4) Callback must-not-block timeout | `callback_must_not_block_progress_timeout` |

Run under PRTE ≥ 4.1 / OpenPMIx ≥ 6.1:

```bash
export PMIX_PREFIX=${PMIX_PREFIX:-$HOME/.local/openpmix-6.1.0}
export LD_LIBRARY_PATH=$PMIX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export PATH=/path/to/prte-4.1/bin:$PATH

cargo test --test threading_mt_via_prterun -- --test-threads=1
./scripts/run_daemon_tests.sh THREADING
```

All DVM cases use the **process-wide** `PmixClient` (no bare `Context`, no
per-thread init). Goal (3) applies `InitOptions::external_progress(true)` on
the first connect; the harness runs each ignored test under its own `prterun`
process. Companion example: `examples/external_progress_mt.rs`.

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
| Callback hop / audit | #51, #67 | Done (`threading` helpers + #67 registry audit checklist §9) |
| Server upcall example | #52 | Done (`PmixServerModule` docs + `server_upcall_hop` example) |
| Global FFI mutex | #53 | Deferred |
| MT integration tests | #54 | Done (`tests/threading_mt_via_prterun.rs` + `run_daemon_tests.sh THREADING`) |
| Extra public Send/Sync matrix | #66 | Done (centralized matrix + inventory) |

## 9. Callback registry audit checklist (#67)

Policy (`src/threading.rs`): bridges **never** hold a registry `Mutex` across
user callback execution; one-shot completions encode cbdata with
`cbdata::encode_req_id` / `decode_req_id` (not a raw `Box` pointer); user
panics are **contained** at the `extern "C"` boundary via
`threading::invoke_user_callback` (events also complete the OpenPMIx chain).

| Module | Bridges / registries | Lock scope | cbdata | Panic contain |
|--------|----------------------|------------|--------|---------------|
| `data_ops` | publish / get / lookup / unpublish / fence | remove under lock, invoke after | `encode_req_id` | `invoke_user_callback` |
| `events` | `HANDLER_REGISTRY`, `notification_bridge`, reg bridge | copy/`remove` under lock | handler ref id / `Box` reg state only for nb reg | `catch_unwind` + chain `cbfunc` |
| `query_log` | query / log | remove → invoke | `encode_req_id` | `invoke_user_callback` |
| `security` | credential / validation | remove → invoke | `encode_req_id` | `invoke_user_callback` |
| `allocation` | allocation / job_ctrl / session_ctrl | remove → invoke | `encode_req_id` | `invoke_user_callback` |
| `monitoring` | monitor | remove → `drop` → invoke | `encode_req_id_u64` | `invoke_user_callback` |
| `groups` | construct / invite / join / leave / destruct | remove → invoke | `encode_req_id` (migrated off `Box` cbdata) | `invoke_user_callback` |
| `process_mgmt` | spawn / connect / disconnect `_nb` | remove → invoke | `encode_req_id` (migrated off `Box` cbdata) | `invoke_user_callback` |
| `server` | nspace/client/dmodex/setup/iof/inventory | remove → invoke | `encode_req_id` | `invoke_user_callback` |
| `server/data` | fence/connect/disconnect `_nb` | remove → invoke | `encode_req_id` (migrated off `Box` cbdata) | `invoke_user_callback` |
| `server/pset` | register/deregister resources | remove → invoke | `encode_req_id` | `invoke_user_callback` |
| `utility` IOF | `IOF_REGISTRY` + pull/dereg/push | lock released before user IO/reg/dereg/push cbs | pull: C handle + context ptr (long-lived); dereg/push one-shot `Box` ctx | `invoke_user_callback` |

**Exceptions (documented, not hold-across-user-code):**

- **Events nb registration** parks `HandlerRegState` via `Box` in C `cbdata`
  until the registration completion delivers a ref id (then registry insert).
- **IOF pull** is long-lived: the IO callback is keyed by the C handle in
  `IOF_REGISTRY`; the registration `cbdata` is the context pointer. One-shot
  dereg/push completions still use a boxed context pointer today (no registry
  mutex held across the user call).

**Regression tests:**

- `events::test_event_handler_lock_not_held_during_user_callback`
- `events::test_notification_bridge_user_panic_completes_chain_and_is_contained`
- `groups::test_group_leave_bridge_no_lock_across_user_code`
- `groups::test_group_leave_bridge_contains_user_panic`
- `threading::invoke_user_callback_contains_panic`

No known hold-across-user-code sites remain after this audit.
