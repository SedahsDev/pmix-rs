//! Integration tests for `PMIx_Info_directives_string`.

use pmix::{InfoFlags, utility::info_directives_string};

#[test]
fn info_directives_string_all_defined() {
    let directives = [
        InfoFlags::REQD,
        InfoFlags::QUALIFIER,
        InfoFlags::PERSISTENT,
        InfoFlags::REQD_PROCESSED,
    ];
    for d in directives {
        let result = info_directives_string(d);
        assert!(
            result.is_ok(),
            "info_directives_string({:?}) should return Ok, got {:?}",
            d,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "info_directives_string({:?}) should not be empty",
            d
        );
    }
}

#[test]
fn info_directives_string_return_type() {
    let _r: Result<String, pmix::PmixStatus> = info_directives_string(InfoFlags::REQD);
}
