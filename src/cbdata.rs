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

use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

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


/// Process-global callback registry with a lock-free request-ID sequence.
///
/// The sequence counter is an `AtomicUsize`, so allocating a request ID never
/// takes the callback-map lock; concurrent ops therefore never serialize on
/// the counter. An op pays ONE map lock acquisition instead of the previous
/// SEQ lock + map lock pair.
///
/// Request IDs start at 1 and saturate rather than wrap, so they can never
/// become 0 (which would encode as a null cbdata pointer).
pub struct Registry<T> {
    seq: AtomicUsize,
    map: Mutex<HashMap<usize, T>>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Registry<T> {
    /// Create an empty registry with the sequence starting at zero.
    pub fn new() -> Self {
        Self {
            seq: AtomicUsize::new(0),
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Allocate the next non-zero request ID (saturating, never 0).
    #[inline]
    pub fn next_req_id(&self) -> usize {
        loop {
            let current = self.seq.load(Ordering::Relaxed);
            let next = current.saturating_add(1).max(1);
            if self
                .seq
                .compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return next;
            }
        }
    }

    /// Allocate a request ID and insert its callback under one map lock.
    #[inline]
    pub fn insert_next(&self, value: T) -> usize {
        let req_id = self.next_req_id();
        self.lock().insert(req_id, value);
        req_id
    }

    /// Lock the callback map.
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, HashMap<usize, T>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Remove a callback from the registry.
    #[inline]
    pub fn remove(&self, req_id: usize) -> Option<T> {
        self.lock().remove(&req_id)
    }
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

    #[test]
    fn registry_req_ids_start_at_one_and_never_zero() {
        let reg = Registry::<()>::new();
        assert_eq!(reg.next_req_id(), 1);
        assert_eq!(reg.next_req_id(), 2);
        assert_eq!(reg.next_req_id(), 3);
    }

    #[test]
    fn registry_insert_next_and_remove_roundtrip() {
        let reg = Registry::new();
        let id = reg.insert_next("callback");
        assert_eq!(reg.remove(id), Some("callback"));
        assert_eq!(reg.remove(id), None);
    }

    #[test]
    fn registry_concurrent_req_ids_are_unique() {
        use std::sync::Arc;
        let reg = Arc::new(Registry::<()>::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let reg = Arc::clone(&reg);
            handles.push(std::thread::spawn(move || {
                let mut ids = Vec::new();
                for _ in 0..100 {
                    ids.push(reg.next_req_id());
                }
                ids
            }));
        }
        let mut all: Vec<usize> = Vec::new();
        for h in handles {
            all.extend(h.join().unwrap());
        }
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 800);
        assert!(all.iter().all(|&id| id != 0));
    }
}
