//! Tests for `PmixByteObject` — construction, accessors, and edge cases.
//!
//! These tests exercise the Rust-side wrapper without requiring PMIx_Init.
//! They verify construction, accessor methods, Default/From impls, and
//! edge cases like empty objects and zero-sized data.

use pmix::data_serialization::*;

// ─────────────────────────────────────────────────────────────────────────────
// Construction
// ─────────────────────────────────────────────────────────────────────────────

/// PmixByteObject::new() produces an empty, zeroed byte object.
#[test]
fn test_byte_object_new_is_empty() {
    let obj = PmixByteObject::new();
    assert!(obj.is_empty());
    assert_eq!(obj.size(), 0);
    assert_eq!(obj.as_slice(), &[] as &[u8]);
}

/// PmixByteObject implements Default, which delegates to new().
#[test]
fn test_byte_object_default_is_empty() {
    let obj = PmixByteObject::default();
    assert!(obj.is_empty());
    assert_eq!(obj.size(), 0);
}

/// PmixByteObject::new() can be called multiple times (no state leak).
#[test]
fn test_byte_object_new_multiple() {
    for _ in 0..10 {
        let obj = PmixByteObject::new();
        assert!(obj.is_empty());
        assert_eq!(obj.size(), 0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// From<Vec<u8>> conversion
// ─────────────────────────────────────────────────────────────────────────────

/// From<Vec<u8>> with non-empty data produces correct byte object.
#[test]
fn test_byte_object_from_vec_nonempty() {
    let data = vec![1u8, 2, 3, 4, 5];
    let obj = PmixByteObject::from(data);
    assert!(!obj.is_empty());
    assert_eq!(obj.size(), 5);
    assert_eq!(obj.as_slice(), &[1, 2, 3, 4, 5]);
}

/// From<Vec<u8>> with empty vec produces an empty byte object (not a null pointer).
#[test]
fn test_byte_object_from_vec_empty() {
    let data = Vec::<u8>::new();
    let obj = PmixByteObject::from(data);
    assert!(obj.is_empty());
    assert_eq!(obj.size(), 0);
    assert_eq!(obj.as_slice(), &[] as &[u8]);
}

/// From<Vec<u8>> with single byte.
#[test]
fn test_byte_object_from_vec_single_byte() {
    let data = vec![42u8];
    let obj = PmixByteObject::from(data);
    assert!(!obj.is_empty());
    assert_eq!(obj.size(), 1);
    assert_eq!(obj.as_slice(), &[42]);
}

/// From<Vec<u8>> with large data preserves all bytes.
#[test]
fn test_byte_object_from_vec_large() {
    let data: Vec<u8> = (0..1024).map(|i| i as u8).collect();
    let obj = PmixByteObject::from(data);
    assert!(!obj.is_empty());
    assert_eq!(obj.size(), 1024);
    let slice = obj.as_slice();
    assert_eq!(slice.len(), 1024);
    for i in 0..1024 {
        assert_eq!(slice[i], i as u8);
    }
}

/// From<Vec<u8>> with all-zero data is correctly non-empty.
#[test]
fn test_byte_object_from_vec_all_zeros() {
    let data = vec![0u8; 64];
    let obj = PmixByteObject::from(data);
    assert!(!obj.is_empty());
    assert_eq!(obj.size(), 64);
    assert_eq!(obj.as_slice(), &[0u8; 64]);
}

/// From<Vec<u8>> with max u8 values.
#[test]
fn test_byte_object_from_vec_max_u8() {
    let data = vec![255u8; 32];
    let obj = PmixByteObject::from(data);
    assert!(!obj.is_empty());
    assert_eq!(obj.size(), 32);
    for &byte in obj.as_slice() {
        assert_eq!(byte, 255);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Accessors
// ─────────────────────────────────────────────────────────────────────────────

/// as_slice() on empty object returns empty slice.
#[test]
fn test_byte_object_as_slice_empty() {
    let obj = PmixByteObject::new();
    let slice = obj.as_slice();
    assert!(slice.is_empty());
    assert_eq!(slice.len(), 0);
}

/// as_mut_ptr() returns a non-null pointer to the inner C struct.
#[test]
fn test_byte_object_as_mut_ptr() {
    let mut obj = PmixByteObject::new();
    let ptr = obj.as_mut_ptr();
    // The pointer should be valid (points to the inner field).
    assert!(!ptr.is_null());
}

/// as_mut_ptr() works on a populated byte object.
#[test]
fn test_byte_object_as_mut_ptr_populated() {
    let mut obj = PmixByteObject::from(vec![10u8, 20, 30]);
    let ptr = obj.as_mut_ptr();
    assert!(!ptr.is_null());
}

/// size() returns correct value for constructed objects.
#[test]
fn test_byte_object_size_various() {
    let empty = PmixByteObject::new();
    assert_eq!(empty.size(), 0);

    let small = PmixByteObject::from(vec![1u8]);
    assert_eq!(small.size(), 1);

    let medium = PmixByteObject::from(vec![0u8; 100]);
    assert_eq!(medium.size(), 100);
}

/// is_empty() correctly distinguishes empty vs populated objects.
#[test]
fn test_byte_object_is_empty_various() {
    let empty = PmixByteObject::new();
    assert!(empty.is_empty());

    let default_obj = PmixByteObject::default();
    assert!(default_obj.is_empty());

    let populated = PmixByteObject::from(vec![0u8]);
    assert!(!populated.is_empty());

    let from_empty_vec = PmixByteObject::from(Vec::<u8>::new());
    assert!(from_empty_vec.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Drop / lifecycle
// ─────────────────────────────────────────────────────────────────────────────

/// Multiple byte objects can be created and dropped without issues.
#[test]
fn test_byte_object_lifecycle_many() {
    for i in 0..50 {
        let obj = PmixByteObject::from(vec![i as u8; 16]);
        assert_eq!(obj.size(), 16);
        assert!(!obj.is_empty());
        // obj dropped at end of loop iteration
    }
}

/// Byte object dropped after scope exit (no double-free).
#[test]
fn test_byte_object_drop_in_scope() {
    {
        let obj = PmixByteObject::from(vec![1u8, 2, 3]);
        assert_eq!(obj.size(), 3);
    }
    // obj dropped here, should not crash
    let obj2 = PmixByteObject::new();
    assert!(obj2.is_empty());
}

/// Debug trait works on PmixByteObject.
#[test]
fn test_byte_object_debug() {
    let obj = PmixByteObject::new();
    let debug_str = format!("{:?}", obj);
    assert!(debug_str.contains("PmixByteObject"));
}
