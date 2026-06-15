//! Integration tests for `PMIx_Data_range_string` via the safe `data_range_string()` wrapper.

use pmix::{PmixDataRange, utility::data_range_string};

#[test]
fn data_range_string_all_defined() {
    let values = [
        PmixDataRange::Undef,
        PmixDataRange::Rm,
        PmixDataRange::Local,
        PmixDataRange::Namespace,
        PmixDataRange::Session,
        PmixDataRange::Global,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
    ];
    for v in values {
        let result = data_range_string(v);
        assert!(result.is_ok(), "data_range_string({:?}) should return Ok, got {:?}", v, result);
        assert!(!result.unwrap().is_empty(), "data_range_string({:?}) should not be empty", v);
    }
}

#[test]
fn data_range_string_distinct() {
    let local = data_range_string(PmixDataRange::Local).unwrap();
    let global = data_range_string(PmixDataRange::Global).unwrap();
    assert_ne!(local, global, "Local and Global must differ");
}

#[test]
fn data_range_string_unknown() {
    let result = data_range_string(PmixDataRange::Unknown);
    assert!(
        result.is_ok(),
        "Unknown should handle gracefully, got {:?}",
        result
    );
}

#[test]
fn data_range_string_return_type() {
    let _r: Result<String, pmix::PmixStatus> = data_range_string(PmixDataRange::Session);
}
