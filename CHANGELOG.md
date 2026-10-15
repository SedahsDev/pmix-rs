# Changelog

## Unreleased

- Fix: Add InfoList RAII wrapper to prevent FFI memory leaks in PMIx Info list operations (#526)
- Breaking: Wrapped fabric/device FFI types in native Rust wrappers (PmixCoord, PmixDeviceType, PmixResourceUnit, PmixCoordView, PmixDeviceDistance, PmixDevice, PmixEndpoint, PmixFabric, PmixGeometry, PmixTopology, PmixCpuset). Public API no longer exposes bindgen types. Migration: use `Type::from_raw()` and `type.to_raw()` for conversions.

## 0.1.0

- Initial public-facing packaging pass
- Portable `build.rs` via `PMIX_PREFIX` / `PMIX_INCLUDE_DIR` / `PMIX_LIB_DIR`
- Simple put/get/commit/fence examples
- Full PMIx 5.x bindgen surface + modular safe wrappers
