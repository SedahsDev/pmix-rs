//! Minimal PMIx server example: server_init → (idle) → server_finalize.
//!
//! ```text
//! cargo run --example server_minimal
//! ```
//!
//! Uses a default [`PmixServerModule`] (all callbacks `None`). Real RMs set
//! the callbacks they implement before calling `server_init`.

use pmix::InfoBuilder;
use pmix::server::{PmixServerModule, server_finalize, server_init_minimal};

fn main() {
    println!("pmix-rs server_minimal");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();

    let handle = match server_init_minimal(Some(&module)) {
        Ok(h) => h,
        Err(e) => {
            // Fall back to full server_init with empty info for environments
            // where the minimal path differs.
            match pmix::server::server_init(Some(&module), &info) {
                Ok(h) => h,
                Err(e2) => {
                    eprintln!("server_init failed: {e:?} / {e2:?}");
                    return;
                }
            }
        }
    };

    println!("server initialized (no clients attached in this smoke example)");
    // Real servers block here serving clients.

    match server_finalize(handle) {
        Ok(()) => println!("server_finalize ok"),
        Err(e) => eprintln!("server_finalize: {e:?}"),
    }

    println!("server_minimal done");
}
