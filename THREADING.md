# Threading Model & Send/Sync Inventory

**Date:** 2026-07-27  
**Issues:** [#64](https://github.com/SedahsDev/pmix-rs/issues/64), [#45](https://github.com/SedahsDev/pmix-rs/issues/45)  
**Related:** [#49](https://github.com/SedahsDev/pmix-rs/issues/49)–[#52](https://github.com/SedahsDev/pmix-rs/issues/52), [#54](https://github.com/SedahsDev/pmix-rs/issues/54), [#66](https://github.com/SedahsDev/pmix-rs/issues/66)–[#67](https://github.com/SedahsDev/pmix-rs/issues/67)

Crate-root docs in `src/lib.rs` (`//! # Concurrency model`) match this document.

---

## 1. Strategy (one-liner)

| Layer | Policy |
|--------|--------|
| **Session** | Process-wide `OnceLock<Arc<Inner>>`. Handle is **`Clone + Send + Sync`**. **No** `PhantomData<*mut u8>` on the session Inner. |
| **Client API** | **`PmixClient` only** — no legacy `Context` / `init()`. Explicit `connect` / `disconnect`. **Drop never finalizes.** |
| **C API entry** | Trust OpenPMIx **≥ 6.1** threadshift. No global op mutex by default. |
| **C-owned handles** | `Info`, buffers, fabric, … → **`!Send + !Sync`**. Prefer build-per-call. |
| **Data ops** | Free functions (`put_value`, `get_value`, `commit`, `fence`, `data_ops::*`, …). |
| **Callbacks / upcalls** | PMIx **progress thread**. No blocking PMIx in-handler (#51, #52, #67). |

**Anti-pattern:** `PhantomData<*mut u8>` on `PmixClientInner` while advertising multi-thread clones.

---

## 2. OpenPMIx ≥ 6.1

Threadshift on C entry. Progress must run (internal thread or `external_progress` + `pmix::progress()`). Pre-6.1 needs external sync (unsupported default).

---

## 3. `PmixClient` (only client session)

| Property | Behavior |
|----------|----------|
| Storage | `OnceLock<Arc<PmixClientInner>>` |
| Auto-traits | **`Clone + Send + Sync`** (asserted) |
| State | `Uninitialized → Live → Finalizing → Dead` |
| Connect | `PmixClient::connect_new(info)` or `new()` + `connect` |
| Disconnect | `client.disconnect(info)` or free-fn `finalize(info)` |
| Drop | **No** finalize |
| Identity | `proc()` / `require_proc()` / `rank()` / `require_rank()` |

```rust
let client = pmix::PmixClient::connect_new(None)?;
let w = client.clone();
std::thread::spawn(move || { let _ = w.rank(); });
client.disconnect(None)?;
```

### Server / tool

Still handle-based; same process-wide session pattern is [#49](https://github.com/SedahsDev/pmix-rs/issues/49).

---

## 4. Progress & pinning

| Goal | How |
|------|-----|
| Pin progress CPUs | `InitOptions::bind_progress_thread` |
| Host progress | `InitOptions::external_progress(true)` + `progress()` |
| Stop progress thread | `progress_thread_stop()` |

Deadlocks: external progress without a loop; mutex held across `progress()` + callback; blocking PMIx inside a callback; `progress()` after `progress_thread_stop()`.

---

## 5. Type inventory (summary)

| Type | Send/Sync | Status |
|------|-----------|--------|
| `PmixClient`, `PmixClientState` | **Send + Sync** | Enforced |
| `Proc` | Send + Sync (POD) | Intended |
| `Info` | **!Send + !Sync** | Enforced |
| Buffers / fabric / results | !Send target | #50 |
| Enums / pure builders | Send + Sync | OK |

Full matrix: [#66](https://github.com/SedahsDev/pmix-rs/issues/66). Completing marks: [#50](https://github.com/SedahsDev/pmix-rs/issues/50).

---

## 6. Caller rules

1. Use **`PmixClient`** — clone for workers; `disconnect` once.  
2. Build `Info` on the calling thread.  
3. Callbacks = progress thread — hop before blocking PMIx.  
4. One connect/disconnect cycle per process.  
5. Progress must run.  
6. cbdata: `crate::cbdata::encode_req_id` / `decode_req_id`.

---

## 7. Examples

`examples/client_minimal.rs`, `simple_put_get.rs`, `simple_fence.rs` use `PmixClient::connect_new` / `require_proc` / `disconnect`.

---

## 8. Roadmap

| Item | Issue | Status |
|------|------:|--------|
| Inventory + ≥6.1 | #45 | Done |
| InitOptions / progress stop | #46, #47 | Done |
| PmixClient session | #48 | Done |
| Remove Context/init | this PR | Done |
| Server/tool sessions | #49 | Open |
| C-owned !Send | #50 | Open (`Info` done) |
| Callback hop / audit | #51, #67 | Open |
| Server upcall example | #52 | Open |
| Global FFI mutex | #53 | Deferred |
| MT integration tests | #54 | Open |
| static_assertions matrix | #66 | Open |
