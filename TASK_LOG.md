DO NOT MERGE THIS FILE

# Batch 8 — Allocation Deep Tests

## Coverage Delta
| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Lines | 74.39% | 74.39% | +0% (FFI pass-through, needs PMIx_Init) |
| Branches | 48.07% | 81.54% | **+33.5%** |
| Functions | ~80% | 94.29% | **+14%** |

## Tests
- 90 active, 0 failed, 6 ignored
- File: `tests/allocation_deep.rs` (1193 lines)
- Covers: PmixAllocDirective exhaustiveness, AllocationCallback trait, allocation_request/request_nb, PmixJobCtrlAction, JobControlResults, job_control/job_control_nb, error verification, compile-time Send/Sync assertions

## Notes
- Grok timed out after 600s (50 API calls) — wrote 1193 lines before timeout
- Fixed 6 private field errors (AllocationResults has private fields)
- Added `static_assertions` dev dependency for compile-time trait assertions
- Line coverage didn't move because allocation.rs is FFI pass-through; branch coverage jumped 33.5%
