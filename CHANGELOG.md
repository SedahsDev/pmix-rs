# Changelog

## Unreleased

- Refactor: Split the 6,298-line `src/lib.rs` into focused modules (`error`, `enums`, `types_proc`, `value_kinds`, `client`, `value_ns`, plus the inline test module as `tests.rs`). `lib.rs` is now a 4KB crate root (docs, mod decls, macro, re-exports). No public API change; cross-module private fields became `pub(crate)`.
- Fix: Add InfoList RAII wrapper to prevent FFI memory leaks in PMIx Info list operations (#526)
- Breaking: Wrapped fabric/device FFI types in native Rust wrappers (PmixCoord, PmixDeviceType, PmixResourceUnit, PmixCoordView, PmixDeviceDistance, PmixDevice, PmixEndpoint, PmixFabric, PmixGeometry, PmixTopology, PmixCpuset). Public API no longer exposes bindgen types. Migration: use `Type::from_raw()` and `type.to_raw()` for conversions.

## 0.1.0

- Initial public-facing packaging pass
- Portable `build.rs` via `PMIX_PREFIX` / `PMIX_INCLUDE_DIR` / `PMIX_LIB_DIR`
- Simple put/get/commit/fence examples
- Full PMIx 5.x bindgen surface + modular safe wrappers
