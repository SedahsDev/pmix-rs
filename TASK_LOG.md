# Batch 16 Task Log — fabric round 2

**Branch:** wt/batch16-fabric-round2
**Worktree:** /home/bzf/projects/pmix-rs-worktrees/batch16
**Started:** 2026-06-16
**Status:** COMPLETED

## Results
- Test file: `tests/fabric_deep.rs` — **51 tests**
- Active: **28 passed**
- Ignored: **23** (require PMIx_Init)
- Full suite: **0 failures**
- fabric.rs coverage: 41.23% (unchanged — FFI tests need init)

## Key Discoveries
- `PmixDeviceDistance` fields are **private** — cannot construct directly
- `PmixFabric::as_mut_ptr` is **private** — only internal use
- `PmixDeviceType::UnknownType` (not `Unknown`)
- `fabric_register_nb` takes `Box<dyn FabricCallback>` (non-optional)
- `compute_distances_nb` takes `Box<dyn ComputeDistancesCallback>` (non-optional)
- `InfoBuilder::collect_data()` returns `&mut self` — must use `builder.collect_data(); builder.build()` pattern
- `Info` is not easily constructible with key-value pairs without FFI

## Coverage Impact
Same pattern as Batch 15 — pure-Rust construction tests pass, but FFI paths
require PMIx_Init to exercise. The 23 ignored tests cover:
- fabric_register/deregister/update (sync + async)
- load_topology
- compute_distances (sync + async)
- Full lifecycle tests
