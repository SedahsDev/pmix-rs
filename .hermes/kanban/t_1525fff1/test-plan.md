# PMIx-rs Test Expansion Plan

## Baseline (fixed during this run)
- **Bug found:** `free_value` double-free on `PMIX_ENVAR` type — `Box::from_raw(&mut v.data.envar)` tried to free stack-embedded struct. Fixed in `src/lib.rs`.
- **Before fix:** `cargo test --test lib_type_coverage` crashed with `munmap_chunk(): invalid pointer`
- **After fix:** All 105 tests pass

## Current State
- 137 public functions across 14 modules
- 97 test files (96 integration + daemon_helper)
- ~180 module-level tests in `#[cfg(test)]` blocks
- Target: add dedicated integration tests for functions that only have inline module tests

## Priority 1 — Missing dedicated integration tests (no daemon needed)

### Utility module (24 pub fn, 12 test files — several only tested inline)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `initialized` | utility_initialized.rs ✓ | Already has test |
| `get_version` | utility_Get_version.rs | Version struct fields |
| `scope_string` | utility_Scope_string.rs | All scope values → prose |
| `persistence_string` | utility_Persistence_string.rs | All persistence values → prose |
| `data_range_string` | utility_Data_range_string.rs | All data range values → prose |
| `info_directives_string` | utility_Info_directives_string.rs | All directive values → prose |
| `job_state_string` | utility_Job_state_string.rs | All job state values → prose |
| `link_state_string` | utility_Link_state_string.rs ✓ | Already has test |
| `iof_channel_string` | utility_IOF_channel_string.rs | All IOF channel values → prose |
| `get_attribute_name` | utility_Get_attribute_name.rs | Reverse lookup from attr string |
| `generate_regex` | utility_generate_regex.rs ✓ | Has inline tests, needs integration |
| `heartbeat` | utility_heartbeat.rs | Needs DVM — #[ignore] |
| `progress` | utility_progress.rs | Needs DVM — #[ignore] |

### Data serialization (12 pub fn, 11 test files — good coverage)
| Function | Test File | Status |
|----------|-----------|--------|
| `data_pack<T>` | data_serialization_Data_pack.rs ✓ | Has test |
| `data_unpack<T>` | data_serialization_Data_unpack.rs ✓ | Has test |
| Others | Various | Already covered |

### Events module (6 pub fn, 3 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `register_event_handler_nb` | events_Register_event_handler_nb.rs | Callback compiles, returns request ID |
| `deregister_event_handler_nb` | events_Deregister_event_handler_nb.rs | Callback compiles, returns request ID |
| `notify_event_nb` | events_Notify_event_nb.rs | Callback compiles, returns request ID |

### Process management (9 pub fn, 6 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `connect_nb` | process_mgmt_Connect_nb.rs | Callback compiles, needs DVM → #[ignore] |
| `disconnect_nb` | process_mgmt_Disconnect_nb.rs | Callback compiles, needs DVM → #[ignore] |
| `spawn_nb` | process_mgmt_Spawn_nb.rs | Callback compiles, needs DVM → #[ignore] |

### Tool module (9 pub fn, 5 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `tool_get_servers` | tool_Tool_get_servers.rs | Returns empty/error without daemon |
| `tool_set_server` | tool_Tool_set_server.rs | Sets server info |
| `tool_connect_to_server` | tool_Tool_connect_to_server.rs | Needs daemon → #[ignore] |

### Query/Log module (4 pub fn, 2 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `log_data_nb` | query_log_Log_data_nb.rs | Callback compiles, needs DVM → #[ignore] |
| `query_info_nb` | query_log_Query_info_nb.rs | Callback compiles, needs DVM → #[ignore] |

### Allocation module (4 pub fn, 2 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `allocation_request` | allocation_Allocation_request.rs | Blocking version, needs DVM → #[ignore] |

### Monitoring module (3 pub fn, 2 test files)
| Function | Test File | What to test |
|----------|-----------|--------------|
| `process_monitor_nb` | monitoring_Process_monitor_nb.rs | Callback compiles, needs DVM → #[ignore] |

## Priority 2 — Functions requiring PMIx daemon (#[ignore] with compilation tests)

All `_nb` callback variants that need a running daemon. These tests verify:
1. The function compiles with correct callback signature
2. The function is callable (returns appropriate error without daemon)
3. Marked `#[ignore = "requires PMIx daemon"]`

## Implementation Strategy

Each test file follows the existing pattern:
1. Import from crate root: `use pmix::{...}`
2. `#[test]` functions with `test_` prefix
3. For daemon-required: test compilation + graceful error, mark `#[ignore]`
4. For string converters: test all enum variants
5. Use `daemon_helper` module where applicable

## Batch Plan

**Batch 1 (Priority — no daemon needed):** ~10 test files
- utility_Scope_string.rs
- utility_Persistence_string.rs
- utility_Data_range_string.rs
- utility_Info_directives_string.rs
- utility_Job_state_string.rs
- utility_IOF_channel_string.rs
- utility_Get_attribute_name.rs
- tool_Tool_get_servers.rs
- tool_Tool_set_server.rs
- allocation_Allocation_request.rs

**Batch 2 (Callback compilation tests):** ~10 test files
- events_Register_event_handler_nb.rs
- events_Deregister_event_handler_nb.rs
- events_Notify_event_nb.rs
- process_mgmt_Connect_nb.rs
- process_mgmt_Disconnect_nb.rs
- process_mgmt_Spawn_nb.rs
- query_log_Log_data_nb.rs
- query_log_Query_info_nb.rs
- monitoring_Process_monitor_nb.rs
- data_ops_Publish_nb.rs

**Batch 3 (Server module gaps):** ~5 test files
- server_Server_dmodex_request.rs
- server_Server_iof_deliver.rs
- server_Server_setup_application.rs
- server_Server_setup_local_support.rs
- tool_Tool_connect_to_server.rs
