# pmix-rs Thread-Safety Plan (THREADPLAN)

**Date:** 2026-07-18  
**Target crate:** `/home/bzf/projects/pmix-rs`  
**Reference libraries:**  
- OpenPMIx **6.1.0** — `/home/bzf/projects/prrte/scratch/source/openpmix-6.1.0`  
- PRRTE **4.1.0** — `/home/bzf/projects/prrte/scratch/source/prrte-4.1.0`  
**Spec / docs:** OpenPMIx man pages + headers (`pmix.h`, `pmix_common.h`), OpenPMIx 6.x NEWS, internal `PMIX_THREADSHIFT` / progress-thread implementation.

**Related prior work:** Issue #28 / PR #42 (Info `!Send`/`!Sync` docs) — foundational but **not** a complete threading model.

---

## 1. Executive summary

OpenPMIx **6.1** made a hard turn toward process-wide thread safety:

> *“all APIs are now threadshifted prior to execution for thread safety. Hosts that are providing their own progress engine (in lieu of using the PMIx internal progress thread) must ensure that progress is being provided sufficient to avoid threadlock when calling PMIx APIs.”*  
> — OpenPMIx 6.1.0 NEWS

That means:

| Layer | Who serializes? | Implication for pmix-rs |
|--------|------------------|-------------------------|
| **C library entry** | OpenPMIx `PMIX_THREADSHIFT` onto internal `evbase` / progress thread | Multiple app threads **may** call most `PMIx_*` APIs concurrently **if** the progress engine is running |
| **Progress engine** | One (or more) internal progress thread(s), or the host via `PMIx_Progress` | Without progress, non-blocking ops and many blocking ops can deadlock |
| **Rust wrappers** | **Not** currently designed for multi-thread sharing of owned handles | Even if C is MT-safe, Rust types with raw pointers / process-global state still need `Arc` / `Mutex` / clear `Send`/`Sync` rules |
| **Callbacks / server module** | Delivered on **PMIx progress thread** (or host progress), not the caller's thread | Handlers must not block; blocking APIs need an app-side thread shift |

**Recommended Rust strategy (agrees with your Arc + Mutex instinct):**

1. Treat the **process-global PMIx client/server session** as a single logical object guarded by a process-wide lock **or** documented as “thread-safe C entry + Rust handle discipline”.
2. Wrap **owned C resources** (`Info`, buffers, fabric objects, server/tool handles) in:
   - `Arc<T>` when shared immutably / read-mostly across threads after construction  
   - `Arc<Mutex<T>>` (or `parking_lot::Mutex`) when mutation or Drop/finalize must be exclusive  
3. Keep **pure value types** (`PmixError`, `PmixStatus`, copyable enums, plain `Proc` if it only holds `pmix_proc_t` POD) as `Send + Sync` where true.
4. Expose **progress / pinning** attributes (`PMIX_EXTERNAL_PROGRESS`, `PMIX_BIND_PROGRESS_THREAD`, `PMIX_BIND_REQUIRED`) in the safe API — this is where “pinning” actually lives in PMIx.

---

## 2. What “pinning” means in PMIx (important distinction)

### 2.1 What **can** be pinned (OpenPMIx supports this)

| Mechanism | Attribute / API | Pins what? |
|-----------|-----------------|------------|
| Progress-thread CPU bind | `PMIX_BIND_PROGRESS_THREAD` (`"pmix.bind.pt"`) — comma-delimited CPU ranges | **Internal PMIx progress thread** only |
| Bind required | `PMIX_BIND_REQUIRED` (`"pmix.bind.reqd"`) | Fail init if progress thread cannot bind |
| Host-driven progress | `PMIX_EXTERNAL_PROGRESS` (`"pmix.evext"`) = true | **Disables** reliance on internal progress thread; host must call `PMIx_Progress()` (already wrapped as `pmix::progress()`) |
| Aux event base | `PMIX_EXTERNAL_AUX_EVENT_BASE` | Host libevent base for signals/aux work |
| Stop progress thread | `PMIx_Progress_thread_stop(info, ninfo)` + optional `PMIX_PROGRESS_THREAD_NAME` / `PMIX_PROGRESS_THREAD_FLUSH` | Lifecycle of progress engine independent of finalize |

Evidence in tree:

- `include/pmix_common.h` — attribute definitions  
- `src/runtime/pmix_init.c` — reads `PMIX_EXTERNAL_PROGRESS`, `PMIX_BIND_PROGRESS_THREAD`, `PMIX_BIND_REQUIRED`  
- `src/runtime/pmix_progress_threads.h` — named progress threads, start/stop/pause/resume  
- `include/pmix.h` — `PMIx_Progress`, `PMIx_Progress_thread_stop`

**These do not pin arbitrary application threads calling `PMIx_Put` / `PMIx_Get`.** App threads pin themselves (or the job launcher does).

### 2.2 What **cannot** be pinned by PMIx (or must not be assumed)

| Item | Why |
|------|-----|
| Individual client API calls on random app threads | OpenPMIx **threadshifts** work onto the progress/`evbase` thread; the caller's CPU affinity is not “the PMIx execution CPU” |
| Blocking inside event / module callbacks | Spec/header critical note (e.g. group invite): **must not** call blocking PMIx APIs from the handler without first shifting to an app thread; non-blocking OK but must not wait in-handler |
| Server module upcall bodies that block waiting for PMIx | Upcalls run in PMIx/host progress context; blocking back into PMIx → **deadlock** (documented for groups; same pattern for fence/modex-style upcalls) |
| Sharing one `pmix_info_t*` / buffer concurrently without sync | C objects are not immutable shared state; concurrent free/load is UB regardless of API threadshift |
| “MPI_THREAD_MULTIPLE-style” per-object pinning of every handle | PMIx model is **one library instance + progress engine**, not per-communicator progress threads like some MPI stacks |

### 2.3 PRRTE interaction

PRRTE hosts OpenPMIx as the server-side stack. Progress and listener threads live inside that process. For **client** pmix-rs code under `prterun`, the default is almost always:

- OpenPMIx starts an **internal progress thread**  
- Client APIs are safe to call from multiple threads **only if** you still obey callback rules and Rust ownership  

For **embedded / tool / custom server** hosts that set `PMIX_EXTERNAL_PROGRESS`, the **Rust app or PRRTE host** owns the progress loop — pmix-rs must document and support that mode explicitly.

---

## 3. OpenPMIx execution model (from source)

```
 App thread A ──┐
 App thread B ──┼──► PMIx_* API entry
 App thread C ──┘         │
                          ▼
                 PMIX_THREADSHIFT (event on pmix_globals.evbase)
                          │
                          ▼
              Progress thread (or host PMIx_Progress)
                          │
          ┌───────────────┼────────────────┐
          ▼               ▼                ▼
    GDS / PTL I/O   Event handlers   Server module upcalls
```

Key globals (`pmix_globals` in `pmix_globals.h`):

- `atomic_bool initialized`, `connected`, `progress_thread_stopped`  
- `pmix_event_base_t *evbase` — primary progress  
- `progress_thread_stopped` — many client paths short-circuit or refuse work when stopped  

**Rust takeaway:** Multi-threaded **calls** into OpenPMIx 6.1 are intended to be OK; multi-threaded **ownership of Rust wrappers** and **callback re-entrancy** are the hard parts pmix-rs must solve.

---

## 4. Classification of PMIx API surface (for bindings)

### Class A — Process-global session (need single owner + optional Mutex)

| C API | Notes | Rust direction |
|-------|--------|----------------|
| `PMIx_Init` / `PMIx_Finalize` | One logical session per process (nested tools differ) | `Arc<PmixSession>` or static `OnceLock` + refcount; serialize init/finalize |
| `PMIx_server_init` / `PMIx_server_finalize` | Server library session | Same; RAII handle already moving toward Drop-finalize (#29) |
| `PMIx_tool_init` / finalize | Tool session; pmix-rs already has `TOOL_INITIALIZED` mutex | Promote to first-class session type |
| `PMIx_Progress` / `PMIx_Progress_thread_stop` | Progress control | Safe wrappers + init options for external progress |

### Class B — Concurrent-callable ops (C MT-safe via threadshift; Rust args must not race)

Blocking and non-blocking **client** ops: `Put`, `Get`/`Get_nb`, `Commit`, `Fence`/`Fence_nb`, `Publish*`, `Lookup*`, `Unpublish*`, `Spawn*`, `Connect*`, `Disconnect*`, query/log, fabric register/update, group APIs, etc.

**Rust rules:**

- Passing `&Info` / `&Proc` from multiple threads is OK **only if** the referent is not mutated/dropped  
- Prefer `Arc<Info>` or build ephemeral `Info` per call  
- Callback traits already require `Send` — keep that; deliver via channel to app runtime if work is heavy  

### Class C — Callback / upcall context (no pin; no blocking PMIx)

| Context | Thread | Rules |
|---------|--------|--------|
| `*_nb` completion callbacks | Progress thread | `Send` callbacks; no `fence()`/`get()` blocking; optional `std::thread::spawn` / channel |
| Event handlers (`PMIx_Register_event_handler`) | Progress thread | Same as header critical note for group invite |
| `pmix_server_module_t` function pointers | Server/progress | Must return quickly; complete via `cbfunc` asynchronously |

### Class D — Owned mutable C memory (almost never `Sync`)

| Type | Share across threads? | Pattern |
|------|----------------------|---------|
| `Info` (`*mut pmix_info_t`) | Only under `Mutex` or transfer ownership | `Arc<Mutex<Info>>` or keep `!Send` and never share |
| `PmixDataBuffer` | No concurrent pack/unpack | `Mutex` or single-thread |
| `PmixByteObject` with owned malloc | Careful Drop | Same |
| `PmixFabric` / topology handles | Library-owned updates | Session lock or fabric-level mutex |
| `PmixOwnedValue` | After extract, treat as owned Rust data | Often `Send` if no interior raw ptrs left — audit per variant |

### Class E — Pure data (good `Send + Sync` candidates)

`PmixError`, `PmixStatus`, scope/state enums, `Proc` **if** it only stores `pmix_proc_t` POD (no destructor needing global state).  
`Proc` is currently a plain struct — mark `Send + Sync` explicitly after audit.

---

## 5. Current pmix-rs gaps (as of main ~ post-#36/#38)

| Area | Status | Risk |
|------|--------|------|
| OpenPMIx threadshift (C) | Relies on linked lib ≥ 6.1 behavior | Document MSRV of **library**, not just Rust |
| `progress()` | Exists, minimal | No external-progress init helper; no bind attrs |
| `Progress_thread_stop` | In bindings, not safe API | Needed for clean shutdown tests / embedded hosts |
| Callback registries | `Mutex` + `Send` callbacks | Good pattern; unify; avoid lock across C calls where possible |
| `Context` | Not `Send`; process-global finalize on Drop | Multi-thread drop / multi-Context undefined |
| `Info` | Raw ptr; PR #42 may add `!Send` | Still need shareable pattern (`Arc<Mutex<Info>>` module or session-scoped builder) |
| `PmixDataBuffer` | Comment-only !Send | Enforce in type system |
| Server module callbacks | Stub or typed fn ptrs | No guidance / helper to hop off progress thread |
| Init attributes | Partial `Info` only | Missing first-class `InitOptions { external_progress, bind_progress_cpus, bind_required }` |
| Tests | Mostly single-threaded | Need loom/stress or multi-thread smoke under PRTE |
| Tool init flag | `Mutex<bool>` | Incomplete session model |

---

## 6. Target architecture

```
                    ┌─────────────────────────────┐
                    │   Arc<PmixClient>           │  // or PmixServer / PmixTool
                    │   - session identity        │
                    │   - progress mode           │
                    │   - optional Mutex for      │
                    │     ops that touch Rust     │
                    │     global registries       │
                    └─────────────┬───────────────┘
                                  │ clone
              ┌───────────────────┼───────────────────┐
              ▼                   ▼                   ▼
         Thread 1            Thread 2            Progress
         put/get             fence_nb            (C or host)
              │                   │                   │
              └───────── FFI ─────┴───────────────────┘
                                  │
                         OpenPMIx threadshift
```

### 6.1 Handle patterns

| Pattern | Use when |
|---------|----------|
| `Arc<Client>` | Many threads issue ops; session must stay alive |
| `Arc<Mutex<Info>>` | Rare shared mutable directives array |
| Ephemeral `Info` per call | Preferred — build on stack/thread, pass by value/ref, drop |
| `Client` not `Clone`, ops take `&self` | If all C entry is MT-safe and Rust has no shared mut state — still need `Arc` for lifetime across threads |
| Thread-local cached `Proc` | Rank/self identity without locking |

### 6.2 Callback pattern (mandatory)

```text
PMIx progress thread
    → Rust extern "C" bridge (minimal)
    → try_lock registry / remove callback
    → send result on crossbeam/std channel  OR  spawn_blocking
    → return immediately
App worker thread
    → recv / poll future
    → may call blocking PMIx APIs
```

Do **not** run user `Fn` that might call `fence()` directly on the progress thread without documentation and a “unsafe progress context” marker.

---

## 7. Phased implementation plan

### Phase 0 — Document & inventory (1–2 days)

- Capture OpenPMIx version gate: document “thread-safe C entry requires OpenPMIx ≥ 6.1 (threadshift-all-APIs)”.
- Inventory every public Rust type: `Send` / `Sync` / `!Send` / needs `Mutex`.
- Inventory every `extern "C"` bridge: note “runs on progress thread”.

**Deliverable:** table in this file (Appendix A) kept up to date; CI job optional later.

### Phase 1 — Progress & init options (foundation)

1. Safe `InitOptions` / `ServerInitOptions` / `ToolInitOptions`:
   - `external_progress: bool` → `PMIX_EXTERNAL_PROGRESS`
   - `bind_progress_thread: Option<String>` → `PMIX_BIND_PROGRESS_THREAD`
   - `bind_required: bool` → `PMIX_BIND_REQUIRED`
2. Wrap `PMIx_Progress_thread_stop` as `progress_thread_stop(opts)`.
3. Document deadlock: external progress **without** a host loop calling `progress()` will hang `_nb` and many blocking paths.
4. Example: multi-thread client with default internal progress; example: single-thread host loop with `external_progress`.

### Phase 2 — Session types (`Arc`-friendly)

1. Introduce `PmixClient` (wraps today’s `Context` + init flags).
2. `PmixClient: Clone` via `Arc<Inner>`.
3. Serialize `init`/`finalize` with a process-wide `Mutex` or atomic state machine (`Uninitialized → Live → Finalizing → Dead`).
4. Same for `PmixServer` / `PmixTool` (tool already has a bool mutex — lift to session).
5. Deprecate bare `init() -> Context` or make `Context` a thin alias.

### Phase 3 — Owned handle discipline

1. Complete `!Send`/`!Sync` (or `Mutex`) for: `Info`, `PmixDataBuffer`, fabric/topology wrappers, byte objects with C ownership.
2. Provide `Info` builders that are thread-local by default; `fn share(self) -> Arc<Mutex<Info>>` only if needed.
3. Ensure `Drop` never races: only one owner drops C memory (Arc last-drop OK; Mutex guard during free).
4. Audit `PmixOwnedValue` / payload variants for interior pointers → document which are `Send`.

### Phase 4 — Callback / upcall safety

1. Standardize bridges: never hold registry `Mutex` across user callback execution.
2. Add `ProgressContext` token type (zero-sized) passed only to bridges; mark APIs that must not be called with it.
3. Helpers: `spawn_from_callback(f)` / `channel_callback()` templates for `_nb` and server module.
4. Server module: document + example “complete fence upcall asynchronously”.

### Phase 5 — Optional Rust-level op mutex (policy choice)

**Default proposal:** do **not** global-lock every `put`/`get` if OpenPMIx ≥ 6.1 is required — trust C threadshift; only lock Rust shared state.

**Fallback feature `rust-serialize-ops`:** `Mutex` around all FFI entries for older libpmix or paranoia mode (easier deadlock with external progress — document heavily).

### Phase 6 — Testing

1. Multi-thread smoke: N threads `put` different keys + `fence` under `prterun`.
2. Concurrent `_nb` completions.
3. External progress mode: host thread calls `progress()` while workers issue `_nb`.
4. Callback deadlock regression: handler must not call blocking fence (loom or timeout test).
5. Optional TSAN / `RUSTFLAGS=-Zsanitizer=thread` job on nightly.

### Phase 7 — Docs & version story

1. Crate-level `#!` docs + `THREADING.md` summary linking this plan.
2. README “Threading” section.
3. `package.metadata` / docs.rs feature flags for `external_progress` examples.
4. Forgejo backup sync after GitHub merges (per project workflow).

---

## 8. Concrete type policy (checklist)

| Type | Goal | Mechanism |
|------|------|-----------|
| `PmixError` / `PmixStatus` / enums | `Send + Sync` | Derive / assert |
| `Proc` | `Send + Sync` if POD | Explicit `unsafe impl` or auto if no Drop glue |
| `Info` | `!Send + !Sync` **or** only behind `Mutex` | `PhantomData<*mut u8>` (PR #42) + optional `Arc<Mutex<Info>>` helper |
| `PmixOwnedValue` | Audit; often `Send` after ownership clear | Per-field |
| `PmixDataBuffer` | `!Send + !Sync` | PhantomData |
| `PmixByteObject` | `!Send` if holds C ptr | PhantomData |
| `Context` / `PmixClient` | `Send` via `Arc` inner; not `Sync` unless ops use only `&self` + C MT-safe | `Arc<ClientInner>` |
| `PmixServerHandle` | `Send` after RAII finalize rules clear | Careful Drop |
| `PmixToolHandle` | Same as client session | Arc |
| `PmixServerModule` | `'static` callbacks only; module struct itself not shared mutably during init | Build then pass once |
| Callback traits | Keep `Send` | Already mostly done |
| Registries | `Mutex` | Already; unify poisoning policy (`expect` / recover) |

---

## 9. Pinning decision matrix (app-facing)

| Goal | Supported? | How in pmix-rs |
|------|------------|----------------|
| Pin OpenPMIx progress thread to CPUs | **Yes** | `InitOptions.bind_progress_thread = "0-3"` → attr |
| Require bind success | **Yes** | `bind_required = true` |
| Run without internal progress thread | **Yes** | `external_progress = true` + app/`prte` calls `progress()` |
| Pin which CPU runs `PMIx_Get` work | **No** (not directly) | Work runs on progress thread; bind **that** thread |
| Pin server module upcall thread | **No** | Upcalls on progress/host; hop to app pool |
| Pin multiple independent PMIx instances per process | **No** | One client init model; tools are separate entrypoints |

---

## 10. Risks & non-goals

**Risks**

- Declaring full MT-safety while linking **old** libpmix (< 6.1) without threadshift-all.  
- `external_progress` + holding Rust mutex across `progress()` + callback that tries same mutex → deadlock.  
- Making everything `Mutex` globally → false sense of safety + latency.  
- Forgetting that `Info` Drop frees C memory while another thread still has raw `as_ptr()`.

**Non-goals (this plan)**

- Implementing a full async runtime (`async fn get`) — can be a later layer on channels.  
- Changing OpenPMIx/PRRTE C code.  
- Guaranteeing realtime latency of progress thread.

---

## 11. Suggested merge / issue order

1. Progress + init options (Phase 1)  
2. Session `Arc` types (Phase 2)  
3. Handle `Send`/`Sync` completion (Phase 3) — builds on #28  
4. Callback hop helpers (Phase 4)  
5. Tests (Phase 6) in parallel once 1–2 land  
6. Optional serialize feature (Phase 5) last  

---

## Appendix A — Source anchors (OpenPMIx 6.1.0)

| Topic | Path |
|-------|------|
| Threadshift-all-APIs announcement | `docs/news/news-v6.x.rst` (6.1.0 notes) |
| `PMIX_THREADSHIFT` macro | `src/include/pmix_globals.h` |
| Progress thread API | `src/runtime/pmix_progress_threads.h` |
| Init parsing bind/external progress | `src/runtime/pmix_init.c` (`PMIX_EXTERNAL_PROGRESS`, `PMIX_BIND_*`) |
| Public progress API | `include/pmix.h` — `PMIx_Progress`, `PMIx_Progress_thread_stop` |
| Progress attrs | `include/pmix_common.h` — `PMIX_EXTERNAL_PROGRESS`, `PMIX_BIND_PROGRESS_THREAD`, `PMIX_BIND_REQUIRED`, `PMIX_PROGRESS_THREAD_*` |
| Event-handler blocking warning | `include/pmix.h` group-join section (“thread shifts out of the handler”) |
| Group deadlock note | `docs/how-things-work/sets_groups/group_construct.rst` |
| Internal locks | `src/threads/pmix_threads.h`, `pmix_mutex.h` |

## Appendix B — pmix-rs anchors

| Topic | Path |
|-------|------|
| `progress()` | `src/lib.rs` |
| Bindings for progress stop | `src/bindings.rs` — `PMIx_Progress_thread_stop` |
| Callback registries | `src/data_ops.rs`, `src/server.rs`, `src/query_log.rs`, … |
| Data buffer thread comment | `src/data_serialization.rs` |
| Tool init mutex | `src/tool.rs` — `TOOL_INITIALIZED` |

---

## Appendix C — GitHub issues

Filed on `SedahsDev/pmix-rs`:

| Phase | Issue | Title |
|-------|------:|-------|
| 0 | [#45](https://github.com/SedahsDev/pmix-rs/issues/45) | inventory Send/Sync + OpenPMIx ≥ 6.1 assumption |
| 1a | [#46](https://github.com/SedahsDev/pmix-rs/issues/46) | InitOptions: external progress + progress-thread CPU bind |
| 1b | [#47](https://github.com/SedahsDev/pmix-rs/issues/47) | safe Progress_thread_stop + progress mode docs |
| 2a | [#48](https://github.com/SedahsDev/pmix-rs/issues/48) | Arc-based PmixClient session |
| 2b | [#49](https://github.com/SedahsDev/pmix-rs/issues/49) | Arc session types for server and tool |
| 3 | [#50](https://github.com/SedahsDev/pmix-rs/issues/50) | !Send/!Sync on C-owned buffers; Arc\<Mutex\<T\>\> helpers |
| 4a | [#51](https://github.com/SedahsDev/pmix-rs/issues/51) | callback bridge policy + hop-off-progress helpers |
| 4b | [#52](https://github.com/SedahsDev/pmix-rs/issues/52) | server module upcall guidelines + example |
| 5 | [#53](https://github.com/SedahsDev/pmix-rs/issues/53) | optional rust-serialize-ops feature |
| 6 | [#54](https://github.com/SedahsDev/pmix-rs/issues/54) | multi-thread + external-progress integration tests |

**Suggested order:** #45 → #46 → #47 → #48 → #49 → #50 → #51 → #52 → #54; #53 last (optional).

---

*End of THREADPLAN.md*
