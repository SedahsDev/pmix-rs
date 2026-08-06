# pmix

Low-level Rust bindings for [PMIx](https://pmix.github.io/) (Process Management Interface for Exascale).

Safe-ish wrappers over the OpenPMIx **≥ 6.1** C API via `bindgen`, plus modular helpers for client, server, and tool usage.

**Status:** Active development. Suitable for other Rust HPC projects (OSU micro-benchmarks port, GUPS, custom launchers).

## Features

- Full **build-time** generated bindings (client / server / tool) — written only to `OUT_DIR`
- `PmixError` / `PmixStatus` two-tier status model
- Modules: `data_ops`, `events`, `fabric`, `groups`, `process_mgmt`, `server`, `tool`, …
- `PmixValueBuilder` / `info` helpers
- Large test suite (unit + proptest + many `#[ignore]` daemon tests)


## Threading

OpenPMIx **≥ 6.1** threadshifts C API entry. Rust side:

- Prefer process-wide [`PmixClient`](src/lib.rs) (`Clone + Send + Sync`) — not bare `Context` for multi-thread use
- Build `Info` and other C-owned handles per call (they are `!Send`)
- Data ops remain free functions (`put_value` / `get_value` / `fence` / …)
- Callbacks run on the PMIx **progress** thread — do not block in-handler

Full model, type inventory, progress/pinning, and roadmap: **[THREADING.md](./THREADING.md)**.

## Build prerequisites

| Dependency | Why |
|---|---|
| **OpenPMIx ≥ 6.1** (`libpmix` + headers) | C library this crate binds; minimum matches the documented threading model |
| **libclang** (`libclang-dev` / `clang-devel`) | Required by `bindgen` at **build time** |
| **pkg-config** (optional) | Helps locate PMIx when `PMIX_*` env vars are unset |

There is **no** committed `src/bindings.rs`. Bindings are generated into Cargo’s `OUT_DIR` on every build and never touch the working tree.

### Debian / Ubuntu

```bash
sudo apt-get install -y libclang-dev clang pkg-config
# System libpmix is often < 6.1 — prefer building OpenPMIx yourself (see below).
```

### Fedora / RHEL

```bash
sudo dnf install -y clang-devel clang pkg-config
```

### OpenPMIx ≥ 6.1

If your distro package is older than 6.1, install from source:

```bash
git clone --depth 1 --branch v6.1.0 --recurse-submodules \
  https://github.com/openpmix/openpmix.git
cd openpmix
./autogen.pl
./configure --prefix=$HOME/.local/openpmix-6.1.0
make -j"$(nproc)" && make install
export PMIX_PREFIX=$HOME/.local/openpmix-6.1.0
export LD_LIBRARY_PATH=$PMIX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
```

## Build

```bash
export PMIX_PREFIX=/path/to/openpmix-install   # required if not on default paths
export LD_LIBRARY_PATH=$PMIX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
cargo build
cargo test --lib
```

Also supported: `PMIX_INCLUDE_DIR` + `PMIX_LIB_DIR`. Discovery order: env vars → `pkg-config pmix` → `/usr`, `/usr/local`, `/opt/pmix`, `/opt/prrte`.

`build.rs` refuses OpenPMIx older than **6.1** and fails with a clear message if libclang is missing (bindgen’s panic is caught). A successful `cargo build` leaves `git status` clean.

## Simple API: put / get / commit / fence

```rust
use std::ffi::CString;

fn main() {
    let client = pmix::PmixClient::connect_new(None).expect("connect");
    let proc = client.require_proc();

    let key = CString::new("my_key").unwrap();
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello world")
        .unwrap()
        .build()
        .unwrap();

    pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value)
        .expect("put");
    pmix::commit().expect("commit");
    pmix::fence(&proc, None).expect("fence");

    match pmix::get_value(&proc, b"my_key\0", None) {
        Ok(_) => println!("got value"),
        Err(e) => println!("get: {e:?}"),
    }

    client.disconnect(None).expect("disconnect");
}
```

### Examples

```bash
# Role-based entry points
cargo run --example client_minimal
cargo run --example server_minimal
cargo run --example tool_attach

# Additional demos
cargo run --example simple_put_get
cargo run --example simple_fence
cargo run --example data_packing
cargo run --example callback_hop          # client _nb / events hop-off (#51)
cargo run --example server_upcall_hop     # server fence/modex upcall hop (#52)
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
