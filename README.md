# pmix

Low-level Rust bindings to the [PMIx](https://pmix.github.io/) (Process Management Interface for Exascale) library.

These bindings provide safe(ish) wrappers over the full PMIx 5.x C API using `bindgen`, plus a comprehensive set of Rust-friendly helpers for client, server, and tool usage.

**Status:** Under active development. Functional for basic and advanced use cases when paired with a PMIx-enabled runtime (e.g. PRRTE / Open MPI).

## Features

- Full generated bindings to PMIx client, server, and tool APIs
- Rich error handling via `PmixError` / `PmixStatus`
- Modular safe wrappers (`data_ops`, `events`, `fabric`, `groups`, `process_mgmt`, `server`, `tool`, etc.)
- `InfoBuilder` / `PmixValueBuilder` for ergonomic construction
- Extensive tests (including proptest for serialization and many daemon integration tests)

## Build

Requires:

- A PMIx installation (headers + library)
- `libclang` (for bindgen)

```bash
cargo build
```

The build script tries to auto-generate bindings via `bindgen`. It falls back to a pre-generated `src/bindings.rs` if bindgen fails.

**Note:** By default the build is tuned for a local PRRTE development tree. See `build.rs` and set environment variables or edit paths for your installation.

## Basic Usage (put / get / commit / fence)

```rust
use std::ffi::CString;

fn main() {
    let ctx = pmix::init(None).expect("init failed");

    let key = CString::new("my_key").unwrap();
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello world")
        .unwrap()
        .build()
        .unwrap();

    pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value)
        .expect("put failed");

    pmix::commit().expect("commit failed");
    pmix::fence(ctx.get_proc(), None).expect("fence failed");

    match pmix::get_value(ctx.get_proc(), b"my_key\0", None) {
        Ok(v) => println!("got value"),
        Err(e) => println!("get returned: {:?}", e),
    }

    // Context drops and calls finalize
}
```

See the `examples/` directory for complete runnable versions.

## Examples

Very simple examples using only the core `put` / `get` / `commit` / `fence` APIs:

```bash
cargo run --example simple_put_get
cargo run --example simple_fence
```

These examples are intentionally minimal and avoid advanced features (server, events, fabric, non-blocking callbacks, etc.).

Most examples are marked `#[ignore]` in the test suite because they require a running PMIx daemon (launched via `prterun` or the `prte` service).

## Running with a Daemon

Many tests and examples require a live PMIx server:

```bash
# Start the PRRTE user service (example)
systemctl --user start prte

# Or use prterun directly for tests
prterun -np 2 ./target/debug/examples/simple_put_get
```

A helper script is provided:

```bash
./scripts/run_daemon_tests.sh
```

## API Overview (Simple Path)

- `pmix::init(...)` → `Context`
- `pmix::put_value(scope, key: &CStr, value)`
- `pmix::commit()`
- `pmix::fence(proc, info)`
- `pmix::get_value(proc, key, info)`
- `pmix::finalize(...)`

Higher-level modules exist for events, groups, fabric, server modules, tools, monitoring, etc.

## The `info` Module

`pub mod info;` provides helpers for `Info` construction and will grow to cover the full `PMIx_Info_*` family (publish, lookup, etc.).

Currently includes:
- `info::empty()`
- `info::with_collect_data()`

## Documentation

- See `REVIEW.md` for a detailed community-readiness review
- Module-level docs in the source
- PMIx specification: https://pmix.github.io/

## License

BSD (same spirit as PMIx itself).

## Contributing

Contributions welcome! Focus areas right now:
- More examples
- Portable build (pkg-config)
- Filling out remaining high-level wrappers
- Better documentation

Run `cargo test --lib` for quick checks. Daemon tests require a live PRRTE instance.

---

*This crate is intended for low-level use by other Rust HPC projects (e.g. OSU Micro-Benchmarks port, custom launchers, etc.).*
