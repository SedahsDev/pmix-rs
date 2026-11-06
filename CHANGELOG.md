# Changelog

## Unreleased

- Breaking: Wrapped pmix_group_opt_t in GroupOption enum and pmix_group_operation_t in GroupOperation enum. group_join() and group_join_nb() now take GroupOption instead of pmix_group_opt_t. Migration: use GroupOption::Accept or GroupOption::Decline instead of raw FFI values. GroupOperation implements Display and as_str() for string conversion.
- Breaking: Wrapped pmix_resource_block_directive_t in ResourceBlockDirective enum. Functions resource_block_directive_string, resource_block, and resource_block_nb now take ResourceBlockDirective instead of raw u8. Migration: use ResourceBlockDirective::Standard, ResourceBlockDirective::Debug, etc.
- Breaking: Wrapped pmix_app_t access in PmixAppObject. as_mut_ptr() is now pub(crate); use typed accessors nprocs(), cmd(), argv(), env(), info() instead of direct struct field access.

- Breaking: Wrapped pmix_query_t in PmixQuery with typed builders. pmix_query_t no longer appears in public signatures. Migration: use PmixQuery::builder().keys(...).qualifiers(...).build() and PmixQueryConstructed::as_mut_ptr() for FFI calls.
- Breaking: Wrapped pmix_regattr_t in PmixRegattr with typed accessors. pmix_regattr_t no longer appears in public signatures. Migration: use PmixRegattr::name(), string(), type_(), description() instead of raw field access.
- Refactor: Split the 6,298-line `src/lib.rs` into focused modules (`error`, `enums`, `types_proc`, `value_kinds`, `client`, `value_ns`, plus the inline test module as `tests.rs`). `lib.rs` is now a 4KB crate root (docs, mod decls, macro, re-exports). No public API change; cross-module private fields became `pub(crate)`.
- Fix: Add InfoList RAII wrapper to prevent FFI memory leaks in PMIx Info list operations (#526)
- Fix: Wrap data serialization types (PmixDataBuffer, PmixByteObject) to prevent FFI type leakage (#543)
- Breaking: Wrapped fabric/device FFI types in native Rust wrappers (PmixCoord, PmixDeviceType, PmixResourceUnit, PmixCoordView, PmixDeviceDistance, PmixDevice, PmixEndpoint, PmixFabric, PmixGeometry, PmixTopology, PmixCpuset). Public API no longer exposes bindgen types. Migration: use `Type::from_raw()` and `type.to_raw()` for conversions.
- Breaking: Removed public exposure of `pmix_server_module_t` and related bindgen callback types from the pmix-rs crate. `PmixServerModule::as_c_ptr()` now returns `*const c_void` instead of `*mut pmix_server_module_t`. Migration: use an explicit `as *mut pmix_server_module_t` cast where the old return type was required, and uphold the unsafe FFI contract when using the result. See #550 and #518.

## 0.1.0

- Initial public-facing packaging pass
- Portable `build.rs` via `PMIX_PREFIX` / `PMIX_INCLUDE_DIR` / `PMIX_LIB_DIR`
- Simple put/get/commit/fence examples
- Full PMIx 5.x bindgen surface + modular safe wrappers
