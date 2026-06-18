## Phase 4 Test Expansion Plan for pmix-rs

### Batch 1: fabric.rs — PmixFabric Core Lifecycle
- **Target:** Push fabric.rs from 41.23% to 55% line coverage
- **Estimated tests:** 24 new tests across 3 files
- **Test files to create/extend:**
  - `tests/fabric_Fabric_construction.rs` — 8 tests covering PmixFabric::new/unamed/name/index/is_registered/ninfo
  - `tests/fabric_Fabric_register_basic.rs` — 8 tests covering fabric_register/deregister (blocking) with error validation
  - `tests/fabric_Fabric_update_basic.rs` — 8 tests covering fabric_update (blocking) with error validation
- **Coverage targets:**
  - [ ] PmixFabric::new: test named/unamed construction
  - [ ] PmixFabric::name/index/is_registered/ninfo: test getters on constructed objects
  - [ ] fabric_register: test success path (no server) and ERR_NOENT error path
  - [ ] fabric_deregister: test success path and ERR_NOENT error path
  - [ ] fabric_update: test success path and ERR_NOENT error path
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for PmixFabric
  - [ ] Panic safety: null name input, empty ninfo array
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized fabric ops
  - [ ] Callback trait implementations: NopCallback for nb variants (to be covered in next batch)
- **Ignored tests:** 0 tests (all user-space testable)
- **Estimated coverage gain:** +13.75% lines

### Batch 2: fabric.rs — PmixFabric Non-Blocking Operations
- **Target:** Push fabric.rs from 55% to 65% line coverage
- **Estimated tests:** 18 new tests across 2 files
- **Test files to create/extend:**
  - `tests/fabric_Fabric_register_nb.rs` — 10 tests covering fabric_register_nb/deregister_nb with callback validation
  - `tests/fabric_Fabric_update_nb.rs` — 8 tests covering fabric_update_nb with callback validation
- **Coverage targets:**
  - [ ] fabric_register_nb: test callback invocation with success/error codes
  - [ ] fabric_deregister_nb: test callback invocation with success/error codes
  - [ ] fabric_update_nb: test callback invocation with success/error codes
  - [ ] Callback wrapper validation: proper lifetime and FnOnce handling
- **Key patterns:**
  - [ ] Callback trait implementations: RecordingCallback with Arc<Mutex<>> for async verification
  - [ ] Panic safety: null callback, null directives
  - [ ] Error code verification: PMIX_OPERATION_SUCCEEDED vs PMIX_ERR_BAD_PARAM
  - [ ] Compile-time type checks: Callback trait bounds (Send + 'static)
- **Ignored tests:** 12 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +10% lines

### Batch 3: fabric.rs — Topology and Distance Structures
- **Target:** Push fabric.rs from 65% to 75% line coverage
- **Estimated tests:** 22 new tests across 3 files
- **Test files to create/extend:**
  - `tests/fabric_Topology_basic.rs` — 8 tests covering PmixTopology::new/unamed/source/is_loaded
  - `tests/fabric_Cpuset_basic.rs` — 8 tests covering PmixCpuset::new/as_mut_ptr/uuid/osname/device_type
  - `tests/fabric_DeviceDistance_basic.rs` — 6 tests covering PmixDeviceDistance::mindist/maxdist/distances
- **Coverage targets:**
  - [ ] PmixTopology: construction, source getter, is_loaded state
  - [ ] PmixCpuset: construction, mutator, UUID/OS/device type getters
  - [ ] PmixDeviceDistance: mindist/maxdist getters, distances slice access
  - [ ] DeviceDistances: len/is_empty implementation
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for all structs
  - [ ] Panic safety: null pointer handling in as_mut_ptr
  - [ ] Edge cases: empty uuid/osname strings, zero distances
  - [ ] Debug trait verification: fmt::Debug implementation
- **Ignored tests:** 0 tests
- **Estimated coverage gain:** +10% lines

### Batch 4: fabric.rs — Topology Operations
- **Target:** Push fabric.rs from 75% to 82% line coverage
- **Estimated tests:** 16 new tests across 2 files
- **Test files to create/extend:**
  - `tests/fabric_Load_topology.rs` — 8 tests covering load_topology (blocking/nb) with error validation
  - `tests/fabric_Compute_distances.rs` — 8 tests covering compute_distances (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] load_topology: test success path (no server) and ERR_NOENT error path
  - [ ] load_topology_nb: test callback invocation with proper error codes
  - [ ] compute_distances: test success path and ERR_NOENT error path
  - [ ] compute_distances_nb: test callback invocation with distance results
- **Key patterns:**
  - [ ] Callback trait implementations: ComputeDistancesCallback with result validation
  - [ ] Panic safety: null topology/cpuset pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized fabric
  - [ ] Compile-time type checks: topology/cpuset lifetime constraints
- **Ignored tests:** 12 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +7% lines

### Batch 5: data_serialization.rs — Core Buffer Operations
- **Target:** Push data_serialization.rs from 41.05% to 52% line coverage
- **Estimated tests:** 20 new tests across 3 files
- **Test files to create/extend:**
  - `tests/data_serialization_PmixByteObject.rs` — 8 tests covering PmixByteObject construction/accessors
  - `tests/data_serialization_PmixDataBuffer.rs` — 8 tests covering PmixDataBuffer construction/accessors
  - `tests/data_serialization_Data_copy.rs` — 4 tests covering data_copy/data_copy_payload (user-space validation)
- **Coverage targets:**
  - [ ] PmixByteObject: new(), as_slice(), size(), is_empty(), as_mut_ptr()
  - [ ] PmixDataBuffer: new(), as_mut_ptr(), is_valid(), bytes_allocated(), bytes_used()
  - [ ] data_copy: test type validation and buffer size calculation
  - [ ] data_copy_payload: test pointer arithmetic and length calculation
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for buffer types
  - [ ] Panic safety: null ptr handling in as_mut_ptr, zero-size buffers
  - [ ] Edge cases: max usize buffer sizes, unaligned pointers
  - [ ] Memory safety: validate no double-free in construction/destruction
- **Ignored tests:** 0 tests
- **Estimated coverage gain:** +10.95% lines

### Batch 6: data_serialization.rs — Pack/Unpack Operations
- **Target:** Push data_serialization.rs from 52% to 63% line coverage
- **Estimated tests:** 24 new tests across 3 files
- **Test files to create/extend:**
  - `tests/data_serialization_Data_pack.rs` — 10 tests covering data_pack (user-space validation)
  - `tests/data_serialization_Data_unpack.rs` — 10 tests covering data_unpack (user-space validation)
  - `tests/data_serialization_Data_load_unload.rs` — 4 tests covering data_load/data_unload (user-space validation)
- **Coverage targets:**
  - [ ] data_pack: test PMIX_VAL, PMIX_BUFFER, PMIX_STRING, PMIX_BYTE_OBJECT types
  - [ ] data_unpack: test type conversion and buffer advancement
  - [ ] data_load: test payload copying and buffer reset
  - [ ] data_unload: test payload extraction and buffer reset
  - [ ] Type validation: PMIX_DATA_TYPE range checking
- **Key patterns:**
  - [ ] Callback trait implementations: NopCallback for async variants (covered later)
  - [ ] Panic safety: null buf/values pointers, zero num_vals
  - [ ] Error code verification: PMIX_ERR_BAD_PARAM for invalid types
  - [ ] Compile-time type checks: value type constraints
  - [ ] InfoBuilder/PmixValueBuilder usage patterns
- **Ignored tests:** 0 tests
- **Estimated coverage gain:** +11% lines

### Batch 7: data_serialization.rs — Print/Embed/Compress Operations
- **Target:** Push data_serialization.rs from 63% to 72% line coverage
- **Estimated tests:** 18 new tests across 3 files
- **Test files to create/extend:**
  - `tests/data_serialization_Data_print.rs` — 8 tests covering data_print (user-space validation)
  - `tests/data_serialization_Data_embed.rs` — 6 tests covering data_embed (user-space validation)
  - `tests/data_serialization_Compress.rs` — 4 tests covering data_compress/data_decompress (user-space validation)
- **Coverage targets:**
  - [ ] data_print: test prefix handling and type-specific formatting
  - [ ] data_embed: test buffer nesting and reference counting
  - [ ] data_compress: test input validation and error paths (library bug noted)
  - [ ] data_decompress: test input validation and error paths (library bug noted)
  - [ ] PmixPrintOutput: as_str() method validation
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for print/output types
  - [ ] Panic safety: null src/prefix pointers, empty buffers
  - [ ] Edge cases: unicode prefixes, max-length data types
  - [ ] Debug trait verification for complex types
  - [ ] Known library limitations: compress/decompress tests marked #[ignore] with reason
- **Ignored tests:** 6 tests requiring PMIx_Init marked #[ignore] (reason: compression framework not initialized)
- **Estimated coverage gain:** +9% lines

### Batch 8: data_ops.rs — Publish/Lookup Core
- **Target:** Push data_ops.rs from 56.05% to 68% line coverage
- **Estimated tests:** 22 new tests across 3 files
- **Test files to create/extend:**
  - `tests/data_ops_Publish.rs` — 8 tests covering publish (blocking/nb) with error validation
  - `tests/data_ops_Get.rs` — 8 tests covering get (blocking/nb) with error validation
  - `tests/data_ops_Lookup.rs` — 6 tests covering lookup (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] publish: test success path (no server) and ERR_NOENT error path
  - [ ] publish_nb: test callback invocation with proper error codes
  - [ ] get: test success path and ERR_NOENT error path
  - [ ] get_nb: test callback invocation with value results
  - [ ] lookup: test success path and ERR_NOENT error path
  - [ ] lookup_nb: test callback invocation with lookup results
- **Key patterns:**
  - [ ] Callback trait implementations: PublishCallback/GetValueCallback/LookupCallback
  - [ ] Panic safety: null proc/key/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: info array lifetime constraints
  - [ ] InfoBuilder/PmixValueBuilder usage patterns
- **Ignored tests:** 18 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +11.95% lines

### Batch 9: data_ops.rs — Internal Store/Fence Operations
- **Target:** Push data_ops.rs from 68% to 76% line coverage
- **Estimated tests:** 16 new tests across 2 files
- **Test files to create/extend:**
  - `tests/data_ops_Store_internal.rs` — 8 tests covering store_internal (user-space validation)
  - `tests/data_ops_Fence.rs` — 8 tests covering fence_nb (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] store_internal: test proc/key/value validation and storage
  - [ ] fence_nb: test success path (no server) and ERR_NOENT error path
  - [ ] fence_nb: test callback invocation with proper error codes
  - [ ] PmixPdata: key construction and validation
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for PmixPdata
  - [ ] Panic safety: null proc/key/value pointers
  - [ ] Edge cases: empty key strings, max-length values
  - [ ] Error code verification: PMIX_ERR_BAD_PARAM for invalid proc/key
- **Ignored tests:** 8 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +8% lines

### Batch 10: query_log.rs — Query/Core Operations
- **Target:** Push query_log.rs from 60.38% to 72% line coverage
- **Estimated tests:** 16 new tests across 2 files
- **Test files to create/extend:**
  - `tests/query_log_PmixQuery.rs` — 8 tests covering PmixQuery construction/qualifiers
  - `tests/query_log_QueryInfo.rs` — 8 tests covering query_info (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] PmixQuery: new() with keys, with_qualifiers() chaining
  - [ ] QueryResults: len()/is_empty() implementation
  - [ ] query_info: test success path (no server) and ERR_NOENT error path
  - [ ] query_info_nb: test callback invocation with query results
  - [ ] Key validation: duplicate key handling, null keys array
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for query types
  - [ ] Panic safety: null keys/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: query lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 12 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +11.62% lines

### Batch 11: query_log.rs — Log Operations
- **Target:** Push query_log.rs from 72% to 80% line coverage
- **Estimated tests:** 12 new tests across 2 files
- **Test files to create/extend:**
  - `tests/query_log_LogData.rs` — 6 tests covering log_data (blocking/nb) with error validation
  - `tests/query_log_Callbacks.rs` — 6 tests covering log callback validation
- **Coverage targets:**
  - [ ] log_data: test success path (no server) and ERR_NOENT error path
  - [ ] log_data_nb: test callback invocation with proper error codes
  - [ ] LogCallback: trait implementation and result validation
  - [ ] Directive validation: PMIX_LOG_* flag combinations
- **Key patterns:**
  - [ ] Callback trait implementations: LogCallback with Arc<Mutex<>> recording
  - [ ] Panic safety: null data/directives pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: data/directives lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 12 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +8% lines

### Batch 12: security.rs — Credential Core
- **Target:** Push security.rs from 61.12% to 72% line coverage
- **Estimated tests:** 20 new tests across 3 files
- **Test files to create/extend:**
  - `tests/security_PmixCredential.rs` — 10 tests covering PmixCredential construction/accessors
  - `tests/security_CredentialResults.rs` — 6 tests covering CredentialResults accessors
  - `tests/security_ValidationResults.rs` — 4 tests covering ValidationResults accessors
- **Coverage targets:**
  - [ ] PmixCredential: from_bytes/from_vec/empty()/as_bytes/as_raw/is_empty/len
  - [ ] CredentialResults: info()/len()/is_empty() accessors
  - [ ] ValidationResults: empty()/len()/is_empty() accessors
  - [ ] Edge cases: empty byte vectors, max-length credentials
  - [ ] Memory safety: validate proper byte vector ownership
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for credential types
  - [ ] Panic safety: null byte slice pointers
  - [ ] Debug trait verification for all structs
  - [ ] Default trait verification where applicable
- **Ignored tests:** 0 tests
- **Estimated coverage gain:** +10.88% lines

### Batch 13: security.rs — Credential Operations
- **Target:** Push security.rs from 72% to 80% line coverage
- **Estimated tests:** 18 new tests across 3 files
- **Test files to create/extend:**
  - `tests/security_GetCredential.rs` — 10 tests covering get_credential (blocking/nb) with error validation
  - `tests/security_ValidateCredential.rs` — 8 tests covering validate_credential (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] get_credential: test success path (no server) and ERR_NOENT error path
  - [ ] get_credential_nb: test callback invocation with credential results
  - [ ] validate_credential: test success path (no server) and ERR_NOENT error path
  - [ ] validate_credential_nb: test callback invocation with validation results
  - [ ] Info validation: PMIX_CRED_* directive handling
- **Key patterns:**
  - [ ] Callback trait implementations: CredentialCallback/ValidationCallback
  - [ ] Panic safety: null credential/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: credential/info lifetime constraints
  - [ ] InfoBuilder/PmixValueBuilder usage patterns
- **Ignored tests:** 18 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +8% lines

### Batch 14: monitoring.rs — Process Monitor Core
- **Target:** Push monitoring.rs from 65.54% to 78% line coverage
- **Estimated tests:** 12 new tests across 2 files
- **Test files to create/extend:**
  - `tests/monitoring_MonitorResults.rs` — 6 tests covering MonitorResults accessors
  - `tests/monitoring_ProcessMonitor.rs` — 6 tests covering process_monitor (blocking/nb) with error validation
- **Coverage targets:**
  - [ ] MonitorResults: len()/is_empty() implementation
  - [ ] process_monitor: test success path (no server) and ERR_NOENT error path
  - [ ] process_monitor_nb: test callback invocation with proper error codes
  - [ ] Monitor trait validation: Send/Sync requirements
  - [ ] Directive validation: PMIX_MONITOR_* flag combinations
- **Key patterns:**
  - [ ] Compile-time type checks: Send/Sync for monitor types
  - [ ] Panic safety: null monitor/error/directives pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: monitor/error/directives lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 12 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +12.46% lines

### Batch 15: monitoring.rs — Heartbeat Operation
- **Target:** Push monitoring.rs from 78% to 85% line coverage
- **Estimated tests:** 6 new tests across 1 file
- **Test files to create/extend:**
  - `tests/monitoring_Heartbeat.rs` — 6 tests covering heartbeat() function
- **Coverage targets:**
  - [ ] heartbeat: test success path (no server) and ERR_NOENT error path
  - [ ] heartbeat: test return code validation
  - [ ] Edge cases: repeated heartbeat calls
- **Key patterns:**
  - [ ] Compile-time type checks: function signature validation
  - [ ] Panic safety: no parameters to validate
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Return code consistency: validate against PMix error codes
- **Ignored tests:** 6 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +7% lines

### Batch 16: groups.rs — Group Construct/Destruct Core
- **Target:** Push groups.rs from 67.23% to 76% line coverage
- **Estimated tests:** 20 new tests across 3 files
- **Test files to create/extend:**
  - `tests/groups_GroupConstruct.rs` — 8 tests covering group_construct (blocking/nb) with error validation
  - `tests/groups_GroupDestruct.rs` — 8 tests covering group_destruct (blocking/nb) with error validation
  - `tests/groups_CallbackWrappers.rs` — 4 tests covering GroupConstruct/DestructCallbackWrapper
- **Coverage targets:**
  - [ ] group_construct: test success path (no server) and ERR_NOENT error path
  - [ ] group_construct_nb: test callback invocation with proper error codes
  - [ ] group_destruct: test success path (no server) and ERR_NOENT error path
  - [ ] group_destruct_nb: test callback invocation with proper error codes
  - [ ] CallbackWrapper: new() construction and FnOnce invocation
- **Key patterns:**
  - [ ] Callback trait implementations: GroupConstruct/DestructCallbackWrapper
  - [ ] Panic safety: null group_id/procs/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: group_id/procs/info lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 18 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +8.77% lines

### Batch 17: groups.rs — Group Invite/Join Operations
- **Target:** Push groups.rs from 76% to 82% line coverage
- **Estimated tests:** 18 new tests across 3 files
- **Test files to create/extend:**
  - `tests/groups_GroupInvite.rs` — 8 tests covering group_invite (blocking/nb) with error validation
  - `tests/groups_GroupJoin.rs` — 8 tests covering group_join (blocking/nb) with error validation
  - `tests/groups_CallbackWrappers2.rs` — 2 tests covering GroupInvite/JoinCallbackWrapper
- **Coverage targets:**
  - [ ] group_invite: test success path (no server) and ERR_NOENT error path
  - [ ] group_invite_nb: test callback invocation with proper error codes
  - [ ] group_join: test success path (no server) and ERR_NOENT error path
  - [ ] group_join_nb: test callback invocation with proper error codes
  - [ ] Option validation: PMIX_JOIN_* option handling
- **Key patterns:**
  - [ ] Callback trait implementations: GroupInvite/JoinCallbackWrapper
  - [ ] Panic safety: null group_id/procs/leader/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: group_id/procs/leader/info lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 18 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +6% lines

### Batch 18: groups.rs — Group Leave Operation
- **Target:** Push groups.rs from 82% to 85% line coverage
- **Estimated tests:** 10 new tests across 2 files
- **Test files to create/extend:**
  - `tests/groups_GroupLeave.rs` — 6 tests covering group_leave (blocking/nb) with error validation
  - `tests/groups_CallbackWrappers3.rs` — 4 tests covering GroupLeaveCallbackWrapper
- **Coverage targets:**
  - [ ] group_leave: test success path (no server) and ERR_NOENT error path
  - [ ] group_leave_nb: test callback invocation with proper error codes
  - [ ] GroupLeaveCallbackWrapper: new() construction and FnOnce invocation
- **Key patterns:**
  - [ ] Callback trait implementations: GroupLeaveCallbackWrapper
  - [ ] Panic safety: null group_id/info pointers
  - [ ] Error code verification: PMIX_ERR_NOENT for uninitialized server
  - [ ] Compile-time type checks: group_id/info lifetime constraints
  - [ ] InfoBuilder usage patterns
- **Ignored tests:** 10 tests requiring PMIx_Init marked #[ignore] (reason: needs server initialization)
- **Estimated coverage gain:** +3% lines

## Phase 4 Summary

**Total estimated tests:** 284 new tests across 36 files  
**Total estimated coverage gain:** +17.25% lines (projected from 70.15% to 87.40%)  

### Risk Assessment by Batch

| Batch | Module | Risk Level | Key Risks | Mitigation |
|-------|--------|------------|-----------|------------|
| 1 | fabric | Low | Pure user-space tests | None needed |
| 2 | fabric | Medium | Callback lifetime issues | Use Arc<Mutex<>> pattern from existing tests |
| 3 | fabric | Low | Pure user-space tests | None needed |
| 4 | fabric | Medium | FFI error path validation | Mark server-dependent tests as #[ignore] with clear reason |
| 5 | data_serialization | Low | Pure user-space tests | Validate allocator fixes with proptest |
| 6 | data_serialization | Low | Pure user-space tests | Focus on type validation, avoid actual pack/unpack |
| 7 | data_serialization | Medium | Known library limitations | Mark compress/decompress as #[ignore] with documented reason |
| 8 | data_ops | High | Complex callback chains | Use NopCallback/PanicCallback patterns from existing tests |
| 9 | data_ops | Medium | Internal state validation | Focus on input validation, not actual storage |
| 10 | query_log | Medium | Async callback validation | Use RecordingCallback with timeout guards |
| 11 | query_log | Medium | Directive combination testing | Test common flag combinations from PMIx spec |
| 12 | security | Low | Pure user-space tests | Validate byte vector ownership with Miri |
| 13 | security | High | Credential validation complexity | Focus on error paths, avoid actual credential flows |
| 14 | monitoring | Medium | Monitor trait requirements | Use simple struct implementing Monitor trait |
| 15 | monitoring | Low | Simple function test | Validate return codes only |
| 16 | groups | High | Complex group lifecycle | Focus on construction/destruction error paths |
| 17 | groups | Medium | Nested callback validation | Use existing callback wrapper patterns |
| 18 | groups | Low | Simple leave operation | Validate basic error paths |

**Overall Risk Assessment:** Moderate  
- Primary risk is in async callback testing (mitigated by adopting existing patterns)
- FFI-dependent tests properly isolated with #[ignore] and clear rationale
- User-space testable code prioritized first in each batch to establish baseline
- All tests designed to pass with `cargo test --test-threads=1`
- No source code modifications required - pure test expansion

This plan strategically targets the highest-impact uncovered code paths while maintaining test quality and reliability. The projected coverage gain exceeds the 85% target, providing buffer for any test failures or uncovered edge cases.