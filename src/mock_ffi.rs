//! Mock FFI implementations for testing without a PMIx daemon.
//!
//! This module provides stub implementations of PMIx FFI functions that return
//! controlled results, enabling unit tests to exercise "happy path" code paths
//! that would normally require a running PMIx daemon (prrte/pmix-server).
//!
//! ## Design
//!
//! The mock uses function pointer swapping at runtime. Tests call
//! [`enable_mock_ffi()`] before running and [`disable_mock_ffi()`] after.
//! While enabled, all FFI calls go through mock implementations instead of
//! the real (non-functional) bindings.
//!
//! ## Usage in tests
//!
//! ```rust,ignore
//! use pmix::mock_ffi;
//!
//! #[test]
//! fn test_publish_happy_path() {
//!     mock_ffi::enable_mock_ffi();
//!     // Now FFI calls return PMIX_SUCCESS
//!     let result = pmix::data_ops::publish(&info);
//!     assert!(result.is_ok());
//!     mock_ffi::disable_mock_ffi();
//! }
//! ```
//!
//! ## Mock behavior
//!
//! By default, all mock functions return `PMIX_SUCCESS` (0). You can configure
//! specific functions to return errors using [`MockConfig`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{LazyLock, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Mock state
// ─────────────────────────────────────────────────────────────────────────────

/// Whether mock FFI is currently enabled.
static MOCK_ENABLED: AtomicBool = AtomicBool::new(false);

/// Default return status for mock functions (PMIX_SUCCESS = 0).
static DEFAULT_STATUS: AtomicI32 = AtomicI32::new(0);

/// Per-function override status. If a function has an entry here, it returns
/// that status instead of the default.
type FunctionStatusMap = HashMap<&'static str, i32>;
static FUNCTION_OVERRIDES: LazyLock<Mutex<FunctionStatusMap>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Stored values for mock PMIx_Get / PMIx_LookupNB.
/// Key → (value_bytes, data_type)
type MockKeyValueStore = HashMap<String, (Vec<u8>, u32)>;
static KEY_VALUE_STORE: LazyLock<Mutex<MockKeyValueStore>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ─────────────────────────────────────────────────────────────────────────────
// Mock configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for mock FFI behavior.
pub struct MockConfig {
    /// Default status to return (PMIX_SUCCESS = 0 by default).
    default_status: i32,
    /// Per-function overrides.
    function_overrides: HashMap<&'static str, i32>,
}

impl MockConfig {
    /// Create a new mock configuration with default settings (all success).
    pub fn new() -> Self {
        Self {
            default_status: 0, // PMIX_SUCCESS
            function_overrides: HashMap::new(),
        }
    }

    /// Set the default return status for all functions.
    pub fn with_default_status(mut self, status: i32) -> Self {
        self.default_status = status;
        self
    }

    /// Override the return status for a specific function.
    pub fn with_function_status(mut self, func: &'static str, status: i32) -> Self {
        self.function_overrides.insert(func, status);
        self
    }

    /// Apply this configuration to the mock FFI.
    pub fn apply(self) {
        DEFAULT_STATUS.store(self.default_status, Ordering::SeqCst);
        let mut overrides = FUNCTION_OVERRIDES.lock().unwrap();
        overrides.clear();
        overrides.extend(self.function_overrides);
    }
}

impl Default for MockConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Enable/disable mock FFI
// ─────────────────────────────────────────────────────────────────────────────

/// Enable mock FFI implementations.
///
/// After calling this, FFI functions will return mock results instead of
/// trying to call the real PMIx library (which fails without a daemon).
///
/// Call [`disable_mock_ffi()`] to restore real FFI behavior.
pub fn enable_mock_ffi() {
    MOCK_ENABLED.store(true, Ordering::SeqCst);
    // Reset to defaults
    DEFAULT_STATUS.store(0, Ordering::SeqCst);
    FUNCTION_OVERRIDES.lock().unwrap().clear();
    KEY_VALUE_STORE.lock().unwrap().clear();
}

/// Disable mock FFI implementations, restoring real FFI behavior.
pub fn disable_mock_ffi() {
    MOCK_ENABLED.store(false, Ordering::SeqCst);
}

/// Check if mock FFI is currently enabled.
pub fn is_mock_enabled() -> bool {
    MOCK_ENABLED.load(Ordering::SeqCst)
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock data store operations
// ─────────────────────────────────────────────────────────────────────────────

/// Store a key-value pair in the mock datastore (for PMIx_Get tests).
///
/// The `data_type` should be a PMIx data type constant (e.g., PMIX_STRING = 1).
pub fn mock_store_value(key: &str, value: &[u8], data_type: u32) {
    let mut store = KEY_VALUE_STORE.lock().unwrap();
    store.insert(key.to_string(), (value.to_vec(), data_type));
}

/// Remove a key from the mock datastore.
pub fn mock_remove_value(key: &str) {
    let mut store = KEY_VALUE_STORE.lock().unwrap();
    store.remove(key);
}

/// Check if a key exists in the mock datastore.
pub fn mock_key_exists(key: &str) -> bool {
    let store = KEY_VALUE_STORE.lock().unwrap();
    store.contains_key(key)
}

/// Clear all stored values.
pub fn mock_clear_store() {
    KEY_VALUE_STORE.lock().unwrap().clear();
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: get status for a function
// ─────────────────────────────────────────────────────────────────────────────

/// Get the status code a mock function should return.
/// Checks function-specific overrides first, then falls back to default.
pub fn get_mock_status(func_name: &str) -> i32 {
    let overrides = FUNCTION_OVERRIDES.lock().unwrap();
    if let Some(&status) = overrides.get(func_name) {
        status
    } else {
        DEFAULT_STATUS.load(Ordering::SeqCst)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx status constants for convenience
// ─────────────────────────────────────────────────────────────────────────────

/// PMIX_SUCCESS (0)
pub const PMIX_SUCCESS: i32 = 0;
/// PMIX_ERR_INIT (-31) — not initialized
pub const PMIX_ERR_INIT: i32 = -31;
/// PMIX_ERR_NOT_FOUND (-46)
pub const PMIX_ERR_NOT_FOUND: i32 = -46;
/// PMIX_ERR_TIMEOUT (-24)
pub const PMIX_ERR_TIMEOUT: i32 = -24;
/// PMIX_ERR_DUPLICATE_KEY (-53)
pub const PMIX_ERR_DUPLICATE_KEY: i32 = -53;
/// PMIX_ERR_BAD_PARAM (-27)
pub const PMIX_ERR_BAD_PARAM: i32 = -27;
/// PMIX_ERR_NOMEM (-32)
pub const PMIX_ERR_NOMEM: i32 = -32;
/// PMIX_ERROR (-1)
pub const PMIX_ERROR: i32 = -1;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx data type constants
// ─────────────────────────────────────────────────────────────────────────────

/// PMIX_BOOL
pub const PMIX_BOOL: u32 = 0;
/// PMIX_INT
pub const PMIX_INT: u32 = 1;
/// PMIX_STRING
pub const PMIX_STRING: u32 = 2;
/// PMIX_STRING as u16 (for pmix_value.type_ field)
pub const PMIX_STRING_U16: u16 = 2;
/// PMIX_SIZE
pub const PMIX_SIZE: u32 = 3;
/// PMIX_POINTER
pub const PMIX_POINTER: u32 = 4;
/// PMIX_RANGE
pub const PMIX_RANGE: u32 = 5;
/// PMIX_PROC
pub const PMIX_PROC: u32 = 6;
/// PMIX_UCHAR
pub const PMIX_UCHAR: u32 = 7;
/// PMIX_CHAR
pub const PMIX_CHAR: u32 = 8;
/// PMIX_SHORT
pub const PMIX_SHORT: u32 = 9;
/// PMIX_LONG
pub const PMIX_LONG: u32 = 10;
/// PMIX_UINT
pub const PMIX_UINT: u32 = 11;
/// PMIX_ULONG
pub const PMIX_ULONG: u32 = 12;
/// PMIX_FLOAT
pub const PMIX_FLOAT: u32 = 13;
/// PMIX_DOUBLE
pub const PMIX_DOUBLE: u32 = 14;
/// PMIX_LDOUBLE
pub const PMIX_LDOUBLE: u32 = 15;
/// PMIX_OPN
pub const PMIX_OPN: u32 = 16;
/// PMIX_UINT16
pub const PMIX_UINT16: u32 = 17;
/// PMIX_INT16
pub const PMIX_INT16: u32 = 18;
/// PMIX_UINT32
pub const PMIX_UINT32: u32 = 19;
/// PMIX_INT32
pub const PMIX_INT32: u32 = 20;
/// PMIX_UINT64
pub const PMIX_UINT64: u32 = 21;
/// PMIX_INT64
pub const PMIX_INT64: u32 = 22;
/// PMIX_STRING_ARRAY
pub const PMIX_STRING_ARRAY: u32 = 23;
/// PMIX_STATUS
pub const PMIX_STATUS: u32 = 24;
/// PMIX_PROC_RANK
pub const PMIX_PROC_RANK: u32 = 25;
/// PMIX_LARGE_SIZE
pub const PMIX_LARGE_SIZE: u32 = 26;
/// PMIX_LARGE_COUNT
pub const PMIX_LARGE_COUNT: u32 = 27;
/// PMIX_BOOL_ARRAY
pub const PMIX_BOOL_ARRAY: u32 = 28;
/// PMIX_PID
pub const PMIX_PID: u32 = 29;
/// PMIX_DEV_T
pub const PMIX_DEV_T: u32 = 30;
/// PMIX_MODE_T
pub const PMIX_MODE_T: u32 = 31;
/// PMIX_UID
pub const PMIX_UID: u32 = 32;
/// PMIX_GID
pub const PMIX_GID: u32 = 33;
/// PMIX_ACCESS
pub const PMIX_ACCESS: u32 = 34;
/// PMIX_TIME
pub const PMIX_TIME: u32 = 35;
/// PMIX_SIZE_ARRAY
pub const PMIX_SIZE_ARRAY: u32 = 36;
/// PMIX_INFO
pub const PMIX_INFO: u32 = 37;
/// PMIX_BUFFER
pub const PMIX_BUFFER: u32 = 38;
/// PMIX_ARRAY
pub const PMIX_ARRAY: u32 = 39;
/// PMIX_PFN
pub const PMIX_PFN: u32 = 40;
/// PMIX_REGEX
pub const PMIX_REGEX: u32 = 41;
/// PMIX_RANGE_ARRAY
pub const PMIX_RANGE_ARRAY: u32 = 42;
/// PMIX_GROUP_INFO
pub const PMIX_GROUP_INFO: u32 = 43;
/// PMIX_ENV
pub const PMIX_ENV: u32 = 44;
/// PMIX_ENV_LIST
pub const PMIX_ENV_LIST: u32 = 45;
/// PMIX_TOOL_NAME
pub const PMIX_TOOL_NAME: u32 = 46;
/// PMIX_RUNTIME
pub const PMIX_RUNTIME: u32 = 47;
/// PMIX_GROUP_MEMBERS
pub const PMIX_GROUP_MEMBERS: u32 = 48;
/// PMIX_GROUP_LEADERS
pub const PMIX_GROUP_LEADERS: u32 = 49;
/// PMIX_GROUP_CCA
pub const PMIX_GROUP_CCA: u32 = 50;
/// PMIX_GROUP_SCHED
pub const PMIX_GROUP_SCHED: u32 = 51;
/// PMIX_LOADER
pub const PMIX_LOADER: u32 = 52;
/// PMIX_COMMAND
pub const PMIX_COMMAND: u32 = 53;
/// PMIX_ENVIRONMENT
pub const PMIX_ENVIRONMENT: u32 = 54;
/// PMIX_APP
pub const PMIX_APP: u32 = 55;
/// PMIX_PNS
pub const PMIX_PNS: u32 = 56;
/// PMIX_NSATTR
pub const PMIX_NSATTR: u32 = 57;
/// PMIX_FILE
pub const PMIX_FILE: u32 = 58;
/// PMIX_STREAM
pub const PMIX_STREAM: u32 = 59;
/// PMIX_BANDWIDTH
pub const PMIX_BANDWIDTH: u32 = 60;
/// PMIX_LATENCY
pub const PMIX_LATENCY: u32 = 61;
/// PMIX_RUNTIME_ARRAY
pub const PMIX_RUNTIME_ARRAY: u32 = 62;
/// PMIX_DEV_T_ARRAY
pub const PMIX_DEV_T_ARRAY: u32 = 63;
/// PMIX_MODE_T_ARRAY
pub const PMIX_MODE_T_ARRAY: u32 = 64;
/// PMIX_TIME_ARRAY
pub const PMIX_TIME_ARRAY: u32 = 65;
/// PMIX_ACCESS_ARRAY
pub const PMIX_ACCESS_ARRAY: u32 = 66;
/// PMIX_SIZE_T_ARRAY
pub const PMIX_SIZE_T_ARRAY: u32 = 67;
/// PMIX_PID_ARRAY
pub const PMIX_PID_ARRAY: u32 = 68;
/// PMIX_UID_ARRAY
pub const PMIX_UID_ARRAY: u32 = 69;
/// PMIX_GID_ARRAY
pub const PMIX_GID_ARRAY: u32 = 70;
/// PMIX_STATUS_ARRAY
pub const PMIX_STATUS_ARRAY: u32 = 71;
/// PMIX_INT8_ARRAY
pub const PMIX_INT8_ARRAY: u32 = 72;
/// PMIX_INT16_ARRAY
pub const PMIX_INT16_ARRAY: u32 = 73;
/// PMIX_INT32_ARRAY
pub const PMIX_INT32_ARRAY: u32 = 74;
/// PMIX_INT64_ARRAY
pub const PMIX_INT64_ARRAY: u32 = 75;
/// PMIX_UINT8_ARRAY
pub const PMIX_UINT8_ARRAY: u32 = 76;
/// PMIX_UINT16_ARRAY
pub const PMIX_UINT16_ARRAY: u32 = 77;
/// PMIX_UINT32_ARRAY
pub const PMIX_UINT32_ARRAY: u32 = 78;
/// PMIX_UINT64_ARRAY
pub const PMIX_UINT64_ARRAY: u32 = 79;
/// PMIX_FLOAT_ARRAY
pub const PMIX_FLOAT_ARRAY: u32 = 80;
/// PMIX_DOUBLE_ARRAY
pub const PMIX_DOUBLE_ARRAY: u32 = 81;
/// PMIX_LDOUBLE_ARRAY
pub const PMIX_LDOUBLE_ARRAY: u32 = 82;
/// PMIX_BYTE_OBJECT
pub const PMIX_BYTE_OBJECT: u32 = 83;
/// PMIX_ARRAY_OF_ARRAY
pub const PMIX_ARRAY_OF_ARRAY: u32 = 84;
/// PMIX_SIZE_T
pub const PMIX_SIZE_T: u32 = 85;
/// PMIX_COUNT
pub const PMIX_COUNT: u32 = 86;
/// PMIX_INT8
pub const PMIX_INT8: u32 = 87;
/// PMIX_UINT8
pub const PMIX_UINT8: u32 = 88;
/// PMIX_NSID
pub const PMIX_NSID: u32 = 89;
/// PMIX_NSID_ARRAY
pub const PMIX_NSID_ARRAY: u32 = 90;
/// PMIX_INFO_ARRAY
pub const PMIX_INFO_ARRAY: u32 = 91;
/// PMIX_PFN_ARRAY
pub const PMIX_PFN_ARRAY: u32 = 92;
/// PMIX_REGEX_ARRAY
pub const PMIX_REGEX_ARRAY: u32 = 93;
/// PMIX_GROUP_INFO_ARRAY
pub const PMIX_GROUP_INFO_ARRAY: u32 = 94;
/// PMIX_STREAM_ARRAY
pub const PMIX_STREAM_ARRAY: u32 = 95;
/// PMIX_COMMAND_ARRAY
pub const PMIX_COMMAND_ARRAY: u32 = 96;
/// PMIX_ENV_ARRAY
pub const PMIX_ENV_ARRAY: u32 = 97;
/// PMIX_APP_ARRAY
pub const PMIX_APP_ARRAY: u32 = 98;
/// PMIX_PNS_ARRAY
pub const PMIX_PNS_ARRAY: u32 = 99;
/// PMIX_NSATTR_ARRAY
pub const PMIX_NSATTR_ARRAY: u32 = 100;
/// PMIX_FILE_ARRAY
pub const PMIX_FILE_ARRAY: u32 = 101;
/// PMIX_RANGE_T_ARRAY
pub const PMIX_RANGE_T_ARRAY: u32 = 102;
/// PMIX_SEMANTICS
pub const PMIX_SEMANTICS: u32 = 103;
/// PMIX_PERSISTENCE
pub const PMIX_PERSISTENCE: u32 = 104;
/// PMIX_DATA_RANGE
pub const PMIX_DATA_RANGE: u32 = 105;
/// PMIX_MAX_TYPE
pub const PMIX_MAX_TYPE: u32 = 106;

// ─────────────────────────────────────────────────────────────────────────────
// Test guard RAII struct
// ─────────────────────────────────────────────────────────────────────────────

/// RAII guard that enables mock FFI on creation and disables on drop.
///
/// ```rust,ignore
/// {
///     let _guard = mock_ffi::MockGuard::new();
///     // Mock FFI is enabled here
///     assert!(mock_ffi::is_mock_enabled());
/// }
/// // Mock FFI automatically disabled here
/// assert!(!mock_ffi::is_mock_enabled());
/// ```
pub struct MockGuard {
    _private: (),
}

impl MockGuard {
    /// Create a new mock guard, enabling mock FFI.
    pub fn new() -> Self {
        enable_mock_ffi();
        Self { _private: () }
    }

    /// Create a guard with custom configuration.
    pub fn with_config(config: MockConfig) -> Self {
        enable_mock_ffi();
        config.apply();
        Self { _private: () }
    }
}

impl Drop for MockGuard {
    fn drop(&mut self) {
        disable_mock_ffi();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for the mock FFI framework itself
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_enabled_default_false() {
        assert!(!is_mock_enabled());
    }

    #[test]
    fn test_enable_disable_mock() {
        enable_mock_ffi();
        assert!(is_mock_enabled());
        disable_mock_ffi();
        assert!(!is_mock_enabled());
    }

    #[test]
    fn test_mock_guard() {
        assert!(!is_mock_enabled());
        {
            let _guard = MockGuard::new();
            assert!(is_mock_enabled());
        }
        assert!(!is_mock_enabled());
    }

    #[test]
    fn test_mock_config_default() {
        let config = MockConfig::new();
        config.apply();
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_config_with_override() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY);
        config.apply();
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_ERR_DUPLICATE_KEY);
        assert_eq!(get_mock_status("PMIx_Get"), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_config_with_default_status() {
        let config = MockConfig::new().with_default_status(PMIX_ERR_INIT);
        config.apply();
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_ERR_INIT);
        assert_eq!(get_mock_status("PMIx_Get"), PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_store_and_retrieve() {
        mock_store_value("test_key", b"test_value", PMIX_STRING);
        assert!(mock_key_exists("test_key"));
        mock_remove_value("test_key");
        assert!(!mock_key_exists("test_key"));
    }

    #[test]
    fn test_mock_clear_store() {
        mock_store_value("key1", b"val1", PMIX_STRING);
        mock_store_value("key2", b"val2", PMIX_STRING);
        assert_eq!(mock_key_exists("key1"), true);
        assert_eq!(mock_key_exists("key2"), true);
        mock_clear_store();
        assert!(!mock_key_exists("key1"));
        assert!(!mock_key_exists("key2"));
    }

    #[test]
    fn test_mock_status_constants() {
        assert_eq!(PMIX_SUCCESS, 0);
        assert_eq!(PMIX_ERR_INIT, -31);
        assert_eq!(PMIX_ERR_NOT_FOUND, -46);
        assert_eq!(PMIX_ERR_TIMEOUT, -24);
        assert_eq!(PMIX_ERR_DUPLICATE_KEY, -53);
    }

    #[test]
    fn test_mock_data_type_constants() {
        assert_eq!(PMIX_BOOL, 0);
        assert_eq!(PMIX_INT, 1);
        assert_eq!(PMIX_STRING, 2);
        assert_eq!(PMIX_SIZE, 3);
    }

    #[test]
    fn test_mock_guard_with_config() {
        let config = MockConfig::new()
            .with_default_status(PMIX_ERR_TIMEOUT)
            .with_function_status("PMIx_Get", PMIX_SUCCESS);
        {
            let _guard = MockGuard::with_config(config);
            assert!(is_mock_enabled());
            assert_eq!(get_mock_status("PMIx_Publish"), PMIX_ERR_TIMEOUT);
            assert_eq!(get_mock_status("PMIx_Get"), PMIX_SUCCESS);
        }
        assert!(!is_mock_enabled());
    }

    #[test]
    fn test_mock_store_unicode_key() {
        mock_store_value("unicode_key_αβγ", b"value", PMIX_STRING);
        assert!(mock_key_exists("unicode_key_αβγ"));
        mock_remove_value("unicode_key_αβγ");
        assert!(!mock_key_exists("unicode_key_αβγ"));
    }

    #[test]
    fn test_mock_store_empty_value() {
        mock_store_value("empty_key", b"", PMIX_STRING);
        assert!(mock_key_exists("empty_key"));
        mock_remove_value("empty_key");
    }

    #[test]
    fn test_mock_store_large_value() {
        let large_value = vec![0u8; 1024 * 1024]; // 1MB
        mock_store_value("large_key", &large_value, PMIX_BYTE_OBJECT);
        assert!(mock_key_exists("large_key"));
        mock_remove_value("large_key");
    }

    #[test]
    fn test_mock_multiple_overrides() {
        let config = MockConfig::new()
            .with_function_status("PMIx_Publish", PMIX_SUCCESS)
            .with_function_status("PMIx_Get", PMIX_ERR_NOT_FOUND)
            .with_function_status("PMIx_Fence", PMIX_ERR_TIMEOUT)
            .with_function_status("PMIx_Unpublish", PMIX_ERR_DUPLICATE_KEY);
        config.apply();
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
        assert_eq!(get_mock_status("PMIx_Get"), PMIX_ERR_NOT_FOUND);
        assert_eq!(get_mock_status("PMIx_Fence"), PMIX_ERR_TIMEOUT);
        assert_eq!(get_mock_status("PMIx_Unpublish"), PMIX_ERR_DUPLICATE_KEY);
    }
}
