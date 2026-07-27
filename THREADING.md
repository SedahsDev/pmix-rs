# Threading Model & Send/Sync Inventory

**Date:** 2026-07-27  
**Issues:** [#64](https://github.com/SedahsDev/pmix-rs/issues/64) (this refresh), [#45](https://github.com/SedahsDev/pmix-rs/issues/45) (original inventory)  
**Related open work:** [#49](https://github.com/SedahsDev/pmix-rs/issues/49)–[#52](https://github.com/SedahsDev/pmix-rs/issues/52), [#54](https://github.com/SedahsDev/pmix-rs/issues/54), [#65](https://github.com/SedahsDev/pmix-rs/issues/65)–[#67](https://github.com/SedahsDev/pmix-rs/issues/67)

Crate-root docs in `src/lib.rs` (`//! # Concurrency model`) match this document. Prefer this file for the full inventory.

---

## 1. Strategy (one-liner)

| Layer | Policy |
|--------|--------|
| **Session** | Process-wide `OnceLock<Arc<Inner>>`. Handle is **`Clone + Send + Sync`**. **No** `PhantomData<*mut u8>` on the session Inner (that would make `Arc` `!Send` and defeat multi-thread clones). |
| **C API entry** | Trust OpenPMIx **≥ 6.1** threadshift. Do **not** global-lock every put/get by default. |
| **C-owned handles** | `Info`, buffers, fabric, … → **`!Send + !Sync`** (or stay thread-local). Prefer build-per-call. Optional app-side `Arc<Mutex<T>>` / future `into_shared` (#50). |
| **Data ops** | Free functions (`put_value`, `get_value`, `commit`, `fence`, `data_ops::*`, …) taking `&self`-style inputs; session supplies `Proc`. |
| **Callbacks / upcalls** | Run on **PMIx progress thread**. Keep short; hop to an app thread before blocking PMIx (#51, #52, #67). |
| **Global FFI mutex** | **Not** the default. Optional paranoia only (closed [#53](https://github.com/SedahsDev/pmix-rs/issues/53)). |

**Anti-pattern (do not repeat):** putting `PhantomData<*mut u8>` on `PmixClientInner` while documenting “clone across threads” — that was the first #60 draft bug.

---

## 2. OpenPMIx version assumption

**This crate assumes OpenPMIx ≥ 6.1.**

From the [OpenPMIx 6.1.0 NEWS](https://github.com/openpmix/openpmix/blob/v6.1.0/docs/news/news-v6.x.rst):

> *all APIs are now threadshifted prior to execution for thread safety. Hosts that are providing their own progress engine (in lieu of using the PMIx internal progress thread) must ensure that progress is being provided sufficient to avoid threadlock when calling PMIx APIs.*

| Layer | Who serializes? | Implication |
|-------|-----------------|-------------|
| C library entry | OpenPMIx `PMIX_THREADSHIFT` onto internal `evbase` / progress thread | Multiple Rust threads **may** call most `PMIx_*` APIs concurrently **if** progress is running |
| Progress engine | Internal progress thread(s), or host via `PMIx_Progress` / `pmix::progress()` | Without progress, `_nb` and many blocking paths can deadlock |
| Rust session | Process-wide `PmixClient` (`Send + Sync`) | Clone the client; do not multi-init |
| Rust C-owned values | Type system (`!Send`) + ownership | Do not share `Info`/buffers across threads without a mutex you own |
| Callbacks / server module | Delivered on **PMIx progress thread** | No blocking PMIx in-handler |

**If you link OpenPMIx < 6.1**, C entry is **not** fully serialized — external sync required (single-threaded or app mutex). That is outside the default support story.

---

## 3. Sessions: `PmixClient` (done) and friends

### 3.1 `PmixClient` — preferred client API

| Property | Behavior |
|----------|----------|
| Storage | `OnceLock<Arc<PmixClientInner>>` — **one** process-wide session |
| Auto-traits | **`Clone + Send + Sync`** (asserted in tests) |
| State machine | `Uninitialized → Live → Finalizing → Dead` |
| `connect` / `disconnect` | Serialized on session mutex; double-init → error; double-finalize → no-op |
| Drop | **Does not** call `PMIx_Finalize` (clones must not each finalize) |
| Identity | `proc()` / `rank()` / `proc_with_nspace()` |
| Data path | Free functions + `Proc` from the client |

```rust
// Multi-thread sketch (needs a live PMIx/DVM to run)
let client = pmix::PmixClient::connect_new(None)?;
let w = client.clone();
std::thread::spawn(move || {
    let _ = w.rank();
    // put_value / get_value / fence with w.proc() …
});
client.disconnect(None)?;
```

### 3.2 Legacy `Context` / `init` / `finalize`

- Still supported for compatibility.
- Share the **same** process session state machine as `PmixClient`.
- `Context::Drop` still calls `finalize` (explicit disconnect on client path preferred for MT).
- **Not** the type to clone across threads — use `PmixClient`.

### 3.3 Server / tool sessions

Still handle-based (`PmixServerHandle`, `PmixToolHandle` + flags). Target shape is the same process-wide `Send + Sync` session pattern — see [#49](https://github.com/SedahsDev/pmix-rs/issues/49).

---

## 4. Progress mode & pinning

| Goal | Supported? | How |
|------|------------|-----|
| Pin **progress thread** CPUs | Yes | `InitOptions::bind_progress_thread("0-3")` → `PMIX_BIND_PROGRESS_THREAD` |
| Require bind success | Yes | `InitOptions::bind_required(true)` |
| Host-driven progress | Yes | `InitOptions::external_progress(true)` + call `pmix::progress()` from your loop |
| Stop internal progress thread | Yes | `pmix::progress_thread_stop()` (see also flush/name on `InitOptions` / `InfoBuilder`) |
| Pin which CPU runs a given `PMIx_Get` body | **No** | Work is threadshifted onto the progress/`evbase` path |
| Pin server-module upcall thread | **No** | Hop to an app pool (#52) |

### Deadlock notes

1. **`external_progress(true)` without a host `progress()` loop** — `_nb` and many blocking paths hang.  
2. **Holding a Rust mutex across `progress()`** while a callback tries to take the same mutex — deadlock.  
3. **Blocking PMIx inside a progress-thread callback** — deadlock / threadlock.  
4. **Do not call `progress()` after `progress_thread_stop()`** — event base torn down.

API anchors: `InitOptions` / `InfoBuilder` helpers in `src/lib.rs`; `progress()` / `progress_thread_stop()` in `src/lib.rs`.

---

## 5. Public type inventory

Statuses:

| Status | Meaning |
|--------|---------|
| **Enforced** | Type system + (where noted) `static_assertions` |
| **Intended** | Design target; may still auto-implement `Send` until #50 |
| **POD / pure** | No PMIx heap ownership; safe `Send + Sync` |

Line numbers move; paths are canonical.

### 5.1 Sessions & process identity

| Type | Path | Send/Sync | Status | Notes |
|------|------|-----------|--------|-------|
| `PmixClient` | `src/lib.rs` | **`Send + Sync`** | **Enforced** (`assert_impl_all`) | Process-wide Arc session |
| `PmixClientState` | `src/lib.rs` | `Send + Sync` | **Enforced** | Copy enum |
| `Context` | `src/lib.rs` | not for MT share | Legacy | Drop-finalizes; prefer `PmixClient` |
| `Proc` | `src/lib.rs` (also `src/proc.rs` tree) | **`Send + Sync`** | **Intended / POD** | `Clone`; holds `pmix_proc_t` by value, not a PMIx heap handle |
| `PmixServerHandle` | `src/server/mod.rs` | TBD session | **Intended** → #49 | `active` flag; Drop may finalize |
| `PmixToolHandle` | `src/tool.rs` | TBD session | **Intended** → #49 | Wraps `Proc`; no auto tool_finalize on Drop |

### 5.2 C-owned / must not silently share (`!Send + !Sync` target)

Prefer **ephemeral construction per call**. Share only behind **your** `Mutex` (or future `into_shared`).

| Type | Path | Enforced today? | Notes |
|------|------|-----------------|-------|
| `Info` | `src/lib.rs` | **Yes** (`PhantomData<*mut u8>` + asserts) | Pattern for #50 |
| `InfoBuilder` | `src/lib.rs` | Intended thread-local | Don’t share mid-build |
| `PmixOwnedValue` | `src/lib.rs` / `src/value.rs` | Audit (#50) | Owns value payload; may be `Send` after extract |
| `PmixDataBuffer` | `src/data_serialization.rs` | **Todo** | `*mut pmix_data_buffer_t` |
| `PmixByteObject` | `src/data_serialization.rs` | **Todo** | May hold C buffer |
| `PmixFabric` | `src/fabric.rs` | **Todo** | Registration + module ptr |
| `PmixTopology` | `src/fabric.rs` | **Todo** | |
| `PmixCpuset` | `src/fabric.rs` | **Todo** | |
| `DeviceDistances` | `src/fabric.rs` | **Todo** | |
| `QueryResults` | `src/query_log.rs` | **Todo** | `*mut pmix_info_t` |
| `AllocationResults` | `src/allocation.rs` | **Todo** | |
| `JobControlResults` | `src/allocation.rs` | **Todo** | |
| `SessionControlResults` | `src/allocation.rs` | **Todo** | |
| `MonitorResults` | `src/monitoring.rs` | **Todo** | |
| `PmixCredential` | `src/security.rs` | **Todo** | |
| `CredentialResults` / `ValidationResults` | `src/security.rs` | **Todo** | |
| `CollectInventoryResults` | `src/server/mod.rs` | **Todo** | |
| Callback wrappers (spawn/connect/group/…) | `process_mgmt`, `groups`, … | Internal | Don’t share across threads |

Full `static_assertions` matrix: [#66](https://github.com/SedahsDev/pmix-rs/issues/66). Completing `!Send` marks: [#50](https://github.com/SedahsDev/pmix-rs/issues/50).

### 5.3 Pure Rust / Copy — `Send + Sync`

Enums and plain data (non-exhaustive list):  
`PmixError`, `PmixStatus`, `PmixProcState`, `PmixScope`, `PmixJobState`, `PmixLinkState`, `PmixDeviceType`, `PmixPersistence`, `PmixDataRange`, `PmixDataType`, `PmixAllocDirective`, `PmixJobCtrlAction`, `IOFChannelFlags`, `InfoFlags`, `BuilderError`, `ValueError`, `PmixTimeval`, `PmixEnvar`, `PmixPayload`, `PmixValueBuilder`, `PmixApp` / `PmixAppBuilder`, `PmixQuery` (pure fields), `PmixDeviceDistance`, `PmixBindEnvelope`, `PmixLocality`, `PmixPrintOutput`, `InitOptions`, `PmixPdata` (owned Rust fields + `Proc`).

Many live in `src/lib.rs`; error/status also in `src/error.rs` depending on split state — grep `pub enum PmixError`.

### 5.4 Function-pointer aliases

`EventHandlerRef`, `NotificationFn`, `HandlerRegCbFn`, `OpCbFn`, `SpawnCallback`, etc. in `src/events.rs` / `process_mgmt` — `Send + Sync` as function pointers / integers.

---

## 6. Caller rules

1. **Prefer `PmixClient`** for anything multi-threaded. Clone it; call `disconnect` once.  
2. **Build `Info` (and similar) on the calling thread**; drop before/without moving to another thread unless behind a mutex you control.  
3. **Callbacks = progress thread** — no blocking `fence`/`get`/`publish` in-handler; hop first (#51).  
4. **One logical init/finalize** per process (client session state machine enforces this for the client path).  
5. **Progress must run** — default internal thread, or `external_progress` + `progress()`.  
6. **Server module upcalls** — same non-blocking rule (#52).  
7. **cbdata** for registries: `crate::cbdata::encode_req_id` / `decode_req_id` (not `id << 2`).

---

## 7. FFI surface (by thread context)

### 7.1 Caller → PMIx (threadshifted ≥ 6.1)

Representative entry points (not every wrapper):

| Area | Module | Examples |
|------|--------|----------|
| Lifecycle | `src/lib.rs` | `PmixClient::connect`/`disconnect`, `init`, `finalize`, `progress`, `progress_thread_stop` |
| Core data | `src/lib.rs`, `src/data_ops/` | `put_value`, `get_value`, `commit`, `fence`, `publish`, `lookup`, `unpublish`, `*_nb` |
| Events | `src/events.rs` | register/deregister/notify |
| Process | `src/process_mgmt.rs` | spawn, connect, disconnect, resolve_* |
| Server | `src/server/` | `server_init`/`finalize`, register_*, fence/connect nb |
| Tool | `src/tool.rs` | `tool_init`/`finalize`, attach, get_servers |
| Groups | `src/groups.rs` | construct/invite/join/leave/destruct |
| Fabric | `src/fabric.rs` | register/update/topology/distances |
| Other | allocation, monitoring, query_log, security, utility, data_serialization | job control, query/log, IOF, pack/unpack, … |

### 7.2 PMIx → Rust (progress thread)

All `extern "C"` bridges used as completion/upcall targets — including but not limited to:

| Module | Bridges (names) |
|--------|-----------------|
| `data_ops` | `publish_callback_bridge`, `get_value_callback_bridge`, `lookup_callback_bridge`, `unpublish_callback_bridge`, `fence_callback_bridge` |
| `events` | `notification_bridge`, reg/dereg/notify completions |
| `process_mgmt` | spawn/connect/disconnect bridges |
| `groups` | group_*_callback_bridge |
| `fabric` | fabric_*_cb, compute_distances_cb |
| `allocation` / `monitoring` / `query_log` / `security` | *_callback_bridge |
| `server` | register_* , dmodex, setup_*, inventory, IOF deliver, pset/data nb bridges |
| `utility` | IOF reg/dereg/push/io bridges |

**Bridge policy:** lock registry → remove entry → **unlock** → run user code. Audit: [#67](https://github.com/SedahsDev/pmix-rs/issues/67). Helpers: [#51](https://github.com/SedahsDev/pmix-rs/issues/51).

---

## 8. Examples

Today several examples still use legacy `pmix::init` / `Context` (`examples/client_minimal.rs`, `simple_put_get.rs`, `simple_fence.rs`). Migration to `PmixClient`: [#65](https://github.com/SedahsDev/pmix-rs/issues/65).

Until then, the crate-root `PmixClient` rustdoc example is the canonical multi-thread sketch.

---

## 9. Roadmap (issue board)

| Phase | Issue | Status |
|-------|------:|--------|
| 0 Inventory + ≥ 6.1 docs | #45 | Done (this file’s ancestor) |
| 1a InitOptions bind/external progress | #46 | Done |
| 1b progress_thread_stop | #47 | Done |
| 2a PmixClient session | #48 | Done (PR #60 rewrite) |
| 2b Server/tool sessions | #49 | Open |
| 3 C-owned !Send + helpers | #50 | Open (`Info` done) |
| 4a Callback hop helpers | #51 | Open |
| 4b Server upcall example | #52 | Open |
| 5 Global FFI mutex feature | #53 | **Deferred** (not default) |
| 6 MT + external-progress tests | #54 | Open |
| Docs refresh | #64 | This document |
| Examples → PmixClient | #65 | Open |
| static_assertions matrix | #66 | Open |
| Registry lock audit | #67 | Open |

**Suggested order:** #50/#66 → #65 → #49 → #51/#67 → #52 → #54.

---

## 10. Historical note

An earlier `THREADPLAN.md` (local planning doc) drove #45–#54. Executable truth is: **this file**, crate-root concurrency docs, and the open issues above. Do not reintroduce session-level `!Send` “for safety.”
