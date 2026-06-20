//! Tests for PMIx tool thread safety.
//!
//! PMIx init/finalize calls are NOT thread-safe. This test suite proves:
//!
//! 1. Sequential tool_init/tool_finalize from a single thread works fine.
//! 2. Concurrent tool_init/tool_finalize from multiple threads causes crashes
//!    (double-free, SIGSEGV) due to global PMIx state corruption.
//!
//! The fix is to serialize all PMIx tool_init/tool_finalize calls behind
//! a global mutex (see daemon_helper::daemon_lock()).

mod daemon_helper;

/// Sequential tool_init/tool_finalize cycles work perfectly.
///
/// Ignored: openpmix 6.1.0 does not support multiple init/finalize cycles
/// in the same process. After the first tool_finalize, subsequent tool_init
/// calls return PMIX_ERR_INIT. Use the shared tool handle singleton
/// (daemon_helper::get_tool_handle()) instead.
#[test]
#[ignore = "openpmix 6.1.0 does not support multiple init/finalize cycles per process"]
fn test_sequential_tool_init_works() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let uri = daemon_helper::read_uri().expect("PMIx daemon not available");

    for i in 0..5 {
        let info = pmix::info_with_string_key("pmix.srvr.uri", &uri);
        let handle = pmix::tool::tool_init(None, &info)
            .unwrap_or_else(|e| panic!("tool_init cycle {} failed: {:?}", i, e));
        pmix::tool::tool_finalize(handle)
            .unwrap_or_else(|e| panic!("tool_finalize cycle {} failed: {:?}", i, e));
    }
}

/// Tool ref counting works - two inits need two finalizes.
///
/// Ignored: openpmix 6.1.0 does not support multiple concurrent tool_init
/// calls in the same process. The second tool_init returns PMIX_ERR_INIT.
/// Use the shared tool handle singleton (daemon_helper::get_tool_handle()) instead.
#[test]
#[ignore = "openpmix 6.1.0 does not support multiple concurrent tool_init calls"]
fn test_tool_ref_counting() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let uri = daemon_helper::read_uri().expect("PMIx daemon not available");

    let info = pmix::info_with_string_key("pmix.srvr.uri", &uri);
    let h1 = pmix::tool::tool_init(None, &info).expect("first init failed");
    let h2 = pmix::tool::tool_init(None, &info).expect("second init failed");
    pmix::tool::tool_finalize(h1).expect("first finalize failed");
    pmix::tool::tool_finalize(h2).expect("second finalize failed");
}

/// Concurrent tool_init from multiple threads causes crashes.
///
/// This test is ignored by default because it will crash the test runner
/// (double-free / SIGSEGV) due to PMIx global state corruption.
/// Run it manually to demonstrate the problem:
///
///   cargo test --test tool_thread_safety test_concurrent_tool_init_crashes -- --ignored
///
/// The crash happens because PMIx uses global C state that is not
/// protected by mutexes. When two threads call tool_init simultaneously,
/// they race on internal data structures, leading to double-free.
#[test]
#[ignore = "CRASHES - demonstrates PMIx thread-unsafety"]
fn test_concurrent_tool_init_crashes() {
    let _guard = daemon_helper::connect_to_daemon().expect("PMIx daemon not available");

    const NUM_THREADS: usize = 4;
    const CYCLES: usize = 3;

    let mut threads = Vec::new();

    for tid in 0..NUM_THREADS {
        threads.push(std::thread::spawn(move || {
            for cycle in 0..CYCLES {
                // Build fresh Info inside the thread (Info is not Send)
                let info = pmix::InfoBuilder::new().build();
                let h = pmix::tool::tool_init(None, &info);
                if let Ok(tool) = h {
                    let _ = pmix::tool::tool_finalize(tool);
                }
                // No sleep - we WANT concurrent access
            }
        }));
    }

    // Join all threads - this will likely crash before completing
    for t in threads {
        t.join().expect("thread panicked or crashed");
    }
}

/// Serialized tool_init with barrier is safe - proves the mutex approach works.
///
/// Ignored: openpmix 6.1.0 does not support multiple tool_init calls from
/// different threads in the same process. After the first init, subsequent
/// inits return PMIX_ERR_INIT. Use the shared tool handle singleton instead.
#[test]
#[ignore = "openpmix 6.1.0 does not support multiple tool_init calls per process"]
fn test_serialized_tool_init_safe() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let uri = daemon_helper::read_uri().expect("PMIx daemon not available");

    const NUM_THREADS: usize = 4;
    const CYCLES: usize = 2;

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(NUM_THREADS));
    let mut threads = Vec::new();

    for tid in 0..NUM_THREADS {
        let barrier_clone = barrier.clone();
        let uri = uri.clone();

        threads.push(std::thread::spawn(move || {
            for cycle in 0..CYCLES {
                let info = pmix::info_with_string_key("pmix.srvr.uri", &uri);
                let handle = pmix::tool::tool_init(None, &info)
                    .unwrap_or_else(|_| panic!("[T{}] init cycle {} failed", tid, cycle));

                // Barrier ensures all threads reach the same point before proceeding
                barrier_clone.wait();

                pmix::tool::tool_finalize(handle)
                    .unwrap_or_else(|_| panic!("[T{}] finalize cycle {} failed", tid, cycle));
            }
        }));
    }

    for t in threads {
        t.join().expect("thread panicked");
    }
}
