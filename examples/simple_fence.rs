//! Very simple fence example.
//!
//! ```text
//! cargo run --example simple_fence
//! ```

fn main() {
    println!("PMIx simple fence example");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("connect_new failed (need prterun/DVM?): {e:?}");
            return;
        }
    };
    let proc = client.require_proc();

    match pmix::fence(&proc, None) {
        Ok(()) => println!("fence succeeded"),
        Err(e) => println!("fence status: {e:?}"),
    }

    let info = pmix::info::empty();
    let _ = pmix::fence(&proc, Some(info));

    let _ = client.disconnect(None);
    println!("Simple fence example done");
}
