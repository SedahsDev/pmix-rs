# Changelog

## Unreleased

- Refactor: Split the 6,298-line `src/lib.rs` into focused modules (`error`, `enums`, `types_proc`, `value_kinds`, `client`, `value_ns`, plus the inline test module as `tests.rs`). `lib.rs` is now a 4KB crate root (docs, mod decls, macro, re-exports). No public API change; cross-module private fields became `pub(crate)`.
- Fix: Add InfoList RAII wrapper to prevent FFI memory leaks in PMIx Info list operations (#526)
- Breaking: Wrapped fabric/device FFI types in native Rust wrappers (PmixCoord, PmixDeviceType, PmixResourceUnit, PmixCoordView, PmixDeviceDistance, PmixDevice, PmixEndpoint, PmixFabric, PmixGeometry, PmixTopology, PmixCpuset). Public API no longer exposes bindgen types. Migration: use `Type::from_raw()` and `type.to_raw()` for conversions.
- Breaking: Wrapped event notification and op callback types for public API use. `pmix_event_notification_cbfunc_fn_t` and `pmix_op_cbfunc_t` are no longer exposed in public wrapper types. Use `NotificationCallbackWrapper` (src/events) and `OpCallbackWrapper` (src/server/callbacks) instead. Note: `PmixServerModule` fields remain as raw function pointers for FFI layout compatibility. Migration: wrap closures with `NotificationCallbackWrapper::new(|...| { ... })` or `OpCallbackWrapper::new(|...| { ... })` when calling public API functions.

## 0.1.0

- Initial public-facing packaging pass
- Portable `build.rs` via `PMIX_PREFIX` / `PMIX_INCLUDE_DIR` / `PMIX_LIB_DIR`
- Simple put/get/commit/fence examples
- Full PMIx 5.x bindgen surface + modular safe wrappers
