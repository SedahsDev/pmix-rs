# Agent Memory

## Project: pmix-rs
- Location: /home/bzf/projects/pmix-rs
- Language: Rust
- PMIx bindings library
- cargo fmt is enforced (imports sorted alphabetically)
- cargo test --lib runs all unit tests (full cargo test times out due to integration tests)

### Test Coverage (as of 2026-07-07)
- lib.rs: 83 new tests covering PmixError, PmixStatus, PmixProcState, PmixScope, PmixJobState, PmixLinkState, PmixDataRange, PmixAllocDirective, IOFChannelFlags, InfoFlags, PmixPayload, PmixValueBuilder, PmixEnvar, Proc
- process_mgmt.rs: 15 new tests covering PmixApp accessors, builder NUL validation, spawn/connect/disconnect validation, callback wrappers
- Total: 498 tests passing

### Key API Notes
- PmixScope: Local/Remote/Global/Internal (not Session/Job/Node/Proc)
- PmixProcState::Error is NOT considered terminated (is_terminated returns false)
- PmixPayload.type_tag() returns u16, PMIX_* constants are u32 — cast with `as u16`
- PmixValueBuilder.string() returns Result<Self, ValueError> — must unwrap before build
- PmixDataRange::Unknown is a unit variant (not tuple)
- PmixLinkState::UnknownState is unit variant, Unknown(u8) is tuple variant
