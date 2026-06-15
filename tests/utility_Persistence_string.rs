//! Integration tests for `PMIx_Persistence_string` via the safe `persistence_string()` wrapper.

use pmix::{PmixPersistence, utility::persistence_string};

#[test]
fn persistence_string_all_defined_values() {
    let values = [
        PmixPersistence::Indefinite,
        PmixPersistence::FirstRead,
        PmixPersistence::Process,
        PmixPersistence::Application,
        PmixPersistence::Session,
        PmixPersistence::Invalid,
    ];
    for v in values {
        let result = persistence_string(v);
        assert!(result.is_ok(), "persistence_string({:?}) should return Ok, got {:?}", v, result);
        assert!(!result.unwrap().is_empty(), "persistence_string({:?}) should not be empty", v);
    }
}

#[test]
fn persistence_string_distinct() {
    let indef = persistence_string(PmixPersistence::Indefinite).unwrap();
    let first = persistence_string(PmixPersistence::FirstRead).unwrap();
    assert_ne!(indef, first, "Indefinite and FirstRead must differ");
}

#[test]
fn persistence_string_unknown() {
    let result = persistence_string(PmixPersistence::Unknown(42));
    assert!(result.is_ok(), "Unknown(42) should handle gracefully, got {:?}", result);
}

#[test]
fn persistence_string_return_type() {
    let _r: Result<String, pmix::PmixStatus> = persistence_string(PmixPersistence::Indefinite);
}
