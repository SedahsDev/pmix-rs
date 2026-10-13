//! Demonstrates PMIx dynamic process management with `PMIx_Spawn`.
//!
//! The parent builds a [`pmix::process_mgmt::PmixApp`], spawns a child job,
//! and queries the child job size. Each child discovers its parent through
//! `pmix.parent` (`PMIX_PARENT_ID`) and reports `pmix.appnum` (`PMIX_APPNUM`).
//!
//! ```text
//! cargo run --example spawn_child_job
//! prterun -n 1 ./target/debug/examples/spawn_child_job
//! ```
//!
//! A bare run, or a run under a DVM without spawn support, reports the PMIx
//! error and exits successfully. The call shape follows MPICH's
//! `src/util/mpir_pmix.inc` and Open MPI's `ompi/ompi/dpm/dpm.c`.

fn main() {
    if std::env::args().any(|arg| arg == "--child") {
        run_child();
    } else {
        run_parent();
    }
}

fn run_child() {
    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("child connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };
    let proc = client.require_proc();

    let parent = match pmix::data_ops::get(&proc, "pmix.parent", None) {
        Ok(value) => {
            // SAFETY: the value is a PMIX_PROC (type_tag() == PMIX_PROC); its
            // union `proc_` arm points to a pmix_proc_t { nspace: [c_char; 256],
            // rank: u32 } allocated by the PMIx library and owned by `value`.
            // We only read it while `value` is alive (this match arm), and
            // PmixOwnedValue's Drop frees it via free_value.
            let raw = value.as_raw();
            let proc = unsafe { &*(*raw).data.proc_ };
            let nspace = unsafe { std::ffi::CStr::from_ptr(proc.nspace.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            format!("{nspace}:{}", proc.rank)
        }
        Err(error) => {
            eprintln!("child PMIX_PARENT_ID get failed: {error:?}");
            "<unavailable>".to_string()
        }
    };
    let appnum = match pmix::data_ops::get(&proc, "pmix.appnum", None) {
        Ok(value) => value.uint32().to_string(),
        Err(error) => {
            eprintln!("child PMIX_APPNUM get failed: {error:?}");
            "<unavailable>".to_string()
        }
    };
    let spawn_env = std::env::var("SPAWN_CHILD").unwrap_or_else(|_| "<unset>".to_string());
    println!(
        "child rank {} connected; parent={parent} appnum={appnum} SPAWN_CHILD={spawn_env}",
        proc.get_rank()
    );

    if let Err(error) = client.disconnect(None) {
        eprintln!("child disconnect failed: {error:?}");
    }
}

fn run_parent() {
    println!("pmix-rs: spawn_child_job parent");
    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("parent connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let executable = match std::env::current_exe() {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(error) => {
            eprintln!("could not determine current executable: {error}");
            let _ = client.disconnect(None);
            return;
        }
    };
    let mut builder = pmix::process_mgmt::PmixApp::builder();
    let app = match builder
        .cmd(&executable)
        .arg("--child")
        .env("SPAWN_CHILD=1")
        .maxprocs(2)
        .build()
    {
        Ok(app) => app,
        Err(error) => {
            eprintln!("could not build child application: {error}");
            let _ = client.disconnect(None);
            return;
        }
    };

    let child_nspace = match pmix::process_mgmt::spawn(&[], &[app]) {
        Ok(nspace) => nspace,
        Err(error) => {
            eprintln!("spawn not supported by this DVM / no DVM? ({error:?})");
            let _ = client.disconnect(None);
            return;
        }
    };
    println!("spawned child nspace: {child_nspace}");

    let child_wildcard = match pmix::Proc::new(&child_nspace, pmix::RANK_WILDCARD) {
        Ok(proc) => proc,
        Err(error) => {
            eprintln!("could not create child wildcard process: {error}");
            let _ = client.disconnect(None);
            return;
        }
    };
    match pmix::data_ops::get(&child_wildcard, "pmix.job.size", None) {
        Ok(value) => println!("child job size: {}", value.uint64()),
        Err(error) => eprintln!("child PMIX_JOB_SIZE get failed: {error:?}"),
    }

    if let Err(error) = client.disconnect(None) {
        eprintln!("parent disconnect failed: {error:?}");
    }
}

// PMIX_PROC values are read through `as_raw()`; `bytes_copy` reads the wrong
// union arm for a PMIX_PROC value.
