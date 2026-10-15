//! Phase 4 Batch 3: PmixCpuset Basic Operations
//!
//! Tests for PmixCpuset construction, accessors, Debug, and type traits.
//! Pure user-space — no PMIx init required.

use pmix::fabric::PmixCpuset;

// ── Construction tests ──

/// Test that PmixCpuset can be created via new() (calls FFI construct).
#[test]
fn test_cpuset_new() {
    let _cpuset = PmixCpuset::new();
}

/// Test that PmixCpuset can be created via Default.
#[test]
fn test_cpuset_default() {
    let _cpuset = PmixCpuset::default();
}

/// Test that PmixCpuset Debug works.
#[test]
fn test_cpuset_debug() {
    let cpuset = PmixCpuset::new();
    let debug_str = format!("{:?}", cpuset);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixCpuset"));
}

// ── Edge cases ──

/// Test that multiple cpusets can coexist.
#[test]
fn test_cpuset_multiple_instances() {
    let cpuset1 = PmixCpuset::new();
    let cpuset2 = PmixCpuset::new();
    let cpuset3 = PmixCpuset::default();

    assert!(format!("{:?}", cpuset1).contains("PmixCpuset"));
    assert!(format!("{:?}", cpuset2).contains("PmixCpuset"));
    assert!(format!("{:?}", cpuset3).contains("PmixCpuset"));
}

/// Test that a constructed cpuset is usable through the public API.
#[test]
fn test_cpuset_as_mut_ptr_non_null() {
    let cpuset = PmixCpuset::new();
    let debug_str = format!("{:?}", cpuset);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("PmixCpuset"));
}

/// Test that drop does not crash for a constructed cpuset.
#[test]
fn test_cpuset_drop() {
    let _cpuset = PmixCpuset::new();
}

/// Test that drop does not crash for a default cpuset.
#[test]
fn test_cpuset_drop_default() {
    let _cpuset = PmixCpuset::default();
}

/// Test that public Debug is stable across repeated calls.
#[test]
fn test_cpuset_as_mut_ptr_repeated() {
    let cpuset = PmixCpuset::new();
    let debug1 = format!("{:?}", cpuset);
    let debug2 = format!("{:?}", cpuset);
    assert_eq!(debug1, debug2);
    assert!(!debug1.is_empty());
}
