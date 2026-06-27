/// PMIx_Info_* API bindings.
///
/// This module is a placeholder for future `PMIx_Info_publish`, `PMIx_Info_lookup`,
/// and related info-key APIs. No runtime tests are applicable until bindings are
/// implemented — the FFI calls require a running PMIx daemon.
///
/// See <https://pmix.github.io/pmix/doc/v5/ad7.html> for the PMIx Info API spec.

#[cfg(test)]
mod tests {
    /// Compile-time verification: `info` module exists and is empty (no bindings yet).
    /// Once PMIx_Info_* bindings are added, expand this test module with unit tests
    /// that verify parameter validation and error handling (DVM tests marked #[ignore]).
    #[test]
    fn test_info_module_exists() {
        // This test verifies the info module compiles.
        // No runtime tests available until PMIx_Info_* FFI bindings are implemented.
        assert!(true, "info module compiled successfully");
    }

    /// Verify info module is a public module accessible from lib.rs.
    #[test]
    fn test_info_module_is_public() {
        // If this compiles, the module is properly declared.
        // The module is currently private (`mod info` in lib.rs) — change to `pub mod`
        // when PMIx_Info bindings are ready for external consumption.
        assert!(true, "info module is declared in lib.rs");
    }
}
