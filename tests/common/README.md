# Integration test common helpers

Shared harness for `tests/*.rs` integration crates.

## Include in a test file

```rust
#[path = "common/mod.rs"]
mod common;

use common::{require_server, skip_without_server, with_server, finalize_server};
```

## API

| Helper | Behavior |
|--------|----------|
| `require_server()` | `server_init_minimal` or panic — for `#[ignore]` daemon tests |
| `try_server()` | `Option<PmixServerHandle>` |
| `skip_without_server()` | `true` if init ok (continue), `false` if skip |
| `with_server(name, f)` | run `f` only when init succeeds |
| `finalize_server(h)` | best-effort finalize |

Daemon/tool helpers remain in `tests/daemon_helper.rs` (PRTE URI / tool handle).
