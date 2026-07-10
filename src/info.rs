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

    #[test]
    fn test_empty_info() {
        let info = empty();
        assert_eq!(len(&info), 0);
    }

    #[test]
    fn test_with_collect() {
        let info = with_collect_data();
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_string_kv() {
        let info = string_kv("test.key", "hello");
        assert_eq!(len(&info), 1);
    }

    #[test]
    fn test_builder() {
        let mut b = builder();
        b.collect_data();
        let info = b.build();
        assert_eq!(len(&info), 1);
    }
}
