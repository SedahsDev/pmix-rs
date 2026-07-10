# pmix

Low-level Rust bindings for [PMIx](https://pmix.github.io/) (Process Management Interface for Exascale).

Safe-ish wrappers over the PMIx 5.x C API via `bindgen`, plus modular helpers for client, server, and tool usage.

**Status:** Active development. Suitable for other Rust HPC projects (OSU micro-benchmarks port, GUPS, custom launchers).

## Features

- Full generated bindings (client / server / tool)
- `PmixError` / `PmixStatus` two-tier status model
- Modules: `data_ops`, `events`, `fabric`, `groups`, `process_mgmt`, `server`, `tool`, …
- `PmixValueBuilder` / `info` helpers
- Large test suite (unit + proptest + many `#[ignore]` daemon tests)

## Build

Requires a PMIx install (headers + `libpmix`) and optionally `libclang` for bindgen.

```bash
export PMIX_PREFIX=/path/to/pmix-or-prrte-install
export LD_LIBRARY_PATH=$PMIX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
cargo build
cargo test --lib
```

Also supported: `PMIX_INCLUDE_DIR` + `PMIX_LIB_DIR`. Fallbacks: `/usr`, `/usr/local`, `/opt/pmix`. Offline fallback: pre-generated `src/bindings.rs`.

See also: [`../BUILDING.md`](../BUILDING.md).

## Simple API: put / get / commit / fence

```rust
use std::ffi::CString;

fn main() {
    let ctx = pmix::init(None).expect("init");

    let key = CString::new("my_key").unwrap();
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello world")
        .unwrap()
        .build()
        .unwrap();

    pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value)
        .expect("put");
    pmix::commit().expect("commit");
    pmix::fence(ctx.get_proc(), None).expect("fence");

    match pmix::get_value(ctx.get_proc(), b"my_key\0", None) {
        Ok(_) => println!("got value"),
        Err(e) => println!("get: {e:?}"),
    }
    // Context drop → finalize
}
```

### Examples

```bash
cargo run --example simple_put_get
cargo run --example simple_fence
cargo run --example data_packing
```

Under a PMIx DVM:

```bash
prterun -np 2 ./target/debug/examples/simple_put_get
# or
./scripts/run_daemon_tests.sh
```

## License

BSD-style (see `LICENSE`).

## Further reading

- [`REVIEW.md`](./REVIEW.md) — community readiness review
- [PMIx specification](https://pmix.github.io/)
