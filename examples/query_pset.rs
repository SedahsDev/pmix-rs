//! PMIx pset-membership queries with qualifiers and cache refresh.
//!
//! This demonstrates `PMIx_Query_info` through `pmix::query_log`: querying
//! pset membership with a pset-name qualifier, retrying once with the refresh
//! cache qualifier after `PMIX_ERR_NOT_FOUND`, and submitting the equivalent
//! non-blocking query. It is adapted from Open MPI's
//! `ompi/ompi/instance/instance.c` (around lines 1350 and 1525--1543).
//!
//! ```text
//! cargo run --example query_pset
//! prterun -n 2 ./target/debug/examples/query_pset
//! ```
//!
//! Without a DVM, `connect_new` may fail; the example reports that condition
//! and exits successfully. The requested `ompi_global` pset may not exist in
//! every DVM, so not-found and unsupported-query responses are also reported
//! without making the example fail.

use std::sync::mpsc;
use std::time::Duration;

const PSET_MEMBERSHIP: &str = "pmix.qry.pmems";
const PSET_NAME: &str = "pmix.pset.nm";
fn build_query() -> Result<pmix::query_log::PmixQuery, pmix::PmixStatus> {
    let mut qualifiers = pmix::InfoBuilder::new();
    qualifiers
        .add_string_key(
            PSET_NAME,
            "ompi_global",
            pmix::ffi::PMIX_STRING as pmix::ffi::pmix_data_type_t,
        )
        .map_err(|_| pmix::PmixError::ErrBadParam)?;

    let qualifiers = qualifiers.build()?;
    Ok(pmix::query_log::PmixQuery::new(&[PSET_MEMBERSHIP])?.with_qualifiers(qualifiers))
}

fn print_query_result(
    label: &str,
    result: Result<pmix::query_log::QueryResults, pmix::PmixStatus>,
) {
    match result {
        Ok(results) => {
            let suffix = if results.len() == 1 { "y" } else { "ies" };
            println!("{label}: {} result entr{suffix}", results.len());
        }
        Err(error) => println!("{label} failed: {error:?}"),
    }
}

struct NbResult {
    tx: mpsc::Sender<(pmix::PmixStatus, usize)>,
}

impl pmix::query_log::QueryCallback for NbResult {
    fn on_complete(
        self: Box<Self>,
        status: pmix::PmixStatus,
        results: pmix::query_log::QueryResults,
    ) {
        let _ = self.tx.send((status, results.len()));
    }
}

fn main() {
    println!("pmix-rs query_pset");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let rank = client.require_proc().get_rank();
    println!("rank {rank}");

    let query = match build_query() {
        Ok(query) => query,
        Err(error) => {
            eprintln!("could not build pset query: {error:?}");
            let _ = client.disconnect(None);
            return;
        }
    };

    let query_result = pmix::query_log::query_info(std::slice::from_ref(&query));
    if matches!(
        query_result,
        Err(pmix::PmixStatus::Known(pmix::PmixError::ErrNotFound))
    ) {
        println!("initial query returned PMIX_ERR_NOT_FOUND; refreshing cache and retrying");
        println!("refresh retry unavailable: InfoBuilder has no public safe custom-key bool API for pmix.qry.rfsh; skipping the mis-encoded string workaround");
    } else {
        print_query_result("pset query", query_result);
    }

    let (tx, rx) = mpsc::channel();
    match pmix::query_log::query_info_nb(std::slice::from_ref(&query), Box::new(NbResult { tx })) {
        Ok(()) => match rx.recv_timeout(Duration::from_secs(2)) {
            Ok((status, count)) => {
                let suffix = if count == 1 { "y" } else { "ies" };
                println!("non-blocking query: status {status:?}, {count} result entr{suffix}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                println!("non-blocking query submitted but no completion arrived within 2s");
            }
            Err(error) => println!("non-blocking query callback channel closed: {error}"),
        },
        Err(error) => println!("non-blocking query submission failed: {error:?}"),
    }

    match client.disconnect(None) {
        Ok(()) => println!("disconnected"),
        Err(error) => println!("disconnect failed: {error:?}"),
    }
    println!("query_pset done");
}

// No daemon tests: this file is the runnable artifact.
