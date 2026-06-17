# Round 3 Test Expansion Plan

**Generated:** 2026-06-17
**Model:** nvidia/nemotron-3-ultra-550b-a55b:free
**Starting coverage:** 68.04% lines (7610 lines, 3692 tests, 143 files)
**Batches:** 9
**Projected:** ~71-73% overall

## Summary

| Batch | Module | Current Cov | Target | Est. Tests | Key Gap |
|-------|--------|-------------|--------|------------|---------|
| 1 | fabric | 42.8% | 55%+ | 52 | FFI error returns, callback execution, Drop |
| 2 | data_serialization | 43.4% | 55%+ | 68 | Type erasure exhaustiveness, buffer ownership |
| 3 | monitoring | 54.9% | 65%+ | 22 | Unique callback signatures (`&mut self`, `&self`) |
| 4 | query_log | 54.3% | 65%+ | 35 | Qualifier enums, non-blocking callbacks |
| 5 | security | 56.2% | 65%+ | 48 | Credential types, crypto, auth callbacks |
| 6 | data_ops | 56.3% | 65%+ | 44 | Comparison/arithmetic operator matrices |
| 7 | groups | 59.1% | 68%+ | 42 | Membership callbacks, bulk ops, fence |
| 8 | allocation | 74.4% | 80%+ | 55 | Sparse test density — directive enums, callbacks |
| 9 | tool | 67.0% | 75%+ | 38 | IOF callbacks, server info keys, connection state |

**Total: ~404 new tests** across 9 batches

## Priority Order

1. **Batch 8 (allocation)** — highest ROI (35 tests for 367 lines)
2. **Batch 1 (fabric)** — lowest coverage, high line count
3. **Batch 2 (data_serialization)** — lowest coverage, many tests but deep FFI gaps
4. **Batch 5 (security)** — credential/crypto paths completely untested
5. **Batch 7 (groups)** — callback/bulk op gaps
6. **Batch 4 (query_log)** — qualifier enum coverage
7. **Batch 6 (data_ops)** — operator matrix
8. **Batch 9 (tool)** — IOF/connection state
9. **Batch 3 (monitoring)** — smallest module, unique callbacks

## FFI Safety Checklist
- [ ] Every test calling FFI without `PMIx_Init` marked `#[ignore]`
- [ ] No explicit `drop`/`free`/`release` calls — rely on `Drop` impls
- [ ] `catch_unwind(AssertUnwindSafe(...))` for all panic tests on non-Copy types
- [ ] `PmixServerHandle`, `PmixCpuset`, `PmixByteObject` — never `Clone`/`Copy` in tests
- [ ] Callback `on_complete` signatures verified at compile-time
- [ ] Empty string / NUL byte tests expect `Err(PmixError::ERR_BAD_PARAM)` not FFI passthrough
- [ ] `server_deregister_nspace`/`server_deregister_client` tested for `()` return (not `Result`)
