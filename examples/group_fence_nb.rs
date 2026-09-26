//! PMIx group-fence and nodemap example adapted from MPICH's
//! `src/util/mpir_pmix.inc`.
//!
//! It demonstrates a restricted fence over an explicit process group, a
//! non-blocking `fence_nb` completion callback with an application-thread
//! progress loop, the wildcard whole-namespace blocking fence, and
//! `resolve_peers`/`resolve_nodes` nodemap queries.
//!
//! ```text
//! cargo run --example group_fence_nb
//! prterun -n 2 ./target/debug/examples/group_fence_nb
//! ```
//!
//! Running without a DVM is supported: connection and operation failures are
//! reported and the example exits successfully.

use std::sync::mpsc;
use std::time::{Duration, Instant};

const JOB_SIZE: &str = "pmix.job.size";
const JOB_NSPACE: &str = "pmix.job.nspace";
const LOCAL_PEERS: &str = "pmix.lpeers";

struct FenceWaiter {
    tx: mpsc::Sender<pmix::PmixStatus>,
}

impl pmix::data_ops::FenceCallback for FenceWaiter {
    fn on_complete(self: Box<Self>, status: pmix::PmixStatus) {
        // This callback runs on PMIx's progress thread. Only send the already
        // owned status; the application thread performs the wait and printing.
        let _ = self.tx.send(status);
    }
}

fn main() {
    println!("pmix-rs group_fence_nb");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let own_proc = client.require_proc();
    println!("rank {}", own_proc.get_rank());
    let wildcard = match client.proc_with_nspace(pmix::RANK_WILDCARD) {
        Ok(proc) => proc,
        Err(error) => {
            eprintln!("could not create wildcard process: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    match pmix::data_ops::get(&wildcard, JOB_SIZE, None) {
        Ok(value) => println!("job size: {}", value.uint64()),
        Err(error) => eprintln!("{JOB_SIZE} get failed: {error:?}"),
    }

    let local_ranks = match pmix::data_ops::get(&wildcard, LOCAL_PEERS, None) {
        Ok(value) => match value.string_copy() {
            Ok(peers) => {
                let ranks: Vec<u32> = peers
                    .split(',')
                    .filter_map(|peer| peer.trim().parse().ok())
                    .collect();
                println!("local peers: {ranks:?}");
                ranks
            }
            Err(error) => {
                eprintln!("{LOCAL_PEERS} is not valid UTF-8: {error}");
                Vec::new()
            }
        },
        Err(error) => {
            eprintln!("{LOCAL_PEERS} get failed: {error:?}");
            Vec::new()
        }
    };

    let mut local_procs = Vec::new();
    for rank in local_ranks {
        match client.proc_with_nspace(rank) {
            Ok(proc) => local_procs.push(proc),
            Err(error) => eprintln!("could not create local peer {rank}: {error:?}"),
        }
    }
    if local_procs.is_empty() {
        local_procs.push(own_proc.clone());
        println!("using own process as the local fence group");
    }
    println!("restricted fence group size: {}", local_procs.len());

    let mut info_builder = pmix::InfoBuilder::new();
    info_builder.collect_data();
    let collect_data = match info_builder.build() {
        Ok(info) => info,
        Err(error) => {
            eprintln!("could not build collect-data info: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    match pmix::data_ops::fence_nb(
        &local_procs,
        Some(&collect_data),
        Box::new(FenceWaiter { tx }),
    ) {
        Ok(()) => {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(status) => {
                        println!("restricted fence_nb completed: {status}");
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {
                        pmix::progress();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        println!("restricted fence_nb timed out after 5s");
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        println!("restricted fence_nb callback channel disconnected");
                        break;
                    }
                }
            }
        }
        Err(error) => println!("restricted fence_nb rejected: {error:?}"),
    }

    match pmix::fence(&wildcard, Some(collect_data)) {
        Ok(()) => println!("wildcard whole-nspace fence completed"),
        Err(error) => println!("wildcard whole-nspace fence failed: {error:?}"),
    }

    let nspace = match pmix::data_ops::get(&own_proc, JOB_NSPACE, None) {
        Ok(value) => match value.string_copy() {
            Ok(nspace) => Some(nspace),
            Err(error) => {
                eprintln!("{JOB_NSPACE} is not valid UTF-8: {error}");
                None
            }
        },
        Err(error) => {
            eprintln!("{JOB_NSPACE} get failed: {error:?}");
            None
        }
    };
    if let Some(nspace) = nspace.as_deref() {
        match pmix::process_mgmt::resolve_peers(None, Some(nspace)) {
            Ok(peers) => {
                println!("resolved peers in {nspace}: {}", peers.len());
                for peer in peers {
                    println!("  peer {nspace}/{}", peer.get_rank());
                }
            }
            Err(error) => println!("resolve_peers failed (DVM may not support it): {error:?}"),
        }
        match pmix::process_mgmt::resolve_nodes(nspace) {
            Ok(nodes) => println!("resolved nodes: {nodes}"),
            Err(error) => println!("resolve_nodes failed (DVM may not support it): {error:?}"),
        }
    } else {
        match pmix::process_mgmt::resolve_peers(None, None) {
            Ok(peers) => {
                println!("resolved peers (namespace omitted): {}", peers.len());
                for peer in peers {
                    println!("  peer rank {}", peer.get_rank());
                }
            }
            Err(error) => println!("resolve_peers failed (DVM may not support it): {error:?}"),
        }
        println!("resolve_nodes skipped: namespace discovery was unavailable");
    }

    let _ = client.disconnect(None);
    println!("group_fence_nb done");
}

// The PMIx C API and comparison implementation are documented in MPICH's
// `src/util/mpir_pmix.inc`; this example intentionally uses only safe wrappers.
