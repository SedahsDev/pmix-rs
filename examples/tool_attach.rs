//! Minimal PMIx tool example: connect → optional attach → disconnect.
//!
//! ```text
//! cargo run --example tool_attach
//! ```
//!
//! Connecting to a live server usually needs `PMIX_SERVER_URI*` / a URI file.
//! This example still compiles as a smoke check and exits cleanly when no
//! server is available.

use pmix::info::empty;
use pmix::tool::{tool_attach_to_server, tool_is_connected, PmixTool};

fn main() {
    println!("pmix-rs tool_attach");

    let info = empty();
    let tool = match PmixTool::connect_new(None, &info) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("PmixTool::connect_new failed (no server?): {e:?}");
            return;
        }
    };

    println!(
        "tool live={} connected={}",
        tool.is_live(),
        tool_is_connected()
    );
    if let Some(proc) = tool.proc() {
        println!("tool nspace={:?} rank={}", proc.nspace(), proc.rank());
    }

    let attach_info = empty();
    match tool_attach_to_server(None, true, &attach_info) {
        Ok((maybe_tool, maybe_server)) => {
            println!(
                "attach ok tool_id={} server_id={}",
                maybe_tool.is_some(),
                maybe_server.is_some()
            );
        }
        Err(e) => println!("attach: {e:?} (ok without a server)"),
    }

    let _ = tool.disconnect();
    println!("tool_attach done");
}
