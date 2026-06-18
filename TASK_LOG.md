# Phase 4 Batch 3 (prterun): Fabric DVM Test Isolation

**Branch:** `wt/p4b4-topology`
**Worktree:** `../pmix-rs-wt-p4b4-topology`
**Completed:** 2026-06-18
**Commit:** `d058601`

## Scope

Split `fabric_ffi_via_prterun.rs` into 4 files to isolate PMIx state corruption issues discovered during DVM test development. Cover fabric.rs lines 252 and 301 (directive else branches).

## Result

### Test Files (4 total)

| File | Tests | Type | Run Command |
|------|-------|------|-------------|
| `fabric_ffi_via_prterun.rs` | 21 | Standalone (no PMIx init) | `cargo test --test fabric_ffi_via_prterun -- --test-threads=1` |
| `fabric_dvm_via_prterun.rs` | 7 | Safe DVM (batch) | `prterun -np 1 cargo test --test fabric_dvm_via_prterun -- --include-ignored --test-threads=1` |
| `fabric_directives_via_prterun.rs` | 3 | Directive (individual) | `prterun -np 1 cargo test --test fabric_directives_via_prterun <name> -- --include-ignored` |
| `fabric_isolated_via_prterun.rs` | 7 | Isolated (individual) | `prterun -np 1 cargo test --test fabric_isolated_via_prterun <name> -- --include-ignored` |

### Coverage

- **fabric.rs line 252** (`fabric_register` else branch): `DA:252,1` via `test_fabric_register_with_directives_via_dvm`
- **fabric.rs line 301** (`fabric_register_nb` else branch): `DA:301,1` via `test_fabric_register_nb_with_directives_via_dvm`

### Key Findings

- `get_or_init` on `PMIX_CONTEXT` causes cross-test PMIx state corruption
- `set()` works but only allows one test per process (OnceLock limitation)
- NB callback tests must run individually to avoid `free(): invalid pointer`
- `PMIX_COLLECT_DATA` with `PMIX_BOOL` triggers PRRTE "UNSUPPORTED TYPE 53440" warning but FFI still succeeds
- Directive tests successfully cover the `else` branches in `fabric_register` and `fabric_register_nb`

### Test Results

- All 21 standalone tests pass in batch
- All 7 DVM tests pass individually under prterun
- All 3 directive tests pass individually under prterun
- All 7 isolated tests pass individually under prterun
- Full test suite: all passing with 0 failures across 158 test files
