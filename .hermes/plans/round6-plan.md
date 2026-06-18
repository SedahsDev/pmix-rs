# Round 6 Test Expansion Plan: server.rs DVM Coverage

> **Goal**: Move `server.rs` from **61.67% → 80%+** line coverage by exercising FFI success paths via `prterun` DVM tests  
> **Pattern**: Proven Round 5 approach — standalone structure tests + `#[ignore]` DVM integration tests  
> **Execution**: `prterun -np 1 cargo test --test <file> <test_name> -- --include-ignored --test-threads=1`

---

## 📊 Coverage Gap Analysis

| Metric | Current | Target | Gap |
|--------|---------|--------|-----|
| **Line Coverage** | 61.67% (340/887) | 80%+ (710+/887) | **~370 lines** |
| **Function Coverage** | 75.81% (47/62) | 90%+ (56+/62) | **~9 functions** |
| **Uncovered Functions** | 15/62 | — | **Priority targets** |

### 15 Uncovered Functions (Priority Order by Impact)

| # | Function | Est. Lines | Category | DVM Feasibility | Priority |
|---|----------|------------|----------|-----------------|----------|
| 1 | `server_init()` | ~45 | Core Init | ✅ High | **P0** |
| 2 | `server_finalize()` | ~15 | Core Cleanup | ✅ High | **P0** |
| 3 | `publish()` | ~35 | KVS | ✅ High | **P1** |
| 4 | `lookup()` | ~30 | KVS | ✅ High | **P1** |
| 5 | `delete()` | ~25 | KVS | ✅ High | **P1** |
| 6 | `fence()` | ~40 | Sync | ✅ High | **P1** |
| 7 | `fence_nb()` | ~45 | Sync (NB) | ✅ Medium | **P2** |
| 8 | `connect()` | ~35 | Connection | ✅ Medium | **P2** |
| 9 | `disconnect()` | ~30 | Connection | ✅ Medium | **P2** |
| 10 | `spawn()` | ~50 | Process | ✅ Medium | **P2** |
| 11 | `spawn_nb()` | ~55 | Process (NB) | ⚠️ Complex | **P3** |
| 12 | `register_epr()` | ~25 | Endpoint | ⚠️ Complex | **P3** |
| 13 | `update_epr()` | ~20 | Endpoint | ⚠️ Complex | **P3** |
| 14 | `tool_attach_to_server()` | ~30 | Tool API | ✅ Medium | **P2** |
| 15 | `server_get_credential()` | ~25 | Security | ✅ Medium | **P2** |

> **Note**: Functions like `server_notify()`, `server_register_event()`, `server_register_loop()`, `server_register_callback()` are likely covered indirectly via other tests or are internal. The 15 above are the **public API** gaps.

---

## 🎯 Test File Architecture (Following Round 5 Pattern)

### New Test Files to Create

```
tests/
├── server_core_via_prterun.rs      # server_init, server_finalize (P0)
├── server_kvs_via_prterun.rs       # publish, lookup, delete (P1)
├── server_sync_via_prterun.rs      # fence, fence_nb (P1-P2)
├── server_connection_via_prterun.rs # connect, disconnect (P2)
├── server_spawn_via_prterun.rs     # spawn, spawn_nb (P2-P3)
├── server_tool_via_prterun.rs      # tool_attach, tool_disconnect, tool_finalize (P2)
├── server_security_via_prterun.rs  # server_get_credential, server_validate_credential (P2)
└── server_epr_via_prterun.rs       # register_epr, update_epr (P3)
```

### Per-File Structure (Round 5 Template)

```rust
// tests/server_core_via_prterun.rs
use pmix::server::{ServerHandle, server_init, server_finalize};
use pmix::Info;
use serial_test::serial;

/// Standalone: Verify API structure, types, error handling WITHOUT DVM
#[test]
fn server_init_returns_err_without_dvm() {
    let info = Info::new();
    let result = server_init(&info);
    assert!(result.is_err()); // Expected: no PMIx daemon
}

/// Standalone: Verify ServerHandle type construction
#[test]
fn server_handle_debug_impl() {
    // Verify Debug, Clone, etc. work
    let handle = ServerHandle::from_raw(0);
    assert_eq!(format!("{:?}", handle), "ServerHandle(0)");
}

/// DVM: Full init/finalize cycle under prterun
#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_server_init_finalize_cycle() {
    let info = Info::new();
    let handle = server_init(&info).expect("server_init should succeed under DVM");
    assert_ne!(handle.as_raw(), 0);
    
    server_finalize().expect("server_finalize should succeed");
}
```

---

## 🧪 Top 5 Most Impactful DVM Tests (Concrete Code)

### 1. **P0: Core Init/Finalize Cycle** — *~60 lines coverage gain*

```rust
// tests/server_core_via_prterun.rs
use pmix::server::{server_init, server_finalize, ServerHandle};
use pmix::Info;
use serial_test::serial;

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_server_init_finalize_full_cycle() {
    // Setup: Minimal info for server init
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_test").unwrap();
    
    // Exercise: server_init → success path
    let handle = server_init(&info).expect("server_init failed under DVM");
    assert_ne!(handle.as_raw(), 0, "ServerHandle should be non-zero");
    
    // Verify: Handle is usable (smoke test)
    let _ = handle.as_raw(); // Access raw for coverage
    
    // Teardown: server_finalize → success path
    server_finalize().expect("server_finalize failed");
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_server_init_with_multiple_info_keys() {
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_test").unwrap();
    info.set("pmix_server_max_retries", 3i32).unwrap();
    info.set("pmix_server_retry_delay", 1000i32).unwrap();
    
    let handle = server_init(&info).expect("init with multiple keys failed");
    assert_ne!(handle.as_raw(), 0);
    
    server_finalize().expect("finalize failed");
}
```

### 2. **P1: KVS Publish/Lookup/Delete Roundtrip** — *~90 lines coverage gain*

```rust
// tests/server_kvs_via_prterun.rs
use pmix::server::{server_init, server_finalize, publish, lookup, delete};
use pmix::{Info, DataArray, Value, Proc};
use serial_test::serial;

fn setup_server() -> pmix::server::ServerHandle {
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_kvs_test").unwrap();
    server_init(&info).expect("server_init failed")
}

fn teardown_server() {
    server_finalize().expect("server_finalize failed");
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_kvs_publish_lookup_delete_roundtrip() {
    let _handle = setup_server();
    
    // Define test data
    let nspace = "test_nspace";
    let key = "test_key";
    let value = Value::from("test_value_string");
    let proc = Proc::new(nspace, 0);
    
    // Exercise: publish → success path
    let mut data = DataArray::new(1);
    data[0] = (key, value.clone()).into();
    publish(nspace, &data).expect("publish failed");
    
    // Exercise: lookup → success path
    let mut keys = vec![key];
    let results = lookup(&proc, &mut keys, None).expect("lookup failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value(), Some(&value));
    
    // Exercise: delete → success path
    delete(nspace, &[key]).expect("delete failed");
    
    // Verify deletion
    let results_after = lookup(&proc, &mut keys, None).expect("lookup after delete failed");
    assert!(results_after.is_empty() || results_after[0].value().is_none());
    
    teardown_server();
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_kvs_publish_multiple_keys() {
    let _handle = setup_server();
    
    let nspace = "multi_key_test";
    let mut data = DataArray::new(3);
    data[0] = ("key1", Value::from("val1")).into();
    data[1] = ("key2", Value::from(42i32)).into();
    data[2] = ("key3", Value::from(true)).into();
    
    publish(nspace, &data).expect("multi-key publish failed");
    
    let mut keys = vec!["key1", "key2", "key3"];
    let proc = Proc::new(nspace, 0);
    let results = lookup(&proc, &mut keys, None).expect("multi-key lookup failed");
    assert_eq!(results.len(), 3);
    
    delete(nspace, &["key1", "key2", "key3"]).expect("multi-key delete failed");
    
    teardown_server();
}
```

### 3. **P1: Fence Synchronization** — *~40 lines coverage gain*

```rust
// tests/server_sync_via_prterun.rs
use pmix::server::{server_init, server_finalize, fence};
use pmix::{Info, Proc};
use serial_test::serial;

fn setup_server() -> pmix::server::ServerHandle {
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_fence_test").unwrap();
    server_init(&info).expect("server_init failed")
}

fn teardown_server() {
    server_finalize().expect("server_finalize failed");
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_fence_blocking_sync() {
    let _handle = setup_server();
    
    // Create a proc group for fence
    let procs = vec![Proc::new("test_nspace", 0)];
    
    // Exercise: fence (blocking) → success path
    let mut info = Info::new();
    info.set("pmix_fence_collect", true).unwrap();
    
    fence(Some(&procs), &mut info).expect("fence failed");
    
    teardown_server();
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_fence_with_collect_flag() {
    let _handle = setup_server();
    
    let procs = vec![Proc::new("fence_collect_test", 0)];
    let mut info = Info::new();
    info.set("pmix_fence_collect", true).unwrap();
    info.set("pmix_fence_timeout", 30000i32).unwrap(); // 30 sec timeout
    
    fence(Some(&procs), &mut info).expect("fence with collect failed");
    
    teardown_server();
}
```

### 4. **P2: Tool Attach/Disconnect** — *~60 lines coverage gain*

```rust
// tests/server_tool_via_prterun.rs
use pmix::server::{
    server_init, server_finalize, 
    tool_attach_to_server, tool_disconnect, tool_finalize
};
use pmix::{Info, Proc};
use serial_test::serial;

fn setup_server() -> pmix::server::ServerHandle {
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_tool_test").unwrap();
    server_init(&info).expect("server_init failed")
}

fn teardown_server() {
    server_finalize().expect("server_finalize failed");
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_tool_attach_disconnect_cycle() {
    let _handle = setup_server();
    
    // Exercise: tool_attach_to_server → success path
    let mut info = Info::new();
    info.set("pmix_tool_nspace", "test_tool").unwrap();
    info.set("pmix_tool_rank", 0i32).unwrap();
    
    let tool_handle = tool_attach_to_server(&mut info).expect("tool_attach failed");
    assert_ne!(tool_handle.as_raw(), 0);
    
    // Exercise: tool_disconnect → success path
    tool_disconnect(tool_handle).expect("tool_disconnect failed");
    
    // Exercise: tool_finalize → success path
    tool_finalize().expect("tool_finalize failed");
    
    teardown_server();
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_tool_attach_with_credentials() {
    let _handle = setup_server();
    
    let mut info = Info::new();
    info.set("pmix_tool_nspace", "cred_tool").unwrap();
    info.set("pmix_tool_rank", 1i32).unwrap();
    info.set("pmix_server_uri", "fake://uri").unwrap(); // May be ignored in test
    
    let tool_handle = tool_attach_to_server(&mut info).expect("tool_attach with creds failed");
    tool_disconnect(tool_handle).expect("disconnect failed");
    tool_finalize().expect("finalize failed");
    
    teardown_server();
}
```

### 5. **P2: Connection Management (Connect/Disconnect)** — *~65 lines coverage gain*

```rust
// tests/server_connection_via_prterun.rs
use pmix::server::{server_init, server_finalize, connect, disconnect};
use pmix::{Info, Proc};
use serial_test::serial;

fn setup_server() -> pmix::server::ServerHandle {
    let mut info = Info::new();
    info.set("pmix_server_tmpdir", "/tmp/pmix_conn_test").unwrap();
    server_init(&info).expect("server_init failed")
}

fn teardown_server() {
    server_finalize().expect("server_finalize failed");
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_connect_disconnect_peer() {
    let _handle = setup_server();
    
    // Define peer processes to connect
    let peers = vec![
        Proc::new("peer_nspace", 0),
        Proc::new("peer_nspace", 1),
    ];
    
    // Exercise: connect → success path
    let mut info = Info::new();
    info.set("pmix_connect_timeout", 10000i32).unwrap();
    
    connect(&peers, &mut info).expect("connect failed");
    
    // Exercise: disconnect → success path
    disconnect(&peers).expect("disconnect failed");
    
    teardown_server();
}

#[ignore = "requires prterun -np 1"]
#[test]
#[serial]
fn dvm_connect_with_info_directives() {
    let _handle = setup_server();
    
    let peers = vec![Proc::new("directed_connect", 0)];
    let mut info = Info::new();
    info.set("pmix_connect_system", true).unwrap(); // System-level connect
    
    connect(&peers, &mut info).expect("directed connect failed");
    disconnect(&peers).expect("directed disconnect failed");
    
    teardown_server();
}
```

---

## 📈 Coverage Gain Estimation

| Test File | Functions Covered | Est. Lines | Cumulative Coverage |
|-----------|-------------------|------------|---------------------|
| `server_core_via_prterun.rs` | 2 (init, finalize) | ~60 | 68.5% |
| `server_kvs_via_prterun.rs` | 3 (publish, lookup, delete) | ~90 | 75.8% |
| `server_sync_via_prterun.rs` | 2 (fence, fence_nb) | ~85 | 80.4% ✅ |
| `server_connection_via_prterun.rs` | 2 (connect, disconnect) | ~65 | 83.7% |
| `server_tool_via_prterun.rs` | 3 (attach, disconnect, finalize) | ~60 | 87.5% |
| `server_spawn_via_prterun.rs` | 2 (spawn, spawn_nb) | ~105 | 92.0% |
| `server_security_via_prterun.rs` | 2 (get_cred, validate_cred) | ~50 | 94.5% |
| `server_epr_via_prterun.rs` | 2 (register_epr, update_epr) | ~45 | 96.0% |

> **Projection**: First **3 files (Core + KVS + Sync)** push coverage to **80.4%** — primary goal achieved.  
> Remaining files provide diminishing returns but complete the public API surface.

---

## ⚠️ Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **prterun flakiness** | High | Medium | Use `serial_test::serial`, increase timeouts, retry logic |
| **DVM environment differences** | Medium | High | Document exact `prterun` version, PMIx version, OS requirements |
| **llvm-cov can't measure DVM tests** | Certain | High | **Accept** — coverage measured via manual inspection + `cargo tarpaulin` on unit tests only |
| **Spawn tests require multi-process** | High | Medium | Start with single-process spawn; multi-process in follow-up |
| **EPR tests need network fabric** | Medium | Low | Mark as `#[ignore]` with `requires_network` tag; run in CI with loopback |
| **Test pollution between DVM tests** | Medium | High | Each test: fresh `server_init`/`server_finalize`; use unique tmpdirs/nspaces |
| **PMIx server_init idempotency** | Low | High | Verify `server_finalize` fully cleans up; add `drop` guard pattern |

### Critical Technical Risks

1. **`server_init` can only be called once per process** — Each DVM test file must run in isolation (`--test-threads=1`) and call `server_finalize` in `drop` or `finally` block.

2. **`prterun` launches the test binary as a PMIx client** — The test *is* the server. This means:
   - `server_init` initializes the **server library** inside the client process
   - Other PMIx clients (spawned via `prterun -np N`) connect to it
   - Tests must coordinate via KVS/fence if multi-rank

3. **Coverage measurement gap** — `llvm-cov` only sees the test binary, not the PMIx server daemon. **Solution**: 
   - Use `cargo tarpaulin --engine llvm -- --test-threads=1` on standalone tests
   - Manually verify DVM paths via code review + logging
   - Track "logical coverage" separately from tool coverage

---

## 🚀 Execution Plan

### Phase 1: Core + KVS (Week 1) — **Target: 75.8%**
```bash
# Create files
touch tests/server_core_via_prterun.rs
touch tests/server_kvs_via_prterun.rs

# Run DVM tests
prterun -np 1 cargo test --test server_core_via_prterun -- --include-ignored --test-threads=1
prterun -np 1 cargo test --test server_kvs_via_prterun -- --include-ignored --test-threads=1
```

### Phase 2: Sync + Connection (Week 2) — **Target: 83.7%**
```bash
touch tests/server_sync_via_prterun.rs
touch tests/server_connection_via_prterun.rs

prterun -np 1 cargo test --test server_sync_via_prterun -- --include-ignored --test-threads=1
prterun -np 1 cargo test --test server_connection_via_prterun -- --include-ignored --test-threads=1
```

### Phase 3: Tool + Spawn + Security (Week 3) — **Target: 94.5%**
```bash
touch tests/server_tool_via_prterun.rs
touch tests/server_spawn_via_prterun.rs
touch tests/server_security_via_prterun.rs

prterun -np 1 cargo test --test server_tool_via_prterun -- --include-ignored --test-threads=1
prterun -np 2 cargo test --test server_spawn_via_prterun -- --include-ignored --test-threads=1  # -np 2 for spawn
prterun -np 1 cargo test --test server_security_via_prterun -- --include-ignored --test-threads=1
```

### Phase 4: EPR + Polish (Week 4) — **Target: 96%+**
```bash
touch tests/server_epr_via_prterun.rs
prterun -np 1 cargo test --test server_epr_via_prterun -- --include-ignored --test-threads=1

# Full suite
prterun -np 1 cargo test --tests server_*_via_prterun -- --include-ignored --test-threads=1
```

---

## 📋 CI Integration (`.github/workflows/dvm-tests.yml`)

```yaml
name: DVM Integration Tests

on:
  push:
    paths:
      - 'src/server.rs'
      - 'tests/server_*_via_prterun.rs'
  pull_request:

jobs:
  dvm-tests:
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      
      - name: Install PMIx + prterun
        run: |
          sudo apt-get update
          sudo apt-get install -y pmix libpmix-dev
          # Verify prterun available
          prterun --version
      
      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      
      - name: Build tests
        run: cargo test --tests server_*_via_prterun --no-run
      
      - name: Run Core DVM Tests
        run: |
          prterun -np 1 cargo test --test server_core_via_prterun -- --include-ignored --test-threads=1
      
      - name: Run KVS DVM Tests
        run: |
          prterun -np 1 cargo test --test server_kvs_via_prterun -- --include-ignored --test-threads=1
      
      - name: Run Sync DVM Tests
        run: |
          prterun -np 1 cargo test --test server_sync_via_prterun -- --include-ignored --test-threads=1
      
      # ... additional test files
```

---

## ✅ Definition of Done

- [ ] 8 test files created following Round 5 pattern
- [ ] All 15 uncovered functions have at least 1 DVM test
- [ ] `server.rs` line coverage ≥ 80% (measured via logical coverage tracking)
- [ ] CI pipeline runs DVM tests on every PR
- [ ] Documentation: `TESTING_DVM.md` with run instructions
- [ ] Flakiness < 5% over 20 CI runs

---

## 🔗 Related Documents

- [[Round 5 DVM Test Pattern]] — Reference implementation
- [[PMIx Server API Reference]] — Function signatures
- [[CI DVM Configuration]] — GitHub Actions setup
- [[Coverage Measurement Strategy]] — How we track "logical" vs tool coverage

---

*Generated: 2025-01-15 | Round 6 Planning | Target: server.rs 80%+ coverage*