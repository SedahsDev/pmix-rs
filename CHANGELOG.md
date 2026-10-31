# Changelog

## Unreleased

- Refactor: Split the 6,298-line `src/lib.rs` into focused modules (`error`, `enums`, `types_proc`, `value_kinds`, `client`, `value_ns`, plus the inline test module as `tests.rs`). `lib.rs` is now a 4KB crate root (docs, mod decls, macro, re-exports). No public API change; cross-module private fields became `pub(crate)`.
- Fix: Add InfoList RAII wrapper to prevent FFI memory leaks in PMIx Info list operations (#526)
- Breaking: Wrapped fabric/device FFI types in native Rust wrappers (PmixCoord, PmixDeviceType, PmixResourceUnit, PmixCoordView, PmixDeviceDistance, PmixDevice, PmixEndpoint, PmixFabric, PmixGeometry, PmixTopology, PmixCpuset). Public API no longer exposes bindgen types. Migration: use `Type::from_raw()` and `type.to_raw()` for conversions.
- Breaking: Removed public exposure of `pmix_server_module_t` and related bindgen callback types from the pmix-rs crate. `PmixServerModule::as_c_ptr()` now returns `*const c_void` instead of `*mut pmix_server_module_t`. Migration: use an explicit `as *mut pmix_server_module_t` cast where the old return type was required, and uphold the unsafe FFI contract when using the result. See #550 and #518.

## 0.1.0

- Initial public-facing packaging pass
- Portable `build.rs` via `PMIX_PREFIX` / `PMIX_INCLUDE_DIR` / `PMIX_LIB_DIR`
- Simple put/get/commit/fence examples
- Full PMIx 5.x bindgen surface + modular safe wrappers
