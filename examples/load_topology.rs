//! Load and report the hardware topology provided by a PMIx server.
//!
//! This demonstrates the `PMIx_Load_topology` / hwloc pattern used by MPICH's
//! `src/util/mpir_pmi.c`: connect to PMIx, load the topology, print its source,
//! and disconnect. The fabric module is the natural placement because it owns
//! the safe `PmixTopology` and `load_topology` wrappers.
//!
//! ```text
//! cargo run --example load_topology
//! prterun -n 2 ./target/debug/examples/load_topology
//! ```
//!
//! Without a DVM, or when the DVM cannot provide a topology, this example
//! reports the condition and exits successfully. MPICH similarly falls back to
//! its own hwloc discovery when PMIx cannot provide a topology.

fn main() {
    println!("pmix-rs load_topology");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let rank = client.require_proc().get_rank();
    println!("rank {rank}");

    let mut topology = pmix::fabric::topology_construct();
    match pmix::fabric::load_topology(&mut topology) {
        Ok(()) => {
            println!("topology source: {:?}", topology.source());
            if topology
                .source()
                .is_some_and(|source| source.contains("hwloc"))
            {
                println!("topology was provided by hwloc");
            }
        }
        Err(error) => {
            eprintln!(
                "PMIx could not provide a topology ({error:?}); MPICH falls back to its own hwloc discovery"
            );
        }
    }

    let _ = client.disconnect(None);
    println!("load_topology done");
}
