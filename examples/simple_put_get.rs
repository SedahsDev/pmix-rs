//! Very simple example using put / get / commit / fence.
//!
//! ```text
//! cargo run --example simple_put_get
//! prterun -np 1 target/debug/examples/simple_put_get
//! ```

use std::ffi::CString;

fn main() {
    println!("PMIx Rust simple put/get/commit/fence example");

    let client = pmix::PmixClient::connect_new(None).expect("PmixClient::connect_new failed");
    let proc = client.require_proc();

    let key = CString::new("simple_example_key").unwrap();
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello from the pmix rust example")
        .expect("string value")
        .build()
        .expect("build value");

    pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value).expect("put_value failed");
    pmix::commit().expect("commit failed");
    pmix::fence(&proc, None).expect("fence failed");

    match pmix::get_value(&proc, b"simple_example_key\0", None) {
        Ok(_val) => println!("Got value successfully via get_value"),
        Err(e) => {
            println!("get_value status (expected without full DVM in some envs): {e:?}");
        }
    }

    client.disconnect(None).expect("disconnect failed");
    println!("Simple put/get/commit/fence example finished");
}
