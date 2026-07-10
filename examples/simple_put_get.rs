//! Very simple example using put / get / commit / fence.
//!
//! Run with: cargo run --example simple_put_get
//! (Usually requires a PMIx daemon via prterun or prte)

use std::ffi::CString;

fn main() {
    println!("PMIx Rust simple put/get/commit/fence example");

    // Init (Context finalizes on drop)
    let ctx = pmix::init(None).expect("pmix::init failed");

    // Key (null terminated for get)
    let key = CString::new("simple_example_key").unwrap();

    // Build a string value
    let mut value = pmix::PmixValueBuilder::new()
        .string("hello from the pmix rust example")
        .expect("string value")
        .build()
        .expect("build value");

    // Put the value using Global scope
    pmix::put_value(pmix::PmixScope::Global.to_raw(), &key, &mut value).expect("put_value failed");

    // Commit the puts
    pmix::commit().expect("commit failed");

    // Fence for visibility
    pmix::fence(ctx.get_proc(), None).expect("fence failed");

    // Try to get it back
    match pmix::get_value(ctx.get_proc(), b"simple_example_key\0", None) {
        Ok(_val) => println!("Got value successfully via get_value"),
        Err(e) => {
            println!(
                "get_value status (expected without full DVM in some envs): {:?}",
                e
            );
        }
    }

    println!("Simple put/get/commit/fence example finished");
}
