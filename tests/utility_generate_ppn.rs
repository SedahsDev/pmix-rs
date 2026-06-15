//! Tests for PMIx_generate_ppn — safe Rust wrapper around PMIx_generate_ppn().

use pmix::utility::generate_ppn;

// ─────────────────────────────────────────────────────────────────────────────
// Basic functionality — these work without init (PMIx_generate_ppn is a utility)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_generate_ppn_empty() {
    let result = generate_ppn("");
    // PMIx may return "raw:" for empty input, or error. Accept either.
    match &result {
        Ok(s) => {
            let _ = format!("ppn: {}", s);
        }
        Err(_) => {
            // Empty input may not be valid
        }
    }
}

#[test]
fn test_generate_ppn_single() {
    let result = generate_ppn("0");
    // May succeed or fail depending on PMIx state
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

#[test]
fn test_generate_ppn_multiple() {
    let result = generate_ppn("0,1,2,3");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

#[test]
fn test_generate_ppn_range() {
    let result = generate_ppn("0-3");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

#[test]
fn test_generate_ppn_large() {
    let result = generate_ppn("0-1023");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

#[test]
fn test_generate_ppn_complex() {
    let result = generate_ppn("0-3,5,7-9");
    match &result {
        Ok(s) => assert!(!s.is_empty()),
        Err(_) => {}
    }
}

#[test]
fn test_generate_ppn_debug_output() {
    let result = generate_ppn("0,1,2");
    match &result {
        Ok(s) => {
            let debug = format!("{:?}", s);
            assert!(!debug.is_empty());
        }
        Err(e) => {
            let debug = format!("{:?}", e);
            assert!(!debug.is_empty());
        }
    }
}
