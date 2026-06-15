//! Integration tests for `PMIx_IOF_channel_string` via the safe `iof_channel_string()` wrapper.

use pmix::{IOFChannelFlags, utility::iof_channel_string};

#[test]
fn iof_channel_string_all_defined() {
    let channels = [
        IOFChannelFlags::NO_CHANNELS,
        IOFChannelFlags::STDIN,
        IOFChannelFlags::STDOUT,
        IOFChannelFlags::STDERR,
        IOFChannelFlags::STDDIAG,
        IOFChannelFlags::ALL_CHANNELS,
    ];
    for c in channels {
        let result = iof_channel_string(c);
        assert!(
            result.is_ok(),
            "iof_channel_string({:?}) should return Ok, got {:?}",
            c,
            result
        );
        assert!(
            !result.unwrap().is_empty(),
            "iof_channel_string({:?}) should not be empty",
            c
        );
    }
}

#[test]
fn iof_channel_string_distinct() {
    let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
    let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
    assert_ne!(stdin, stdout, "STDIN and STDOUT must differ");
}

#[test]
fn iof_channel_string_return_type() {
    let _r: Result<String, pmix::PmixStatus> = iof_channel_string(IOFChannelFlags::STDOUT);
}
