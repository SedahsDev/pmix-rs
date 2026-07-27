//! Minimal PMIx server example: connect → idle → disconnect.
//!
//! ```text
//! cargo run --example server_minimal
//! ```
//!
//! Uses a default [`PmixServerModule`] (all callbacks `None`). Real RMs set
//! the callbacks they implement before calling `PmixServer::connect_new`.

use pmix::server::{PmixServer, PmixServerModule};
use pmix::InfoBuilder;

fn main() {
    println!("pmix-rs server_minimal");

    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();

    let server = match PmixServer::connect_new(Some(&module), &info) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("PmixServer::connect_new failed: {e:?}");
            return;
        }
    };

    println!("server live={}", server.is_live());
    // Clone is cheap (Arc); Drop does not finalize.
    let _worker = server.clone();

    if let Err(e) = server.disconnect() {
        eprintln!("disconnect failed: {e:?}");
        return;
    }
    println!("server_minimal done");
}
