//! PMIx Info helpers.
//!
//! Thin ergonomic layer over crate-root [`Info`] / [`InfoBuilder`].

pub use crate::{Info, InfoBuilder, InfoFlags, PmixStatus, info_with_string_key};

/// Create an empty `Info` list (length 0).
pub fn empty() -> Info {
    InfoBuilder::new().build()
}

/// Info list with `PMIX_COLLECT_DATA` set (common fence/get pattern).
pub fn with_collect_data() -> Info {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    builder.build()
}

/// Single string key/value info entry (no 13-byte key limit).
pub fn string_kv(key: &str, value: &str) -> Info {
    info_with_string_key(key, value)
}

/// Builder starting point (same as [`InfoBuilder::new`]).
pub fn builder() -> InfoBuilder {
    InfoBuilder::new()
}

/// Length of an info array.
pub fn len(info: &Info) -> usize {
    info.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_info() {
        let info = empty();
        assert_eq!(len(&info), 0);
    }

    #[test]
    fn test_empty_info_is_empty() {
        let info = empty();
        assert!(info.is_empty());
    }

    #[test]
    fn test_with_collect_data_non_empty() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv() {
        let info = string_kv("pmix.srvr.uri", "tcp://127.0.0.1:1");
        assert_eq!(info.len(), 1);
    }

    #[test]
    fn test_info_as_ptr_returns_ptr() {
        let info = empty();
        let _ = info.as_ptr();
    }

    #[test]
    fn test_info_as_ptr_collect_data() {
        let info = with_collect_data();
        assert!(!info.as_ptr().is_null() || info.len() == 0);
    }

    #[test]
    fn test_info_is_empty_false_for_collect_data() {
        assert!(!with_collect_data().is_empty());
    }

    #[test]
    fn test_info_is_empty_false_for_string_kv() {
        assert!(!string_kv("k", "v").is_empty());
    }
}
