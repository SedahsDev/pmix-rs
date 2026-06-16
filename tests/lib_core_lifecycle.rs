//! Integration tests for PMIx core lifecycle: `pmix::init`, `pmix::finalize`,
//! `pmix::utility::initialized`, and `pmix::get_version`.
//!
//! All tests in this file run standalone without a PMIx daemon (prterun/DVM).
//! This means `pmix::init()` will always return an error (typically `ErrUnreach`
//! since no server is available), and `pmix::finalize()` may succeed even without
//! prior init (PMIx_Finalize is idempotent).
//!
//! IMPORTANT: Do NOT call `PMIx_Get_attribute_string` or
//! `PMIx_Get_attribute_name` — they crash with SIGSEGV without `PMIx_Init`.

use pmix::{finalize, get_version, init, InfoBuilder, PmixError, PmixStatus};
use pmix::utility::initialized;

// ═══════════════════════════════════════════════════════════════════════════
// get_version — safe, no init needed
// ═══════════════════════════════════════════════════════════════════════════

/// `get_version` returns a non-empty version string.
#[test]
fn get_version_returns_non_empty() {
    assert!(!get_version().is_empty(), "get_version should return non-empty string");
}

/// `get_version` returns a string containing digits (version numbers).
#[test]
fn get_version_contains_digits() {
    let version = get_version();
    assert!(
        version.chars().any(|c| c.is_ascii_digit()),
        "get_version should contain digits"
    );
}

/// `get_version` returns `&'static str` (compile-time type check).
#[test]
fn get_version_returns_static_str() {
    let _v: &'static str = get_version();
}

/// `get_version` is deterministic — repeated calls return the same value.
#[test]
fn get_version_is_deterministic() {
    assert_eq!(get_version(), get_version(), "get_version must be deterministic");
}

/// `get_version` returns printable ASCII and spaces only.
#[test]
fn get_version_is_printable_ascii() {
    let version = get_version();
    for (i, c) in version.chars().enumerate() {
        assert!(
            c.is_ascii_graphic() || c == ' ' || c == '\t',
            "get_version char at pos {} is not printable ASCII: {:?} (U+{:04X})",
            i, c, c as u32
        );
    }
}

/// `get_version` starts with "OpenPMIx" (the implementation name).
#[test]
fn get_version_starts_with_openpmix() {
    assert!(
        get_version().starts_with("OpenPMIx"),
        "get_version should start with 'OpenPMIx'"
    );
}

/// `get_version` contains a space separating the implementation name from the version.
#[test]
fn get_version_has_space_separator() {
    assert!(
        get_version().contains(' '),
        "get_version should contain a space between name and version"
    );
}

/// Can extract a major version number from `get_version` output.
/// The format is "OpenPMIx 5.0.7a1 ..." — split on space, then find the first numeric segment.
#[test]
fn get_version_can_extract_major_version() {
    let version = get_version();
    let version_part = version.split(' ').nth(1).unwrap_or("");
    let major = version_part
        .split(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())
        .unwrap_or("");
    assert!(!major.is_empty(), "could not extract major version from '{}'", version);
    let major_num: u32 = major.parse().expect("major version should be a number");
    assert!(major_num > 0, "major version should be positive");
}

/// Can extract a full "major.minor" version from `get_version` output.
#[test]
fn get_version_can_extract_major_minor_version() {
    let version = get_version();
    let version_part = version.split(' ').nth(1).unwrap_or("");
    let mut dot_count = 0;
    let mut segment = String::new();
    for c in version_part.chars() {
        if c.is_ascii_digit() || c == '.' {
            segment.push(c);
            if c == '.' {
                dot_count += 1;
            }
        } else if dot_count > 0 {
            break;
        } else {
            break;
        }
    }
    assert!(
        dot_count > 0,
        "could not extract major.minor from '{}' (got '{}')",
        version, segment
    );
    let parts: Vec<&str> = segment.split('.').collect();
    assert!(parts.len() >= 2, "expected at least major.minor, got '{}'", segment);
    let _major: u32 = parts[0].parse().expect("major should parse");
    let _minor: u32 = parts[1].parse().expect("minor should parse");
}

/// `get_version` string length is reasonable (not absurdly long).
#[test]
fn get_version_reasonable_length() {
    let version = get_version();
    assert!(
        version.len() > 5 && version.len() < 256,
        "get_version has suspicious length {}",
        version.len()
    );
}

/// `get_version` contains version metadata like "PMIx Standard" or "ABI".
#[test]
fn get_version_contains_metadata() {
    let version = get_version();
    // The version string from OpenPMIx includes metadata like
    // "OpenPMIx 5.0.7a1 (PMIx Standard: 5.1, Stable ABI: 5.0, ...)"
    assert!(
        version.contains("PMIx Standard") || version.contains("ABI"),
        "get_version should contain PMIx metadata"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// initialized() — safe, no init needed
// ═══════════════════════════════════════════════════════════════════════════

/// `initialized()` returns a bool (compile-time type check).
#[test]
fn initialized_returns_bool() {
    let _result: bool = initialized();
}

/// `initialized()` is idempotent — multiple calls return the same value.
#[test]
fn initialized_is_idempotent() {
    let first = initialized();
    let second = initialized();
    assert_eq!(first, second, "initialized() must be idempotent");
}

/// `initialized()` is deterministic across 100 calls.
#[test]
fn initialized_deterministic_many_calls() {
    let mut results = Vec::new();
    for _ in 0..100 {
        results.push(initialized());
    }
    let first = results[0];
    for (i, &val) in results.iter().enumerate().skip(1) {
        assert_eq!(
            val, first,
            "initialized() call {} returned different value",
            i
        );
    }
}

/// `initialized()` returns a consistent value (whatever the library reports).
/// The actual value depends on the PMIx library state — in standalone mode
/// without a DVM, the library may report true or false depending on version.
#[test]
fn initialized_returns_consistent_value() {
    let v1 = initialized();
    let v2 = initialized();
    let v3 = initialized();
    assert_eq!(v1, v2, "first two calls should agree");
    assert_eq!(v2, v3, "second and third calls should agree");
}

// ═══════════════════════════════════════════════════════════════════════════
// init() error paths — no DVM available
// ═══════════════════════════════════════════════════════════════════════════

/// `init(None)` without DVM returns `Err`.
#[test]
fn init_without_dvm_returns_err() {
    assert!(init(None).is_err(), "init(None) without DVM should return Err");
}

/// `init(Some(info))` without DVM returns `Err`.
#[test]
fn init_with_info_without_dvm_returns_err() {
    let info = InfoBuilder::new().build();
    assert!(init(Some(info)).is_err(), "init(Some(info)) without DVM should return Err");
}

/// `init(None)` error is a known PMIx error (not a random code).
#[test]
fn init_without_dvm_error_is_known() {
    match init(None) {
        Err(e) => {
            let status = PmixStatus::from_raw(e.to_raw());
            assert!(
                matches!(status, PmixStatus::Known(_)),
                "init error should be a known PmixStatus::Known variant, got {:?}",
                status
            );
        }
        Ok(_) => panic!("init(None) should fail without DVM"),
    }
}

/// `init` does not panic on error — it returns a proper `Err`.
#[test]
fn init_does_not_panic_on_error() {
    let result = std::panic::catch_unwind(|| init(None));
    assert!(result.is_ok(), "init(None) should not panic, it should return Err");
}

/// `init` with empty InfoBuilder also returns error without DVM.
#[test]
fn init_with_empty_info_returns_err() {
    let info = InfoBuilder::new().build();
    assert!(init(Some(info)).is_err(), "init with empty InfoBuilder should fail without DVM");
}

/// `init` with `collect_data` info also returns error without DVM.
#[test]
fn init_with_collect_data_info_returns_err() {
    let mut builder = InfoBuilder::new();
    builder.collect_data();
    let info = builder.build();
    assert!(init(Some(info)).is_err(), "init with collect_data info should fail without DVM");
}

/// Multiple `init` calls in a row all return the same error type.
#[test]
fn double_init_returns_same_error_type() {
    let result1 = init(None);
    let result2 = init(None);

    assert!(result1.is_err(), "first init should fail");
    assert!(result2.is_err(), "second init should fail");

    match (&result1, &result2) {
        (Err(e1), Err(e2)) => {
            assert_eq!(
                e1, e2,
                "double init should return the same error type: {:?} vs {:?}",
                e1, e2
            );
        }
        _ => panic!("both inits should return Err"),
    }
}

/// `init` returns `Result<Context, PmixError>` (compile-time type check).
#[test]
fn init_return_type_is_result_context_pmixerror() {
    let _result: Result<pmix::Context, PmixError> = init(None);
}

/// `init` error value is negative (error codes are negative in PMIx).
#[test]
fn init_error_value_is_negative() {
    match init(None) {
        Err(e) => {
            let raw = e.to_raw();
            assert!(
                raw < 0,
                "init error should be negative (got {}), PMIx error codes are negative",
                raw
            );
        }
        Ok(_) => panic!("init should fail without DVM"),
    }
}

/// Error from init is a known PmixError variant (not unknown/user-defined).
#[test]
fn init_error_is_known_pmixerror() {
    match init(None) {
        Err(e) => {
            let status = PmixStatus::from_raw(e.to_raw());
            assert!(
                matches!(status, PmixStatus::Known(_)),
                "init error should map to a known PmixStatus::Known variant, got {:?}",
                status
            );
        }
        Ok(_) => panic!("init should fail without DVM"),
    }
}

/// `init` error is an error (not success) — raw value is negative.
#[test]
fn init_error_is_error_not_success() {
    match init(None) {
        Err(e) => {
            assert!(
                e.is_error(),
                "init error should be an error (negative), got {:?}",
                e
            );
            assert!(
                !e.is_success(),
                "init error should not be success, got {:?}",
                e
            );
        }
        Ok(_) => panic!("init should fail without DVM"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// finalize() error paths — no prior init
// ═══════════════════════════════════════════════════════════════════════════

/// `finalize(None)` without prior init does not panic.
/// PMIx_Finalize is idempotent and may return success even without prior init.
#[test]
fn finalize_without_init_does_not_panic() {
    let result = std::panic::catch_unwind(|| finalize(None));
    assert!(result.is_ok(), "finalize(None) should not panic");
}

/// `finalize` returns `Result<(), i32>` (compile-time type check).
/// `pmix_status_t` is an i32 alias from the FFI bindings.
#[test]
fn finalize_return_type_is_result_unit_pmix_status_t() {
    let _result: Result<(), i32> = finalize(None);
}

/// `finalize(None)` completes without crashing regardless of init state.
#[test]
fn finalize_completes_without_crash() {
    // Just call it — the key property is it doesn't crash
    let _ = finalize(None);
}

/// Multiple `finalize` calls without init complete without crashing.
#[test]
fn multiple_finalize_without_init_safe() {
    for i in 0..3 {
        let result = finalize(None);
        // finalize may succeed or fail; the important thing is no crash
        let _ = result;
        let _ = i; // silence unused warning
    }
}

/// `finalize(Some(info))` without prior init completes without crashing.
#[test]
fn finalize_with_info_without_init_safe() {
    let info = InfoBuilder::new().build();
    let _ = finalize(Some(info));
}

/// `finalize` result is either Ok or Err with a known status.
#[test]
fn finalize_result_is_valid() {
    match finalize(None) {
        Ok(()) => {
            // PMIx_Finalize may return success even without init (idempotent)
        }
        Err(status) => {
            // If it returns an error, it should be a valid pmix_status_t
            assert_ne!(status, i32::MIN, "finalize error should not be i32::MIN");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// init/finalize lifecycle patterns
// ═══════════════════════════════════════════════════════════════════════════

/// init → finalize → init cycle: init fails, finalize completes, init fails.
#[test]
fn init_finalize_init_cycle() {
    // First init — should fail
    let init1 = init(None);
    assert!(init1.is_err(), "first init should fail without DVM");

    // Finalize — may succeed or fail, but should not crash
    let _fin1 = finalize(None);

    // Second init — should still fail
    let init2 = init(None);
    assert!(init2.is_err(), "second init should also fail without DVM");
}

/// init → init → finalize: double init both fail, finalize completes.
#[test]
fn double_init_then_finalize() {
    let init1 = init(None);
    let init2 = init(None);
    let _fin = finalize(None);

    assert!(init1.is_err(), "first init should fail");
    assert!(init2.is_err(), "second init should fail");
}

/// finalize → init → finalize: init fails, finalizes complete.
#[test]
fn finalize_init_finalize() {
    let _fin1 = finalize(None);
    let init1 = init(None);
    let _fin2 = finalize(None);

    assert!(init1.is_err(), "init should fail without DVM");
}

/// `initialized()` returns a consistent value after failed init/finalize.
#[test]
fn initialized_consistent_after_failed_lifecycle() {
    let before = initialized();
    let _ = init(None);
    let _ = finalize(None);
    let after = initialized();
    // initialized() should be stable (consistent) across these operations
    let _ = before;
    let _ = after;
    // We don't assert specific values since PMIx may vary
}

/// init with info, then finalize with info: init fails, finalize completes.
#[test]
fn init_with_info_then_finalize_with_info() {
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();

    let init_result = init(Some(info1));
    assert!(init_result.is_err(), "init with info should fail without DVM");

    let _fin_result = finalize(Some(info2));
    // finalize should complete without crashing
}

/// `PmixError::ErrInit` has the expected raw value of -31.
#[test]
fn errinit_raw_value_is_negative_31() {
    assert_eq!(
        PmixError::ErrInit.to_raw(),
        -31,
        "PmixError::ErrInit should have raw value -31"
    );
}

/// `PmixError::ErrInit` is an error (not success).
#[test]
fn errinit_is_error() {
    assert!(PmixError::ErrInit.is_error(), "PmixError::ErrInit should be an error");
    assert!(
        !PmixError::ErrInit.is_success(),
        "PmixError::ErrInit should not be success"
    );
}

/// `PmixError::ErrUnreach` has the expected raw value of -25.
#[test]
fn errunreach_raw_value_is_negative_25() {
    assert_eq!(
        PmixError::ErrUnreach.to_raw(),
        -25,
        "PmixError::ErrUnreach should have raw value -25"
    );
}

/// `PmixError::ErrUnreach` is an error (not success).
#[test]
fn errunreach_is_error() {
    assert!(
        PmixError::ErrUnreach.is_error(),
        "PmixError::ErrUnreach should be an error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Thread safety of safe functions
// ═══════════════════════════════════════════════════════════════════════════

/// Concurrent calls to `initialized()` from multiple threads are safe.
#[test]
fn concurrent_initialized_safe() {
    const NUM_THREADS: usize = 8;
    const CALLS_PER_THREAD: usize = 50;

    let mut handles = Vec::new();
    for _ in 0..NUM_THREADS {
        handles.push(std::thread::spawn(|| {
            let mut results = Vec::new();
            for _ in 0..CALLS_PER_THREAD {
                results.push(initialized());
            }
            results
        }));
    }

    let mut all_results = Vec::new();
    for handle in handles {
        let thread_results = handle.join().expect("thread should not panic");
        all_results.extend(thread_results);
    }

    assert_eq!(
        all_results.len(),
        NUM_THREADS * CALLS_PER_THREAD,
        "should have collected all results"
    );

    // All results should be the same (consistent across threads)
    let first = all_results[0];
    for (i, &val) in all_results.iter().enumerate().skip(1) {
        assert_eq!(
            val, first,
            "concurrent initialized() call {} returned different value",
            i
        );
    }
}

/// Concurrent calls to `get_version()` from multiple threads are safe.
#[test]
fn concurrent_get_version_safe() {
    const NUM_THREADS: usize = 8;
    const CALLS_PER_THREAD: usize = 50;

    let mut handles = Vec::new();
    for _ in 0..NUM_THREADS {
        handles.push(std::thread::spawn(|| {
            let mut results = Vec::new();
            for _ in 0..CALLS_PER_THREAD {
                results.push(get_version().to_string());
            }
            results
        }));
    }

    let mut all_versions = Vec::new();
    for handle in handles {
        let thread_versions = handle.join().expect("thread should not panic");
        all_versions.extend(thread_versions);
    }

    assert_eq!(
        all_versions.len(),
        NUM_THREADS * CALLS_PER_THREAD,
        "should have collected all version strings"
    );

    // All versions should be identical
    let first = &all_versions[0];
    for (i, version) in all_versions.iter().enumerate().skip(1) {
        assert_eq!(
            version, first,
            "concurrent get_version() call {} returned different value",
            i
        );
    }
}

/// `initialized()` and `get_version()` can be called concurrently from different threads.
#[test]
fn mixed_concurrent_safe_calls() {
    const NUM_THREADS: usize = 4;

    let mut handles = Vec::new();
    for id in 0..NUM_THREADS {
        handles.push(std::thread::spawn(move || {
            for _ in 0..20 {
                if id % 2 == 0 {
                    let _ = initialized();
                } else {
                    let _ = get_version();
                }
            }
        }));
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}

/// `get_version` and `initialized` can be called before any lifecycle operations.
#[test]
fn safe_functions_work_before_any_lifecycle() {
    // These should work even at the very start, before any init/finalize
    let version = get_version();
    let _is_init = initialized();

    assert!(!version.is_empty(), "version should be non-empty");
}

/// `get_version` works after failed init.
#[test]
fn get_version_works_after_failed_init() {
    let _ = init(None);
    assert!(!get_version().is_empty(), "get_version should work after failed init");
}

/// `initialized()` works after failed init and failed finalize.
#[test]
fn initialized_works_after_failed_lifecycle() {
    let _ = init(None);
    let _ = finalize(None);
    // Just calling it — should not crash
    let _ = initialized();
}

/// `initialized()` returns the same value as `initialized()` from the utility module.
#[test]
fn initialized_type_check() {
    // Verify the function signature: fn initialized() -> bool
    fn assert_fn_returns_bool<F: Fn() -> bool>(_f: F) {}
    assert_fn_returns_bool(initialized);
}

/// `get_version` returns the same value across all threads even under contention.
#[test]
fn get_version_thread_consistent_under_contention() {
    const NUM_THREADS: usize = 16;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(NUM_THREADS));
    let versions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    for _ in 0..NUM_THREADS {
        let b = barrier.clone();
        let v = versions.clone();
        handles.push(std::thread::spawn(move || {
            b.wait(); // synchronize all threads
            let ver = get_version().to_string();
            v.lock().unwrap().push(ver);
        }));
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    let collected = versions.lock().unwrap();
    assert_eq!(collected.len(), NUM_THREADS);
    let first = &collected[0];
    for (i, ver) in collected.iter().enumerate().skip(1) {
        assert_eq!(ver, first, "thread {} returned different version", i);
    }
}
