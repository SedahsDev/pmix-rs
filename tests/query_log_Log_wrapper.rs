//! Wrapper tests for query_log.rs — Log Operations.
//!
//! Tests exercise `log_data` and `log_data_nb` wrapper logic without PMIx_Init.
//! FFI calls return errors gracefully, so we can test error paths and callback
//! registration/cleanup.
//!
//! Coverage targets:
//!   - log_data (lines 501-534) — FFI call + error path
//!   - log_data_nb (lines 606-665) — callback reg + FFI + cleanup

use pmix::query_log::*;
use pmix::{InfoBuilder, PmixStatus};

// ─────────────────────────────────────────────────────────────────────────────
// log_data — synchronous log
// ─────────────────────────────────────────────────────────────────────────────

/// log_data with empty data and directives returns error.
#[test]
fn test_log_data_empty() {
    let data: Vec<pmix::Info> = vec![];
    let directives: Vec<pmix::Info> = vec![];
    let result = log_data(&data, &directives);
    assert!(result.is_err());
}

/// log_data with data but no directives returns error.
#[test]
fn test_log_data_with_data() {
    let info = InfoBuilder::new().build();
    let data = vec![info];
    let directives: Vec<pmix::Info> = vec![];
    let result = log_data(&data, &directives);
    assert!(result.is_err());
}

/// log_data with directives but no data returns error.
#[test]
fn test_log_data_with_directives() {
    let data: Vec<pmix::Info> = vec![];
    let info = InfoBuilder::new().build();
    let directives = vec![info];
    let result = log_data(&data, &directives);
    assert!(result.is_err());
}

/// log_data with both data and directives returns error.
#[test]
fn test_log_data_with_both() {
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();
    let data = vec![info1];
    let directives = vec![info2];
    let result = log_data(&data, &directives);
    assert!(result.is_err());
}

/// log_data with multiple data entries returns error.
#[test]
fn test_log_data_multiple_entries() {
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();
    let data = vec![info1, info2];
    let directives: Vec<pmix::Info> = vec![];
    let result = log_data(&data, &directives);
    assert!(result.is_err());
}

/// log_data is deterministic.
#[test]
fn test_log_data_deterministic() {
    let data: Vec<pmix::Info> = vec![];
    let directives: Vec<pmix::Info> = vec![];
    let r1 = log_data(&data, &directives);
    let r2 = log_data(&data, &directives);
    assert_eq!(r1.is_err(), r2.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// log_data_nb — non-blocking log
// ─────────────────────────────────────────────────────────────────────────────

/// log_data_nb with empty data returns error, callback not invoked.
#[test]
fn test_log_data_nb_empty() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl LogCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let data: Vec<pmix::Info> = vec![];
    let directives: Vec<pmix::Info> = vec![];
    let result = log_data_nb(&data, &directives, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// log_data_nb with data returns error, callback not invoked.
#[test]
fn test_log_data_nb_with_data() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl LogCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info = InfoBuilder::new().build();
    let data = vec![info];
    let directives: Vec<pmix::Info> = vec![];
    let result = log_data_nb(&data, &directives, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// log_data_nb with directives returns error, callback not invoked.
#[test]
fn test_log_data_nb_with_directives() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl LogCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let data: Vec<pmix::Info> = vec![];
    let info = InfoBuilder::new().build();
    let directives = vec![info];
    let result = log_data_nb(&data, &directives, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// log_data_nb with both data and directives returns error.
#[test]
fn test_log_data_nb_with_both() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    struct Cb {
        c: Arc<AtomicBool>,
    }
    impl LogCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {
            self.c.store(true, Ordering::SeqCst);
        }
    }
    let info1 = InfoBuilder::new().build();
    let info2 = InfoBuilder::new().build();
    let data = vec![info1];
    let directives = vec![info2];
    let result = log_data_nb(&data, &directives, Box::new(Cb { c: Arc::clone(&called) }));
    assert!(result.is_err());
    assert!(
        !called.load(Ordering::SeqCst),
        "callback should not be invoked on immediate failure"
    );
}

/// log_data_nb is deterministic.
#[test]
fn test_log_data_nb_deterministic() {
    struct Cb;
    impl LogCallback for Cb {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let data: Vec<pmix::Info> = vec![];
    let directives: Vec<pmix::Info> = vec![];
    let r1 = log_data_nb(&data, &directives, Box::new(Cb));
    let r2 = log_data_nb(&data, &directives, Box::new(Cb));
    assert_eq!(r1.is_err(), r2.is_err());
}
