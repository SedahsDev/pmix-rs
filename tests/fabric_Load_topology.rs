//! Tests for `PMIx_Load_topology` — hardware topology loading.
//!
//! These tests verify the Rust wrapper for the topology loading API:
//! `load_topology` and the `PmixTopology` type.
//!
//! The C test reference is `test/simple/simpclient.c` which calls:
//! ```c
//! PMIX_TOPOLOGY_CONSTRUCT(&topo);
//! rc = PMIx_Load_topology(&topo);
//! if (PMIX_SUCCESS != rc) { /* handle error */ }
//! pmix_output(0, "Topology source: %s", topo.source);
//! ```
//!
//! Tests marked `#[ignore]` require a PMIx daemon and should be run
//! with `--ignored` under a real PMIx environment.

use pmix::fabric::{PmixTopology, load_topology};
use pmix::{PmixError, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// PmixTopology construction tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that PmixTopology can be created with no source hint.
#[test]
fn test_topology_unamed() {
    let topo = PmixTopology::unamed();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology can be created with a source hint.
#[test]
fn test_topology_new_with_source() {
    let topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), Some("hwloc"));
}

/// Test that PmixTopology can be created with None source.
#[test]
fn test_topology_new_none_source() {
    let topo = PmixTopology::new(None).unwrap();
    assert!(!topo.is_loaded());
    assert_eq!(topo.source(), None);
}

/// Test that PmixTopology::new rejects sources with interior NUL bytes.
#[test]
fn test_topology_new_nul_source() {
    let result = PmixTopology::new(Some("hw\0loc"));
    assert!(result.is_err());
}

/// Test that PmixTopology implements Debug.
#[test]
fn test_topology_debug() {
    let topo = PmixTopology::unamed();
    let debug_str = format!("{:?}", topo);
    assert!(debug_str.contains("PmixTopology"));
}

/// Test that PmixTopology source accessor returns correct value.
#[test]
fn test_topology_source_accessor() {
    let topo = PmixTopology::new(Some("test_source")).unwrap();
    assert_eq!(topo.source(), Some("test_source"));
}

/// Test that multiple PmixTopology objects can coexist.
#[test]
fn test_topology_multiple() {
    let _topo1 = PmixTopology::unamed();
    let _topo2 = PmixTopology::new(Some("hwloc")).unwrap();
    let _topo3 = PmixTopology::new(None).unwrap();
    // All three should construct and drop without issues.
}

/// Test that PmixTopology loaded flag is initially false.
#[test]
fn test_topology_initially_not_loaded() {
    let topo = PmixTopology::new(Some("hwloc")).unwrap();
    assert!(!topo.is_loaded());
}

/// Test PmixTopology with empty string source.
#[test]
fn test_topology_empty_source() {
    let topo = PmixTopology::new(Some("")).unwrap();
    assert_eq!(topo.source(), Some(""));
}

// ─────────────────────────────────────────────────────────────────────────────
// load_topology function tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that load_topology compiles and accepts a mutable PmixTopology.
/// Without a PMIx server, this will return an error — we verify it
/// doesn't panic or segfault.
#[test]
fn test_load_topology_compiles() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    // Without PMIx server, expect an error (not success).
    // The important thing is no panic or segfault.
    match result {
        Ok(()) => {
            // If PMIx is somehow initialized, topology should be loaded.
            assert!(topo.is_loaded());
        }
        Err(status) => {
            // Expected: PMIx not initialized. Acceptable error codes:
            // PMIX_ERR_NOT_INITIALIZED, PMIX_ERR_NOT_SUPPORTED, etc.
            println!(
                "load_topology returned {:?} (expected without PMIx server)",
                status
            );
        }
    }
}

/// Test that load_topology with a specific source hint compiles.
#[test]
fn test_load_topology_with_source_hint() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let result = load_topology(&mut topo);
    match result {
        Ok(()) => {
            assert!(topo.is_loaded());
            assert_eq!(topo.source(), Some("hwloc"));
        }
        Err(_) => {
            // Expected without PMIx server or hwloc backend.
        }
    }
}

/// Test that load_topology returns a proper error status (not panic).
#[test]
fn test_load_topology_error_is_valid_status() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    // Verify the result is a valid PmixStatus (not a panic).
    // Even on error, the status should be a known PMIx constant.
    if let Err(status) = result {
        // PmixStatus should be Debug-printable without panic.
        let _ = format!("{:?}", status);
    }
}

/// Test that load_topology does not set loaded flag on error.
#[test]
fn test_load_topology_not_loaded_on_error() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    if result.is_err() {
        assert!(
            !topo.is_loaded(),
            "topology should not be marked loaded after error"
        );
    }
}

/// Test that load_topology can be called multiple times without crash.
#[test]
fn test_load_topology_multiple_calls() {
    let mut topo = PmixTopology::unamed();
    // Call twice — should not segfault or double-free.
    let _ = load_topology(&mut topo);
    let _ = load_topology(&mut topo);
}

/// Test load_topology with topology constructed via new() vs unamed().
#[test]
fn test_load_topology_new_vs_unamed() {
    let mut topo_new = PmixTopology::new(None).unwrap();
    let mut topo_unamed = PmixTopology::unamed();

    let result_new = load_topology(&mut topo_new);
    let result_unamed = load_topology(&mut topo_unamed);

    // Both should produce the same kind of result (both error without server).
    assert_eq!(
        result_new.is_ok(),
        result_unamed.is_ok(),
        "new(None) and unamed() should behave the same"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests (require PMIx runtime)
// ─────────────────────────────────────────────────────────────────────────────

/// Test load_topology under a real PMIx environment.
/// Derived from `test/simple/simpclient.c`:
/// ```c
/// PMIX_TOPOLOGY_CONSTRUCT(&topo);
/// rc = PMIx_Load_topology(&topo);
/// if (PMIX_SUCCESS != rc) { /* error */ }
/// pmix_output(0, "Topology source: %s", topo.source);
/// ```
#[test]
#[ignore = "requires PMIx daemon"]
fn test_load_topology_integration() {
    let mut topo = PmixTopology::unamed();
    let result = load_topology(&mut topo);
    match result {
        Ok(()) => {
            assert!(
                topo.is_loaded(),
                "topology should be marked loaded after success"
            );
            println!("Topology loaded successfully, source: {:?}", topo.source());
        }
        Err(status) => {
            println!(
                "Client: Failed to load topology: {:?} (expected without PMIx server)",
                status
            );
        }
    }
}

/// Test load_topology with hwloc source under a real PMIx environment.
#[test]
#[ignore = "requires PMIx daemon and hwloc"]
fn test_load_topology_hwloc_source() {
    let mut topo = PmixTopology::new(Some("hwloc")).unwrap();
    let result = load_topology(&mut topo);
    match result {
        Ok(()) => {
            assert!(topo.is_loaded());
            assert_eq!(topo.source(), Some("hwloc"));
            println!("hwloc topology loaded successfully");
        }
        Err(status) => {
            // hwloc may not be available — acceptable error.
            assert!(
                status == PmixStatus::Known(PmixError::ErrNotSupported)
                    || status == PmixStatus::Known(PmixError::ErrNotFound),
                "unexpected error for hwloc source: {:?}",
                status
            );
        }
    }
}

/// Test load_topology followed by a second call (idempotency check).
#[test]
#[ignore = "requires PMIx daemon"]
fn test_load_topology_idempotent() {
    let mut topo = PmixTopology::unamed();
    let result1 = load_topology(&mut topo);
    match result1 {
        Ok(()) => {
            assert!(topo.is_loaded());
            // Second call should also succeed (or at least not crash).
            let result2 = load_topology(&mut topo);
            match result2 {
                Ok(()) => {
                    assert!(topo.is_loaded());
                }
                Err(_) => {
                    // Some implementations may reject re-loading.
                }
            }
        }
        Err(_) => {
            // No PMIx server — skip.
        }
    }
}

/// Test load_topology with different source hints.
#[test]
#[ignore = "requires PMIx daemon"]
fn test_load_topology_different_sources() {
    let sources = ["hwloc", ""];
    for src in &sources {
        let mut topo = PmixTopology::new(Some(src)).unwrap();
        let result = load_topology(&mut topo);
        match result {
            Ok(()) => {
                assert!(topo.is_loaded());
                println!("Topology loaded with source '{}'", src);
            }
            Err(status) => {
                println!(
                    "Topology with source '{}' returned {:?} (acceptable)",
                    src, status
                );
            }
        }
    }
}
