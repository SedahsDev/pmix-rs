//! Property-based tests for data serialization roundtrips.
//!
//! Tests the fundamental property: `pack(x) → unload → load → unpack(x')` yields `x == x'`.
//! Uses proptest to generate thousands of random inputs across all scalar PMIx data types.
//!
//! These tests require `PMIx_Init` and must be run under the DVM:
//! ```bash
//! prterun -np 1 cargo test --test data_serialization_proptest -- --ignored --test-threads=1
//! ```

use pmix::{data_serialization::*, init, PmixDataType};
use proptest::prelude::*;
use std::sync::OnceLock;

// ─────────────────────────────────────────────────────────────────────────────
// Singleton PMIx init — PMIx can only be initialized once per process.
// Proptest may call the test closure many times (shrinking), so we need
// a shared Context that lives for the whole test run.
// ─────────────────────────────────────────────────────────────────────────────

static PMIX_CTX: OnceLock<pmix::Context> = OnceLock::new();

fn ensure_init() -> &'static pmix::Context {
    PMIX_CTX.get_or_init(|| init(None).expect("PMIx_Init failed — run under prterun"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: pack → unload → load → unpack roundtrip (within PMIx session)
// ─────────────────────────────────────────────────────────────────────────────

fn roundtrip_scalar<T: Copy + PartialEq>(input: T, data_type: PmixDataType) -> Result<T, String> {
    let mut out: T = unsafe { std::mem::zeroed() };

    let mut buf = data_buffer_create().map_err(|e| format!("buffer create failed: {:?}", e))?;
    data_pack(None, &buf, &input, 1, data_type).map_err(|e| format!("pack failed: {:?}", e))?;

    let payload = data_unload(&buf).map_err(|e| format!("unload failed: {:?}", e))?;
    data_buffer_release(&mut buf);

    let buf2 = data_buffer_create().map_err(|e| format!("buffer create failed: {:?}", e))?;
    data_load(&buf2, &payload).map_err(|e| format!("load failed: {:?}", e))?;

    let mut count: i32 = 1;
    data_unpack(None, &buf2, &mut out, &mut count, data_type)
        .map_err(|e| format!("unpack failed: {:?}", e))?;

    if count != 1 {
        return Err(format!("expected 1 value, got {}", count));
    }

    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalar roundtrip properties — 12 types (require PMIx_Init via prterun)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_int8(val in any::<i8>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Int8).unwrap();
        prop_assert_eq!(val, out, "int8 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_int16(val in any::<i16>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Int16).unwrap();
        prop_assert_eq!(val, out, "int16 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_int32(val in any::<i32>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Int32).unwrap();
        prop_assert_eq!(val, out, "int32 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_int64(val in any::<i64>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Int64).unwrap();
        prop_assert_eq!(val, out, "int64 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_uint8(val in any::<u8>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Uint8).unwrap();
        prop_assert_eq!(val, out, "uint8 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_uint16(val in any::<u16>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Uint16).unwrap();
        prop_assert_eq!(val, out, "uint16 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_uint32(val in any::<u32>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Uint32).unwrap();
        prop_assert_eq!(val, out, "uint32 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_uint64(val in any::<u64>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Uint64).unwrap();
        prop_assert_eq!(val, out, "uint64 roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_float(val in (f32::MIN..=f32::MAX).prop_filter("nan/inf", |v| v.is_finite())) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Float).unwrap();
        prop_assert_eq!(val, out, "float roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_double(val in (f64::MIN..=f64::MAX).prop_filter("nan/inf", |v| v.is_finite())) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Double).unwrap();
        prop_assert_eq!(val, out, "double roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_bool(val in any::<bool>()) {
        ensure_init();
        let packed_val: u8 = if val { 1 } else { 0 };
        let out = roundtrip_scalar(packed_val, PmixDataType::Bool).unwrap();
        prop_assert_eq!(packed_val, out, "bool roundtrip failed for {}", val);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_size(val in any::<usize>()) {
        ensure_init();
        let out = roundtrip_scalar(val, PmixDataType::Size).unwrap();
        prop_assert_eq!(val, out, "size roundtrip failed for {}", val);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Multi-value roundtrip properties (require PMIx_Init via prterun)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_multi_int32(values in proptest::collection::vec(any::<i32>(), 1..=64)) {
        ensure_init();
        let num_vals = values.len() as i32;
        let mut buf = data_buffer_create().unwrap();
        for val in &values {
            data_pack(None, &buf, val, 1, PmixDataType::Int32).unwrap();
        }
        let payload = data_unload(&buf).unwrap();
        data_buffer_release(&mut buf);

        let buf2 = data_buffer_create().unwrap();
        data_load(&buf2, &payload).unwrap();

        let mut out: Vec<i32> = Vec::with_capacity(values.len());
        for _ in 0..num_vals {
            let mut v: i32 = 0;
            let mut count: i32 = 1;
            data_unpack(None, &buf2, &mut v, &mut count, PmixDataType::Int32).unwrap();
            out.push(v);
        }
        prop_assert_eq!(values, out, "multi-int32 roundtrip failed for {} values", num_vals);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_multi_uint64(values in proptest::collection::vec(any::<u64>(), 1..=32)) {
        ensure_init();
        let num_vals = values.len() as i32;
        let mut buf = data_buffer_create().unwrap();
        for val in &values {
            data_pack(None, &buf, val, 1, PmixDataType::Uint64).unwrap();
        }
        let payload = data_unload(&buf).unwrap();
        data_buffer_release(&mut buf);

        let buf2 = data_buffer_create().unwrap();
        data_load(&buf2, &payload).unwrap();

        let mut out: Vec<u64> = Vec::with_capacity(values.len());
        for _ in 0..num_vals {
            let mut v: u64 = 0;
            let mut count: i32 = 1;
            data_unpack(None, &buf2, &mut v, &mut count, PmixDataType::Uint64).unwrap();
            out.push(v);
        }
        prop_assert_eq!(values, out, "multi-uint64 roundtrip failed for {} values", num_vals);
    }

    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_roundtrip_multi_float(values in proptest::collection::vec((f32::MIN..=f32::MAX).prop_filter("nan/inf", |v: &f32| v.is_finite()), 1..=64)) {
        ensure_init();
        let num_vals = values.len() as i32;
        let mut buf = data_buffer_create().unwrap();
        for val in &values {
            data_pack(None, &buf, val, 1, PmixDataType::Float).unwrap();
        }
        let payload = data_unload(&buf).unwrap();
        data_buffer_release(&mut buf);

        let buf2 = data_buffer_create().unwrap();
        data_load(&buf2, &payload).unwrap();

        let mut out: Vec<f32> = Vec::with_capacity(values.len());
        for _ in 0..num_vals {
            let mut v: f32 = 0.0;
            let mut count: i32 = 1;
            data_unpack(None, &buf2, &mut v, &mut count, PmixDataType::Float).unwrap();
            out.push(v);
        }
        prop_assert_eq!(values, out, "multi-float roundtrip failed for {} values", num_vals);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer size property (requires PMIx_Init via prterun)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    #[ignore] // requires PMIx_Init — run under prterun
    fn prop_buffer_grows_with_data(val in any::<i32>()) {
        ensure_init();
        let buf = data_buffer_create().unwrap();
        let initial = buf.bytes_allocated();
        data_pack(None, &buf, &val, 1, PmixDataType::Int32).unwrap();
        let after = buf.bytes_allocated();
        prop_assert!(after >= initial, "buffer should not shrink after packing");
        prop_assert!(buf.bytes_used() >= 4, "buffer should have at least 4 bytes used for int32");
    }
}
