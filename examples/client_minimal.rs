//! Minimal PMIx client: connect → put → commit → fence → get → disconnect.
//!
//! ```text
//! cargo run --example client_minimal
//! prterun -n 1 target/debug/examples/client_minimal
//! ```
//!
//! Without a DVM, `connect_new` may fail — that is expected in bare `cargo run`.

use std::ffi::CString;

fn main() {
    println!("pmix-rs client_minimal");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {e:?}");
            return;
        }
    };
    let proc = client.require_proc();

    let key = CString::new("client_minimal_key").expect("key");
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello from client_minimal")
        .expect("string")
        .build()
        .expect("build");

    if let Err(e) = pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value) {
        eprintln!("put_value failed: {e:?}");
        let _ = client.disconnect(None);
        return;
    }
    if let Err(e) = pmix::commit() {
        eprintln!("commit failed: {e:?}");
        let _ = client.disconnect(None);
        return;
    }
    if let Err(e) = pmix::fence(&proc, None) {
        eprintln!("fence failed: {e:?}");
        let _ = client.disconnect(None);
        return;
    }

    match pmix::get_value(&proc, b"client_minimal_key\0", None) {
        Ok(_) => println!("get_value ok"),
        Err(e) => println!("get_value: {e:?} (ok without full DVM in some envs)"),
    }

    let _ = client.disconnect(None);
    println!("client_minimal done");
}
