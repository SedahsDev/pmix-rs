# Batch 20 Task Log — groups round 2

**Branch:** wt/batch20-groups-round2
**Worktree:** /home/bzf/projects/pmix-rs-worktrees/batch20
**Started:** 2026-06-16
**Status:** COMPLETED

## Results
- Test file: `tests/groups_deep.rs` — **54 tests**
- Active tests: **32 passed**
- Ignored tests: **22** (require PMIx_Init)
- Full suite: **0 failures**
- Coverage: groups.rs **67.23%** (unchanged — FFI-heavy)
- TOTAL: **68.94%** lines

## Key Discoveries
- `pmix_group_opt_t` is an enum (PMIX_GROUP_DECLINE, PMIX_GROUP_ACCEPT) — not an integer
- `Proc` is NOT Copy or Debug — must clone for reuse
- `group_join` takes `pmix_group_opt_t` as 3rd arg, `&[Info]` as 4th
- `group_join_nb` takes `&[Info]` before callback
- All 5 callback wrappers: Construct, Invite, Join, Leave, Destruct
- `group_construct` rejects empty group_id and empty procs with BAD_PARAM
