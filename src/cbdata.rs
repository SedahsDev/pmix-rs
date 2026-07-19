//! Portable encoding of non-zero request IDs as opaque `*mut c_void` cbdata.
//!
//! PMIx non-blocking APIs pass an opaque `void *cbdata` through to C callbacks.
//! Historically this crate encoded IDs with `(id << 2) as *mut c_void` to avoid
//! null and force alignment. That integer bit-shifting is a portability and
//! provenance hazard under strict-provenance rules.
//!
//! Instead we:
//! 1. Guarantee request IDs start at 1 (never zero / null).
//! 2. Use [`std::ptr::with_exposed_provenance_mut`] / [`pointer::addr`] so the
//!    conversion is explicit and portable.

use std::os::raw::c_void;

/// Encode a non-zero request ID as opaque cbdata for a PMIx C callback.
///
/// # Panics
/// Debug builds panic if `req_id == 0` (would become a null pointer).
#[inline]
pub fn encode_req_id(req_id: usize) -> *mut c_void {
    debug_assert!(
        req_id != 0,
        "request id must be non-zero to avoid null cbdata"
    );
    std::ptr::with_exposed_provenance_mut::<c_void>(req_id)
}

/// Decode opaque cbdata back into the request ID.
#[inline]
pub fn decode_req_id(cbdata: *mut c_void) -> usize {
    cbdata.addr()
}

/// Encode a `u64` request ID (used by some monitoring paths).
#[inline]
pub fn encode_req_id_u64(req_id: u64) -> *mut c_void {
    debug_assert!(
        req_id != 0,
        "request id must be non-zero to avoid null cbdata"
    );
    std::ptr::with_exposed_provenance_mut::<c_void>(req_id as usize)
}

/// Decode opaque cbdata into a `u64` request ID.
#[inline]
pub fn decode_req_id_u64(cbdata: *mut c_void) -> u64 {
    cbdata.addr() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_small_ids() {
        for id in [1usize, 2, 3, 42, 999, usize::MAX / 4] {
            let p = encode_req_id(id);
            assert!(!p.is_null());
            assert_eq!(decode_req_id(p), id);
        }
    }

    #[test]
    fn never_null_for_nonzero() {
        assert!(!encode_req_id(1).is_null());
    }
}
