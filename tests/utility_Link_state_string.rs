//! Integration tests for `PMIx_Link_state_string` via the safe `link_state_string()` wrapper.
//!
//! These tests call into the real PMIx library. They do NOT require a running
//! PMIx daemon — `PMIx_Link_state_string` only looks up a static string table
//! inside the library.

use pmix::{utility::link_state_string, PmixLinkState};

/// `link_state_string` returns `Ok(String)` for PMIX_LINK_STATE_UNKNOWN (0).
///
/// The PMIx spec defines `PMIx_Link_state_string` as returning a non-null,
/// null-terminated string for any valid `pmix_link_state_t`. For
/// PMIX_LINK_STATE_UNKNOWN, the library returns "UNKNOWN".
#[test]
fn link_state_string_unknown_returns_ok() {
    let state = PmixLinkState::UnknownState;
    let result = link_state_string(state);
    assert!(
        result.is_ok(),
        "link_state_string(UNKNOWN) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "UNKNOWN");
}

/// `link_state_string` returns "INACTIVE" for PMIX_LINK_DOWN (1).
#[test]
fn link_state_string_down_returns_inactive() {
    let state = PmixLinkState::LinkDown;
    let result = link_state_string(state);
    assert!(
        result.is_ok(),
        "link_state_string(LINK_DOWN) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "INACTIVE");
}

/// `link_state_string` returns "ACTIVE" for PMIX_LINK_UP (2).
#[test]
fn link_state_string_up_returns_active() {
    let state = PmixLinkState::LinkUp;
    let result = link_state_string(state);
    assert!(
        result.is_ok(),
        "link_state_string(LINK_UP) should return Ok, got {:?}",
        result
    );
    let desc = result.unwrap();
    assert_eq!(desc, "ACTIVE");
}

/// `link_state_string` handles all known link states without error.
#[test]
fn link_state_string_all_known() {
    let states = [
        (PmixLinkState::UnknownState, "UNKNOWN"),
        (PmixLinkState::LinkDown, "INACTIVE"),
        (PmixLinkState::LinkUp, "ACTIVE"),
    ];
    for (state, expected) in states {
        let result = link_state_string(state);
        assert!(
            result.is_ok(),
            "link_state_string({:?}) should return Ok, got {:?}",
            state,
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "link_state_string({:?}) should not return empty string",
            state
        );
        assert_eq!(
            desc, expected,
            "link_state_string({:?}) should return '{}'",
            state, expected
        );
    }
}

/// `link_state_string` is deterministic — the same state always
/// produces the same string description.
#[test]
fn link_state_string_is_deterministic() {
    let state = PmixLinkState::LinkUp;
    let first = link_state_string(state).unwrap();
    let second = link_state_string(state).unwrap();
    assert_eq!(
        first, second,
        "link_state_string must be deterministic for the same input"
    );
}

/// `link_state_string` returns different strings for different states.
#[test]
fn link_state_string_distinct_for_different_states() {
    let up = link_state_string(PmixLinkState::LinkUp).unwrap();
    let down = link_state_string(PmixLinkState::LinkDown).unwrap();
    assert_ne!(
        up, down,
        "link_state_string(LINK_UP) and link_state_string(LINK_DOWN) must return different strings"
    );
}

/// `link_state_string` returns a `Result<String, pmix::PmixStatus>`, not a raw pointer.
///
/// This is a compile-time type check — if the function signature changes,
/// this test will fail to compile.
#[test]
fn link_state_string_returns_result_string() {
    let state = PmixLinkState::LinkUp;
    let _result: Result<String, pmix::PmixStatus> = link_state_string(state);
}

/// `PmixLinkState::from_raw` maps known values correctly.
#[test]
fn link_state_from_raw() {
    assert_eq!(PmixLinkState::from_raw(0), PmixLinkState::UnknownState);
    assert_eq!(PmixLinkState::from_raw(1), PmixLinkState::LinkDown);
    assert_eq!(PmixLinkState::from_raw(2), PmixLinkState::LinkUp);
}

/// `PmixLinkState::to_raw` returns the correct C values.
#[test]
fn link_state_to_raw() {
    assert_eq!(PmixLinkState::UnknownState.to_raw(), 0);
    assert_eq!(PmixLinkState::LinkDown.to_raw(), 1);
    assert_eq!(PmixLinkState::LinkUp.to_raw(), 2);
}

/// `PmixLinkState::from_raw` / `to_raw` roundtrip for all values.
#[test]
fn link_state_roundtrip() {
    for raw in [0u8, 1, 2, 42, 255] {
        let state = PmixLinkState::from_raw(raw);
        assert_eq!(state.to_raw(), raw);
    }
}
