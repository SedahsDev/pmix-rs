//! Very simple fence example.
//
//! cargo run --example simple_fence

fn main() {
    println!("PMIx simple fence example");

    let ctx = pmix::init(None).expect("init failed");

    // Just fence
    let result = pmix::fence(ctx.get_proc(), None);
    match result {
        Ok(()) => println!("fence succeeded"),
        Err(e) => println!("fence status: {:?}", e),
    }

    // Optional: fence with empty info
    let info = pmix::info::empty();
    let _ = pmix::fence(ctx.get_proc(), Some(info));

    println!("Simple fence example done");
}
