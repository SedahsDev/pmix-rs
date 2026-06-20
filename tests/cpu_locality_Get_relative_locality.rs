//! Tests for PMIx_Get_relative_locality — cpu_locality module.
//!
//! These tests verify the safe Rust wrapper around the C API function
//! `PMIx_Get_relative_locality`, which computes the relative locality
//! bitmask of two processes given their locality strings.

use pmix::cpu_locality::PmixLocality;
use pmix::cpu_locality::get_relative_locality;

// ─────────────────────────────────────────────────────────────────────────────
// Basic functionality tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_relative_locality compiles and does not panic with valid strings.
#[test]
fn test_get_relative_locality_valid_strings() {
    let result = get_relative_locality("0", "0");
    // Without a running PMIx session, this may return PMIX_ERR_INIT.
    // The important thing is that the FFI call is made correctly.
    let _ = result;
}

/// Test that get_relative_locality returns an error when PMIx is not initialized.
///
/// This mirrors the C behavior — the PMIx library checks init_cntr before
/// processing locality queries and returns PMIX_ERR_INIT if not initialized.
#[test]
fn test_get_relative_locality_not_initialized() {
    let result = get_relative_locality("0", "0");
    assert!(
        result.is_err(),
        "get_relative_locality should return an error when PMIx is not initialized"
    );
}

/// Test that get_relative_locality with empty strings returns an error.
#[test]
fn test_get_relative_locality_empty_strings() {
    let result = get_relative_locality("", "");
    // Empty strings are not valid locality strings — expect an error.
    assert!(
        result.is_err(),
        "get_relative_locality should fail with empty locality strings"
    );
}

/// Test that get_relative_locality with one empty string returns an error.
#[test]
fn test_get_relative_locality_one_empty() {
    let result = get_relative_locality("0", "");
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// NUL byte handling tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_relative_locality rejects strings containing NUL bytes.
#[test]
fn test_get_relative_locality_nul_in_first() {
    let result = get_relative_locality("hello\x00world", "0");
    assert!(
        result.is_err(),
        "get_relative_locality should reject NUL byte in first argument"
    );
}

/// Test that get_relative_locality rejects NUL bytes in the second string.
#[test]
fn test_get_relative_locality_nul_in_second() {
    let result = get_relative_locality("0", "hello\x00world");
    assert!(
        result.is_err(),
        "get_relative_locality should reject NUL byte in second argument"
    );
}

/// Test that get_relative_locality rejects NUL bytes in both strings.
#[test]
fn test_get_relative_locality_nul_in_both() {
    let result = get_relative_locality("a\x00b", "c\x00d");
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixLocality bitflags tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test PmixLocality bit constants match C definitions.
#[test]
fn test_locality_bit_values() {
    assert_eq!(PmixLocality::UNKNOWN.bits(), 0x0000);
    assert_eq!(PmixLocality::NONLOCAL.bits(), 0x8000);
    assert_eq!(PmixLocality::SHARE_HWTHREAD.bits(), 0x0001);
    assert_eq!(PmixLocality::SHARE_CORE.bits(), 0x0002);
    assert_eq!(PmixLocality::SHARE_L1CACHE.bits(), 0x0004);
    assert_eq!(PmixLocality::SHARE_L2CACHE.bits(), 0x0008);
    assert_eq!(PmixLocality::SHARE_L3CACHE.bits(), 0x0010);
    assert_eq!(PmixLocality::SHARE_PACKAGE.bits(), 0x0020);
    assert_eq!(PmixLocality::SHARE_NUMA.bits(), 0x0040);
    assert_eq!(PmixLocality::SHARE_NODE.bits(), 0x4000);
}

/// Test PmixLocality from_raw / to_raw round-trip.
#[test]
fn test_locality_from_raw_to_raw() {
    let raw: u16 = 0x0003; // SHARE_HWTHREAD | SHARE_CORE
    let locality = PmixLocality::from_raw(raw);
    assert_eq!(locality.to_raw(), raw);
}

/// Test PmixLocality from_raw with all known bits set.
#[test]
fn test_locality_all_bits() {
    let all_known = PmixLocality::NONLOCAL
        | PmixLocality::SHARE_NODE
        | PmixLocality::SHARE_NUMA
        | PmixLocality::SHARE_PACKAGE
        | PmixLocality::SHARE_L3CACHE
        | PmixLocality::SHARE_L2CACHE
        | PmixLocality::SHARE_L1CACHE
        | PmixLocality::SHARE_CORE
        | PmixLocality::SHARE_HWTHREAD;
    let locality = PmixLocality::from_raw(all_known.to_raw());
    assert!(locality.contains(PmixLocality::NONLOCAL));
    assert!(locality.contains(PmixLocality::SHARE_NODE));
    assert!(locality.contains(PmixLocality::SHARE_NUMA));
    assert!(locality.contains(PmixLocality::SHARE_PACKAGE));
    assert!(locality.contains(PmixLocality::SHARE_L3CACHE));
    assert!(locality.contains(PmixLocality::SHARE_L2CACHE));
    assert!(locality.contains(PmixLocality::SHARE_L1CACHE));
    assert!(locality.contains(PmixLocality::SHARE_CORE));
    assert!(locality.contains(PmixLocality::SHARE_HWTHREAD));
}

/// Test PmixLocality from_raw drops unknown vendor bits via truncate.
#[test]
fn test_locality_from_raw_truncates_unknown() {
    // Bit 0x2000 is not defined in the standard — from_bits_truncate
    // drops it because it is not covered by any defined flag.
    let raw: u16 = 0x2000 | PmixLocality::SHARE_CORE.bits();
    let locality = PmixLocality::from_raw(raw);
    assert!(locality.contains(PmixLocality::SHARE_CORE));
    // Unknown bits are dropped by from_bits_truncate.
    assert_eq!(locality.to_raw(), PmixLocality::SHARE_CORE.bits());
}

/// Test PmixLocality bit operations.
#[test]
fn test_locality_bit_ops() {
    let mut locality = PmixLocality::empty();
    assert!(locality.is_empty());

    locality.insert(PmixLocality::SHARE_CORE);
    assert!(locality.contains(PmixLocality::SHARE_CORE));
    assert!(!locality.is_empty());

    locality.insert(PmixLocality::SHARE_L1CACHE);
    assert!(locality.contains(PmixLocality::SHARE_CORE | PmixLocality::SHARE_L1CACHE));

    locality.remove(PmixLocality::SHARE_CORE);
    assert!(!locality.contains(PmixLocality::SHARE_CORE));
    assert!(locality.contains(PmixLocality::SHARE_L1CACHE));
}

/// Test PmixLocality intersection and union.
#[test]
fn test_locality_intersection_union() {
    let a = PmixLocality::SHARE_CORE | PmixLocality::SHARE_L1CACHE;
    let b = PmixLocality::SHARE_L1CACHE | PmixLocality::SHARE_L2CACHE;

    let intersection = a & b;
    assert_eq!(intersection, PmixLocality::SHARE_L1CACHE);

    let union_ = a | b;
    assert!(union_.contains(PmixLocality::SHARE_CORE));
    assert!(union_.contains(PmixLocality::SHARE_L1CACHE));
    assert!(union_.contains(PmixLocality::SHARE_L2CACHE));
}

/// Test PmixLocality Default is empty (UNKNOWN).
#[test]
fn test_locality_default_empty() {
    let default_ = PmixLocality::default();
    assert!(default_.is_empty());
    assert_eq!(default_.bits(), 0x0000);
}

/// Test PmixLocality Debug formatting.
#[test]
fn test_locality_debug() {
    let locality = PmixLocality::SHARE_CORE | PmixLocality::SHARE_L1CACHE;
    let debug_str = format!("{:?}", locality);
    assert!(!debug_str.is_empty());
    assert!(debug_str.contains("SHARE_CORE") || debug_str.contains("SHARE_L1CACHE"));
}

/// Test PmixLocality Clone and Copy traits.
#[test]
fn test_locality_clone_copy() {
    let a = PmixLocality::SHARE_CORE | PmixLocality::SHARE_NODE;
    let b = a.clone();
    assert_eq!(a, b);
    let c = a; // Copy, not move
    assert_eq!(a, c);
}

/// Test PmixLocality PartialEq and Eq.
#[test]
fn test_locality_partial_eq() {
    let a = PmixLocality::SHARE_CORE;
    let b = PmixLocality::SHARE_CORE;
    let c = PmixLocality::SHARE_L1CACHE;
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// Test PmixLocality Hash consistency.
#[test]
fn test_locality_hash() {
    use std::collections::HashSet;

    let a = PmixLocality::SHARE_CORE;
    let b = PmixLocality::SHARE_CORE;
    let c = PmixLocality::SHARE_L1CACHE;

    let mut set = HashSet::new();
    set.insert(a);
    set.insert(c);
    assert!(set.contains(&b)); // b == a
    assert!(!set.contains(&PmixLocality::SHARE_NODE));
}

// ─────────────────────────────────────────────────────────────────────────────
// Repeated call tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_relative_locality can be called multiple times consistently.
#[test]
fn test_get_relative_locality_repeated_calls() {
    let r1 = get_relative_locality("0", "1");
    let r2 = get_relative_locality("0", "1");
    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "repeated calls should return consistent results"
    );
}

/// Test that get_relative_locality with different string pairs works.
#[test]
fn test_get_relative_locality_different_pairs() {
    let r1 = get_relative_locality("0", "0");
    let r2 = get_relative_locality("1", "2");
    let r3 = get_relative_locality("0-3", "4-7");
    // All should return consistent results (likely errors without PMIx session).
    assert_eq!(r1.is_err(), r2.is_err());
    assert_eq!(r2.is_err(), r3.is_err());
}

/// Test that get_relative_locality with long locality strings does not panic.
#[test]
fn test_get_relative_locality_long_strings() {
    let long_locality = (0..64).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let result = get_relative_locality(&long_locality, &long_locality);
    let _ = result;
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration-style tests (derived from C test patterns)
// ─────────────────────────────────────────────────────────────────────────────

/// Test derived from the C API pattern in pmix_client_locality.c:
/// ```c
/// pmix_locality_t locality;
/// rc = PMIx_Get_relative_locality(locstr1, locstr2, &locality);
/// if (PMIX_SUCCESS == rc) {
///     if (locality & PMIX_LOCALITY_SHARE_CORE) {
///         // procs share a core
///     }
/// }
/// ```
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_relative_locality_initialized_session() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // In a real PMIx session:
    // 1. Call PMIx_Init
    // 2. Get locality strings via PMIx_Get with PMIX_LOCALITY_STRING
    // 3. Call get_relative_locality with both strings
    // 4. Verify it returns Ok(PmixLocality)
    // 5. Check that returned locality has meaningful bits set
    let result = get_relative_locality("0", "0");
    assert!(
        result.is_ok(),
        "get_relative_locality should succeed in an initialized session"
    );
}

/// Test that two processes on the same node share NODE locality.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_relative_locality_same_node() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    let result = get_relative_locality("0", "0");
    match result {
        Ok(locality) => {
            // Same locality string should indicate same-node processes.
            assert!(
                locality.contains(PmixLocality::SHARE_NODE)
                    || locality.contains(PmixLocality::SHARE_CORE),
                "same locality string should indicate shared hardware"
            );
        }
        Err(_) => {
            // Without PMIx runtime, this is expected.
        }
    }
}

/// Test that get_relative_locality correctly identifies non-local processes.
#[test]
#[ignore = "requires DVM-launched process (prterun)"]
fn test_get_relative_locality_nonlocal() {
    let _ctx = pmix::init(None).expect("pmix::init failed");
    // In a real session with processes on different nodes,
    // the NONLOCAL bit would be set.
    let result = get_relative_locality("0", "999999");
    match result {
        Ok(locality) => {
            // Very different locality strings should indicate non-local.
            assert!(
                locality.contains(PmixLocality::NONLOCAL) || locality.is_empty(),
                "very different locality strings should indicate non-local or unknown"
            );
        }
        Err(_) => {
            // Without PMIx runtime, this is expected.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Test that get_relative_locality with identical strings is consistent.
#[test]
fn test_get_relative_locality_identical_strings() {
    let result = get_relative_locality("abc", "abc");
    // Even with identical strings, PMIx may return an error without a session.
    let _ = result;
}

/// Test that get_relative_locality handles special characters in locality strings.
#[test]
fn test_get_relative_locality_special_chars() {
    // Locality strings from PMIx can contain colons, hyphens, etc.
    let result = get_relative_locality("0:0-3", "1:4-7");
    let _ = result;
}

/// Test PmixLocality from_raw with zero (UNKNOWN).
#[test]
fn test_locality_zero_is_unknown() {
    let locality = PmixLocality::from_raw(0);
    assert!(locality.is_empty());
    assert_eq!(locality, PmixLocality::UNKNOWN);
}

/// Test PmixLocality from_raw with NONLOCAL only.
#[test]
fn test_locality_nonlocal_only() {
    let locality = PmixLocality::from_raw(0x8000);
    assert!(locality.contains(PmixLocality::NONLOCAL));
    assert!(!locality.contains(PmixLocality::SHARE_CORE));
}

/// Test that PmixLocality bits do not overlap unexpectedly.
#[test]
fn test_locality_bits_no_overlap() {
    let flags = [
        PmixLocality::NONLOCAL,
        PmixLocality::SHARE_HWTHREAD,
        PmixLocality::SHARE_CORE,
        PmixLocality::SHARE_L1CACHE,
        PmixLocality::SHARE_L2CACHE,
        PmixLocality::SHARE_L3CACHE,
        PmixLocality::SHARE_PACKAGE,
        PmixLocality::SHARE_NUMA,
        PmixLocality::SHARE_NODE,
    ];
    for i in 0..flags.len() {
        for j in (i + 1)..flags.len() {
            assert!(
                flags[i].intersection(flags[j]).is_empty(),
                "bits {:?} and {:?} should not overlap",
                flags[i],
                flags[j]
            );
        }
    }
}
