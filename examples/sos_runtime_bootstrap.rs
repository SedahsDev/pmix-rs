//! Minimal PMIx bootstrap used by Sandia OpenSHMEM's symmetric-heap exchange.
//!
//! This demonstrates the connect, wildcard discovery, per-rank byte-object
//! publish, commit, collect-data fence, peer exchange, and disconnect sequence
//! from SOS `src/runtime-pmix.c`.
//!
//! ```text
//! cargo run --example sos_runtime_bootstrap
//! prterun -n 2 ./target/debug/examples/sos_runtime_bootstrap
//! ```
//!
//! Without a DVM, `connect_new` may fail; the example prints a message and
//! exits successfully in that case.

use std::ffi::CString;

const LOCAL_SIZE: &[u8] = b"PMIX_LOCAL_SIZE\0";
const LOCAL_PEERS: &[u8] = b"PMIX_LOCAL_PEERS\0";
const UNIV_SIZE: &[u8] = b"PMIX_UNIV_SIZE\0";

fn main() {
    println!("pmix-rs sos_runtime_bootstrap");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let rank = client.require_proc().get_rank();
    println!("rank {rank}");
    let wildcard = match client.proc_with_nspace(pmix::RANK_WILDCARD) {
        Ok(proc) => proc,
        Err(error) => {
            eprintln!("could not create wildcard process: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    let job_size = match pmix::get_value(&wildcard, pmix::JOB_SIZE, None) {
        Ok(value) => {
            let size = value.uint64();
            println!("job size: {size}");
            size as u32
        }
        Err(error) => {
            eprintln!("PMIX_JOB_SIZE get failed: {error:?}; using rank-local exchange");
            1
        }
    };

    match pmix::get_value(&wildcard, LOCAL_SIZE, None) {
        Ok(value) => println!("local size: {}", value.size()),
        Err(error) => eprintln!("PMIX_LOCAL_SIZE get failed: {error:?}"),
    }

    match pmix::get_value(&wildcard, LOCAL_PEERS, None) {
        Ok(value) => match value.string_copy() {
            Ok(peers) => {
                let ranks: Vec<u32> = peers
                    .split(',')
                    .filter_map(|peer| peer.trim().parse().ok())
                    .collect();
                println!("local peers: {ranks:?}");
            }
            Err(error) => eprintln!("PMIX_LOCAL_PEERS is not valid UTF-8: {error}"),
        },
        Err(error) => eprintln!("PMIX_LOCAL_PEERS get failed: {error:?}"),
    }

    match pmix::get_value(&wildcard, UNIV_SIZE, None) {
        Ok(value) => println!("universe size: {}", value.size()),
        Err(error) => eprintln!("PMIX_UNIV_SIZE get failed: {error:?}"),
    }

    let key = format!("sos:{rank}");
    let key_c = match CString::new(key.as_str()) {
        Ok(key) => key,
        Err(error) => {
            eprintln!("could not create publish key: {error}");
            let _ = client.disconnect(None);
            return;
        }
    };
    let payload = format!("hello-from-{rank}").into_bytes();
    let builder = match pmix::PmixValueBuilder::new().byte_object(&payload) {
        Ok(builder) => builder,
        Err(error) => {
            eprintln!("could not build rank payload: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };
    let mut value = match builder.build() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("could not finalize rank payload: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    if let Err(error) = pmix::put_value(pmix::PmixScope::Global.to_raw(), &key_c, &mut value) {
        eprintln!("put_value failed: {error:?}");
        let _ = client.disconnect(None);
        return;
    }
    if let Err(error) = pmix::commit() {
        eprintln!("commit failed: {error:?}");
        let _ = client.disconnect(None);
        return;
    }

    let mut info_builder = pmix::InfoBuilder::new();
    info_builder.collect_data();
    let info = match info_builder.build() {
        Ok(info) => info,
        Err(error) => {
            eprintln!("could not build collect-data info: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };
    if let Err(error) = pmix::fence(&wildcard, Some(info)) {
        eprintln!("fence failed: {error:?}");
        let _ = client.disconnect(None);
        return;
    }

    for peer_rank in 0..job_size {
        let peer = match client.proc_with_nspace(peer_rank) {
            Ok(peer) => peer,
            Err(error) => {
                eprintln!("could not create process for rank {peer_rank}: {error:?}");
                continue;
            }
        };
        let peer_key = format!("sos:{peer_rank}");
        let peer_key = match CString::new(peer_key) {
            Ok(key) => key,
            Err(error) => {
                eprintln!("could not create key for rank {peer_rank}: {error}");
                continue;
            }
        };
        match pmix::get_value(&peer, peer_key.as_bytes_with_nul(), None) {
            Ok(peer_value) => println!("rank {peer_rank} payload: {:?}", peer_value.bytes_copy()),
            Err(error) => eprintln!("get for rank {peer_rank} failed: {error:?}"),
        }
    }

    let _ = client.disconnect(None);
    println!("sos_runtime_bootstrap done");
}
