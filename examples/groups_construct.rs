//! Demonstrate PMIx group construction/destruction and returned membership info.
//!
//! This follows the group-based communicator setup pattern described in MPICH's
//! `src/util/mpir_pmix.inc`: construct a group from process members, inspect the
//! returned group membership information, then destruct the group.
//!
//! ```text
//! cargo run --example groups_construct
//! prterun -n 2 ./target/debug/examples/groups_construct
//! ```
//!
//! A bare run has no DVM and exits successfully after reporting the connection
//! failure. Group support is DVM-dependent, so NOT_SUPPORTED and UNREACH are
//! reported as graceful outcomes rather than process failures.

fn main() {
    println!("pmix-rs groups_construct");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let rank = client.require_proc().get_rank();
    println!("rank {rank}");
    let own_proc = match client.proc_with_nspace(rank) {
        Ok(proc) => proc,
        Err(error) => {
            eprintln!("could not create own process handle: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    if rank == 0 {
        let group_procs = [own_proc];
        match pmix::groups::group_construct("example_grp", &group_procs, &[]) {
            Ok(results) => {
                println!(
                    "group_construct succeeded: {} result entries",
                    results.len()
                );
                for (index, info) in results.iter().enumerate() {
                    println!("result {index}: {} entries", info.len());
                }
                // PMIx owns the returned result array as one allocation. The
                // current Vec<Info> mapping exposes element pointers, so do not
                // drop those aliases individually in this demonstration.
                std::mem::forget(results);
                match pmix::groups::group_destruct("example_grp", &[]) {
                    Ok(()) => println!("group_destruct succeeded"),
                    Err(error) => println!("group_destruct failed: {error:?}; continuing"),
                }
            }
            Err(pmix::PmixStatus::Known(pmix::PmixError::ErrNotSupported)) => {
                println!("group_construct: PMIX_ERR_NOT_SUPPORTED (group support is unavailable)");
            }
            Err(pmix::PmixStatus::Known(pmix::PmixError::ErrUnreach)) => {
                println!("group_construct: PMIX_ERR_UNREACH (group DVM is unavailable)");
            }
            Err(error) => println!("group_construct failed: {error:?}; continuing"),
        }
    }

    match client.disconnect(None) {
        Ok(()) => println!("disconnected"),
        Err(error) => println!("disconnect failed: {error:?}"),
    }
    println!("groups_construct done");
}

// No daemon tests: this file is the runnable artifact.
