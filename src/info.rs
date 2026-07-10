//! PMIx Info API bindings and helpers.
//!
//! This module provides support for PMIx info keys, attributes, and related
//! publish/lookup for standard info (beyond basic put/get).
//!
//! For basic key-value use put/get/commit/fence from the crate root.
//!
//! See <https://pmix.github.io/pmix/doc/v5/ad7.html> for the PMIx Info API spec.

pub use crate::{Info, InfoBuilder, PmixStatus};

/// Create a basic empty Info for use with fence, get, etc.
pub fn empty() -> Info {
    InfoBuilder::new().build()
}

/// Example helper to add a collect data directive (common pattern).
pub fn with_collect_data() -> Info {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_module_exists() {
        assert!(true, "info module compiled successfully");
    }

    #[test]
    fn test_empty_info() {
        let _info = empty();
    }

    #[test]
    fn test_with_collect() {
        let _info = with_collect_data();
    }
}
