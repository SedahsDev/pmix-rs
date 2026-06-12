# PMIx 4.1.1 Specification Analysis

## Overview

This document maps the PMIx 4.1.1 specification (from `pmix-standard-v4.1.pdf`) against the existing Rust bindings in `src/ffi.rs` and safe wrappers in `src/lib.rs`.

## Existing Code Coverage

### `src/ffi.rs` — FFI Declarations

The existing `ffi.rs` has a **stub** with only one FFI declaration:
```rust
pub fn PMIx_Init(...) -> pmix_status_t;
```
This is clearly a starting point, not a complete binding.

### `src/lib.rs` — Safe Wrappers

The existing `lib.rs` has extensive safe wrappers covering:

**Core lifecycle:**
- `Pmix::init()` -> `PMIx_Init`
- `Pmix::finalize()` -> `PMIx_Finalize`

**Data operations:**
- `Pmix::put()` -> `PMIx_Put`
- `Pmix::commit()` -> `PMIx_Commit`
- `Pmix::get()` -> `PMIx_Get`
- `Pmix::fence()` -> `PMIx_Fence`
- `Pmix::resolve_peers()` -> `PMIx_Resolve_peers`
- `Pmix::resolve_nodes()` -> `PMIx_Resolve_nodes`

**Event handling:**
- `Pmix::register_event_handler()` -> `PMIx_Register_event_handler`
- `Pmix::deregister_event_handler()` -> `PMIx_Deregister_event_handler`
- `Pmix::notify_event()` -> `PMIx_Notify_event`

**Process management:**
- `Pmix::abort()` -> `PMIx_Abort`
- `Pmix::spawn()` -> `PMIx_Spawn`
- `Pmix::connect()` -> `PMIx_Connect`
- `Pmix::disconnect()` -> `PMIx_Disconnect`

**Server module:**
- `PmixServerModule::new()` -> `pmix_server_module_t` construction

**Data types implemented:**
- `PmixProc`, `PmixInfo`, `PmixValue`, `PmixDataBuffer`, `PmixPdata`, `PmixQuery`
- `PmixServerModule` (partial — only v1x/v2x interfaces)

## Complete PMIx 4.1.1 API Inventory

### Client APIs (100 functions total)

#### 1. Utility / Version Functions (16) — STRING conversion + init check

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Error_string` | MISSING | `pmix_status_t -> const char*` |
| `PMIx_Proc_state_string` | MISSING | `pmix_proc_state_t -> const char*` |
| `PMIx_Scope_string` | MISSING | `pmix_scope_t -> const char*` |
| `PMIx_Persistence_string` | MISSING | `pmix_persistence_t -> const char*` |
| `PMIx_Data_range_string` | MISSING | `pmix_data_range_t -> const char*` |
| `PMIx_Info_directives_string` | MISSING | `pmix_info_directives_t -> const char*` |
| `PMIx_Data_type_string` | MISSING | `pmix_data_type_t -> const char*` |
| `PMIx_Alloc_directive_string` | MISSING | `pmix_alloc_directive_t -> const char*` |
| `PMIx_IOF_channel_string` | MISSING | `pmix_iof_channel_t -> const char*` |
| `PMIx_Job_state_string` | MISSING | `pmix_job_state_t -> const char*` |
| `PMIx_Get_attribute_string` | MISSING | `char* -> const char*` |
| `PMIx_Get_attribute_name` | MISSING | `char* -> const char*` |
| `PMIx_Link_state_string` | MISSING | `pmix_link_state_t -> const char*` |
| `PMIx_Device_type_string` | MISSING | `pmix_device_type_t -> const char*` |
| `PMIx_Initialized` | MISSING | Returns bool |
| `PMIx_Get_version` | MISSING | Returns `pmix_version_info_t*` |

#### 2. Lifecycle (5) — Core init/finalize/progress

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Init` | COVERED | FFI stub exists, safe wrapper in lib.rs |
| `PMIx_Finalize` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Progress` | MISSING | `void -> pmix_status_t` |
| `PMIx_server_init` | COVERED | `pmix_server_module_t*` + info |
| `PMIx_server_finalize` | MISSING | `void -> pmix_status_t` |

#### 3. Data Collection (16) — Put/Get/Fence/Publish/Lookup

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Fence` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Fence_nb` | MISSING | Non-blocking variant with callback |
| `PMIx_Get` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Get_nb` | MISSING | Non-blocking variant with callback |
| `PMIx_Put` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Store_internal` | MISSING | `proc* + key + value` |
| `PMIx_Commit` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Publish` | MISSING | `info[] + ninfo -> pmix_status_t` |
| `PMIx_Publish_nb` | MISSING | Non-blocking with callback |
| `PMIx_Lookup` | MISSING | `pdata[] + ndata + keys[] + nkeys + info[] + ninfo` |
| `PMIx_Lookup_nb` | MISSING | Non-blocking with callback |
| `PMIx_Unpublish` | MISSING | `keys[] + nkeys + info[] + ninfo` |
| `PMIx_Unpublish_nb` | MISSING | Non-blocking with callback |
| `PMIx_Resolve_peers` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Resolve_nodes` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Query_info` | MISSING | `query[] + nqueries + info[] -> pdata** + size_t*` |
| `PMIx_Query_info_nb` | MISSING | Non-blocking with callback |

#### 4. Event Notification (3) — Already covered

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Register_event_handler` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Deregister_event_handler` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Notify_event` | COVERED | Safe wrapper in lib.rs |

#### 5. Data Packing/Unpacking (9)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Data_pack` | MISSING | `proc* + data_buffer* + type + value + size` |
| `PMIx_Data_unpack` | MISSING | `proc* + data_buffer* + type + value + size` |
| `PMIx_Data_copy` | MISSING | `dest* + src + type + size` |
| `PMIx_Data_print` | MISSING | `output** + prefix + type + value` |
| `PMIx_Data_copy_payload` | MISSING | `dest* + src*` |
| `PMIx_Data_load` | MISSING | `dest* + type + data + size` |
| `PMIx_Data_unload` | MISSING | `src* + type + data* + size*` |
| `PMIx_Data_compress` | MISSING | `inbytes + size + outbytes* + outsize*` |
| `PMIx_Data_decompress` | MISSING | `inbytes + size + outbytes* + outsize*` |

#### 6. Process Management (10)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Abort` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Spawn` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Spawn_nb` | MISSING | Non-blocking with callback |
| `PMIx_Connect` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Connect_nb` | MISSING | Non-blocking with callback |
| `PMIx_Disconnect` | COVERED | Safe wrapper in lib.rs |
| `PMIx_Disconnect_nb` | MISSING | Non-blocking with callback |
| `PMIx_Heartbeat` | MISSING | `void -> pmix_status_t` |

#### 7. Process Locality (7)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Load_topology` | MISSING | `topology* -> pmix_status_t` |
| `PMIx_Get_relative_locality` | MISSING | `locality1 + locality2 + locality*` |
| `PMIx_Parse_cpuset_string` | MISSING | `cpuset_string + cpuset*` |
| `PMIx_Get_cpuset` | MISSING | `cpuset* + ref` |
| `PMIx_Compute_distances` | MISSING | `topo + devs + ndevs + distances* + ndistances` |
| `PMIx_Compute_distances_nb` | MISSING | Non-blocking with callback |

#### 8. Job Management (8)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Allocation_request` | MISSING | `directive + info[] + targets + ntargets + nodelist + nnodes` |
| `PMIx_Allocation_request_nb` | MISSING | Non-blocking with callback |
| `PMIx_Job_control` | MISSING | `targets + ntargets + command + info[] + return_info* + nreturn_info` |
| `PMIx_Job_control_nb` | MISSING | Non-blocking with callback |
| `PMIx_Process_monitor` | MISSING | `monitor + events + nevents + info[]` |
| `PMIx_Process_monitor_nb` | MISSING | Non-blocking with callback |
| `PMIx_Log` | MISSING | `data[] + ndata + targets + ntargets + return_info* + nreturn_info` |
| `PMIx_Log_nb` | MISSING | Non-blocking with callback |

#### 9. Process Groups (10) — NEW in 4.1

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Group_construct` | MISSING | `grp + members + nmembers + info[]` |
| `PMIx_Group_construct_nb` | MISSING | Non-blocking with callback |
| `PMIx_Group_destruct` | MISSING | `grp + info[]` |
| `PMIx_Group_destruct_nb` | MISSING | Non-blocking with callback |
| `PMIx_Group_invite` | MISSING | `grp + invitees + ninvitees + info[]` |
| `PMIx_Group_invite_nb` | MISSING | Non-blocking with callback |
| `PMIx_Group_join` | MISSING | `grp + info[]` |
| `PMIx_Group_join_nb` | MISSING | Non-blocking with callback |
| `PMIx_Group_leave` | MISSING | `grp + info[]` |
| `PMIx_Group_leave_nb` | MISSING | Non-blocking with callback |

#### 10. Fabric Support (6) — NEW in 4.1

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Fabric_register` | MISSING | `fabric* + info[] + ninfo` |
| `PMIx_Fabric_register_nb` | MISSING | Non-blocking with callback |
| `PMIx_Fabric_update` | MISSING | `fabric*` |
| `PMIx_Fabric_update_nb` | MISSING | Non-blocking with callback |
| `PMIx_Fabric_deregister` | MISSING | `fabric*` |
| `PMIx_Fabric_deregister_nb` | MISSING | Non-blocking with callback |

#### 11. Security (4)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_Get_credential` | MISSING | `info[] + ninfo + targets + ntargets + cred*` |
| `PMIx_Get_credential_nb` | MISSING | Non-blocking with callback |
| `PMIx_Validate_credential` | MISSING | `cred* + info[] + ninfo` |
| `PMIx_Validate_credential_nb` | MISSING | Non-blocking with callback |

#### 12. IO Forwarding (3)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_IOF_pull` | MISSING | `procs + nprocs + channels + nchannels + info[] + deregister` |
| `PMIx_IOF_push` | MISSING | `targets + ntargets + channel + buf + info[]` |
| `PMIx_IOF_deregister` | MISSING | `iofhdlr + info[]` |

#### 13. Tool/Debugger APIs (7)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_tool_init` | MISSING | `proc* + info[] + ninfo` |
| `PMIx_tool_finalize` | MISSING | `void` |
| `PMIx_tool_disconnect` | MISSING | `server*` |
| `PMIx_tool_attach_to_server` | MISSING | `proc* + server + info[] + ninfo` |
| `PMIx_tool_get_servers` | MISSING | `servers** + nservers*` |
| `PMIx_tool_set_server` | MISSING | `server* + info[] + ninfo` |
| `PMIx_tool_connect_to_server` | MISSING | (internal, see spec) |

#### 14. Server-Specific APIs (20)

| Function | Status | Notes |
|----------|--------|-------|
| `PMIx_generate_regex` | MISSING | `input + output*` |
| `PMIx_generate_ppn` | MISSING | `input + ppn*` |
| `PMIx_server_register_nspace` | MISSING | `nspace + nprocs + procs + info[]` |
| `PMIx_server_deregister_nspace` | MISSING | `nspace` |
| `PMIx_server_register_resources` | MISSING | `info[] + ninfo + resources + nresources` |
| `PMIx_server_deregister_resources` | MISSING | `info[] + ninfo + resources + nresources` |
| `PMIx_server_register_client` | MISSING | `proc* + info[] + ninfo` |
| `PMIx_server_deregister_client` | MISSING | `proc*` |
| `PMIx_server_setup_fork` | MISSING | `proc* + info[] + ninfo + env*` |
| `PMIx_server_dmodex_request` | MISSING | `proc* + keys + nkeys + info[] + callback` |
| `PMIx_server_setup_application` | MISSING | `nspace + procs + nprocs + info[] + callback` |
| `PMIx_Register_attributes` | MISSING | `function + regattr + nregattr + info[]` |
| `PMIx_server_setup_local_support` | MISSING | `nspace + info[] + ninfo` |
| `PMIx_server_IOF_deliver` | MISSING | `source + channel + buf` |
| `PMIx_server_collect_inventory` | MISSING | `directives[] + ninfo + callback` |
| `PMIx_server_deliver_inventory` | MISSING | `info[] + ninfo + callback` |
| `PMIx_server_generate_locality_string` | MISSING | `cpuset* + output*` |
| `PMIx_server_generate_cpuset_string` | MISSING | `cpuset* + output*` |
| `PMIx_server_define_process_set` | MISSING | `members[] + nmembers + name + info[]` |
| `PMIx_server_delete_process_set` | MISSING | `pset_name` |

## Data Structures Required

### Already in lib.rs
- `PmixProc` (pmix_proc_t)
- `PmixInfo` (pmix_info_t)
- `PmixValue` (pmix_value_t)
- `PmixDataBuffer` (pmix_data_buffer_t)
- `PmixPdata` (pmix_pdata_t)
- `PmixQuery` (pmix_query_t)
- `PmixServerModule` (pmix_server_module_t — partial)

### Missing — Need Rust Types

| Spec Type | Fields | Notes |
|-----------|--------|-------|
| `pmix_app_t` | `cmd*, args**, env**, maxprocs, info[], ninfo, cwd*, personality*` | Spawn |
| `pmix_byte_object_t` | `bytes*, size` | Binary blob |
| `pmix_data_array_t` | `type_, array*, size` | Typed array |
| `pmix_proc_info_t` | `proc, state, host*, info[], ninfo` | Process info |
| `pmix_topology_t` | `name*, levels[], nlevels` | Topology |
| `pmix_cpuset_t` | `size, bitmap*` | CPU set bitmap |
| `pmix_bind_envelope_t` | enum | BIND_THREAD, BIND_LCORE, BIND_NUMA, BIND_PACKAGE, BIND_BOARD |
| `pmix_device_distance_t` | `devtype, min, max` | Device distance |
| `pmix_endpoint_t` | `name*, uri*` | Fabric endpoint |
| `pmix_coord_t` | `x, y, z` (u32) | Fabric coordinate |
| `pmix_geometry_t` | `xsize, ysize, zsize` (u32) | Fabric geometry |
| `pmix_fabric_t` | `devtype, endpoints[], endpoints_size, coords[], coords_size, geometry, link_state` | Fabric registration |
| `pmix_regattr_t` | `key*, type_, direction, description*` | Attribute registration |
| `pmix_server_module_t` | Full 4.1 struct (see below) | Server callbacks |

### Callback Types Required

| Spec Type | Signature | Notes |
|-----------|-----------|-------|
| `pmix_release_cbfunc_t` | `(status, name*, cbdata*) -> ()` | Release callback |
| `pmix_op_cbfunc_t` | `(status, cbdata*) -> ()` | Generic op callback |
| `pmix_value_cbfunc_t` | `(status, cbdata*, value*) -> ()` | Value callback |
| `pmix_info_cbfunc_t` | `(status, cbdata*, info[], ninfo) -> ()` | Info callback |
| `pmix_hdlr_reg_cbfunc_t` | `(status, evhdlr_ref*, cbdata*) -> ()` | Handler reg callback |
| `pmix_lookup_cbfunc_t` | `(status, cbdata*, info[], ninfo, pdata[], npdata) -> ()` | Lookup callback |
| `pmix_notification_fn_t` | `(evhdlr_code, evhdlr_ref, source, table_range, info[], cbdata, complete_fn) -> ()` | Event handler |
| `pmix_spawn_cbfunc_t` | `(status, cbdata*, pinfo[], npinfo) -> ()` | Spawn callback |
| `pmix_device_dist_cbfunc_t` | `(status, cbdata*, distances[], ndistances) -> ()` | Device distance callback |
| `pmix_dmodex_response_fn_t` | `(status, cbdata*, info[], ninfo, blob*) -> ()` | Dmodex response |
| `pmix_setup_application_cbfunc_t` | `(status, cbdata*, procs[], nprocs, info[], ninfo) -> ()` | Setup app callback |
| `pmix_modex_cbfunc_t` | `(status, cbdata*, info[], ninfo) -> ()` | Modex callback |
| `pmix_connection_cbfunc_t` | `(status, cbdata*) -> ()` | Connection callback |
| `pmix_server_tool_connection_fn_t` | `(server, info[], ninfo, cbdata, cbfunc) -> ()` | Tool connection |
| `pmix_tool_connection_cbfunc_t` | `(status, cbdata*) -> ()` | Tool connection callback |
| `pmix_credential_cbfunc_t` | `(status, cbdata*, cred*) -> ()` | Credential callback |
| `pmix_validation_cbfunc_t` | `(status, cbdata*, info[], ninfo) -> ()` | Validation callback |
| `pmix_iof_cbfunc_t` | `(status, cbdata*, iofhdlr*) -> ()` | IOF callback |

### Server Module Callbacks (v4x additions)

The `pmix_server_module_t` has these v4x additions not in the current code:
- `pmix_server_grp_fn_t` — group operations callback
- `pmix_server_fabric_fn_t` — fabric operations callback
- `pmix_server_client_connected2_fn_t` — updated client connected callback

## Enumerations Required

| Enum | Values | Notes |
|------|--------|-------|
| `pmix_status_t` | `PMIX_SUCCESS`, `PMIX_ERROR`, `PMIX_ERR_EXISTS`, `PMIX_ERR_EXISTS_OUTSIDE_SCOPE`, `PMIX_ERR_INVALID_CRED`, `PMIX_ERR_WOULD_BLOCK`, `PMIX_ERR_UNKNOWN_DATA_TYPE`, `PMIX_ERR_TYPE_MISMATCH`, `PMIX_ERR_UNPACK_INADEQUATE_SPACE`, `PMIX_ERR_UNPACK_READ_PAST_END_OF_BUFFER`, `PMIX_ERR_UNPACK_FAILURE`, `PMIX_ERR_PACK_FAILURE`, `PMIX_ERR_NO_PERMISSIONS`, `PMIX_ERR_TIMEOUT`, `PMIX_ERR_UNREACH`, `PMIX_ERR_BAD_PARAM`, `PMIX_ERR_EMPTY`, `PMIX_ERR_RESOURCE_BUSY`, `PMIX_ERR_OUT_OF_RESOURCE`, `PMIX_ERR_INIT`, `PMIX_ERR_NOMEM`, `PMIX_ERR_NOT_FOUND`, `PMIX_ERR_NOT_SUPPORTED`, `PMIX_ERR_PARAM_VALUE_NOT_SUPPORTED`, `PMIX_ERR_COMM_FAILURE`, `PMIX_ERR_LOST_CONNECTION`, `PMIX_ERR_INVALID_OPERATION`, `PMIX_OPERATION_IN_PROGRESS`, `PMIX_OPERATION_SUCCEEDED`, `PMIX_ERR_PARTIAL_SUCCESS` | 30 status codes, values are implementation-defined (only PMIX_SUCCESS=0 is fixed) |
| `pmix_scope_t` | `PMIX_SCOPE_UNDEF, PMIX_LOCAL, PMIX_SESSION, PMIX_NAMESPACE, PMIX_GLOBAL` | Data scope |
| `pmix_persistence_t` | `PMIX_PERSIST_UNDEF, PMIX_PERSIST_SESSION, PMIX_PERSIST_APP, PMIX_PERSIST_PROC, PMIX_PERSIST_RANGE` | Data persistence |
| `pmix_data_range_t` | `PMIX_RANGE_UNDEF, PMIX_RANGE_PROC_LOCAL, PMIX_RANGE_NODE, PMIX_RANGE_WHOLE_HOST, PMIX_RANGE_NAMESPACE, PMIX_RANGE_SESSION, PMIX_RANGE_GLOBAL` | Data range |
| `pmix_info_directives_t` | `PMIX_INFO_DIRECTIVE_UNDEF, PMIX_INFO_DIRECTIVE_REQUIRED, PMIX_INFO_DIRECTIVE_OPTIONAL` | Info directives |
| `pmix_data_type_t` | 40+ types (see spec section 3.3) | Data packing types |
| `pmix_proc_state_t` | `PMIX_PROC_STATE_UNDEF, PMIX_PROC_STATE_READY_TO_LAUNCH, PMIX_PROC_STATE_LAUNCH_NEEDED, PMIX_PROC_STATE_RUNNING, PMIX_PROC_STATE_TERMINATED, PMIX_PROC_STATE_ABORTED, PMIX_PROC_STATE_FAILED_TO_START, PMIX_PROC_STATE_RUNTIME_FAILURE, PMIX_PROC_STATE_THREAD_BEGIN, PMIX_PROC_STATE_THREAD_END, PMIX_PROC_STATE_COMPUTING, PMIX_PROC_STATE_COMMUNICATING, PMIX_PROC_STATE_WAITING, PMIX_PROC_STATE_SLEEPING, PMIX_PROC_STATE_SUSPENDED, PMIX_PROC_STATE_RESTARTING, PMIX_PROC_STATE_RESTARTED, PMIX_PROC_STATE_CHECKPOINTED, PMIX_PROC_STATE_CHECKPOINTING, PMIX_PROC_STATE_MIGRATING, PMIX_PROC_STATE_MIGRATED` | Process states |
| `pmix_iof_channel_t` | `PMIX_IOF_STDOUT_CHANNEL, PMIX_IOF_STDERR_CHANNEL, PMIX_IOF_STDIN_CHANNEL` | IO channels |
| `pmix_alloc_directive_t` | `PMIX_ALLOC_DIRECTIVE_NONE, PMIX_ALLOC_DIRECTIVE_REPLACE, PMIX_ALLOC_DIRECTIVE_ADD, PMIX_ALLOC_DIRECTIVE_REMOVE` | Allocation directives |
| `pmix_job_state_t` | Various job states (see spec) | Job states |
| `pmix_device_type_t` | Bitmask: GPU, NIC, storage, etc. | Device types |
| `pmix_coord_view_t` | `PMIX_COORD_VIEW_UNDEF, PMIX_COORD_VIEW_1D, PMIX_COORD_VIEW_2D, PMIX_COORD_VIEW_3D` | Fabric coordinate view |
| `pmix_link_state_t` | `PMIX_LINK_STATE_UNDEF, PMIX_LINK_STATE_UP, PMIX_LINK_STATE_DOWN, PMIX_LINK_STATE_DEGRADED` | Fabric link state |

## Summary of Work Required

### Phase 1: FFI Bindings (ffi.rs)
Add all ~100 FFI function declarations. Group by category:
1. Utility/string conversion functions (16)
2. Core lifecycle (5)
3. Data collection (16)
4. Data packing/unpacking (9)
5. Process management (10)
6. Process locality (7)
7. Job management (8)
8. Process groups (10)
9. Fabric support (6)
10. Security (4)
11. IO forwarding (3)
12. Tool APIs (7)
13. Server APIs (20)

### Phase 2: Data Types (types.rs)
Create new Rust types for all missing structures:
- `PmixApp`, `PmixByteObject`, `PmixDataArray`, `PmixProcInfo`
- `PmixTopology`, `PmixCpuset`, `PmixDeviceDistance`
- `PmixEndpoint`, `PmixCoord`, `PmixGeometry`, `PmixFabric`
- `PmixRegAttr`, `PmixBindEnvelope`

### Phase 3: Enumerations (enums.rs)
Define all PMIx enums as Rust enums with proper repr:
- `PmixStatus`, `PmixScope`, `PmixPersistence`, `PmixDataRange`
- `PmixInfoDirective`, `PmixDataType`, `PmixProcState`
- `PmixIOFChannel`, `PmixAllocDirective`, `PmixJobState`
- `PmixDeviceType`, `PmixCoordView`, `PmixLinkState`

### Phase 4: Callback Types (callbacks.rs)
Define FFI-safe callback types with proper `extern "C"` signatures:
- 18 callback types identified above

### Phase 5: Safe Wrappers (lib.rs)
Add safe wrappers for all ~80 missing functions. Prioritize:
1. Non-blocking variants of existing functions
2. Publish/Lookup/Unpublish operations
3. Query operations
4. IO forwarding
5. Tool APIs
6. Server support functions

### Phase 6: Server Module Completion
Complete `PmixServerModule` to include v3x and v4x callbacks:
- `get_credential`, `validate_credential`
- `iof_pull`, `push_stdin`
- `group`, `fabric`, `client_connected2`

## Priority Recommendations

**High priority** (core functionality):
1. Non-blocking variants of existing wrappers (Fence_nb, Get_nb, Connect_nb, Disconnect_nb, Spawn_nb)
2. Publish/Lookup/Unpublish (blocking + non-blocking)
3. Query_info / Query_info_nb
4. Data packing/unpacking (9 functions)
5. IO forwarding (3 functions)

**Medium priority** (advanced features):
6. Process groups (10 functions) — new in 4.1
7. Process locality (7 functions)
8. Job management (8 functions)
9. Tool APIs (7 functions)

**Lower priority** (specialized):
10. Fabric support (6 functions) — new in 4.1
11. Security (4 functions)
12. Server module completion
13. Utility/string functions (16 functions)
