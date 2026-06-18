//! Phase 4 Batch 3: PmixCpuset Basic Operations
//!
//! Tests for PmixCpuset construction, accessors, Debug, and type traits.
//! Pure user-space — no PMIx init required.

use pmix::fabric::PmixCpuset;

// ── Construction tests ──

/// Test that PmixCpuset can be created via new() (calls FFI construct).
#[test]
fn test_cpuset_new() {
    let mut cpuset = PmixCpuset::new();
    let _ptr = cpuset.as_mut_ptr();
}

/// Test that PmixCpuset can be created via Default.
#[test]
fn test_cpuset_default() {
    let mut cpuset = PmixCpuset::default();
    let _ptr = cpuset.as_mut_ptr();
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
    let mut cpuset1 = PmixCpuset::new();
    let mut cpuset2 = PmixCpuset::new();
    let mut cpuset3 = PmixCpuset::default();

    let _ptr1 = cpuset1.as_mut_ptr();
    let _ptr2 = cpuset2.as_mut_ptr();
    let _ptr3 = cpuset3.as_mut_ptr();
}

/// Test that as_mut_ptr returns a non-null pointer.
#[test]
fn test_cpuset_as_mut_ptr_non_null() {
    let mut cpuset = PmixCpuset::new();
    let ptr = cpuset.as_mut_ptr();
    assert!(!ptr.is_null());
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

/// Test that as_mut_ptr can be called multiple times.
#[test]
fn test_cpuset_as_mut_ptr_repeated() {
    let mut cpuset = PmixCpuset::new();
    let ptr1 = cpuset.as_mut_ptr();
    let ptr2 = cpuset.as_mut_ptr();
    assert_eq!(ptr1, ptr2);
}
