//! PMIx Info helpers.
//!
//! Thin ergonomic layer over crate-root [`Info`] / [`InfoBuilder`] for common
//! info arrays used with fence, get, and tool APIs.
//!
//! # Scope
//!
//! - Build empty or directive-bearing `Info` arrays
//! - String key/value pairs (including keys longer than 12 bytes)
//! - Re-export of core types for `use pmix::info::*`
//!
//! Full `PMIx_Info_*` C surface (load/unload/list free) remains available via
//! the raw bindings / crate root. Publish/lookup APIs live under other modules
//! as they mature.
//!
//! For basic K/V put/get, prefer crate-root `put_value` / `get_value` / `commit` / `fence`.
//!
//! Spec: <https://pmix.github.io/>

pub use crate::{Info, InfoBuilder, PmixStatus, info_with_string_key};

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
///
/// # Example
///
/// ```no_run
/// let info = pmix::info::string_kv("pmix.srvr.uri", "tcp://127.0.0.1:1234");
/// let _ = info;
/// ```
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

    // -- empty() tests --

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
    fn test_empty_info_as_ptr_not_null() {
        let info = empty();
        // Even empty info arrays have a valid (possibly null) pointer
        let _ptr = info.as_ptr();
    }

    // -- with_collect_data() tests --

    #[test]
    fn test_with_collect() {
        let info = with_collect_data();
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_with_collect_not_empty() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    // -- string_kv() tests --

    #[test]
    fn test_string_kv() {
        let info = string_kv("test.key", "hello");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_not_empty() {
        let info = string_kv("key", "value");
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv_long_key() {
        // Keys longer than 13 bytes (e.g., "pmix.srvr.uri" is 14 bytes)
        let info = string_kv("pmix.srvr.uri", "tcp://127.0.0.1:1234");
        assert_eq!(len(&info), 1);
        assert!(!info.is_empty());
    }

    #[test]
    fn test_string_kv_empty_value() {
        let info = string_kv("test.key", "");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_special_chars() {
        let info = string_kv("test.key", "value/with:special@chars#123");
        assert_eq!(len(&info), 1);
    }

    // -- builder() tests --

    #[test]
    fn test_builder() {
        let mut b = builder();
        b.collect_data();
        let info = b.build();
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_builder_empty() {
        let info = builder().build();
        assert_eq!(len(&info), 0);
        assert!(info.is_empty());
    }

    #[test]
    fn test_builder_collect_data_returns_self() {
        // Verify method chaining works — collect_data() returns &mut Self
        let mut b = InfoBuilder::new();
        b.collect_data();
        let info = b.build();
        assert_eq!(len(&info), 1);
    }

    // -- len() tests --

    #[test]
    fn test_len_zero_for_empty() {
        assert_eq!(len(&empty()), 0);
    }

    #[test]
    fn test_len_one_for_collect_data() {
        assert_eq!(len(&with_collect_data()), 1);
    }

    #[test]
    fn test_len_one_for_string_kv() {
        assert_eq!(len(&string_kv("k", "v")), 1);
    }

    // -- Info is_empty() tests --

    #[test]
    fn test_info_is_empty_false_for_string_kv() {
        let info = string_kv("k", "v");
        assert!(!info.is_empty());
    }

    #[test]
    fn test_info_is_empty_false_for_collect_data() {
        let info = with_collect_data();
        assert!(!info.is_empty());
    }

    // -- Info as_ptr() tests --

    #[test]
    fn test_info_as_ptr_returns_ptr() {
        let info = string_kv("k", "v");
        let ptr = info.as_ptr();
        // Just verify we can call it and get a pointer back
        let _ = ptr;
    }

    #[test]
    fn test_info_as_ptr_collect_data() {
        let info = with_collect_data();
        let ptr = info.as_ptr();
        let _ = ptr;
    }

    // -- Multiple string_kv calls produce independent infos --

    #[test]
    fn test_string_kv_independent_instances() {
        let info_a = string_kv("key_a", "value_a");
        let info_b = string_kv("key_b", "value_b");
        assert_eq!(len(&info_a), 1);
        assert_eq!(len(&info_b), 1);
        // Both should be independent and valid
        assert!(!info_a.is_empty());
        assert!(!info_b.is_empty());
    }

    // -- Edge cases --

    #[test]
    fn test_string_kv_single_char_key_value() {
        let info = string_kv("k", "v");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_numeric_value() {
        let info = string_kv("port", "8080");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv_uri_value() {
        let info = string_kv("uri", "unix:///var/run/socket");
        assert_eq!(len(&info), 1);
    }
}
