//! PMIx name service: publish, lookup, unpublish, and RM identity discovery.
//!
//! This demonstrates the PMIx name-service flow used by MPICH in
//! `src/util/mpir_pmix.inc` and by Open MPI in `pmix_base_fns.c`: each rank
//! publishes a name, rank 0 looks up all names and their publishing processes,
//! then unpublishes one name and verifies that it is no longer available.
//!
//! ```text
//! cargo run --example name_service
//! prterun -n 2 ./target/debug/examples/name_service
//! ```
//!
//! Without a DVM, `connect_new` may fail; the example prints a message and
//! exits successfully in that case. Publish, lookup, and unpublish failures
//! are also reported and do not turn this demonstration into a failed run.

fn print_lookup_result(pdata: &pmix::data_ops::PmixPdata) {
    match &pdata.value {
        Some(value) => match value.string_copy() {
            Ok(value) => println!(
                "lookup {}: publisher rank {}, value {value:?}",
                pdata.key,
                pdata.proc.get_rank()
            ),
            Err(error) => eprintln!(
                "lookup {}: publisher rank {}, value is not valid UTF-8: {error}",
                pdata.key,
                pdata.proc.get_rank()
            ),
        },
        None => println!("lookup {}: no value returned", pdata.key),
    }
}

fn lookup_names(job_size: u32) -> bool {
    let mut requests: Vec<_> = (0..job_size)
        .map(|rank| pmix::data_ops::PmixPdata::new(&format!("name.{rank}")))
        .collect();

    match pmix::data_ops::lookup(&mut requests, None) {
        Ok((status, results)) => {
            println!("lookup all: status {status:?}");
            for result in &results {
                print_lookup_result(result);
            }
            true
        }
        Err(error) => {
            eprintln!("lookup all failed: {error:?}");
            false
        }
    }
}

fn main() {
    println!("pmix-rs name_service");

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
            let size = value.uint64().min(1024) as u32;
            println!("job size: {size}");
            size.max(1)
        }
        Err(error) => {
            eprintln!("job size lookup failed: {error:?}; using rank-local size");
            rank.saturating_add(1).max(1)
        }
    };

    let key = format!("name.{rank}");
    let published_value = format!("rank-{rank}");
    let mut builder = pmix::InfoBuilder::new();
    if let Err(error) = builder.add_string_key(
        &key,
        &published_value,
        pmix::ffi::PMIX_STRING as pmix::ffi::pmix_data_type_t,
    ) {
        eprintln!("could not build publish entry {key}: {error}");
        let _ = client.disconnect(None);
        return;
    }
    let info = match builder.build() {
        Ok(info) => info,
        Err(error) => {
            eprintln!("could not finalize publish entry {key}: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };
    match pmix::data_ops::publish(&info) {
        Ok(()) => println!("publish {key} = {published_value:?}: success"),
        Err(error) => eprintln!("publish {key} failed: {error:?}"),
    }

    match pmix::data_ops::get(&wildcard, "pmix.rm.name", None) {
        Ok(value) => match value.string_copy() {
            Ok(name) if !name.is_empty() => println!("RM name: {name}"),
            Ok(_) => println!("RM name: unavailable (empty)"),
            Err(error) => eprintln!("RM name is not valid UTF-8: {error}"),
        },
        Err(error) => eprintln!("RM name lookup failed: {error:?}"),
    }
    match pmix::data_ops::get(&wildcard, "pmix.rm.version", None) {
        Ok(value) => match value.string_copy() {
            Ok(version) if !version.is_empty() => println!("RM version: {version}"),
            Ok(_) => println!("RM version: unavailable (empty)"),
            Err(error) => eprintln!("RM version is not valid UTF-8: {error}"),
        },
        Err(error) => eprintln!("RM version lookup failed: {error:?}"),
    }

    if let Err(error) = pmix::fence(&wildcard, None) {
        eprintln!("pre-lookup synchronization fence failed: {error:?}");
    }

    if rank == 0 {
        if !lookup_names(job_size) {
            let _ = client.disconnect(None);
            println!("name_service done");
            return;
        }

        let keys = ["name.0"];
        match pmix::data_ops::unpublish(Some(&keys), None) {
            Ok(()) => println!("unpublish name.0: success"),
            Err(error) => eprintln!("unpublish name.0 failed: {error:?}"),
        }

        let mut request = [pmix::data_ops::PmixPdata::new("name.0")];
        match pmix::data_ops::lookup(&mut request, None) {
            Ok((status, results)) => {
                println!("lookup name.0 after unpublish: status {status:?}");
                for result in &results {
                    print_lookup_result(result);
                }
            }
            Err(error) => eprintln!("lookup name.0 after unpublish failed: {error:?}"),
        }

        match pmix::data_ops::unpublish(None, None) {
            Ok(()) => println!("unpublish remaining names: success"),
            Err(error) => eprintln!("unpublish remaining names failed: {error:?}"),
        }
    }

    let _ = client.disconnect(None);
    println!("name_service done");
}
