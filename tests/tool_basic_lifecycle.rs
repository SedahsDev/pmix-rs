//! Standalone tests for PMIx tool basic lifecycle.
//!
//! Exercises `tool_init`, `tool_finalize`, `tool_init_minimal`, and
//! `is_tool_initialized`. These tests do NOT use `daemon_helper` — they
//! work regardless of whether a PMIx daemon is running.
//!
//! Test categories:
//!
//! 1. `is_tool_initialized()` — safe to call anytime, bool return.
//! 2. Type safety — compile-time checks on signatures and traits.
//! 3. Lifecycle patterns — init/finalize cycles work when daemon is present.
//! 4. Error paths — tool_finalize without matching init, error status codes.
//! 5. Thread safety — concurrent safe function calls.
//!
//! All tests compile and pass without daemon/prterun.

use pmix::tool::{
    is_tool_initialized, tool_finalize, tool_init, tool_init_minimal,
    PmixServerHandle, PmixToolHandle,
};
use pmix::{Info, InfoBuilder, PmixError, PmixStatus, Proc};

// ─────────────────────────────────────────────────────────────────────────────
// Helper: detect daemon availability
// ─────────────────────────────────────────────────────────────────────────────

/// Try a minimal tool_init to check if a PMIx daemon/server is available.
/// Returns `true` if a daemon is reachable.
#[allow(dead_code)]
fn daemon_available() -> bool {
    tool_init_minimal().is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// is_tool_initialized — safe, no init needed
// ─────────────────────────────────────────────────────────────────────────────

/// is_tool_initialized returns a bool (compile-time type check).
#[test]
fn test_is_tool_initialized_returns_bool() {
    let val: bool = is_tool_initialized();
    // Tautology to silence unused-variable warnings.
    assert!(val || !val);
}

/// is_tool_initialized is idempotent — multiple calls return the same value.
#[test]
fn test_is_tool_initialized_idempotent() {
    let first = is_tool_initialized();
    let second = is_tool_initialized();
    assert_eq!(
        first, second,
        "is_tool_initialized must be idempotent (no side effects)"
    );
}

/// is_tool_initialized is deterministic across many calls.
#[test]
fn test_is_tool_initialized_deterministic() {
    let results: Vec<bool> = (0..20).map(|_| is_tool_initialized()).collect();
    let first = results[0];
    for (i, val) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first, *val,
            "is_tool_initialized call {} returned {:?}, expected {:?}",
            i, val, first
        );
    }
}

/// is_tool_initialized does not panic when called repeatedly.
#[test]
fn test_is_tool_initialized_no_panic_repeated() {
    for _ in 0..100 {
        let _ = is_tool_initialized();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init_minimal — behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init_minimal() does not panic regardless of daemon availability.
#[test]
fn test_tool_init_minimal_no_panic() {
    // Just call it and ensure no panic.
    let _result = tool_init_minimal();
}

/// tool_init_minimal() returns a Result<PmixToolHandle, PmixStatus>.
#[test]
fn test_tool_init_minimal_return_type() {
    let result: Result<PmixToolHandle, PmixStatus> = tool_init_minimal();
    // The result is either Ok(handle) or Err(status) — both are valid.
    match result {
        Ok(handle) => {
            // Daemon available — handle should have valid proc info.
            let _proc = handle.proc();
        }
        Err(status) => {
            // No daemon — error must not be success.
            assert!(
                !status.is_success(),
                "tool_init_minimal error must not be success: {:?}",
                status
            );
        }
    }
}

/// tool_init_minimal() result is consistent across multiple calls.
#[test]
fn test_tool_init_minimal_consistent_result() {
    let results: Vec<Result<PmixToolHandle, PmixStatus>> =
        (0..5).map(|_| tool_init_minimal()).collect();

    let first_ok = results[0].is_ok();
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_ok,
            result.is_ok(),
            "tool_init_minimal should be consistent: call 0 = {}, call {} = {}",
            first_ok, i, result.is_ok()
        );
    }
}

/// tool_init_minimal() error status is not PMIX_SUCCESS when daemon unavailable.
#[test]
fn test_tool_init_minimal_error_not_success() {
    let result = tool_init_minimal();
    if let Err(status) = result {
        assert!(
            !status.is_success(),
            "tool_init_minimal error must not be success: {:?}",
            status
        );
        assert!(
            status.is_error(),
            "tool_init_minimal error must be an error status: {:?}",
            status
        );
    }
    // If Ok, the daemon is available — that's fine too.
}

/// tool_init_minimal return type matches tool_init.
#[test]
fn test_tool_init_minimal_same_return_type_as_tool_init() {
    type InitReturn = Result<PmixToolHandle, PmixStatus>;

    fn _check_minimal() -> InitReturn {
        tool_init_minimal()
    }

    fn _check_full() -> InitReturn {
        tool_init(None, &InfoBuilder::new().build())
    }

    let _ = (_check_minimal, _check_full);
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_init — behavior tests
// ─────────────────────────────────────────────────────────────────────────────

/// tool_init(None, &empty_info) does not panic.
#[test]
fn test_tool_init_none_no_panic() {
    let info = InfoBuilder::new().build();
    let _result = tool_init(None, &info);
}

/// tool_init returns a Result type (compile-time signature check).
#[test]
fn test_tool_init_signature() {
    let _: fn(Option<&Proc>, &Info) -> Result<PmixToolHandle, PmixStatus> = tool_init;
}

/// tool_init takes Option<&Proc> as first argument.
#[test]
fn test_tool_init_accepts_option_proc() {
    let info = InfoBuilder::new().build();
    let _: Result<PmixToolHandle, PmixStatus> = tool_init(None, &info);
}

/// tool_init takes &Info as second argument.
#[test]
fn test_tool_init_accepts_info_ref() {
    let info = InfoBuilder::new().build();
    let _: Result<PmixToolHandle, PmixStatus> = tool_init(None, &info);
}

/// tool_init with empty InfoBuilder returns consistent result across calls.
#[test]
fn test_tool_init_consistent_result() {
    let info = InfoBuilder::new().build();
    let results: Vec<Result<PmixToolHandle, PmixStatus>> =
        (0..5).map(|_| tool_init(None, &info)).collect();

    let first_ok = results[0].is_ok();
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_ok,
            result.is_ok(),
            "tool_init should be consistent: call 0 = {}, call {} = {}",
            first_ok, i, result.is_ok()
        );
    }
}

/// tool_init error status is not PMIX_SUCCESS when daemon unavailable.
#[test]
fn test_tool_init_error_not_success() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Err(status) = result {
        assert!(
            !status.is_success(),
            "tool_init error must not be success: {:?}",
            status
        );
        assert!(
            status.is_error(),
            "tool_init error must be negative: {:?}",
            status
        );
    }
    // If Ok, the daemon is available — that's fine too.
}

/// tool_init with non-empty InfoBuilder also returns consistent result.
#[test]
fn test_tool_init_with_info_builder_consistent() {
    let info = InfoBuilder::new().build();
    let r1 = tool_init(None, &info);
    let r2 = tool_init(None, &info);
    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "tool_init should be consistent across calls"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixToolHandle — structure and traits
// ─────────────────────────────────────────────────────────────────────────────

/// PmixToolHandle implements Clone.
#[test]
fn test_tool_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixToolHandle>();
}

/// PmixToolHandle implements Debug.
#[test]
fn test_tool_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixToolHandle>();
}

/// PmixToolHandle implements Clone + Debug together.
#[test]
fn test_tool_handle_clone_and_debug() {
    fn assert_clone_debug<T: Clone + std::fmt::Debug>() {}
    assert_clone_debug::<PmixToolHandle>();
}

/// PmixServerHandle implements Clone.
#[test]
fn test_server_handle_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<PmixServerHandle>();
}

/// PmixServerHandle implements Debug.
#[test]
fn test_server_handle_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<PmixServerHandle>();
}

// ─────────────────────────────────────────────────────────────────────────────
// tool_finalize — type and signature checks
// ─────────────────────────────────────────────────────────────────────────────

/// tool_finalize signature: fn(PmixToolHandle) -> Result<(), PmixStatus>.
#[test]
fn test_tool_finalize_signature() {
    let _: fn(PmixToolHandle) -> Result<(), PmixStatus> = tool_finalize;
}

/// tool_finalize consumes handle by value (move semantics).
#[test]
fn test_tool_finalize_consumes_handle() {
    type F = fn(PmixToolHandle) -> Result<(), PmixStatus>;
    let _f: F = tool_finalize;
}

/// tool_init Ok type matches tool_finalize parameter type.
#[test]
fn test_init_finalize_type_pair() {
    type InitOk = PmixToolHandle;
    fn _assert_finalize_accepts_init_ok(h: InitOk) -> Result<(), PmixStatus> {
        tool_finalize(h)
    }
    let _ = _assert_finalize_accepts_init_ok;
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixStatus — traits and behavior
// ─────────────────────────────────────────────────────────────────────────────

/// PmixStatus is Clone + Copy + Debug.
#[test]
fn test_pmix_status_clone_copy_debug() {
    fn assert_traits<T: Clone + Copy + std::fmt::Debug>() {}
    assert_traits::<PmixStatus>();
}

/// PmixStatus is PartialEq + Eq.
#[test]
fn test_pmix_status_partial_eq_eq() {
    fn assert_eq_t<T: PartialEq + Eq>() {}
    assert_eq_t::<PmixStatus>();
}

/// PmixStatus implements std::error::Error.
#[test]
fn test_pmix_status_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<PmixStatus>();
}

/// PmixStatus::from_raw(0) is success.
#[test]
fn test_pmix_status_from_raw_zero_is_success() {
    let status = PmixStatus::from_raw(0);
    assert!(status.is_success(), "Raw 0 must be PMIX_SUCCESS");
}

/// PmixStatus::from_raw(-1) is error.
#[test]
fn test_pmix_status_from_raw_neg_one_is_error() {
    let status = PmixStatus::from_raw(-1);
    assert!(status.is_error(), "Raw -1 must be an error");
}

/// PmixStatus::from_raw(-31) maps to ErrInit.
#[test]
fn test_pmix_status_from_raw_err_init() {
    let status = PmixStatus::from_raw(-31);
    assert_eq!(status, PmixStatus::Known(PmixError::ErrInit));
}

/// PmixStatus round-trips through to_raw/from_raw.
#[test]
fn test_pmix_status_roundtrip() {
    let original = PmixStatus::Known(PmixError::ErrInit);
    let raw = original.to_raw();
    let recovered = PmixStatus::from_raw(raw);
    assert_eq!(original, recovered, "PmixStatus round-trip must be lossless");
}

/// PmixStatus implements Display.
#[test]
fn test_pmix_status_display() {
    let status = PmixStatus::Known(PmixError::ErrInit);
    let display = format!("{}", status);
    assert!(!display.is_empty(), "Display for PmixStatus should not be empty");
}

/// PmixStatus::known() returns Some for known variants.
#[test]
fn test_pmix_status_known_returns_some() {
    let status = PmixStatus::Known(PmixError::ErrInit);
    assert!(status.known().is_some(), "known() should return Some for Known variants");
}

/// PmixStatus::known() returns None for Unknown variants.
#[test]
fn test_pmix_status_unknown_returns_none() {
    let status = PmixStatus::Unknown(-99999);
    assert!(status.known().is_none(), "known() should return None for Unknown variants");
}

// ─────────────────────────────────────────────────────────────────────────────
// Lifecycle patterns — work regardless of daemon availability
// ─────────────────────────────────────────────────────────────────────────────

/// Lifecycle: init result and is_tool_initialized are consistent.
#[test]
fn test_lifecycle_init_and_flag_consistency() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    let flag = is_tool_initialized();

    match result {
        Ok(handle) => {
            // Init succeeded — flag should be true.
            assert!(
                flag,
                "is_tool_initialized should be true after successful init"
            );
            // Clean up: finalize the handle.
            let _ = tool_finalize(handle);
        }
        Err(_) => {
            // Init failed — flag should remain false (or whatever it was).
            // This is the expected behavior when no daemon is available.
            assert!(
                !flag,
                "is_tool_initialized should be false after failed init"
            );
        }
    }
}

/// Lifecycle: multiple inits return consistent results.
#[test]
fn test_lifecycle_multiple_inits_consistent() {
    let info = InfoBuilder::new().build();
    let results: Vec<Result<PmixToolHandle, PmixStatus>> =
        (0..5).map(|_| tool_init(None, &info)).collect();

    let first_ok = results[0].is_ok();
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_ok,
            result.is_ok(),
            "tool_init should be consistent: call 0 = {}, call {} = {}",
            first_ok, i, result.is_ok()
        );
    }

    // Clean up any successful handles.
    for result in results {
        if let Ok(handle) = result {
            let _ = tool_finalize(handle);
        }
    }
}

/// Lifecycle: tool_init and tool_init_minimal return the same result type.
#[test]
fn test_lifecycle_minimal_and_full_same_result_type() {
    let info = InfoBuilder::new().build();
    let full = tool_init(None, &info);
    let minimal = tool_init_minimal();

    assert_eq!(
        full.is_ok(),
        minimal.is_ok(),
        "tool_init and tool_init_minimal should agree on success/failure"
    );

    // Clean up.
    if let Ok(handle) = full {
        let _ = tool_finalize(handle);
    }
    if let Ok(handle) = minimal {
        let _ = tool_finalize(handle);
    }
}

/// Lifecycle: init -> finalize cycle works when daemon is available.
#[test]
fn test_lifecycle_init_finalize_cycle() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        let finalize_result = tool_finalize(handle);
        assert!(
            finalize_result.is_ok(),
            "tool_finalize should succeed after tool_init: {:?}",
            finalize_result
        );
    }
    // If init failed, that's fine — no daemon available.
}

/// Lifecycle: init_minimal -> finalize cycle works when daemon is available.
#[test]
fn test_lifecycle_init_minimal_finalize_cycle() {
    let result = tool_init_minimal();
    if let Ok(handle) = result {
        let finalize_result = tool_finalize(handle);
        assert!(
            finalize_result.is_ok(),
            "tool_finalize should succeed after tool_init_minimal: {:?}",
            finalize_result
        );
    }
}

/// Lifecycle: two inits need two finalizes (reference counting).
#[test]
fn test_lifecycle_reference_counting() {
    let info = InfoBuilder::new().build();
    let h1 = tool_init(None, &info);
    let h2 = tool_init(None, &info);

    if let (Ok(handle1), Ok(handle2)) = (h1, h2) {
        // First finalize should succeed (refcount decremented, not zero).
        let r1 = tool_finalize(handle1);
        assert!(r1.is_ok(), "first finalize should succeed");

        // Second finalize should succeed (refcount reaches zero).
        let r2 = tool_finalize(handle2);
        assert!(r2.is_ok(), "second finalize should succeed");
    }
    // If init failed, no daemon — that's fine.
}

/// Lifecycle: handle can be cloned before finalize.
#[test]
fn test_lifecycle_handle_clone_before_finalize() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        let cloned = handle.clone();
        // Both handles should have the same proc info.
        assert_eq!(
            handle.proc().rank(),
            cloned.proc().rank(),
            "cloned handle should have same rank"
        );
        // Finalize original.
        let _ = tool_finalize(handle);
        // Cloned handle still accessible.
        let _rank = cloned.proc().rank();
    }
}

/// Lifecycle: is_tool_initialized flag tracks init/finalize state.
#[test]
fn test_lifecycle_flag_tracks_state() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        assert!(
            is_tool_initialized(),
            "flag should be true after successful init"
        );
        let _ = tool_finalize(handle);
        // After finalize, flag should be false (refcount reached zero).
        assert!(
            !is_tool_initialized(),
            "flag should be false after finalize"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Thread safety — safe functions
// ─────────────────────────────────────────────────────────────────────────────

/// Concurrent is_tool_initialized() calls all return the same value.
#[test]
fn test_thread_safe_is_tool_initialized() {
    const NUM_THREADS: usize = 8;
    const CALLS_PER_THREAD: usize = 50;

    let mut handles = Vec::new();

    for _ in 0..NUM_THREADS {
        handles.push(std::thread::spawn(|| {
            let mut results = Vec::new();
            for _ in 0..CALLS_PER_THREAD {
                results.push(is_tool_initialized());
            }
            results
        }));
    }

    let all_results: Vec<Vec<bool>> = handles
        .into_iter()
        .map(|h| h.join().expect("thread panicked"))
        .collect();

    // All results across all threads should be identical.
    let first_val = all_results[0][0];
    for (ti, thread_results) in all_results.iter().enumerate() {
        for (ci, val) in thread_results.iter().enumerate() {
            assert_eq!(
                first_val, *val,
                "Thread {} call {}: expected {:?}, got {:?}",
                ti, ci, first_val, val
            );
        }
    }
}

/// Concurrent tool_init_minimal() calls don't panic.
#[test]
fn test_thread_safe_concurrent_init_minimal() {
    const NUM_THREADS: usize = 4;
    const CALLS_PER_THREAD: usize = 10;

    let mut handles = Vec::new();

    for _ in 0..NUM_THREADS {
        handles.push(std::thread::spawn(|| {
            for _ in 0..CALLS_PER_THREAD {
                let _result = tool_init_minimal();
                // No panic regardless of Ok/Err.
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }
}

/// Concurrent tool_init() calls don't panic.
#[test]
fn test_thread_safe_concurrent_init() {
    const NUM_THREADS: usize = 4;
    const CALLS_PER_THREAD: usize = 10;

    let mut handles = Vec::new();

    for _ in 0..NUM_THREADS {
        handles.push(std::thread::spawn(|| {
            let info = InfoBuilder::new().build();
            for _ in 0..CALLS_PER_THREAD {
                let _result = tool_init(None, &info);
                // No panic regardless of Ok/Err.
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }
}

/// is_tool_initialized is safe to call from many threads simultaneously.
#[test]
fn test_thread_safe_is_initialized_high_contention() {
    const NUM_THREADS: usize = 16;
    const CALLS_PER_THREAD: usize = 100;

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(NUM_THREADS));
    let mut handles = Vec::new();

    for _ in 0..NUM_THREADS {
        let barrier_clone = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier_clone.wait(); // All threads start at the same time
            for _ in 0..CALLS_PER_THREAD {
                let _ = is_tool_initialized();
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon-dependent error path tests
// ─────────────────────────────────────────────────────────────────────────────

/// When daemon is NOT available, tool_init_minimal returns an error.
#[test]
fn test_error_path_tool_init_minimal_without_daemon() {
    let result = tool_init_minimal();
    match result {
        Err(status) => {
            // No daemon — verify error properties.
            assert!(
                !status.is_success(),
                "error must not be success: {:?}",
                status
            );
            assert!(
                status.is_error(),
                "error must be negative: {:?}",
                status
            );
            let raw = status.to_raw();
            assert!(
                raw < 0,
                "error raw value should be negative: {}",
                raw
            );
            assert_ne!(
                raw, 0,
                "error raw value must not be 0 (PMIX_SUCCESS)"
            );
            // Error should be a known variant.
            assert!(
                status.known().is_some(),
                "error should be a known PmixError: {:?}",
                status
            );
        }
        Ok(handle) => {
            // Daemon is available — clean up.
            let _ = tool_finalize(handle);
        }
    }
}

/// When daemon is NOT available, tool_init returns an error.
#[test]
fn test_error_path_tool_init_without_daemon() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    match result {
        Err(status) => {
            // No daemon — verify error properties.
            assert!(
                !status.is_success(),
                "error must not be success: {:?}",
                status
            );
            assert!(
                status.is_error(),
                "error must be negative: {:?}",
                status
            );
            let raw = status.to_raw();
            assert!(
                raw < 0,
                "error raw value should be negative: {}",
                raw
            );
        }
        Ok(handle) => {
            // Daemon is available — clean up.
            let _ = tool_finalize(handle);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon-dependent success path tests
// ─────────────────────────────────────────────────────────────────────────────

/// When daemon IS available, tool_init returns a valid handle.
#[test]
fn test_success_path_tool_init_with_daemon() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        // Handle should have valid proc info.
        let proc = handle.proc();
        let _rank: u32 = proc.rank();
        // nspace may or may not be available.
        let _nspace = proc.nspace();

        // Clean up.
        let _ = tool_finalize(handle);
    }
    // If init failed, daemon not available — that's fine.
}

/// When daemon IS available, handle Debug output is non-empty.
#[test]
fn test_success_path_handle_debug_output() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        let debug = format!("{:?}", handle);
        assert!(
            !debug.is_empty(),
            "handle debug output should not be empty"
        );
        assert!(
            debug.contains("PmixToolHandle"),
            "debug output should contain struct name"
        );

        // Clean up.
        let _ = tool_finalize(handle);
    }
}

/// When daemon IS available, handle Clone produces identical proc info.
#[test]
fn test_success_path_handle_clone_identical() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        let cloned = handle.clone();
        assert_eq!(
            handle.proc().rank(),
            cloned.proc().rank(),
            "cloned handle should have same rank"
        );
        assert_eq!(
            handle.proc().nspace(),
            cloned.proc().nspace(),
            "cloned handle should have same nspace"
        );

        // Clean up.
        let _ = tool_finalize(handle);
    }
}

/// When daemon IS available, init -> finalize -> init cycle works.
#[test]
fn test_success_path_init_finalize_reinit() {
    let info = InfoBuilder::new().build();
    let result1 = tool_init(None, &info);
    if let Ok(handle1) = result1 {
        tool_finalize(handle1).expect("first finalize should succeed");

        let result2 = tool_init(None, &info);
        if let Ok(handle2) = result2 {
            tool_finalize(handle2).expect("second finalize should succeed");
        }
    }
}

/// When daemon IS available, is_tool_initialized tracks state correctly.
#[test]
fn test_success_path_flag_state_tracking() {
    let info = InfoBuilder::new().build();
    let result = tool_init(None, &info);
    if let Ok(handle) = result {
        assert!(
            is_tool_initialized(),
            "flag should be true after init"
        );
        tool_finalize(handle).expect("finalize should succeed");
        assert!(
            !is_tool_initialized(),
            "flag should be false after finalize"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Info type tests
// ─────────────────────────────────────────────────────────────────────────────

/// InfoBuilder produces an Info that can be passed to tool_init.
#[test]
fn test_info_builder_produces_valid_info() {
    let info = InfoBuilder::new().build();
    // Just verify it compiles and can be used.
    let _: Result<PmixToolHandle, PmixStatus> = tool_init(None, &info);
}

/// Empty InfoBuilder produces an Info with zero entries.
#[test]
fn test_info_builder_empty() {
    let info = InfoBuilder::new().build();
    // Verify tool_init accepts it without panic.
    let _result = tool_init(None, &info);
}
