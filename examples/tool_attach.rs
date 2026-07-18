//! Minimal PMIx tool example: tool_init → optional attach → query path → finalize.
//!
//! ```text
//! cargo run --example tool_attach
//! ```
//!
//! Connecting to a live server usually needs `PMIX_SERVER_URI*` / a URI file.
//! This example still compiles as a smoke check and exits cleanly when no
//! server is available.

use pmix::info::empty;
use pmix::tool::{
    tool_attach_to_server, tool_finalize, tool_init, tool_is_connected, PmixToolHandle,
};

fn main() {
    println!("pmix-rs tool_attach");

    let info = empty();
    let handle: PmixToolHandle = match tool_init(None, &info) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("tool_init failed (no server/URI?): {e:?}");
            return;
        }
    };

    println!(
        "tool_init ok; connected={}",
        tool_is_connected()
    );

    // Attempt attach when a server identity is desired. Without a DVM this
    // typically returns an error — still useful as an API walkthrough.
    let attach_info = empty();
    match tool_attach_to_server(Some(handle.proc()), true, &attach_info) {
        Ok((_tool, server)) => {
            println!("tool_attach_to_server ok; server={:?}", server.as_ref().map(|s| s.proc().get_rank()));
        }
        Err(e) => {
            println!("tool_attach_to_server: {e:?} (expected without daemon)");
        }
    }

    match tool_finalize(handle) {
        Ok(()) => println!("tool_finalize ok"),
        Err(e) => eprintln!("tool_finalize: {e:?}"),
    }

    println!("tool_attach done");
}
