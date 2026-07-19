# Threading Model & Send/Sync Inventory

**Date:** 2026-07-18
**Related:** [THREADPLAN.md](./THREADPLAN.md) — parent plan
**Issue:** [#45](https://github.com/SedahsDev/pmix-rs/issues/45)

---

## 1. OpenPMIx Version Assumption

**This crate assumes OpenPMIx ≥ 6.1.**

From the [OpenPMIx 6.1.0 NEWS](https://github.com/openpmix/openpmix/blob/v6.1.0/docs/news/news-v6.x.rst):

> *all APIs are now threadshifted prior to execution for thread safety. Hosts that are providing their own progress engine (in lieu of using the PMIx internal progress thread) must ensure that progress is being provided sufficient to avoid threadlock when calling PMIx APIs.*

**What this means for pmix-rs callers:**

| Layer | Who serializes? | Implication |
|-------|-----------------|-------------|
| C library entry | OpenPMIx `PMIX_THREADSHIFT` onto internal `evbase` / progress thread | Multiple Rust threads **may** call most `PMIx_*` APIs concurrently, provided the progress engine is running |
| Progress engine | One (or more) internal progress thread(s), or the host via `PMIx_Progress` | Without progress, non-blocking ops and many blocking ops can deadlock |
| Rust wrappers | **Not** currently designed for multi-thread sharing of owned handles | Even if C is MT-safe, Rust types with raw pointers / process-global state still need `Arc<Mutex<T>>` wrappers for shared access |
| Callbacks / server module | Delivered on **PMIx progress thread** (not the caller's thread) | Handlers must not block; blocking APIs need an app-side thread shift |

**If you link against OpenPMIx < 6.1**, most APIs are **not** internally serialized. In that case, all `PMIx_*` calls must be externally synchronized (single-threaded or mutex-guarded).

---

## 2. Public Type Inventory

Every public Rust type with its intended `Send` / `Sync` status.

### 2.1 Definitely `!Send` / `!Sync` (raw handle wrappers)

These types wrap raw pointers to PMIx-allocated memory or process-global state. They must **not** be shared across threads without a `Mutex`.

| Type | Location | Rationale |
|------|----------|-----------|
| `Info` | `src/lib.rs:2932` | Wraps `*mut pmix_info_t` — PMIx-allocated array |
| `InfoBuilder` | `src/lib.rs:2998` | Owns `Vec<InfoEntry>` with `CString` keys — pre-build state, not MT-safe to share mid-build |
| `Context` | `src/lib.rs:3098` | Wraps `*mut pmix_proc_t` — process-global init state |
| `PmixOwnedValue` | `src/lib.rs:2847` | Wraps `*mut pmix_value_t` — PMIx-allocated value |
| `Proc` | `src/lib.rs:2927` | Wraps `*mut pmix_proc_t` — PMIx-allocated proc identifier |
| `PmixServerHandle` | `src/server/mod.rs:295` | Tracks server init state — process-global singleton |
| `PmixToolHandle` | `src/tool.rs:65` | Wraps tool connection state — process-global |
| `PmixCredential` | `src/security.rs:56` | Wraps `*mut pmix_credential_t` — PMIx-allocated credential |
| `PmixByteObject` | `src/data_serialization.rs:97` | Wraps `pmix_byte_object_t` with PMIx-allocated buffer |
| `PmixDataBuffer` | `src/data_serialization.rs:211` | Wraps `*mut pmix_data_buffer_t` — PMIx-allocated buffer |
| `PmixProcRef` | `src/data_serialization.rs:63` | Borrowed `&'a Proc` — inherits Proc's thread constraints |
| `AllocationResults` | `src/allocation.rs:160` | Wraps PMIx-allocated job/id strings |
| `JobControlResults` | `src/allocation.rs:529` | Wraps PMIx-allocated result strings |
| `SessionControlResults` | `src/allocation.rs:853` | Wraps PMIx-allocated result data |
| `MonitorResults` | `src/monitoring.rs:66` | Wraps PMIx-allocated monitoring data |
| `QueryResults` | `src/query_log.rs:232` | Wraps `*mut pmix_info_t` array |
| `CredentialResults` | `src/security.rs:320` | Wraps PMIx-allocated credential |
| `ValidationResults` | `src/security.rs:546` | Wraps PMIx-allocated validation data |
| `PmixFabric` | `src/fabric.rs:85` | Wraps `*mut pmix_fabric_t` — PMIx-allocated |
| `PmixTopology` | `src/fabric.rs:611` | Wraps `*mut pmix_topology_t` — PMIx-allocated |
| `PmixCpuset` | `src/fabric.rs:728` | Wraps `*mut pmix_cpuset_t` — PMIx-allocated |
| `DeviceDistances` | `src/fabric.rs:914` | Wraps `*mut pmix_device_distances_t` — PMIx-allocated |
| `SpawnCallbackWrapper` | `src/process_mgmt.rs:486` | Boxed callback + cbdata — not MT-safe to share |
| `ConnectCallbackWrapper` | `src/process_mgmt.rs:774` | Boxed callback + cbdata |
| `DisconnectCallbackWrapper` | `src/process_mgmt.rs:996` | Boxed callback + cbdata |
| `GroupConstructCallbackWrapper` | `src/groups.rs:120` | Boxed callback + cbdata |
| `GroupInviteCallbackWrapper` | `src/groups.rs:329` | Boxed callback + cbdata |
| `GroupJoinCallbackWrapper` | `src/groups.rs:535` | Boxed callback + cbdata |
| `GroupLeaveCallbackWrapper` | `src/groups.rs:689` | Boxed callback + cbdata |
| `GroupDestructCallbackWrapper` | `src/groups.rs:811` | Boxed callback + cbdata |

### 2.2 Pure Rust / Copy types — `Send + Sync`

These types contain no raw pointers to PMIx-allocated memory. They are safe to share across threads.

| Type | Location | Rationale |
|------|----------|-----------|
| `PmixError` | `src/lib.rs:81` | Pure Rust enum with owned strings |
| `PmixStatus` | `src/lib.rs:406` | Newtype around `i32`, derives Copy |
| `PmixProcState` | `src/lib.rs:729` | Pure Rust enum |
| `PmixScope` | `src/lib.rs:958` | Pure Rust enum |
| `PmixJobState` | `src/lib.rs:1045` | Pure Rust enum |
| `PmixLinkState` | `src/lib.rs:1160` | Pure Rust enum |
| `PmixDeviceType` | `src/lib.rs:1237` | Pure Rust enum |
| `PmixPersistence` | `src/lib.rs:1325` | Pure Rust enum |
| `PmixDataRange` | `src/lib.rs:1412` | Pure Rust enum |
| `PmixDataType` | `src/lib.rs:1515` | Pure Rust enum |
| `PmixAllocDirective` | `src/lib.rs:1966` | Pure Rust enum |
| `IOFChannelFlags` | `src/lib.rs:2018` | Newtype around `u16` |
| `BuilderError` | `src/lib.rs:2090` | Pure Rust enum |
| `ValueError` | `src/lib.rs:2136` | Pure Rust enum |
| `PmixTimeval` | `src/lib.rs:2177` | Pure Rust struct with two `i64` fields |
| `PmixEnvar` | `src/lib.rs:2200` | Pure Rust struct with `String` fields |
| `InfoFlags` | `src/lib.rs:2264` | Newtype around `u32` |
| `PmixPayload` | `src/lib.rs:2311` | Pure Rust enum with owned data |
| `PmixValueBuilder` | `src/lib.rs:2420` | Pure Rust builder (no raw pointers) |
| `PmixApp` | `src/process_mgmt.rs:162` | Pure Rust struct with `String`/`Vec<String>` fields |
| `PmixAppBuilder` | `src/process_mgmt.rs:219` | Pure Rust builder |
| `PmixQuery` | `src/query_log.rs:63` | Pure Rust struct |
| `PmixDeviceDistance` | `src/fabric.rs:822` | Pure Rust struct with `u16` fields |
| `PmixBindEnvelope` | `src/cpu_locality.rs:38` | Pure Rust enum |
| `PmixLocality` | `src/cpu_locality.rs:199` | Bitflags struct, `Send + Sync` |
| `PmixPrintOutput` | `src/data_serialization.rs:837` | Pure Rust struct with owned strings |
| `PmixJobCtrlAction` | `src/allocation.rs:476` | Pure Rust enum |

### 2.3 Type aliases — inherit constraints

| Alias | Location | Underlying type | Constraint |
|-------|----------|-----------------|------------|
| `EventHandlerRef` | `src/events.rs:62` | `usize` | `Send + Sync` (just an integer) |
| `NotificationFn` | `src/events.rs:93` | `Option<unsafe extern "C" fn(...)>` | `Send + Sync` (function pointers are) |
| `HandlerRegCbFn` | `src/events.rs:122` | `unsafe extern "C" fn(...)` | `Send + Sync` (function pointer) |
| `OpCbFn` | `src/events.rs:131` | `Option<unsafe extern "C" fn(...)>` | `Send + Sync` (function pointer) |
| `SpawnCallback` | `src/process_mgmt.rs:322` | `unsafe extern "C" fn(...)` | `Send + Sync` (function pointer) |

### 2.4 Summary

| Category | Count | Send/Sync |
|----------|-------|-----------|
| Raw handle wrappers | 26 | `!Send`, `!Sync` |
| Pure Rust types | 25 | `Send + Sync` |
| Type aliases | 4 | `Send + Sync` (function pointers / integers) |

**Recommendation:** Add `PhantomData<*mut u8>` fields to all 26 raw-handle types to enforce `!Send`/`!Sync` at compile time. See PR #42 for the pattern already applied to `Info`.

---

## 3. FFI Bridge Inventory

Every `extern "C"` bridge function in pmix-rs, categorized by thread context.

### 3.1 Client-side API calls (runs on caller thread → threadshifted by OpenPMIx ≥ 6.1)

These are Rust functions that call `ffi::PMIx_*`. On OpenPMIx ≥ 6.1, the C side threadshifts to the progress thread internally.

| Function | File | FFI Call |
|----------|------|----------|
| `init()` | `src/lib.rs:3135` | `PMIx_Init` |
| `finalize()` | `src/lib.rs:3267` | `PMIx_Finalize` |
| `fence()` | `src/lib.rs:3244` | `PMIx_Fence` |
| `is_initialized()` | `src/utility/mod.rs:46` | `PMIx_Initialized` |
| `publish()` | `src/data_ops/mod.rs` | `PMIx_Publish` |
| `get_value()` | `src/data_ops/mod.rs` | `PMIx_Get` |
| `lookup()` | `src/data_ops/mod.rs` | `PMIx_Lookup_nspace` |
| `unpublish()` | `src/data_ops/mod.rs` | `PMIx_Unpublish` |
| `fence_nb()` | `src/data_ops/mod.rs` | `PMIx_Fence_nb` |
| `spawn()` | `src/process_mgmt.rs:452` | `PMIx_Spawn` |
| `connect()` | `src/server/data.rs:470` | `PMIx_Connect` |
| `disconnect()` | `src/server/data.rs` | `PMIx_Disconnect` |
| `resolve_peers()` | `src/process_mgmt.rs:1173` | `PMIx_Resolve_peers` |
| `resolve_nodes()` | `src/process_mgmt.rs:1250` | `PMIx_Resolve_nodes` |
| `fabric_register()` | `src/fabric.rs:275` | `PMIx_Fabric_register` |
| `fabric_update()` | `src/fabric.rs:427` | `PMIx_Fabric_update` |
| `fabric_update_nb()` | `src/fabric.rs:470` | `PMIx_Fabric_update_nb` |
| `fabric_deregister()` | `src/fabric.rs:516` | `PMIx_Fabric_deregister` |
| `load_topology()` | `src/fabric.rs:1059` | `PMIx_Load_topology` |
| `register_event_handler()` | `src/events.rs` | `PMIx_Register_event_handler` |
| `deregister_event_handler()` | `src/events.rs:382` | `PMIx_Deregister_event_handler` |
| `notify_event()` | `src/events.rs` | `PMIx_Notify_event` |
| `server_init()` | `src/server/mod.rs` | `PMIx_server_init` |
| `server_finalize()` | `src/server/mod.rs:322` | `PMIx_server_finalize` |
| `tool_init()` | `src/tool.rs` | `PMIx_tool_init` |
| `tool_finalize()` | `src/tool.rs` | `PMIx_tool_finalize` |
| `tool_is_connected()` | `src/tool.rs:794` | `PMIx_tool_is_connected` |
| `group_construct()` | `src/groups.rs` | `PMIx_Group_construct` |
| `group_invite()` | `src/groups.rs` | `PMIx_Group_invite` |
| `group_join()` | `src/groups.rs` | `PMIx_Group_join` |
| `group_leave()` | `src/groups.rs:674` | `PMIx_Group_leave` |
| `group_destruct()` | `src/groups.rs:796` | `PMIx_Group_destruct` |
| `register_nspace()` | `src/server/mod.rs` | `PMIx_server_register_nspace` |
| `deregister_nspace()` | `src/server/mod.rs` | `PMIx_server_deregister_nspace` |
| `register_client()` | `src/server/mod.rs` | `PMIx_server_register_client` |
| `deregister_client()` | `src/server/mod.rs` | `PMIx_server_deregister_client` |
| `job_control()` | `src/allocation.rs` | `PMIx_server_job_control` |
| `allocate()` | `src/allocation.rs` | `PMIx_server_allocate` |
| `session_control()` | `src/allocation.rs` | `PMIx_server_session_control` |
| `monitor()` | `src/monitoring.rs` | `PMIx_server_monitor` |
| `query()` | `src/query_log.rs` | `PMIx_Query` |
| `log()` | `src/query_log.rs` | `PMIx_Log` |
| `get_credential()` | `src/security.rs` | `PMIx_server_get_credential` |
| `validate_credential()` | `src/security.rs` | `PMIx_server_validate_credential` |
| `iof_channel_register()` | `src/utility/mod.rs` | `PMIx_IOF_channel_register` |
| `iof_channel_deregister()` | `src/utility/mod.rs` | `PMIx_IOF_channel_deregister` |
| `iof_push()` | `src/utility/mod.rs` | `PMIx_IOF_push` |
| `iof_pull()` | `src/utility/mod.rs` | `PMIx_IOF_pull` |
| `pdata_construct()` | `src/server/data.rs:127` | `PMIx_Pdata_construct` |
| `pdata_destruct()` | `src/server/data.rs:175` | `PMIx_Pdata_destruct` |
| `data_buffer_create()` | `src/data_serialization.rs:309` | `PMIx_Data_buffer_create` |
| `data_buffer_release()` | `src/data_serialization.rs:262` | `PMIx_Data_buffer_release` |
| `byte_object_destruct()` | `src/data_serialization.rs:175` | `PMIx_Byte_object_destruct` |
| `data_unload()` | `src/data_serialization.rs:584` | `PMIx_Data_unload` |
| `data_copy_payload()` | `src/data_serialization.rs:716` | `PMIx_Data_copy_payload` |
| `app_create()` | `src/process_mgmt.rs:360` | `PMIx_App_create` |
| `app_free()` | `src/process_mgmt.rs:374` | `PMIx_App_free` |
| `topology_destruct()` | `src/fabric.rs:701` | `PMIx_Topology_destruct` |
| `cpuset_construct()` | `src/fabric.rs:751` | `PMIx_Cpuset_construct` |
| `cpuset_destruct()` | `src/fabric.rs:797` | `PMIx_Cpuset_destruct` |

### 3.2 Callback bridges (runs on PMIx progress thread)

These `extern "C"` functions are invoked by the PMIx library as callbacks. They run on the **PMIx internal progress thread**, NOT the caller's thread.

| Bridge | File | Invoked by |
|--------|------|------------|
| `notification_bridge` | `src/events.rs:143` | PMIx notification delivery |
| `publish_callback_bridge` | `src/data_ops/mod.rs:89` | `PMIx_server_publish` upcall |
| `get_value_callback_bridge` | `src/data_ops/mod.rs:212` | `PMIx_Get` completion |
| `lookup_callback_bridge` | `src/data_ops/mod.rs:634` | `PMIx_Lookup_nspace` completion |
| `unpublish_callback_bridge` | `src/data_ops/mod.rs:854` | `PMIx_Unpublish` completion |
| `fence_callback_bridge` | `src/data_ops/mod.rs:1177` | `PMIx_Fence_nb` completion |
| `spawn_callback_bridge` | `src/process_mgmt.rs:539` | `PMIx_Spawn` completion |
| `connect_callback_bridge` | `src/process_mgmt.rs:826` | `PMIx_Connect` completion |
| `disconnect_callback_bridge` | `src/process_mgmt.rs:1048` | `PMIx_Disconnect` completion |
| `fabric_register_cb` | `src/fabric.rs:334` | `PMIx_Fabric_register` completion |
| `fabric_update_cb` | `src/fabric.rs:462` | `PMIx_Fabric_update_nb` completion |
| `fabric_deregister_cb` | `src/fabric.rs:561` | `PMIx_Fabric_deregister` completion |
| `compute_distances_cb` | `src/fabric.rs:1247` | Distance computation completion |
| `group_construct_callback_bridge` | `src/groups.rs:145` | `PMIx_Group_construct` upcall |
| `group_invite_callback_bridge` | `src/groups.rs:352` | `PMIx_Group_invite` upcall |
| `group_join_callback_bridge` | `src/groups.rs:560` | `PMIx_Group_join` upcall |
| `group_leave_callback_bridge` | `src/groups.rs:715` | `PMIx_Group_leave` completion |
| `group_destruct_callback_bridge` | `src/groups.rs:837` | `PMIx_Group_destruct` completion |
| `allocation_callback_bridge` | `src/allocation.rs:303` | `PMIx_server_allocate` completion |
| `job_control_callback_bridge` | `src/allocation.rs:669` | `PMIx_server_job_control` completion |
| `session_control_callback_bridge` | `src/allocation.rs:860` | `PMIx_server_session_control` completion |
| `monitor_callback_bridge` | `src/monitoring.rs:136` | `PMIx_server_monitor` upcall |
| `query_callback_bridge` | `src/query_log.rs:357` | `PMIx_Query` completion |
| `log_callback_bridge` | `src/query_log.rs:566` | `PMIx_Log` completion |
| `credential_callback_bridge` | `src/security.rs:366` | Credential request completion |
| `validation_callback_bridge` | `src/security.rs:700` | Validation completion |
| `reg_callback_bridge` | `src/utility/mod.rs:1048` | IOF register completion |
| `dereg_callback_bridge` | `src/utility/mod.rs:1259` | IOF deregister completion |
| `push_callback_bridge` | `src/utility/mod.rs:1550` | IOF push completion |
| `io_callback_bridge` | `src/utility/mod.rs:999` | IOF delivery |
| `register_nspace_callback_bridge` | `src/server/mod.rs:538` | `PMIx_server_register_nspace` completion |
| `deregister_nspace_callback_bridge` | `src/server/mod.rs:753` | `PMIx_server_deregister_nspace` completion |
| `register_client_callback_bridge` | `src/server/mod.rs:914` | `PMIx_server_register_client` completion |
| `deregister_client_callback_bridge` | `src/server/mod.rs:1086` | `PMIx_server_deregister_client` completion |
| `dmodex_request_callback_bridge` | `src/server/mod.rs:1458` | `PMIx_server_dmodex_req` upcall |
| `setup_application_callback_bridge` | `src/server/mod.rs:1661` | `PMIx_server_setup_app` upcall |
| `setup_local_support_callback_bridge` | `src/server/mod.rs:1930` | `PMIx_server_setup_local` completion |
| `iof_deliver_callback_bridge` | `src/server/mod.rs:2135` | IOF deliver completion |
| `collect_inventory_callback_bridge` | `src/server/mod.rs:2391` | Inventory collection upcall |
| `deliver_inventory_callback_bridge` | `src/server/mod.rs:2610` | Inventory delivery completion |
| `fence_nb_callback_bridge` | `src/server/data.rs:375` | Server fence_nb upcall |
| `connect_nb_callback_bridge` | `src/server/data.rs:508` | Server connect_nb upcall |
| `disconnect_nb_callback_bridge` | `src/server/data.rs:646` | Server disconnect_nb upcall |
| `register_resources_callback_bridge` | `src/server/pset.rs:266` | Register resources completion |
| `deregister_resources_callback_bridge` | `src/server/pset.rs:464` | Deregister resources completion |

### 3.3 Callback bridge summary

| Category | Count | Thread |
|----------|-------|--------|
| Client API calls (caller → PMIx) | 62 | Caller thread (threadshifted internally by PMIx ≥ 6.1) |
| Callback bridges (PMIx → caller) | 40 | PMIx progress thread |

---

## 4. Key Rules for Callers

1. **Never share `!Send` types across threads** — use `Arc<Mutex<T>>` if you need shared access to `Context`, `Info`, `PmixOwnedValue`, etc.
2. **Callbacks run on the PMIx progress thread** — keep them short. Do not call blocking PMIx APIs from within a callback (deadlock risk). If you must, dispatch to your own thread pool.
3. **Single init/finalize cycle** — prefer one `init()` / `finalize()` pair per process. Multiple cycles are not guaranteed to work.
4. **Progress engine must run** — if you provide your own progress engine instead of using the PMIx internal one, ensure it runs during API calls to avoid threadlock.
5. **Server upcalls** — `PmixServerModule` callbacks are invoked by PMIx on its internal threads. The callback bridges handle cbdata routing, but user-provided callback logic should be non-blocking.

---

## 5. Future Work

- [ ] Add `PhantomData<*mut u8>` to all 26 raw-handle types (enforce `!Send`/`!Sync` at compile time) — see PR #42 for the pattern
- [ ] Add `static_assertions` compile-time tests for all public types
- [ ] Consider `Arc<Mutex<T>>` wrapper types for common multi-threaded use patterns
- [ ] Document thread-safety guarantees per-function in doc comments
