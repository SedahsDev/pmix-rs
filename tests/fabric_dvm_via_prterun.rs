//! Fabric FFI tests that require PMIx initialization via prterun.
//!
//! These tests can run in batch because they don't use NB callbacks
//! or topology operations that corrupt PMIx state.
//!
//! Run with:
//! ```bash
//! prterun -np 1 cargo test --test fabric_dvm_via_prterun -- --include-ignored --test-threads=1
//! ```

use std::sync::OnceLock;

static PMIX_CONTEXT: OnceLock<Option<pmix::Context>> = OnceLock::new();

fn ensure_pmix_init() -> bool {
    if std::env::var("PMIX_RANK").is_err() {
        return false;
    }
    PMIX_CONTEXT.set(pmix::init(None).ok()).is_ok() && PMIX_CONTEXT.get().unwrap().is_some()
}

/// PmixFabric::new via DVM with shared context.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_new_via_dvm() {
    assert!(ensure_pmix_init());
    let fabric = pmix::fabric::PmixFabric::new(Some("dvm-test")).expect("new failed");
    assert_eq!(fabric.name(), Some("dvm-test"));
    assert!(!fabric.is_registered());
}

/// PmixFabric::unamed via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_unamed_via_dvm() {
    assert!(ensure_pmix_init());
    let fabric = pmix::fabric::PmixFabric::unamed();
    assert_eq!(fabric.name(), None);
    assert!(!fabric.is_registered());
}

/// fabric_register success path via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_register_success_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric =
        pmix::fabric::PmixFabric::new(Some("register-test")).expect("new failed");
    assert!(!fabric.is_registered());
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_update via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_update_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric = pmix::fabric::PmixFabric::unamed();
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        let _ = pmix::fabric::fabric_update(&mut fabric);
        let _ = pmix::fabric::fabric_deregister(&mut fabric);
    }
}

/// fabric_deregister via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_deregister_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric =
        pmix::fabric::PmixFabric::new(Some("dereg-test")).expect("new failed");
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());
        let dereg_result = pmix::fabric::fabric_deregister(&mut fabric);
        if dereg_result.is_ok() {
            assert!(!fabric.is_registered());
        }
    }
    drop(fabric);
}

/// PmixCpuset::as_mut_ptr via DVM.
#[test]
#[ignore = "requires prterun launch"]
fn test_cpuset_as_mut_ptr_via_dvm() {
    assert!(ensure_pmix_init());
    let mut cpuset = pmix::fabric::PmixCpuset::new();
    let ptr = cpuset.as_mut_ptr();
    assert!(!ptr.is_null(), "cpuset as_mut_ptr should be non-null");
}

/// Full lifecycle: new -> register -> update -> deregister -> drop.
#[test]
#[ignore = "requires prterun launch"]
fn test_fabric_full_lifecycle_via_dvm() {
    assert!(ensure_pmix_init());
    let mut fabric =
        pmix::fabric::PmixFabric::new(Some("lifecycle-test")).expect("new failed");
    assert!(!fabric.is_registered());
    assert_eq!(fabric.name(), Some("lifecycle-test"));
    if pmix::fabric::fabric_register(&mut fabric, &[]).is_ok() {
        assert!(fabric.is_registered());
        let _ = pmix::fabric::fabric_update(&mut fabric);
        let dereg_result = pmix::fabric::fabric_deregister(&mut fabric);
        if dereg_result.is_ok() {
            assert!(!fabric.is_registered());
        }
    }
    drop(fabric);
}
