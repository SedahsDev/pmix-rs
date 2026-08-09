//! OpenSSS-UCX-style PMIx peer exchange with per-rank dynamic keys.
//!
//! This demonstrates wildcard discovery, four per-rank keys with multiple
//! value types, and the commit -> collect-data fence -> exchange sequence used
//! to distribute worker blobs, packed rkeys, and heap metadata. The key format
//! is simplified to one value per rank (`w:{rank}`, `r:{rank}`, `b:{rank}`, and
//! `s:{rank}`) because this example has no real UCX memory region to describe.
//!
//! ```text
//! cargo run --example osss_ucx_peer_exchange
//! prterun -n 2 ./target/debug/examples/osss_ucx_peer_exchange
//! ```
//!
//! Without a DVM, `connect_new` may fail; the example prints a message and
//! exits successfully in that case. The exchange pattern is adapted from
//! OpenSSS-UCX `src/shmemc/ucx/pmix_client.c`.

use std::ffi::CString;

const LOCAL_PEERS: &str = "pmix.lpeers";
const LOCAL_SIZE: &str = "pmix.local.size";
const UNIV_SIZE: &str = "pmix.univ.size";

fn main() {
    println!("pmix-rs osss_ucx_peer_exchange");

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

    let job_size = match pmix::data_ops::get(&wildcard, "pmix.job.size", None) {
        Ok(value) => {
            let size = value.uint64();
            println!("job size: {size}");
            size.min(u32::MAX as u64) as u32
        }
        Err(error) => {
            eprintln!("PMIX_JOB_SIZE get failed: {error:?}; using rank-local exchange");
            1
        }
    };

    match pmix::data_ops::get(&wildcard, UNIV_SIZE, None) {
        Ok(value) => println!("universe size: {}", value.size()),
        Err(error) => eprintln!("PMIX_UNIV_SIZE get failed: {error:?}"),
    }
    match pmix::data_ops::get(&wildcard, LOCAL_SIZE, None) {
        Ok(value) => println!("local size: {}", value.size()),
        Err(error) => eprintln!("PMIX_LOCAL_SIZE get failed: {error:?}"),
    }
    match pmix::data_ops::get(&wildcard, LOCAL_PEERS, None) {
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

    let values = [
        (
            format!("w:{rank}"),
            pmix::PmixValueBuilder::new().byte_object(format!("worker-blob-{rank}").as_bytes()),
            "worker blob",
        ),
        (
            format!("r:{rank}"),
            pmix::PmixValueBuilder::new().byte_object(format!("rkey-{rank}").as_bytes()),
            "packed rkey",
        ),
    ];
    for (key, builder, name) in values {
        let builder = match builder {
            Ok(builder) => builder,
            Err(error) => {
                eprintln!("could not build {name}: {error:?}");
                let _ = client.disconnect(None);
                return;
            }
        };
        let mut value = match builder.build() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("could not finalize {name}: {error:?}");
                let _ = client.disconnect(None);
                return;
            }
        };
        let key_c = match CString::new(key) {
            Ok(key) => key,
            Err(error) => {
                eprintln!("could not create {name} key: {error}");
                let _ = client.disconnect(None);
                return;
            }
        };
        if let Err(error) = pmix::put_value(pmix::PmixScope::Global.to_raw(), &key_c, &mut value) {
            eprintln!("put_value for {name} failed: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    }

    let scalar_values = [
        (
            format!("b:{rank}"),
            pmix::PmixValueBuilder::new().uint64(0x1000 + rank as u64),
            "heap base",
        ),
        (
            format!("s:{rank}"),
            pmix::PmixValueBuilder::new().size(4096 + rank as usize),
            "heap size",
        ),
    ];
    for (key, builder, name) in scalar_values {
        let mut value = match builder.build() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("could not build {name}: {error:?}");
                let _ = client.disconnect(None);
                return;
            }
        };
        let key_c = match CString::new(key) {
            Ok(key) => key,
            Err(error) => {
                eprintln!("could not create {name} key: {error}");
                let _ = client.disconnect(None);
                return;
            }
        };
        if let Err(error) = pmix::put_value(pmix::PmixScope::Global.to_raw(), &key_c, &mut value) {
            eprintln!("put_value for {name} failed: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
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
        eprintln!("collect-data fence failed: {error:?}");
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
        let worker_key = format!("w:{peer_rank}");
        let rkey_key = format!("r:{peer_rank}");
        let base_key = format!("b:{peer_rank}");
        let size_key = format!("s:{peer_rank}");
        let expected_worker = format!("worker-blob-{peer_rank}").into_bytes();
        let expected_rkey = format!("rkey-{peer_rank}").into_bytes();

        let worker_ok = match pmix::data_ops::get(&peer, &worker_key, None) {
            Ok(value) => value.bytes_copy() == expected_worker,
            Err(error) => {
                eprintln!("get {worker_key} failed: {error:?}");
                false
            }
        };
        let rkey_ok = match pmix::data_ops::get(&peer, &rkey_key, None) {
            Ok(value) => value.bytes_copy() == expected_rkey,
            Err(error) => {
                eprintln!("get {rkey_key} failed: {error:?}");
                false
            }
        };
        let base_ok = match pmix::data_ops::get(&peer, &base_key, None) {
            Ok(value) => value.uint64() == 0x1000 + peer_rank as u64,
            Err(error) => {
                eprintln!("get {base_key} failed: {error:?}");
                false
            }
        };
        let size_ok = match pmix::data_ops::get(&peer, &size_key, None) {
            Ok(value) => value.size() == 4096 + peer_rank as usize,
            Err(error) => {
                eprintln!("get {size_key} failed: {error:?}");
                false
            }
        };
        println!(
            "peer {peer_rank}: worker_blob={worker_ok} packed_rkey={rkey_ok} heap_base={base_ok} heap_size={size_ok}"
        );
    }

    let _ = client.disconnect(None);
    println!("osss_ucx_peer_exchange done");
}
