//! Mock FFI implementations for testing without a PMIx daemon.
//!
//! **Availability:** this module is compiled only under `cfg(test)` or the
//! optional Cargo feature `mock_ffi`:
//!
//! ```toml
//! pmix = { version = "0.1", features = ["mock_ffi"] }
//! ```
//!
//! Prefer [`MockGuard`] in tests so enable/disable is RAII-safe (including
//! panic paths). Direct [`enable_mock_ffi`] / [`disable_mock_ffi`] remain for
//! low-level control.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_ulong;
use std::os::unix::raw::{gid_t, uid_t};

// ─────────────────────────────────────────────────────────────────────────────
// Mock state — thread-local to avoid race conditions in parallel tests
// ─────────────────────────────────────────────────────────────────────────────

// Thread-local mock state to ensure test isolation when tests run in parallel.
//
// Each thread gets its own independent mock FFI state. This prevents race
// conditions where one test's enable/disable interferes with another test's
// expectations about the global state.
thread_local! {
    /// Whether mock FFI is currently enabled for this thread.
    static MOCK_ENABLED: RefCell<bool> = RefCell::new(false);

    /// Default return status for mock functions (PMIX_SUCCESS = 0).
    static DEFAULT_STATUS: RefCell<i32> = RefCell::new(0);

    /// Per-function override status. If a function has an entry here, it returns
    /// that status instead of the default.
    static FUNCTION_OVERRIDES: RefCell<HashMap<&'static str, i32>> = RefCell::new(HashMap::new());

    /// Stored values for mock PMIx_Get / PMIx_LookupNB.
    /// Key → (value_bytes, data_type)
    static KEY_VALUE_STORE: RefCell<HashMap<String, (Vec<u8>, u32)>> = RefCell::new(HashMap::new());
}

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
        DEFAULT_STATUS.with(|cell| {
            *cell.borrow_mut() = self.default_status;
        });
        FUNCTION_OVERRIDES.with(|cell| {
            let mut overrides = cell.borrow_mut();
            overrides.clear();
            overrides.extend(self.function_overrides);
        });
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
    MOCK_ENABLED.with(|cell| {
        *cell.borrow_mut() = true;
    });
    // Reset to defaults
    DEFAULT_STATUS.with(|cell| {
        *cell.borrow_mut() = 0;
    });
    FUNCTION_OVERRIDES.with(|cell| {
        cell.borrow_mut().clear();
    });
    KEY_VALUE_STORE.with(|cell| {
        cell.borrow_mut().clear();
    });
}

/// Disable mock FFI implementations, restoring real FFI behavior.
pub fn disable_mock_ffi() {
    MOCK_ENABLED.with(|cell| {
        *cell.borrow_mut() = false;
    });
}

/// Check if mock FFI is currently enabled.
pub fn is_mock_enabled() -> bool {
    MOCK_ENABLED.with(|cell| *cell.borrow())
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock data store operations
// ─────────────────────────────────────────────────────────────────────────────

/// Store a key-value pair in the mock datastore (for PMIx_Get tests).
///
/// The `data_type` should be a PMIx data type constant (e.g., PMIX_STRING = 1).
pub fn mock_store_value(key: &str, value: &[u8], data_type: u32) {
    KEY_VALUE_STORE.with(|cell| {
        let mut store = cell.borrow_mut();
        store.insert(key.to_string(), (value.to_vec(), data_type));
    });
}

/// Remove a key from the mock datastore.
pub fn mock_remove_value(key: &str) {
    KEY_VALUE_STORE.with(|cell| {
        let mut store = cell.borrow_mut();
        store.remove(key);
    });
}

/// Check if a key exists in the mock datastore.
pub fn mock_key_exists(key: &str) -> bool {
    KEY_VALUE_STORE.with(|cell| {
        let store = cell.borrow();
        store.contains_key(key)
    })
}

/// Clear all stored values.
pub fn mock_clear_store() {
    KEY_VALUE_STORE.with(|cell| {
        cell.borrow_mut().clear();
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: get status for a function
// ─────────────────────────────────────────────────────────────────────────────

/// Get the status code a mock function should return.
/// Checks function-specific overrides first, then falls back to default.
pub fn get_mock_status(func_name: &str) -> i32 {
    FUNCTION_OVERRIDES.with(|cell| {
        let overrides = cell.borrow();
        if let Some(&status) = overrides.get(func_name) {
            return status;
        }
        drop(overrides);
        DEFAULT_STATUS.with(|status_cell| *status_cell.borrow())
    })
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
/// PMIX_ERR_PACK_FAILURE (-21)
pub const PMIX_ERR_PACK_FAILURE: i32 = -21;
/// PMIX_ERR_UNPACK_FAILURE (-20)
pub const PMIX_ERR_UNPACK_FAILURE: i32 = -20;
/// PMIX_ERROR (-1)
pub const PMIX_ERROR: i32 = -1;
/// PMIX_ERR_NOT_SUPPORTED (-47)
pub const PMIX_ERR_NOT_SUPPORTED: i32 = -47;
/// PMIX_ERR_PARTIAL_SUCCESS (-52)
pub const PMIX_ERR_PARTIAL_SUCCESS: i32 = -52;

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

// ─────────────────────────────────────────────────────────────────────────────
// Mock data serialization FFI implementations
// ─────────────────────────────────────────────────────────────────────────────
// These provide controlled mock behavior for the 13 PMIx data serialization
// FFI functions used by data_serialization.rs. Each mock function returns
// the configured status (default PMIX_SUCCESS) and performs minimal but
// realistic state mutations so that calling code paths exercise their
// success branches without requiring a real PMIx daemon.

use std::ffi::CStr;

/// Mock implementation of `PMIx_Data_buffer_create()`.
///
/// Returns a non-null pointer (0x1 as a sentinel) when mock is enabled,
/// or null when disabled (to force real FFI path).
pub fn mock_data_buffer_create() -> *mut std::ffi::c_void {
    if is_mock_enabled() {
        // Return a sentinel pointer that won't crash on null checks
        0x1_0000 as *mut std::ffi::c_void
    } else {
        std::ptr::null_mut()
    }
}

/// Mock implementation of `PMIx_Data_buffer_release()`.
///
/// No-op when mock is enabled (the pointer is a sentinel, not real memory).
/// Returns the configured mock status.
pub fn mock_data_buffer_release(_buf: *mut std::ffi::c_void) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Data_buffer_release")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Byte_object_construct()`.
///
/// Initializes a byte object to empty state. Returns mock status.
pub fn mock_byte_object_construct() -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Byte_object_construct")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Byte_object_destruct()`.
///
/// No-op destructor when mock is enabled.
pub fn mock_byte_object_destruct() -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Byte_object_destruct")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_pack()`.
///
/// Simulates packing data into a buffer. Returns mock status.
pub fn mock_data_pack(num_vals: i32, data_type: u32) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_pack");
        // Validate parameters for realistic behavior
        if num_vals < 0 || data_type >= PMIX_MAX_TYPE {
            return PMIX_ERR_BAD_PARAM;
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_unpack()`.
///
/// Simulates unpacking data from a buffer. Returns mock status.
pub fn mock_data_unpack(max_num_values: *mut i32, data_type: u32) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_unpack");
        if data_type >= PMIX_MAX_TYPE {
            return PMIX_ERR_BAD_PARAM;
        }
        // Simulate writing unpack count
        if !max_num_values.is_null() {
            unsafe {
                *max_num_values = 1;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_unload()`.
///
/// Simulates extracting a byte object from a buffer.
pub fn mock_data_unload() -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Data_unload")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_load()`.
///
/// Simulates loading a byte object into a buffer.
pub fn mock_data_load(size: usize) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_load");
        if size == 0 {
            return PMIX_ERR_BAD_PARAM;
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_copy()`.
///
/// Simulates copying a value of given type.
pub fn mock_data_copy(data_type: u32) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_copy");
        if data_type >= PMIX_MAX_TYPE {
            return PMIX_ERR_BAD_PARAM;
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_copy_payload()`.
///
/// Simulates copying a byte object payload.
pub fn mock_data_copy_payload(src_size: usize) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_copy_payload");
        if src_size == 0 {
            return PMIX_ERR_BAD_PARAM;
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_print()`.
///
/// Simulates printing a value. Returns mock status.
pub fn mock_data_print(data_type: u32) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_print");
        if data_type >= PMIX_MAX_TYPE {
            return PMIX_ERR_BAD_PARAM;
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_embed()`.
///
/// Simulates embedding one buffer into another.
pub fn mock_data_embed() -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Data_embed")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_compress()`.
///
/// Simulates compression. Returns mock status and sets output length.
pub fn mock_data_compress(input_len: usize, out_len: *mut usize) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_compress");
        if input_len == 0 {
            return PMIX_ERR_BAD_PARAM;
        }
        // Simulate ~70% compression ratio
        if !out_len.is_null() {
            unsafe {
                *out_len = (input_len as f64 * 0.7) as usize;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Data_decompress()`.
///
/// Simulates decompression. Returns mock status and sets output length.
pub fn mock_data_decompress(input_len: usize, out_len: *mut usize) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Data_decompress");
        if input_len == 0 {
            return PMIX_ERR_BAD_PARAM;
        }
        // Simulate decompression restoring original size
        if !out_len.is_null() {
            unsafe {
                *out_len = input_len;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Get the mock byte object store for inspection in tests.
pub fn mock_get_store() -> HashMap<String, (Vec<u8>, u32)> {
    KEY_VALUE_STORE.with(|cell| cell.borrow().clone())
}

/// Get the count of stored keys in the mock datastore.
pub fn mock_store_count() -> usize {
    KEY_VALUE_STORE.with(|cell| cell.borrow().len())
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock server FFI implementations
// ─────────────────────────────────────────────────────────────────────────────
// These provide controlled mock behavior for the PMIx server-side FFI
// functions used by server.rs. Each returns the configured status
// (default PMIX_SUCCESS) so tests can exercise happy-path code paths
// without a real PMIx daemon.

/// Mock implementation of `PMIx_server_init()`.
pub fn mock_server_init(
    _module: *mut std::ffi::c_void,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_init")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_finalize()`.
pub fn mock_server_finalize() -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_finalize")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_register_nspace()`.
pub fn mock_server_register_nspace(
    _nspace: *const std::os::raw::c_char,
    _nprocs: i32,
    _info: *mut std::ffi::c_void,
    _info_len: c_ulong,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_register_nspace")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_register_client()`.
pub fn mock_server_register_client(
    _proc: *const std::ffi::c_void,
    _credential_size: *mut usize,
    _credential: *mut *mut std::os::raw::c_char,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_register_client")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_publish()`.
pub fn mock_server_publish(
    _proc: *const std::ffi::c_void,
    _key: *const std::os::raw::c_char,
    _val: *const std::ffi::c_void,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_publish")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_lookup()`.
pub fn mock_server_lookup(
    _cred: *const std::ffi::c_void,
    _key: *const std::os::raw::c_char,
    _scope: *const std::ffi::c_void,
    _scope_size: usize,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _val: *mut *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_lookup")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_unpublish()` (delete).
pub fn mock_server_delete(
    _proc: *const std::ffi::c_void,
    _key: *const std::os::raw::c_char,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_delete")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_fence()`.
pub fn mock_server_fence(
    _procs: *const std::ffi::c_void,
    _nprocs: usize,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _retvals: *mut *mut std::ffi::c_void,
    _nretvals: *mut usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_fence")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_fence_nb()`.
pub fn mock_server_fence_nb(
    _procs: *const std::ffi::c_void,
    _nprocs: usize,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_fence_nb")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_dmodex_request()`.
pub fn mock_server_dmodex_request(
    _proc: *const std::ffi::c_void,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_dmodex_request")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_setup_application()`.
pub fn mock_server_setup_application(
    _nspace: *const std::os::raw::c_char,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_setup_application")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_setup_local_support()`.
pub fn mock_server_setup_local_support(
    _nspace: *const std::os::raw::c_char,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_setup_local_support")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_register_resources()`.
pub fn mock_server_register_resources(
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_register_resources")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_deregister_resources()`.
pub fn mock_server_deregister_resources(
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_deregister_resources")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_tool_attach_to_server()`.
pub fn mock_server_tool_attach_to_server(
    _uid: uid_t,
    _gid: gid_t,
    _credential: *const std::ffi::c_void,
    _credential_size: usize,
    _info: *mut std::ffi::c_void,
    _info_len: usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_tool_attach_to_server")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_get_credential()`.
pub fn mock_server_get_credential(
    _info: *mut std::ffi::c_void,
    _info_len: usize,
    _credential: *mut *mut std::os::raw::c_char,
    _credential_size: *mut usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_get_credential")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_define_process_set()`.
pub fn mock_server_define_process_set(
    _members: *const std::ffi::c_void,
    _nmembers: usize,
    _pset_name: *const std::os::raw::c_char,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_define_process_set")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_server_delete_process_set()`.
pub fn mock_server_delete_process_set(_pset_name: *mut std::os::raw::c_char) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_server_delete_process_set")
    } else {
        PMIX_ERR_INIT
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock utility FFI implementations
// ─────────────────────────────────────────────────────────────────────────────
// These provide controlled mock behavior for the PMIx utility FFI functions
// used by utility.rs: generate_regex, generate_ppn, get_attribute_string,
// and get_attribute_name. Each returns the configured status (default
// PMIX_SUCCESS) so tests can exercise happy-path code paths without a
// real PMIx daemon.

/// Mock implementation of `PMIx_generate_regex()`.
///
/// Simulates generating a compressed regex from a node list.
/// When mock is enabled, writes a mock regex string to the output pointer
/// and returns the configured mock status.
pub fn mock_generate_regex(
    input: *const std::os::raw::c_char,
    regex: *mut *mut std::os::raw::c_char,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_generate_regex");
        // Validate input
        if input.is_null() {
            return PMIX_ERR_BAD_PARAM;
        }
        // On success, write a mock regex string to the output pointer.
        // The caller is responsible for freeing this (in real code via free()).
        // In mock tests, we just check the status and don't dereference.
        if status == PMIX_SUCCESS && !regex.is_null() {
            let mock_regex = std::ffi::CString::new("pmix:mock_regex").unwrap();
            let mock_ptr = mock_regex.into_raw() as *mut std::os::raw::c_char;
            unsafe {
                *regex = mock_ptr;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_generate_ppn()`.
///
/// Simulates generating a compressed PPN string from process rank ranges.
/// When mock is enabled, writes a mock PPN string to the output pointer
/// and returns the configured mock status.
pub fn mock_generate_ppn(
    input: *const std::os::raw::c_char,
    ppn: *mut *mut std::os::raw::c_char,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_generate_ppn");
        // Validate input
        if input.is_null() {
            return PMIX_ERR_BAD_PARAM;
        }
        // On success, write a mock PPN string to the output pointer.
        if status == PMIX_SUCCESS && !ppn.is_null() {
            let mock_ppn = std::ffi::CString::new("pmix:mock_ppn").unwrap();
            let mock_ptr = mock_ppn.into_raw() as *mut std::os::raw::c_char;
            unsafe {
                *ppn = mock_ptr;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Get_attribute_string()`.
///
/// Simulates looking up a canonical attribute string.
/// When mock is enabled, returns a pointer to the input (simulating
/// the behavior when the attribute is recognized).
pub fn mock_get_attribute_string(
    attribute: *const std::os::raw::c_char,
) -> *const std::os::raw::c_char {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Get_attribute_string");
        if status != PMIX_SUCCESS {
            return std::ptr::null();
        }
        // Return the input string as the canonical form
        attribute
    } else {
        std::ptr::null()
    }
}

/// Mock implementation of `PMIx_Get_attribute_name()`.
///
/// Simulates reverse lookup of an attribute key from a canonical string.
/// When mock is enabled, returns a pointer to the input (simulating
/// the behavior when the string is recognized).
pub fn mock_get_attribute_name(
    attrstring: *const std::os::raw::c_char,
) -> *const std::os::raw::c_char {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Get_attribute_name");
        if status != PMIX_SUCCESS {
            return std::ptr::null();
        }
        // Return the input string as the attribute key
        attrstring
    } else {
        std::ptr::null()
    }
}

/// Mock implementation of `PMIx_Register_attributes()`.
///
/// Simulates registering host environment attributes for a PMIx function.
/// When mock is enabled, returns the configured mock status.
pub fn mock_register_attributes(
    _function: *mut std::os::raw::c_char,
    _attrs: *mut *mut std::os::raw::c_char,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Register_attributes")
    } else {
        PMIX_ERR_INIT
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock IOF FFI implementations
// ─────────────────────────────────────────────────────────────────────────────
// These provide controlled mock behavior for the PMIx IO forwarding FFI
// functions used by utility.rs: PMIx_IOF_pull, PMIx_IOF_deregister,
// PMIx_IOF_push. Each returns the configured status (default PMIX_SUCCESS)
// so tests can exercise happy-path code paths without a real PMIx daemon.

thread_local! {
    /// Mock IOF registration handle counter (auto-incremented per call).
    static MOCK_IOF_HANDLE: RefCell<usize> = RefCell::new(1);
    /// Mock IOF registry: maps handles to their context pointers.
    static MOCK_IOF_REGISTRY: RefCell<HashMap<usize, *mut std::os::raw::c_void>> = RefCell::new(HashMap::new());
}

/// Set the mock IOF handle counter to a specific value.
pub fn mock_set_iof_handle(handle: usize) {
    MOCK_IOF_HANDLE.with(|cell| {
        *cell.borrow_mut() = handle;
    });
}

/// Reset the mock IOF registry (clear all handles).
pub fn mock_reset_iof_registry() {
    MOCK_IOF_REGISTRY.with(|cell| {
        cell.borrow_mut().clear();
    });
}

/// Mock implementation of `PMIx_IOF_pull()`.
///
/// When mock is enabled, returns the configured mock status.
/// On success, returns a mock handle (auto-incrementing counter)
/// and stores the context pointer for later deregistration.
pub fn mock_iof_pull(
    _procs: *const crate::ffi::pmix_proc_t,
    _nprocs: usize,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
    _channel: crate::ffi::pmix_iof_channel_t,
    _cbfunc: Option<
        unsafe extern "C" fn(
            usize,
            crate::ffi::pmix_iof_channel_t,
            *mut crate::ffi::pmix_proc_t,
            *mut crate::ffi::pmix_byte_object_t,
            *mut crate::ffi::pmix_info_t,
            usize,
        ),
    >,
    regcbfunc: Option<
        unsafe extern "C" fn(crate::ffi::pmix_status_t, usize, *mut std::os::raw::c_void),
    >,
    regcbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_IOF_pull");
        if status == PMIX_SUCCESS {
            // Generate a mock handle
            let handle = MOCK_IOF_HANDLE.with(|cell| {
                let h = *cell.borrow();
                *cell.borrow_mut() = h + 1;
                h
            });
            // Store the context pointer for later deregistration
            if !regcbdata.is_null() {
                MOCK_IOF_REGISTRY.with(|cell| {
                    cell.borrow_mut().insert(handle, regcbdata);
                });
            }
            // If a registration callback is provided, invoke it with the handle
            if let Some(cb) = regcbfunc {
                unsafe { cb(status, handle, regcbdata); }
            }
            // In blocking mode (no regcbfunc), return the handle as status
            // (matching real PMIx behavior where blocking returns the handle)
            if regcbfunc.is_none() {
                return handle as i32;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_IOF_deregister()`.
///
/// When mock is enabled, returns the configured mock status.
/// Cleans up the mock registry entry for the given handle.
pub fn mock_iof_deregister(
    handle: usize,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
    cbfunc: Option<unsafe extern "C" fn(crate::ffi::pmix_status_t, *mut std::os::raw::c_void)>,
    cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_IOF_deregister");
        // Remove the handle from the mock registry
        MOCK_IOF_REGISTRY.with(|cell| {
            cell.borrow_mut().remove(&handle);
        });
        // Invoke the callback if provided (async mode)
        if let Some(cb) = cbfunc {
            unsafe { cb(status, cbdata); }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_IOF_push()`.
///
/// When mock is enabled, returns the configured mock status.
/// Does not actually push data — just simulates the operation.
pub fn mock_iof_push(
    _targets: *const crate::ffi::pmix_proc_t,
    _ntargets: usize,
    _bo: *mut crate::ffi::pmix_byte_object_t,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
    cbfunc: Option<unsafe extern "C" fn(crate::ffi::pmix_status_t, *mut std::os::raw::c_void)>,
    cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_IOF_push");
        // Invoke the callback if provided (async mode)
        if let Some(cb) = cbfunc {
            unsafe { cb(status, cbdata); }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock fabric FFI implementations
// ─────────────────────────────────────────────────────────────────────────────
// These provide controlled mock behavior for the PMIx fabric FFI functions
// used by fabric.rs: Fabric_register, Fabric_update, Fabric_deregister,
// Load_topology, Compute_distances. Each returns the configured status
// (default PMIX_SUCCESS) so tests can exercise happy-path code paths without
// a real PMIx daemon.

thread_local! {
    /// Mock fabric state: whether the fabric is registered.
    static MOCK_FABRIC_REGISTERED: RefCell<bool> = RefCell::new(false);
    /// Mock fabric index value.
    static MOCK_FABRIC_INDEX: RefCell<usize> = RefCell::new(1);
    /// Mock topology state: whether topology is loaded.
    static MOCK_TOPOLOGY_LOADED: RefCell<bool> = RefCell::new(false);
    /// Mock device distances to return from Compute_distances.
    static MOCK_DEVICE_DISTANCES: RefCell<Vec<(String, String, u64, u16, u16)>> = RefCell::new(Vec::new());
}

/// Set whether the mock fabric is registered.
pub fn mock_set_fabric_registered(registered: bool) {
    MOCK_FABRIC_REGISTERED.with(|cell| {
        *cell.borrow_mut() = registered;
    });
}

/// Set the mock fabric index value.
pub fn mock_set_fabric_index(index: usize) {
    MOCK_FABRIC_INDEX.with(|cell| {
        *cell.borrow_mut() = index;
    });
}

/// Set whether the mock topology is loaded.
pub fn mock_set_topology_loaded(loaded: bool) {
    MOCK_TOPOLOGY_LOADED.with(|cell| {
        *cell.borrow_mut() = loaded;
    });
}

/// Set mock device distances to return from Compute_distances.
pub fn mock_set_device_distances(distances: Vec<(String, String, u64, u16, u16)>) {
    MOCK_DEVICE_DISTANCES.with(|cell| {
        *cell.borrow_mut() = distances;
    });
}

/// Mock implementation of `PMIx_Fabric_register()`.
///
/// When mock is enabled, populates the fabric struct with mock values
/// (index=1, module=null) and returns the configured mock status.
pub fn mock_fabric_register(
    fabric: *mut crate::ffi::pmix_fabric_t,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_register");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            unsafe {
                (*fabric).index = 1;
                (*fabric).info = std::ptr::null_mut();
                (*fabric).ninfo = 0;
                (*fabric).module = std::ptr::null_mut();
            }
            MOCK_FABRIC_REGISTERED.with(|cell| {
                *cell.borrow_mut() = true;
            });
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Fabric_register_nb()`.
pub fn mock_fabric_register_nb(
    fabric: *mut crate::ffi::pmix_fabric_t,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::os::raw::c_void)>,
    _cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_register_nb");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            unsafe {
                (*fabric).index = 1;
                (*fabric).info = std::ptr::null_mut();
                (*fabric).ninfo = 0;
                (*fabric).module = std::ptr::null_mut();
            }
            MOCK_FABRIC_REGISTERED.with(|cell| {
                *cell.borrow_mut() = true;
            });
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Fabric_update()`.
pub fn mock_fabric_update(fabric: *mut crate::ffi::pmix_fabric_t) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_update");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            let index = MOCK_FABRIC_INDEX.with(|cell| *cell.borrow());
            unsafe {
                (*fabric).index = index;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Fabric_update_nb()`.
pub fn mock_fabric_update_nb(
    fabric: *mut crate::ffi::pmix_fabric_t,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::os::raw::c_void)>,
    _cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_update_nb");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            let index = MOCK_FABRIC_INDEX.with(|cell| *cell.borrow());
            unsafe {
                (*fabric).index = index;
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Fabric_deregister()`.
pub fn mock_fabric_deregister(fabric: *mut crate::ffi::pmix_fabric_t) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_deregister");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            unsafe {
                (*fabric).index = 0;
                (*fabric).info = std::ptr::null_mut();
                (*fabric).ninfo = 0;
                (*fabric).module = std::ptr::null_mut();
            }
            MOCK_FABRIC_REGISTERED.with(|cell| {
                *cell.borrow_mut() = false;
            });
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Fabric_deregister_nb()`.
pub fn mock_fabric_deregister_nb(
    fabric: *mut crate::ffi::pmix_fabric_t,
    _cbfunc: Option<unsafe extern "C" fn(i32, *mut std::os::raw::c_void)>,
    _cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Fabric_deregister_nb");
        if status == PMIX_SUCCESS && !fabric.is_null() {
            unsafe {
                (*fabric).index = 0;
                (*fabric).info = std::ptr::null_mut();
                (*fabric).ninfo = 0;
                (*fabric).module = std::ptr::null_mut();
            }
            MOCK_FABRIC_REGISTERED.with(|cell| {
                *cell.borrow_mut() = false;
            });
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Load_topology()`.
pub fn mock_load_topology(topo: *mut crate::ffi::pmix_topology_t) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Load_topology");
        if status == PMIX_SUCCESS && !topo.is_null() {
            unsafe {
                (*topo).topology = std::ptr::null_mut();
            }
            MOCK_TOPOLOGY_LOADED.with(|cell| {
                *cell.borrow_mut() = true;
            });
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Compute_distances()`.
///
/// When mock is enabled, writes the mock device distances to the output
/// pointers and returns the configured mock status.
pub fn mock_compute_distances(
    _topo: *mut crate::ffi::pmix_topology_t,
    _cpuset: *mut crate::ffi::pmix_cpuset_t,
    _info: *mut crate::ffi::pmix_info_t,
    _ninfo: usize,
    distances: *mut *mut crate::ffi::pmix_device_distance_t,
    ndist: *mut usize,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Compute_distances");
        if status == PMIX_SUCCESS && !distances.is_null() && !ndist.is_null() {
            let mock_distances = MOCK_DEVICE_DISTANCES.with(|cell| cell.borrow().clone());
            if !mock_distances.is_empty() {
                let count = mock_distances.len();
                // Allocate array of device_distance_t
                let layout = std::alloc::Layout::from_size_align(
                    std::mem::size_of::<crate::ffi::pmix_device_distance_t>() * count,
                    std::mem::align_of::<crate::ffi::pmix_device_distance_t>(),
                ).unwrap();
                let ptr = unsafe { std::alloc::alloc(layout) as *mut crate::ffi::pmix_device_distance_t };
                if !ptr.is_null() {
                    for (i, (uuid, osname, dtype, mind, maxd)) in mock_distances.iter().enumerate() {
                        let entry = unsafe { &mut *ptr.add(i) };
                        entry.uuid = std::ffi::CString::new(uuid.as_str()).unwrap().into_raw();
                        entry.osname = std::ffi::CString::new(osname.as_str()).unwrap().into_raw();
                        entry.type_ = *dtype;
                        entry.mindist = *mind;
                        entry.maxdist = *maxd;
                    }
                    unsafe {
                        *distances = ptr;
                        *ndist = count;
                    }
                }
            }
        }
        status
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Compute_distances_nb()`.
pub fn mock_compute_distances_nb(
    _topo: *mut crate::ffi::pmix_topology_t,
    _cpuset: *mut crate::ffi::pmix_cpuset_t,
    _info: *mut crate::ffi::pmix_info_t,
    _ninfo: usize,
    _cbfunc: Option<unsafe extern "C" fn(
        i32,
        *mut crate::ffi::pmix_device_distance_t,
        usize,
        *mut std::os::raw::c_void,
        Option<unsafe extern "C" fn(*mut std::os::raw::c_void)>,
        *mut std::os::raw::c_void,
    )>,
    _cbdata: *mut std::os::raw::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Compute_distances_nb")
    } else {
        PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Topology_destruct()`.
pub fn mock_topology_destruct(_topo: *mut crate::ffi::pmix_topology_t) {
    MOCK_TOPOLOGY_LOADED.with(|cell| {
        *cell.borrow_mut() = false;
    });
}

/// Mock implementation of `PMIx_Cpuset_construct()`.
pub fn mock_cpuset_construct(_cpuset: *mut crate::ffi::pmix_cpuset_t) {
    // No-op in mock
}

/// Mock implementation of `PMIx_Cpuset_destruct()`.
pub fn mock_cpuset_destruct(_cpuset: *mut crate::ffi::pmix_cpuset_t) {
    // No-op in mock
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock query/log FFI implementations
// ─────────────────────────────────────────────────────────────────────────────

/// Mock implementation of `PMIx_Query_create()`.
/// Allocates a pmix_query_t struct and returns a pointer.
pub fn mock_query_create(n: usize) -> *mut crate::ffi::pmix_query_t {
    let _ = n;
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Query_create");
        if status == PMIX_SUCCESS {
            let ptr = unsafe {
                libc::calloc(1, std::mem::size_of::<crate::ffi::pmix_query_t>())
                    as *mut crate::ffi::pmix_query_t
            };
            if !ptr.is_null() {
                unsafe {
                    (*ptr).keys = std::ptr::null_mut();
                    (*ptr).qualifiers = std::ptr::null_mut();
                    (*ptr).nqual = 0;
                }
            }
            ptr
        } else {
            std::ptr::null_mut()
        }
    } else {
        std::ptr::null_mut()
    }
}

/// Mock implementation of `PMIx_Query_free()`.
/// Frees a pmix_query_t struct allocated by mock_query_create.
pub fn mock_query_free(p: *mut crate::ffi::pmix_query_t, n: usize) {
    if is_mock_enabled() {
        for i in 0..n {
            let ptr = unsafe { p.add(i) };
            unsafe {
                libc::free(ptr as *mut std::ffi::c_void);
            }
        }
    }
}

/// Mock implementation of `PMIx_Query_info()`.
/// Simulates a query response by writing to the output parameters.
pub fn mock_query_info(
    _queries: *mut crate::ffi::pmix_query_t,
    _nqueries: usize,
    results: *mut *mut crate::ffi::pmix_info_t,
    nresults: *mut usize,
) -> i32 {
    if is_mock_enabled() {
        let status = get_mock_status("PMIx_Query_info");
        // Return empty results regardless of status
        unsafe {
            *results = std::ptr::null_mut();
            *nresults = 0;
        }
        status
    } else {
        crate::mock_ffi::PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Query_info_nb()`.
/// Simulates an async query — returns status immediately.
pub fn mock_query_info_nb(
    _queries: *mut crate::ffi::pmix_query_t,
    _nqueries: usize,
    _cbfunc: Option<unsafe extern "C" fn(
        crate::ffi::pmix_status_t,
        *mut crate::ffi::pmix_info_t,
        usize,
        *mut std::ffi::c_void,
        crate::ffi::pmix_release_cbfunc_t,
        *mut std::ffi::c_void,
    )>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Query_info_nb")
    } else {
        crate::mock_ffi::PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Log()`.
pub fn mock_log(
    _data: *const crate::ffi::pmix_info_t,
    _ndata: usize,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Log")
    } else {
        crate::mock_ffi::PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Log_nb()`.
pub fn mock_log_nb(
    _data: *const crate::ffi::pmix_info_t,
    _ndata: usize,
    _directives: *const crate::ffi::pmix_info_t,
    _ndirs: usize,
    _cbfunc: Option<unsafe extern "C" fn(crate::ffi::pmix_status_t, *mut std::ffi::c_void)>,
    _cbdata: *mut std::ffi::c_void,
) -> i32 {
    if is_mock_enabled() {
        get_mock_status("PMIx_Log_nb")
    } else {
        crate::mock_ffi::PMIX_ERR_INIT
    }
}

/// Mock implementation of `PMIx_Info_free()`.
/// No-op in mock mode (handles are fake).
pub fn mock_info_free(_info: *mut crate::ffi::pmix_info_t, _ninfo: usize) {
    // In mock mode, handles are fake — just skip
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for the mock FFI framework itself
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Core enable/disable tests ──────────────────────────────────────────

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
    fn test_enable_resets_state() {
        // Enable, set some state, disable, re-enable — state should be clean
        enable_mock_ffi();
        mock_store_value("key1", b"val1", PMIX_STRING);
        disable_mock_ffi();
        enable_mock_ffi();
        assert!(!mock_key_exists("key1"));
        disable_mock_ffi();
    }

    #[test]
    fn test_double_enable_is_idempotent() {
        enable_mock_ffi();
        enable_mock_ffi();
        assert!(is_mock_enabled());
        disable_mock_ffi();
    }

    #[test]
    fn test_double_disable_is_safe() {
        enable_mock_ffi();
        disable_mock_ffi();
        disable_mock_ffi(); // Should not panic
        assert!(!is_mock_enabled());
    }

    // ── MockConfig tests ───────────────────────────────────────────────────

    #[test]
    fn test_mock_config_default() {
        let config = MockConfig::new();
        config.apply();
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_config_with_override() {
        let config = MockConfig::new().with_function_status("PMIx_Publish", PMIX_ERR_DUPLICATE_KEY);
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

    #[test]
    fn test_mock_config_override_takes_precedence() {
        let config = MockConfig::new()
            .with_default_status(PMIX_ERR_NOMEM)
            .with_function_status("PMIx_Get", PMIX_SUCCESS);
        config.apply();
        assert_eq!(get_mock_status("PMIx_Get"), PMIX_SUCCESS);
        assert_eq!(get_mock_status("PMIx_Publish"), PMIX_ERR_NOMEM);
    }

    #[test]
    fn test_mock_config_chaining() {
        let config = MockConfig::new()
            .with_default_status(PMIX_SUCCESS)
            .with_function_status("A", PMIX_ERR_INIT)
            .with_function_status("B", PMIX_ERR_TIMEOUT)
            .with_function_status("C", PMIX_ERR_BAD_PARAM);
        config.apply();
        assert_eq!(get_mock_status("A"), PMIX_ERR_INIT);
        assert_eq!(get_mock_status("B"), PMIX_ERR_TIMEOUT);
        assert_eq!(get_mock_status("C"), PMIX_ERR_BAD_PARAM);
        assert_eq!(get_mock_status("D"), PMIX_SUCCESS);
    }

    // ── Key-value store tests ──────────────────────────────────────────────

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
    fn test_mock_store_overwrite() {
        mock_store_value("key", b"first", PMIX_STRING);
        assert!(mock_key_exists("key"));
        mock_store_value("key", b"second", PMIX_INT);
        assert!(mock_key_exists("key"));
        let store = mock_get_store();
        let (val, dtype) = &store["key"];
        assert_eq!(val.as_slice(), b"second");
        assert_eq!(*dtype, PMIX_INT);
        mock_clear_store();
    }

    #[test]
    fn test_mock_store_count() {
        assert_eq!(mock_store_count(), 0);
        mock_store_value("a", b"1", PMIX_STRING);
        assert_eq!(mock_store_count(), 1);
        mock_store_value("b", b"2", PMIX_STRING);
        assert_eq!(mock_store_count(), 2);
        mock_remove_value("a");
        assert_eq!(mock_store_count(), 1);
        mock_clear_store();
    }

    #[test]
    fn test_mock_get_store() {
        mock_store_value("k1", b"v1", PMIX_STRING);
        mock_store_value("k2", b"v2", PMIX_INT);
        let store = mock_get_store();
        assert_eq!(store.len(), 2);
        assert!(store.contains_key("k1"));
        assert!(store.contains_key("k2"));
        mock_clear_store();
    }

    #[test]
    fn test_mock_remove_nonexistent_key() {
        mock_remove_value("does_not_exist");
        assert!(!mock_key_exists("does_not_exist"));
    }

    #[test]
    fn test_mock_store_binary_data() {
        let binary = vec![0, 1, 2, 255, 128, 64, 32, 16];
        mock_store_value("binary_key", &binary, PMIX_BYTE_OBJECT);
        let store = mock_get_store();
        let (val, _) = &store["binary_key"];
        assert_eq!(val.as_slice(), binary.as_slice());
        mock_clear_store();
    }

    // ── Status constant tests ──────────────────────────────────────────────

    #[test]
    fn test_mock_status_constants() {
        assert_eq!(PMIX_SUCCESS, 0);
        assert_eq!(PMIX_ERR_INIT, -31);
        assert_eq!(PMIX_ERR_NOT_FOUND, -46);
        assert_eq!(PMIX_ERR_TIMEOUT, -24);
        assert_eq!(PMIX_ERR_DUPLICATE_KEY, -53);
    }

    #[test]
    fn test_mock_status_constants_extended() {
        assert_eq!(PMIX_ERR_BAD_PARAM, -27);
        assert_eq!(PMIX_ERR_NOMEM, -32);
        assert_eq!(PMIX_ERROR, -1);
    }

    #[test]
    fn test_mock_status_negative_values() {
        assert!(PMIX_ERR_INIT < 0);
        assert!(PMIX_ERR_NOT_FOUND < 0);
        assert!(PMIX_SUCCESS >= 0);
    }

    // ── Data type constant tests ───────────────────────────────────────────

    #[test]
    fn test_mock_data_type_constants() {
        assert_eq!(PMIX_BOOL, 0);
        assert_eq!(PMIX_INT, 1);
        assert_eq!(PMIX_STRING, 2);
        assert_eq!(PMIX_SIZE, 3);
    }

    #[test]
    fn test_mock_data_type_constants_extended() {
        assert_eq!(PMIX_POINTER, 4);
        assert_eq!(PMIX_RANGE, 5);
        assert_eq!(PMIX_PROC, 6);
        assert_eq!(PMIX_UCHAR, 7);
        assert_eq!(PMIX_CHAR, 8);
        assert_eq!(PMIX_SHORT, 9);
        assert_eq!(PMIX_LONG, 10);
        assert_eq!(PMIX_UINT, 11);
    }

    #[test]
    fn test_mock_data_type_numeric_types() {
        assert_eq!(PMIX_FLOAT, 13);
        assert_eq!(PMIX_DOUBLE, 14);
        assert_eq!(PMIX_LDOUBLE, 15);
        assert_eq!(PMIX_UINT16, 17);
        assert_eq!(PMIX_INT16, 18);
        assert_eq!(PMIX_UINT32, 19);
        assert_eq!(PMIX_INT32, 20);
        assert_eq!(PMIX_UINT64, 21);
        assert_eq!(PMIX_INT64, 22);
    }

    #[test]
    fn test_mock_data_type_array_types() {
        assert_eq!(PMIX_STRING_ARRAY, 23);
        assert_eq!(PMIX_BOOL_ARRAY, 28);
        assert_eq!(PMIX_SIZE_ARRAY, 36);
        assert_eq!(PMIX_INT64_ARRAY, 75);
        assert_eq!(PMIX_UINT64_ARRAY, 79);
        assert_eq!(PMIX_FLOAT_ARRAY, 80);
        assert_eq!(PMIX_DOUBLE_ARRAY, 81);
    }

    #[test]
    fn test_mock_data_type_max_type() {
        assert_eq!(PMIX_MAX_TYPE, 106);
        // All defined types should be less than MAX_TYPE
        assert!(PMIX_STRING < PMIX_MAX_TYPE);
        assert!(PMIX_BYTE_OBJECT < PMIX_MAX_TYPE);
        assert!(PMIX_SEMANTICS < PMIX_MAX_TYPE);
    }

    #[test]
    fn test_mock_data_type_special_types() {
        assert_eq!(PMIX_BUFFER, 38);
        assert_eq!(PMIX_BYTE_OBJECT, 83);
        assert_eq!(PMIX_INFO, 37);
        assert_eq!(PMIX_ARRAY, 39);
        assert_eq!(PMIX_SEMANTICS, 103);
        assert_eq!(PMIX_PERSISTENCE, 104);
        assert_eq!(PMIX_DATA_RANGE, 105);
    }

    // ── Mock data serialization FFI tests ──────────────────────────────────

    #[test]
    fn test_mock_data_buffer_create_returns_sentinel() {
        enable_mock_ffi();
        let ptr = mock_data_buffer_create();
        assert!(!ptr.is_null());
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_buffer_create_null_when_disabled() {
        disable_mock_ffi();
        let ptr = mock_data_buffer_create();
        assert!(ptr.is_null());
    }

    #[test]
    fn test_mock_data_buffer_release_returns_success() {
        enable_mock_ffi();
        let status = mock_data_buffer_release(std::ptr::null_mut());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_buffer_release_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_buffer_release(std::ptr::null_mut());
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_byte_object_construct_returns_success() {
        enable_mock_ffi();
        let status = mock_byte_object_construct();
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_byte_object_construct_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_byte_object_construct();
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_byte_object_destruct_returns_success() {
        enable_mock_ffi();
        let status = mock_byte_object_destruct();
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_pack_returns_success() {
        enable_mock_ffi();
        let status = mock_data_pack(1, PMIX_STRING);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_pack_bad_param_negative_vals() {
        enable_mock_ffi();
        let status = mock_data_pack(-1, PMIX_STRING);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_pack_bad_param_invalid_type() {
        enable_mock_ffi();
        let status = mock_data_pack(1, PMIX_MAX_TYPE);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_pack_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_pack(1, PMIX_STRING);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_pack_zero_vals_ok() {
        enable_mock_ffi();
        let status = mock_data_pack(0, PMIX_INT);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_unpack_returns_success() {
        enable_mock_ffi();
        let mut count: i32 = 0;
        let status = mock_data_unpack(&mut count, PMIX_STRING);
        assert_eq!(status, PMIX_SUCCESS);
        assert_eq!(count, 1);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_unpack_bad_param_invalid_type() {
        enable_mock_ffi();
        let mut count: i32 = 0;
        let status = mock_data_unpack(&mut count, PMIX_MAX_TYPE);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_unpack_null_max_num_values() {
        enable_mock_ffi();
        let status = mock_data_unpack(std::ptr::null_mut(), PMIX_INT);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_unpack_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let mut count: i32 = 0;
        let status = mock_data_unpack(&mut count, PMIX_STRING);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_unload_returns_success() {
        enable_mock_ffi();
        let status = mock_data_unload();
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_unload_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_unload();
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_load_returns_success() {
        enable_mock_ffi();
        let status = mock_data_load(100);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_load_bad_param_zero_size() {
        enable_mock_ffi();
        let status = mock_data_load(0);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_load_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_load(100);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_copy_returns_success() {
        enable_mock_ffi();
        let status = mock_data_copy(PMIX_STRING);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_copy_bad_param_invalid_type() {
        enable_mock_ffi();
        let status = mock_data_copy(PMIX_MAX_TYPE);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_copy_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_copy(PMIX_STRING);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_copy_payload_returns_success() {
        enable_mock_ffi();
        let status = mock_data_copy_payload(256);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_copy_payload_bad_param_zero_size() {
        enable_mock_ffi();
        let status = mock_data_copy_payload(0);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_copy_payload_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_copy_payload(256);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_print_returns_success() {
        enable_mock_ffi();
        let status = mock_data_print(PMIX_STRING);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_print_bad_param_invalid_type() {
        enable_mock_ffi();
        let status = mock_data_print(PMIX_MAX_TYPE);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_print_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_print(PMIX_STRING);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_embed_returns_success() {
        enable_mock_ffi();
        let status = mock_data_embed();
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_embed_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let status = mock_data_embed();
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_compress_returns_success() {
        enable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_compress(1000, &mut out_len);
        assert_eq!(status, PMIX_SUCCESS);
        // ~70% compression ratio
        assert_eq!(out_len, 700);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_compress_bad_param_zero_input() {
        enable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_compress(0, &mut out_len);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_compress_null_output() {
        enable_mock_ffi();
        let status = mock_data_compress(100, std::ptr::null_mut());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_compress_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_compress(100, &mut out_len);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_data_decompress_returns_success() {
        enable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_decompress(700, &mut out_len);
        assert_eq!(status, PMIX_SUCCESS);
        assert_eq!(out_len, 700);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_decompress_bad_param_zero_input() {
        enable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_decompress(0, &mut out_len);
        assert_eq!(status, PMIX_ERR_BAD_PARAM);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_decompress_returns_err_init_when_disabled() {
        disable_mock_ffi();
        let mut out_len: usize = 0;
        let status = mock_data_decompress(700, &mut out_len);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    // ── Mock FFI status override integration tests ─────────────────────────

    #[test]
    fn test_mock_data_pack_with_override() {
        let config =
            MockConfig::new().with_function_status("PMIx_Data_pack", PMIX_ERR_PACK_FAILURE);
        {
            let _guard = MockGuard::with_config(config);
            let status = mock_data_pack(1, PMIX_STRING);
            assert_eq!(status, PMIX_ERR_PACK_FAILURE);
        }
    }

    #[test]
    fn test_mock_data_unpack_with_override() {
        let config =
            MockConfig::new().with_function_status("PMIx_Data_unpack", PMIX_ERR_UNPACK_FAILURE);
        {
            let _guard = MockGuard::with_config(config);
            let mut count: i32 = 0;
            let status = mock_data_unpack(&mut count, PMIX_INT);
            assert_eq!(status, PMIX_ERR_UNPACK_FAILURE);
        }
    }

    #[test]
    fn test_mock_data_compress_with_override() {
        let config = MockConfig::new().with_function_status("PMIx_Data_compress", PMIX_ERR_NOMEM);
        {
            let _guard = MockGuard::with_config(config);
            let mut out_len: usize = 0;
            let status = mock_data_compress(100, &mut out_len);
            assert_eq!(status, PMIX_ERR_NOMEM);
        }
    }

    #[test]
    fn test_mock_all_serialization_functions_enabled() {
        let _guard = MockGuard::new();
        // All 13 mock functions should return PMIX_SUCCESS when enabled
        assert!(!mock_data_buffer_create().is_null());
        assert_eq!(mock_data_buffer_release(std::ptr::null_mut()), PMIX_SUCCESS);
        assert_eq!(mock_byte_object_construct(), PMIX_SUCCESS);
        assert_eq!(mock_byte_object_destruct(), PMIX_SUCCESS);
        assert_eq!(mock_data_pack(1, PMIX_STRING), PMIX_SUCCESS);
        assert_eq!(mock_data_unpack(&mut 0i32, PMIX_INT), PMIX_SUCCESS);
        assert_eq!(mock_data_unload(), PMIX_SUCCESS);
        assert_eq!(mock_data_load(100), PMIX_SUCCESS);
        assert_eq!(mock_data_copy(PMIX_STRING), PMIX_SUCCESS);
        assert_eq!(mock_data_copy_payload(256), PMIX_SUCCESS);
        assert_eq!(mock_data_print(PMIX_INT), PMIX_SUCCESS);
        assert_eq!(mock_data_embed(), PMIX_SUCCESS);
        assert_eq!(mock_data_compress(100, &mut 0usize), PMIX_SUCCESS);
        assert_eq!(mock_data_decompress(100, &mut 0usize), PMIX_SUCCESS);
    }

    #[test]
    fn test_mock_all_serialization_functions_disabled() {
        disable_mock_ffi();
        // All 13 mock functions should return PMIX_ERR_INIT when disabled
        assert!(mock_data_buffer_create().is_null());
        assert_eq!(
            mock_data_buffer_release(std::ptr::null_mut()),
            PMIX_ERR_INIT
        );
        assert_eq!(mock_byte_object_construct(), PMIX_ERR_INIT);
        assert_eq!(mock_byte_object_destruct(), PMIX_ERR_INIT);
        assert_eq!(mock_data_pack(1, PMIX_STRING), PMIX_ERR_INIT);
        assert_eq!(mock_data_unpack(&mut 0i32, PMIX_INT), PMIX_ERR_INIT);
        assert_eq!(mock_data_unload(), PMIX_ERR_INIT);
        assert_eq!(mock_data_load(100), PMIX_ERR_INIT);
        assert_eq!(mock_data_copy(PMIX_STRING), PMIX_ERR_INIT);
        assert_eq!(mock_data_copy_payload(256), PMIX_ERR_INIT);
        assert_eq!(mock_data_print(PMIX_INT), PMIX_ERR_INIT);
        assert_eq!(mock_data_embed(), PMIX_ERR_INIT);
        assert_eq!(mock_data_compress(100, &mut 0usize), PMIX_ERR_INIT);
        assert_eq!(mock_data_decompress(100, &mut 0usize), PMIX_ERR_INIT);
    }

    // ── Thread-local isolation tests ───────────────────────────────────────

    #[test]
    fn test_mock_thread_local_isolation() {
        // Verify that mock state is thread-local by checking that
        // enable/disable on one thread doesn't affect another
        let handle = std::thread::spawn(|| {
            enable_mock_ffi();
            assert!(is_mock_enabled());
            disable_mock_ffi();
            assert!(!is_mock_enabled());
        });
        handle.join().unwrap();
    }

    #[test]
    fn test_mock_store_thread_local_isolation() {
        let handle = std::thread::spawn(|| {
            enable_mock_ffi();
            mock_store_value("thread_key", b"thread_val", PMIX_STRING);
            assert!(mock_key_exists("thread_key"));
            disable_mock_ffi();
        });
        handle.join().unwrap();
        // Main thread should not see the thread-local key
        assert!(!mock_key_exists("thread_key"));
    }

    // ── Edge case tests ────────────────────────────────────────────────────

    #[test]
    fn test_mock_store_many_keys() {
        enable_mock_ffi();
        for i in 0..100 {
            let key = format!("key_{}", i);
            mock_store_value(&key, b"value", PMIX_STRING);
        }
        assert_eq!(mock_store_count(), 100);
        for i in 0..100 {
            let key = format!("key_{}", i);
            assert!(mock_key_exists(&key));
        }
        mock_clear_store();
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_pack_all_valid_types() {
        enable_mock_ffi();
        // Test packing with various valid data types
        for dtype in 0..PMIX_MAX_TYPE {
            let status = mock_data_pack(1, dtype);
            assert_eq!(status, PMIX_SUCCESS, "Failed for dtype={}", dtype);
        }
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_data_copy_all_valid_types() {
        enable_mock_ffi();
        for dtype in 0..PMIX_MAX_TYPE {
            let status = mock_data_copy(dtype);
            assert_eq!(status, PMIX_SUCCESS, "Failed for dtype={}", dtype);
        }
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_compress_various_sizes() {
        enable_mock_ffi();
        for size in [1, 10, 100, 1000, 10000] {
            let mut out_len: usize = 0;
            let status = mock_data_compress(size, &mut out_len);
            assert_eq!(status, PMIX_SUCCESS);
            // Verify ~70% compression
            let expected = (size as f64 * 0.7) as usize;
            assert_eq!(
                out_len, expected,
                "Size {} -> {} (expected {})",
                size, out_len, expected
            );
        }
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_decompress_restores_size() {
        enable_mock_ffi();
        for size in [1, 100, 1000, 10000] {
            let mut out_len: usize = 0;
            let status = mock_data_decompress(size, &mut out_len);
            assert_eq!(status, PMIX_SUCCESS);
            assert_eq!(out_len, size);
        }
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_guard_nested_scopes() {
        assert!(!is_mock_enabled());
        {
            let _g1 = MockGuard::new();
            assert!(is_mock_enabled());
            {
                let _g2 = MockGuard::new();
                assert!(is_mock_enabled());
            }
            // g2 dropped, mock disabled
            assert!(!is_mock_enabled());
        }
        // g1 dropped, mock disabled
        assert!(!is_mock_enabled());
    }

    // ── Mock server FFI tests ──────────────────────────────────────────────

    #[test]
    fn test_mock_server_init_success() {
        enable_mock_ffi();
        let status = mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_init_err_when_disabled() {
        disable_mock_ffi();
        let status = mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_server_finalize_success() {
        enable_mock_ffi();
        let status = mock_server_finalize();
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_finalize_err_when_disabled() {
        disable_mock_ffi();
        let status = mock_server_finalize();
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_server_register_nspace_success() {
        enable_mock_ffi();
        let nspace = std::ffi::CString::new("test").unwrap();
        let status = mock_server_register_nspace(
            nspace.as_ptr(),
            1,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_register_nspace_err_when_disabled() {
        disable_mock_ffi();
        let nspace = std::ffi::CString::new("test").unwrap();
        let status = mock_server_register_nspace(
            nspace.as_ptr(),
            1,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PMIX_ERR_INIT);
    }

    #[test]
    fn test_mock_server_publish_success() {
        enable_mock_ffi();
        let key = std::ffi::CString::new("k").unwrap();
        let status = mock_server_publish(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_lookup_success() {
        enable_mock_ffi();
        let key = std::ffi::CString::new("k").unwrap();
        let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
        let status = mock_server_lookup(
            std::ptr::null_mut(),
            key.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut val,
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_delete_success() {
        enable_mock_ffi();
        let key = std::ffi::CString::new("k").unwrap();
        let status =
            mock_server_delete(std::ptr::null_mut(), key.as_ptr(), std::ptr::null_mut(), 0);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_fence_success() {
        enable_mock_ffi();
        let mut retvals: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut nretvals: usize = 0;
        let status = mock_server_fence(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut retvals,
            &mut nretvals,
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_fence_nb_success() {
        enable_mock_ffi();
        let status = mock_server_fence_nb(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_dmodex_request_success() {
        enable_mock_ffi();
        let status = mock_server_dmodex_request(std::ptr::null_mut(), None, std::ptr::null_mut());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_setup_application_success() {
        enable_mock_ffi();
        let nspace = std::ffi::CString::new("test").unwrap();
        let status = mock_server_setup_application(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_setup_local_support_success() {
        enable_mock_ffi();
        let nspace = std::ffi::CString::new("test").unwrap();
        let status = mock_server_setup_local_support(
            nspace.as_ptr(),
            std::ptr::null_mut(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_register_resources_success() {
        enable_mock_ffi();
        let status =
            mock_server_register_resources(std::ptr::null_mut(), 0, None, std::ptr::null_mut());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_deregister_resources_success() {
        enable_mock_ffi();
        let status =
            mock_server_deregister_resources(std::ptr::null_mut(), 0, None, std::ptr::null_mut());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_tool_attach_to_server_success() {
        enable_mock_ffi();
        let status = mock_server_tool_attach_to_server(
            0,
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_get_credential_success() {
        enable_mock_ffi();
        let mut cred: *mut std::os::raw::c_char = std::ptr::null_mut();
        let mut cred_size: usize = 0;
        let status = mock_server_get_credential(std::ptr::null_mut(), 0, &mut cred, &mut cred_size);
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_define_process_set_success() {
        enable_mock_ffi();
        let pset = std::ffi::CString::new("test").unwrap();
        let status = mock_server_define_process_set(std::ptr::null_mut(), 0, pset.as_ptr());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_delete_process_set_success() {
        enable_mock_ffi();
        let pset = std::ffi::CString::new("test").unwrap();
        let status = mock_server_delete_process_set(pset.into_raw());
        assert_eq!(status, PMIX_SUCCESS);
        disable_mock_ffi();
    }

    #[test]
    fn test_mock_server_init_with_override() {
        let config = MockConfig::new().with_function_status("PMIx_server_init", PMIX_ERR_NOMEM);
        {
            let _guard = MockGuard::with_config(config);
            let status = mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0);
            assert_eq!(status, PMIX_ERR_NOMEM);
        }
    }

    #[test]
    fn test_mock_server_publish_with_override() {
        let config =
            MockConfig::new().with_function_status("PMIx_server_publish", PMIX_ERR_DUPLICATE_KEY);
        {
            let _guard = MockGuard::with_config(config);
            let key = std::ffi::CString::new("k").unwrap();
            let status = mock_server_publish(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            );
            assert_eq!(status, PMIX_ERR_DUPLICATE_KEY);
        }
    }

    #[test]
    fn test_mock_server_lookup_with_override() {
        let config =
            MockConfig::new().with_function_status("PMIx_server_lookup", PMIX_ERR_NOT_FOUND);
        {
            let _guard = MockGuard::with_config(config);
            let key = std::ffi::CString::new("k").unwrap();
            let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
            let status = mock_server_lookup(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut val,
            );
            assert_eq!(status, PMIX_ERR_NOT_FOUND);
        }
    }

    #[test]
    fn test_mock_server_fence_with_override() {
        let config = MockConfig::new().with_function_status("PMIx_server_fence", PMIX_ERR_TIMEOUT);
        {
            let _guard = MockGuard::with_config(config);
            let mut retvals: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut nretvals: usize = 0;
            let status = mock_server_fence(
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut retvals,
                &mut nretvals,
            );
            assert_eq!(status, PMIX_ERR_TIMEOUT);
        }
    }

    #[test]
    fn test_mock_server_all_functions_enabled() {
        let _guard = MockGuard::new();
        // All server mock functions return PMIX_SUCCESS when enabled
        assert_eq!(
            mock_server_init(std::ptr::null_mut(), std::ptr::null_mut(), 0),
            PMIX_SUCCESS
        );
        assert_eq!(mock_server_finalize(), PMIX_SUCCESS);
        let nspace = std::ffi::CString::new("test").unwrap();
        assert_eq!(
            mock_server_register_nspace(
                nspace.as_ptr(),
                1,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut()
            ),
            PMIX_SUCCESS
        );
        let key = std::ffi::CString::new("k").unwrap();
        assert_eq!(
            mock_server_publish(
                std::ptr::null_mut(),
                key.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_delete(std::ptr::null_mut(), key.as_ptr(), std::ptr::null_mut(), 0),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_fence(
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut std::ptr::null_mut::<std::ffi::c_void>(),
                &mut 0usize
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_fence_nb(
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut()
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_dmodex_request(std::ptr::null_mut(), None, std::ptr::null_mut()),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_setup_application(
                nspace.as_ptr(),
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut()
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_setup_local_support(
                nspace.as_ptr(),
                std::ptr::null_mut(),
                0,
                None,
                std::ptr::null_mut()
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_register_resources(std::ptr::null_mut(), 0, None, std::ptr::null_mut()),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_deregister_resources(std::ptr::null_mut(), 0, None, std::ptr::null_mut()),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_tool_attach_to_server(
                0,
                0,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0
            ),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_get_credential(
                std::ptr::null_mut(),
                0,
                &mut std::ptr::null_mut::<std::os::raw::c_char>(),
                &mut 0usize
            ),
            PMIX_SUCCESS
        );
        let pset = std::ffi::CString::new("pset").unwrap();
        assert_eq!(
            mock_server_define_process_set(std::ptr::null_mut(), 0, pset.as_ptr()),
            PMIX_SUCCESS
        );
        assert_eq!(
            mock_server_delete_process_set(std::ffi::CString::new("pset").unwrap().into_raw()),
            PMIX_SUCCESS
        );
    }
}
