//! Minimal PMIx client example: init → put → commit → fence → get → finalize.
//!
//! ```text
//! cargo run --example client_minimal
//! ```
//!
//! Usually needs a PMIx runtime (e.g. `prterun -n 1 target/debug/examples/client_minimal`).
//! Without a DVM, `init` may fail — that is expected in bare `cargo run`.

use std::ffi::CString;

fn main() {
    println!("pmix-rs client_minimal");

    let ctx = match pmix::init(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pmix::init failed (need prterun/DVM?): {e:?}");
            return;
        }
    };

    let key = CString::new("client_minimal_key").expect("key");
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello from client_minimal")
        .expect("string")
        .build()
        .expect("build");

    if let Err(e) = pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value) {
        eprintln!("put_value failed: {e:?}");
        return;
    }
    if let Err(e) = pmix::commit() {
        eprintln!("commit failed: {e:?}");
        return;
    }
    if let Err(e) = pmix::fence(ctx.get_proc(), None) {
        eprintln!("fence failed: {e:?}");
        return;
    }

    match pmix::get_value(ctx.get_proc(), b"client_minimal_key\0", None) {
        Ok(_) => println!("get_value ok"),
        Err(e) => println!("get_value: {e:?} (ok without full DVM in some envs)"),
    }

    // Context drop finalizes
    println!("client_minimal done");
}
