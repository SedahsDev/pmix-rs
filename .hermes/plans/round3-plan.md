# Round 3 Batched Test Expansion Plan for pmix-rs

Based on the coverage analysis, I'm targeting the **7 lowest-coverage modules** (fabric, data_serialization, monitoring, query_log, security, data_ops, groups) plus **allocation** (high line/test ratio gap) and **tool** (test count vs coverage mismatch). Total: **9 batches**.

---

## Batch 1: fabric — FFI Boundary & Error Code Exhaustion
**Target Module:** `fabric.rs` (29 pub fns, 608 lines, **42.76%** → target 55%+)  
**Current Tests:** 487 tests / 13 files — high count but low coverage = FFI-heavy untested paths

### Functions to Test (verify against 29 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `fabric_init` | lifecycle | Requires init — marked `#[ignore]` |
| `fabric_finalize` | lifecycle | Requires init — marked `#[ignore]` |
| `fabric_get_devices` | query | Returns `Vec<PmixFabricDevice>` — untested iteration |
| `fabric_get_device_attr` | query | Attribute enum variants untested |
| `fabric_open_device` | resource | Returns handle — Drop untested |
| `fabric_close_device` | resource | Double-free risk if tested wrong |
| `fabric_create_vni` | vni | VNI attributes untested |
| `fabric_delete_vni` | vni | Cleanup path untested |
| `fabric_get_vni_attr` | vni | Enum coverage incomplete |
| `fabric_get_endpoints` | endpoint | Vec iteration untested |
| `fabric_get_endpoint_attr` | endpoint | Attr variants untested |
| `fabric_connect` | connection | Async callback — `on_complete` untested |
| `fabric_accept` | connection | Server-side callback untested |
| `fabric_disconnect` | connection | Cleanup callback untested |
| `fabric_send` | data | Buffer ownership transfer untested |
| `fabric_recv` | data | Callback with `Box<Self>` untested |
| `fabric_get_opt` / `fabric_set_opt` | options | Opt enum variants untested |

### Test Strategy:
1. **Compile-time type checks**: `Send`/`Sync` on `PmixFabricDevice`, `PmixVni`, `PmixEndpoint`, `PmixFabricHandle` — verify `!Send`/`!Sync` where expected
2. **Panic safety**: `catch_unwind(AssertUnwindSafe(...))` on all public fn — verify no `SIGSEGV` on uninit calls
3. **Error code exhaustion**: Call each fn without `PMIx_Init` → verify `Err(PmixError::ERR_NOT_INITIALIZED)` or `#[ignore]`
4. **Callback trait verification**: `FabricConnectCallback`, `FabricRecvCallback` — verify `on_complete(self: Box<Self>, status: PmixError, ...)` signatures compile
5. **Parameter validation**: Empty strings, NUL bytes in device names, VNI names — verify `Err(PmixError::ERR_BAD_PARAM)` not FFI passthrough
6. **Drop impl verification**: `fabric_open_device` → drop handle → verify no double-free (no explicit `close` call)
7. **Attribute enum roundtrip**: `PmixFabricDeviceAttr::*` / `PmixVniAttr::*` / `PmixEndpointAttr::*` — `from_raw`/`to_raw` coverage

### Estimated Tests: **52 new tests** (2 per fn × 26 testable fns + enum coverage)

### Coverage Gap Analysis:
Current tests focus on happy-path serialization. **Untested**: all error returns from FFI, callback `on_complete` execution paths, Drop impls, attribute enum variants beyond 2-3 common ones, parameter validation at Rust boundary.

---

## Batch 2: data_serialization — Buffer Ownership & Type Erasure
**Target Module:** `data_serialization.rs` (23 pub fns, 343 lines, **43.44%** → target 55%+)  
**Current Tests:** 627 tests / 13 files — most tests but lowest coverage = deep FFI paths untested

### Functions to Test (verify against 23 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `PmixDataBuffer::new` | constructor | Default capacity untested |
| `PmixDataBuffer::with_capacity` | constructor | Capacity edge cases (0, huge) untested |
| `PmixDataBuffer::pack` | pack | All `PmixDataType` variants — only ~8 tested |
| `PmixDataBuffer::unpack` | unpack | Type mismatch error paths untested |
| `PmixDataBuffer::load` / `unload` | buffer mgmt | Ownership transfer untested |
| `PmixDataBuffer::get_bytes` / `set_bytes` | buffer mgmt | `PmixByteObject` lifecycle untested |
| `PmixByteObject::new` / `from_raw` | constructor | `drop` impl untested |
| `PmixValue::new` / `from_raw` | constructor | All `PmixValueType` variants — partial coverage |
| `pack_val` / `unpack_val` | value ops | Nested value types (arrays, structs) untested |
| `pack_proc` / `unpack_proc` | proc ops | `Proc` non-Copy/non-Debug constraints untested |
| `pack_info` / `unpack_info` | info ops | `PmixInfo` array packing untested |
| `pack_data_buffer` / `unpack_data_buffer` | nested | Buffer-in-buffer recursion untested |

### Test Strategy:
1. **Constructor/accessor verification**: `PmixDataBuffer::new()` → `capacity()`, `used()`, `bytes()` — verify initial state
2. **Type erasure exhaustiveness**: Test `pack`/`unpack` for **ALL** `PmixDataType` variants (PMIX_INT, UINT, INT8, UINT8, ..., PMIX_DATA_BUFFER, PMIX_BYTE_OBJECT, PMIX_PROC, PMIX_INFO, PMIX_PDATA, etc.) — current tests cover ~8/20+
3. **Buffer ownership transfer**: `load`/`unload` → verify `PmixByteObject` Drop runs exactly once; `set_bytes` with owned vs borrowed
4. **Panic safety**: `catch_unwind` on `unpack` with undersized buffer, type mismatch — verify panic not SIGSEGV
5. **Error code validation**: `unpack` past end → `ERR_UNPACK_READ_PAST_END`; type mismatch → `ERR_TYPE_MISMATCH`; verify raw values
5. **Non-Send/Sync verification**: `static_assertions::assert_not_impl_any!(PmixDataBuffer: Send, Sync)` — compile-time
6. **Nested packing**: `pack_data_buffer` containing packed `PmixInfo` array → unpack roundtrip
7. **Capacity growth**: Pack until reallocation → verify data integrity

### Estimated Tests: **68 new tests** (3 per `PmixDataType` variant × 20+ types + buffer lifecycle)

### Coverage Gap Analysis:
627 tests exist but focus on `pack`/`unpack` happy paths for primitive types. **Untested**: all error returns, `PmixByteObject` Drop, buffer ownership transfer (`load`/`unload`/`set_bytes`), nested buffer packing, capacity growth paths, `PmixValue` variant exhaustiveness.

---

## Batch 3: monitoring — Callback Lifecycle & Init Guards
**Target Module:** `monitoring.rs` (5 pub fns, 142 lines, **54.93%** → target 65%+)  
**Current Tests:** 70 tests / 3 files — few functions, low coverage = FFI callbacks untested

### Functions to Test (verify against 5 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `monitor_register` | registration | Callback `on_complete(&mut self, status, ...)` — unique signature |
| `monitor_deregister` | registration | Handle cleanup untested |
| `monitor_control` | control | Command enum variants untested |
| `monitor_collect` | collection | `CollectInventoryCallback` takes `&self` not `Box<Self>` — unique |
| `monitor_event_register` | events | Event callback untested |

### Test Strategy:
1. **Callback signature verification**: Compile-time check `MonitorRegisterCallback::on_complete(&mut self, status: PmixError, data: Vec<PmixInfo>)` — differs from standard `Box<Self>` pattern
2. **CollectInventoryCallback verification**: `on_complete(&self, status, inventory: Vec<PmixInfo>)` — `&self` not `Box<Self>`
3. **Init guard tests**: Call each fn without init → verify `Err(PmixError::ERR_NOT_INITIALIZED)` — mark `#[ignore]` if SIGSEGV risk
4. **Parameter validation**: Empty nspace, NUL bytes in event names → `ERR_BAD_PARAM`
5. **Handle lifecycle**: `monitor_register` returns handle → drop → verify deregister not double-called
6. **Command enum coverage**: `PmixMonitorCmd::START`, `STOP`, `PAUSE`, `RESUME`, `RESET` — all variants
7. **Panic safety**: `catch_unwind` on callback registration with panicking closure

### Estimated Tests: **22 new tests** (4-5 per fn × 5 fns + enum coverage)

### Coverage Gap Analysis:
Only 5 public functions but callbacks have **unique signatures** (`&mut self`, `&self` vs standard `Box<Self>`). Current tests likely don't exercise callback execution paths or error returns from FFI layer.

---

## Batch 4: query_log — Query Options & Log Filtering
**Target Module:** `query_log.rs` (8 pub fns, 258 lines, **54.26%** → target 65%+)  
**Current Tests:** 69 tests / 5 files

### Functions to Test (verify against 8 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `query` | query | `PmixQuery` struct construction untested |
| `query_nb` | query | Non-blocking callback untested |
| `log` | log | `PmixLog` severity/facility enum coverage |
| `log_nb` | log | Non-blocking callback untested |
| `query_result_free` | cleanup | Drop impl vs explicit free — double-free risk |
| `log_result_free` | cleanup | Same |
| `get_query_options` | options | Option enum variants untested |
| `set_query_options` | options | Validation untested |

### Test Strategy:
1. **PmixQuery construction**: All fields — `keys`, `qualifiers`, `nspace`, `rank` — verify builder pattern
2. **Qualifier enum exhaustiveness**: `PmixQueryQualifier::*` — all variants tested in query
3. **Non-blocking callbacks**: `QueryCallback::on_complete(self: Box<Self>, status, results: Vec<PmixQueryResult>)` — verify signature
3. **Log severity/facility**: `PmixLogSeverity::*` (EMERG...DEBUG), `PmixLogFacility::*` — all variants
4. **Result ownership**: `query_nb` callback receives `Vec<PmixQueryResult>` — verify Drop order
5. **Error codes**: Query with invalid nspace → `ERR_NSPACE_NOT_FOUND`; log with bad severity → `ERR_BAD_PARAM`
6. **Parameter validation**: NUL bytes in log message, empty keys → `ERR_BAD_PARAM`
7. **Compile-time**: `assert_not_impl_any!(PmixQueryResult: Send, Sync)`

### Estimated Tests: **35 new tests** (4-5 per fn + enum exhaustiveness)

### Coverage Gap Analysis:
Query/log functions have complex option/qualifier enums. Current tests likely use 1-2 variants. Non-blocking callback paths and result cleanup (Drop vs explicit free) untested.

---

## Batch 5: security — Credential Types & Auth Verification
**Target Module:** `security.rs` (17 pub fns, 354 lines, **56.21%** → target 65%+)  
**Current Tests:** 162 tests / 5 files

### Functions to Test (verify against 17 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `get_credential` | cred | All `PmixCredentialType` variants — partial |
| `validate_cred` | cred | Validation error paths untested |
| `get_security_info` | info | Info key coverage incomplete |
| `set_security_info` | info | Write path untested |
| `authenticate` | auth | Callback `on_complete(self: Box<Self>, status, cred)` untested |
| `revoke_auth` | auth | Revocation path untested |
| `get_my_cred` | cred | Self-credential untested |
| `sign_data` / `verify_signature` | crypto | Crypto path untested |
| `encrypt_data` / `decrypt_data` | crypto | Buffer ownership untested |

### Test Strategy:
1. **Credential type exhaustiveness**: `PmixCredentialType::*` (PMIX_CRED_X509, PMIX_CRED_JWT, PMIX_CRED_PEM, ...) — all variants in `get_credential`/`validate_cred`
2. **Callback verification**: `AuthCallback::on_complete(self: Box<Self>, status: PmixError, cred: PmixByteObject)` — compile-time
3. **Crypto buffer ownership**: `sign_data` takes `&[u8]` returns `PmixByteObject` — verify Drop; `verify_signature` takes owned `PmixByteObject`
4. **Encrypt/decrypt roundtrip**: `encrypt_data` → `decrypt_data` → verify original; test empty input, large input
5. **Error codes**: Invalid cred type → `ERR_NOT_SUPPORTED`; bad signature → `ERR_VERIFY_FAILED`; verify raw values
6. **Parameter validation**: NUL in credential data, empty buffers → `ERR_BAD_PARAM`
7. **Non-Copy verification**: `PmixByteObject` in cred — `assert_not_impl_any!(PmixByteObject: Copy)`

### Estimated Tests: **48 new tests** (cred types × 10+ variants + crypto roundtrips + error paths)

### Coverage Gap Analysis:
Security module has crypto and credential paths that require specific FFI setup. Current tests likely cover basic get/set. **Untested**: all credential type variants, crypto operations, auth callback execution, revocation, error returns from FFI.

---

## Batch 6: data_ops — Value Comparison & Arithmetic
**Target Module:** `data_ops.rs` (11 pub fns, 494 lines, **56.28%** → target 65%+)  
**Current Tests:** 188 tests / 10 files

### Functions to Test (verify against 11 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `value_compare` | compare | All `PmixValueType` + `PmixComparisonOp` combos untested |
| `value_compare2` | compare | Two-value comparison edge cases |
| `value_add` / `value_sub` | arithmetic | Overflow, type mismatch, non-numeric types |
| `value_mul` / `value_div` | arithmetic | Same |
| `value_mod` | arithmetic | Modulo edge cases |
| `value_xor` / `value_or` / `value_and` | bitwise | Only integer types valid — error paths untested |
| `value_invert` | bitwise | Unary op, type validation |

### Test Strategy:
1. **Comparison operator exhaustiveness**: `PmixComparisonOp::*` (EQ, NE, LT, LE, GT, GE) × all comparable `PmixValueType` variants
2. **Arithmetic type validation**: `value_add` on strings → `ERR_TYPE_MISMATCH`; on floats → verify precision
3. **Overflow behavior**: `value_add` max uint64 + 1 → verify defined behavior (wrap? error?)
4. **Bitwise type restriction**: `value_xor` on float → `ERR_TYPE_MISMATCH`; verify error code
5. **PmixValue construction**: Test all `PmixValueType` variants as inputs — current tests likely use subset
6. **Error code validation**: Raw `PMIX_ERR_*` values from FFI → `PmixError::from_raw` roundtrip
7. **Compile-time**: `PmixValue` is `!Send`/`!Sync` — verify

### Estimated Tests: **44 new tests** (11 fns × 4 type/op combos)

### Coverage Gap Analysis:
Data ops are pure-Rust wrappers around FFI but current tests cover basic arithmetic. **Untested**: full comparison operator matrix, arithmetic on all numeric types, bitwise type restrictions, overflow semantics, error code propagation from FFI.

---

## Batch 7: groups — Group Lifecycle & Membership Edge Cases
**Target Module:** `groups.rs` (15 pub fns, 491 lines, **59.06%** → target 68%+)  
**Current Tests:** 359 tests / 12 files — good count but gaps in membership ops

### Functions to Test (verify against 15 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `group_construct` | lifecycle | Empty nspace, NUL in group name |
| `group_destruct` | lifecycle | Double-destruct, uninitialized |
| `group_invite` | membership | Invite callback untested |
| `group_join` | membership | Join callback, already-member error |
| `group_leave` | membership | Leave callback, not-member error |
| `group_get_members` | query | Empty group, large group iteration |
| `group_get_info` | query | Info keys coverage |
| `group_add_members` / `delete_members` | bulk | Bulk ops partial failure untested |
| `group_fence` / `group_fence_nb` | sync | Fence callback, timeout handling |

### Test Strategy:
1. **Constructor validation**: `group_construct` with empty nspace, NUL bytes, unicode group names → `ERR_BAD_PARAM`
2. **Membership callback signatures**: `GroupInviteCallback::on_complete(self: Box<Self>, status, ...)` — verify all 3 callbacks
3. **Bulk operation partial failure**: `group_add_members` with mix of valid/invalid ranks → verify partial success error
4. **Fence callback**: `GroupFenceCallback::on_complete(self: Box<Self>, status, data: Vec<PmixByteObject>)` — verify signature
5. **Error codes**: Join existing group → `ERR_ALREADY_MEMBER`; leave non-member → `ERR_NOT_A_MEMBER`; fence timeout → `ERR_TIMEOUT`
6. **Large group iteration**: `group_get_members` with 1000+ procs — verify `Vec<Proc>` construction
7. **Drop safety**: Group handle Drop → verify no double-destruct

### Estimated Tests: **42 new tests** (focus on error paths, callbacks, bulk ops)

### Coverage Gap Analysis:
359 tests exist but focus on happy-path group create/join/leave. **Untested**: invite/join/leave callbacks, bulk ops with partial failure, fence synchronization, error codes for membership state violations, large group handling.

---

## Batch 8: allocation — Sparse Test Coverage Deep Dive
**Target Module:** `allocation.rs` (12 pub fns, 367 lines, **74.39%** → target 80%+)  
**Current Tests:** **35 tests / 3 files / 533 lines** — **lowest test density** (0.095 tests/line vs avg 0.48)

### Functions to Test (verify against 12 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `allocation_get` | query | All `PmixAllocDirective` keys untested |
| `allocation_get_nb` | query | Non-blocking callback untested |
| `allocation_free` | cleanup | Double-free, use-after-free |
| `allocation_modify` | modify | Directive validation untested |
| `get_allocation_id` | query | ID format validation |
| `get_allocation_info` | query | Info key coverage |
| `register_allocation_event` | events | Event callback untested |
| `deregister_allocation_event` | events | Deregistration path |
| `allocation_request` | request | Request callback, directives |
| `allocation_request_nb` | request | Non-blocking request callback |
| `get_allocation_directives` | directives | Directive enum exhaustiveness |
| `validate_allocation` | validate | Validation error paths |

### Test Strategy:
1. **Directive enum exhaustiveness**: `PmixAllocDirective::*` (NODE_LIST, PROC_MAP, TIME_LIMIT, QUEUE, ACCOUNT, PARTITION, ...) — all variants in get/modify/request
2. **Non-blocking callbacks**: `AllocationRequestCallback::on_complete(self: Box<Self>, status, allocation_id)` — verify signature
3. **Event callback**: `AllocationEventCallback::on_complete(&mut self, status, event_code, info)` — `&mut self` pattern
4. **Parameter validation**: Empty allocation ID, NUL in directives → `ERR_BAD_PARAM`
5. **Error codes**: Free invalid ID → `ERR_NOT_FOUND`; modify freed allocation → `ERR_BAD_PARAM`; request with conflicting directives → `ERR_CONFLICT`
5. **Allocation ID format**: Verify returned ID matches expected pattern (nspace + UUID)
6. **Compile-time**: Allocation handle `!Send`/`!Sync`; callback traits `Send`
7. **Drop vs explicit free**: `allocation_free` vs handle Drop — verify exactly one cleanup

### Estimated Tests: **55 new tests** (12 fns × 4-5 coverage points — highest ROI batch)

### Coverage Gap Analysis:
**Only 35 tests for 12 public functions** — ~3 tests/fn vs 15-20 avg. Entire directive enum space, non-blocking paths, event callbacks, and error returns are essentially untested. This is the **highest impact batch** for coverage improvement.

---

## Batch 9: tool — Connection Lifecycle & Info Gaps
**Target Module:** `tool.rs` (13 pub fns, 179 lines, **67.04%** → target 75%+)  
**Current Tests:** 225 tests / 9 files — high count but coverage gap = specific paths missed

### Functions to Test (verify against 13 pub fns):
| Function | Category | Why Untested |
|----------|----------|--------------|
| `tool_connect_to_server` | connection | Connection callback, timeout |
| `tool_disconnect` | connection | Cleanup, double-disconnect |
| `tool_get_server_info` | info | Info key coverage |
| `tool_init` / `tool_finalize` | lifecycle | Init guard, double-init |
| `tool_check_connection` | health | Connection state enum |
| `tool_ping` | health | Ping callback untested |
| `tool_log` / `tool_log_nb` | log | Log from tool context |
| `tool_iof_push` / `tool_iof_pull` | iof | IO forwarding callbacks |
| `tool_iof_close` | iof | Close callback |

### Test Strategy:
1. **Connection callback**: `ToolConnectCallback::on_complete(self: Box<Self>, status, server_info: PmixServerInfo)` — verify signature
2. **Ping callback**: `ToolPingCallback::on_complete(self: Box<Self>, status, latency)` — verify
3. **IOF callbacks**: `ToolIofCallback::on_complete(self: Box<Self>, status, data: PmixByteObject)` — stdin/stdout/stderr
4. **Init/finalize guards**: Double `tool_init` → `ERR_ALREADY_INITIALIZED`; `tool_finalize` without init → `ERR_NOT_INITIALIZED`
5. **Server info keys**: `PmixServerInfoKey::*` — all variants in `tool_get_server_info`
6. **Connection state**: `tool_check_connection` returns `PmixToolConnectionState` — all variants
7. **Error codes**: Connect to invalid URI → `ERR_UNREACH`; disconnect unconnected → `ERR_BAD_PARAM`
8. **Parameter validation**: Empty URI, NUL bytes in URI → `ERR_BAD_PARAM`

### Estimated Tests: **38 new tests** (callbacks + info keys + error paths)

### Coverage Gap Analysis:
225 tests but likely focused on basic connect/disconnect. **Untested**: all callback `on_complete` execution paths, IO forwarding (push/pull/close), server info key exhaustiveness, connection state enum, ping latency measurement, tool-specific log path.

---

## Summary: Round 3 Impact Projection

| Batch | Module | Current Cov | Target | Est. Tests | Key Gap |
|-------|--------|-------------|--------|------------|---------|
| 1 | fabric | 42.8% | 55%+ | 52 | FFI error returns, callback execution, Drop |
| 2 | data_serialization | 43.4% | 55%+ | 68 | Type erasure exhaustiveness, buffer ownership |
| 3 | monitoring | 54.9% | 65%+ | 22 | Unique callback signatures (`&mut self`, `&self`) |
| 4 | query_log | 54.3% | 65%+ | 35 | Qualifier enums, non-blocking callbacks |
| 5 | security | 56.2% | 65%+ | 48 | Credential types, crypto, auth callbacks |
| 6 | data_ops | 56.3% | 65%+ | 44 | Comparison/arithmetic operator matrices |
| 7 | groups | 59.1% | 68%+ | 42 | Membership callbacks, bulk ops, fence |
| 8 | allocation | 74.4% | 80%+ | **55** | **Sparse test density — directive enums, callbacks** |
| 9 | tool | 67.0% | 75%+ | 38 | IOF callbacks, server info keys, connection state |

**Total: ~404 new tests** across 9 batches  
**Projected coverage lift**: +5-8% on targeted modules → **overall ~71-73%** (from 68.04%)

### Priority Order:
1. **Batch 8 (allocation)** — highest ROI (35 tests for 367 lines)
2. **Batch 1 (fabric)** — lowest coverage, high line count
3. **Batch 2 (data_serialization)** — lowest coverage, many tests but many tests = deep FFI gaps
4. **Batch 5 (security)** — credential/crypto paths completely untested
5. **Batch 7 (groups)** — callback/bulk op gaps
6. **Batch 4 (query_log)** — qualifier enum coverage
7. **Batch 6 (data_ops)** — operator matrix
8. **Batch 9 (tool)** — IOF/connection state
9. **Batch 3 (monitoring)** — smallest module, unique callbacks

---

## FFI Safety Checklist for All Batches
- [ ] Every test calling FFI without `PMIx_Init` marked `#[ignore]`
- [ ] No explicit `drop`/`free`/`release` calls — rely on `Drop` impls
- [ ] `catch_unwind(AssertUnwindSafe(...))` for all panic tests on non-Copy types
- [ ] `PmixServerHandle`, `PmixCpuset`, `PmixByteObject` — never `Clone`/`Copy` in tests
- [ ] Callback `on_complete` signatures verified at compile-time
- [ ] Empty string / NUL byte tests expect `Err(PmixError::ERR_BAD_PARAM)` not FFI passthrough
- [ ] `server_deregister_nspace`/`server_deregister_client` tested for `()` return (not `Result`)
