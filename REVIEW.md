# PMIx Rust Bindings — Code Review

**Reviewer:** Sedahs (Grok-4.3 session)  
**Date:** 2026-06-13  
**Scope:** Full crate at `/home/bzf/projects/pmix-rs/`

## Summary

The crate provides comprehensive safe-ish Rust wrappers around the PMIx 5.x C library via bindgen + manual FFI glue. It is functional (all unit + integration tests pass, doctests now pass after fixes), but has several areas that need attention before a 0.2 or 1.0 release.

## 1. Safety & Unsafe Code (Highest Priority)

### Problem: `unsafe fn from_raw` bodies contain unsafe operations without explicit `unsafe` blocks (Rust 2024 edition)

**Location:** `src/fabric.rs:699`
```rust
unsafe fn from_raw(raw: &ffi::pmix_device_distance) -> Self {
    ...
    CStr::from_ptr(raw.uuid).to_string_lossy()...   // unsafe op inside unsafe fn
}
```

**Recommendation:** In Rust 2024, even inside `unsafe fn`, you must wrap unsafe operations in `unsafe { }`. Either:
- Add `unsafe { CStr::from_ptr(...) }`, or
- Change these `from_raw` methods to regular `fn` + document that callers must uphold invariants (preferred for `from_raw` patterns).

Similar pattern exists in `data_serialization.rs:253` and `851`.

### Problem: 436 `unsafe` occurrences with limited safety comments

Many FFI calls lack `// SAFETY:` comments explaining why the call is sound. This makes audit very hard.

**Recommendation:** Add `// SAFETY:` comments to every `unsafe { }` block, especially around `PMIx_*` calls and raw pointer handling.

## 2. Documentation & Doctests

### Status: Much improved

All 61 doctests now compile and pass after the recent fixes. Good work.

### Remaining issues

- `src/cpu_locality.rs:168` — large doc comment above a macro that rustdoc ignores (`unused_doc_comments`).
- Several module-level examples still assume a running PMIx daemon (correctly marked `no_run` or ignored).

**Recommendation:** Either remove the doc comment or move the documentation into the generated items if the macro can be made to emit docs.

## 3. API Ergonomics

### `Info` and `PmixValue` are awkward to construct

Users must write:
```rust
let info = InfoBuilder::new().build();
server_init(Some(&module), &info)
```

instead of the more natural `&[]` or `Info::default()`.

**Recommendation:**
- Implement `Default` for `Info` (returns empty).
- Consider `From<Vec<...>>` or `FromIterator` for `Info`.
- Provide a `pmix::empty_info()` helper or make `&[]` coerce via a newtype.

### `Proc` is `Clone` but `Info` is not

This caused a doctest failure in `process_mgmt`. Inconsistent.

**Recommendation:** Decide on a policy — either make both `Clone` (cheap if they just copy the handle + length) or document why `Info` is not.

### Redundant field names (Clippy)

`src/lib.rs:2202` and `2219`:
```rust
handle: handle,
```
should be `handle,`.

Easy win.

## 4. Build System & Portability

### Hardcoded paths in `build.rs`

```rust
println!("cargo:rustc-link-search=/lib64/");
.clang_arg("-I/usr/lib/x86_64-linux-gnu/pmix2/include/")
```

This only works on the maintainer's Debian box.

**Recommendation:**
- Use `pkg-config` (via `pkg-config` crate) to find PMIx.
- Fall back to common paths or environment variables (`PMIX_INCLUDE_DIR`, `PMIX_LIB_DIR`).
- Document the supported PMIx versions and how to point bindgen at headers.

### Edition 2024

Using the brand-new 2024 edition is brave. It forces the `unsafe` block rule, which is good long-term, but may surprise contributors.

**Recommendation:** Add a `rust-version = "1.85"` to `Cargo.toml` and a comment explaining why 2024 was chosen.

## 5. Testing Strategy

- 15–25% of tests are `#[ignore]` because they require a running `prte` / PMIx server. This is expected for integration tests.
- No `#[cfg(feature = "daemon-tests")]` or similar to easily run the full suite.

**Recommendation:** Add a `daemon` feature flag that enables the ignored tests, or document the exact `prte` command needed in `README.md`.

## 6. Other Observations

- `cstring-array` dependency is used but its version is very loose (`0.1`).
- No `thiserror` or custom error type — everything funnels through `PmixStatus`. Acceptable for a low-level binding but consider a richer error type for higher-level users.
- `PmixServerModule` has ~30 `Option<unsafe extern "C" fn()>` fields — all the same dummy signature. This is probably a placeholder; real callbacks will need distinct signatures.

## Suggested Next Steps (Priority Order)

1. Fix the `unsafe fn from_raw` + `CStr::from_ptr` issues (Rust 2024 compatibility).
2. Add `// SAFETY:` comments to the largest unsafe blocks.
3. Make `Info` implement `Default` and `Clone`.
4. Replace hardcoded paths in `build.rs` with `pkg-config`.
5. Run `cargo clippy -- -D warnings` in CI and fix the remaining lints.
6. Add a `REVIEW.md` update process or link this file from `CONTRIBUTING.md`.

---

*Tail swish.* The bindings are already in much better shape than most C-to-Rust ports. A few targeted cleanups will make this production-ready.