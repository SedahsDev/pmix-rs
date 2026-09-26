//! PMIx job-control example: pause, resume, and kill directives.
//!
//! This demonstrates `PMIx_Job_control` pause/resume/kill directives, the
//! all-process target convention, and the non-blocking job-control variant.
//! The pause and resume calls below use the Rust `allocation::job_control`
//! mapping and the lowercase PMIx directive keys. The corresponding C usage is
//! in Open MPI's `opal/mca/pmix/base/pmix_base_fns.c`.
//!
//! ```text
//! cargo run --example job_control
//! prterun -n 2 ./target/debug/examples/job_control
//! ```
//!
//! Without a DVM, `connect_new` may fail; the example reports that condition
//! and exits successfully. Some DVM configurations do not support job control
//! and return `PMIX_ERR_NOT_SUPPORTED`; that is also handled gracefully.

use std::sync::mpsc;
use std::time::Duration;

const JOB_CTRL_PAUSE: &str = "pmix.jctrl.pause";
const JOB_CTRL_RESUME: &str = "pmix.jctrl.resume";

struct JobControlCompletion {
    sender: mpsc::Sender<(pmix::PmixStatus, usize)>,
}

impl pmix::allocation::JobControlCallback for JobControlCompletion {
    fn on_complete(&self, status: pmix::PmixStatus, results: pmix::allocation::JobControlResults) {
        let _ = self.sender.send((status, results.len()));
    }
}

fn directive(key: &str) -> Result<pmix::Info, pmix::PmixStatus> {
    let mut builder = pmix::InfoBuilder::new();
    builder.add_bool_key(key, true);
    builder.build()
}

fn main() {
    println!("pmix-rs job_control");

    let client = match pmix::PmixClient::connect_new(None) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("PmixClient::connect_new failed (need prterun/DVM?): {error:?}");
            return;
        }
    };

    let rank = client.require_proc().get_rank();
    println!("rank {rank}");

    if rank == 0 {
        let pause = match directive(JOB_CTRL_PAUSE) {
            Ok(info) => info,
            Err(error) => {
                eprintln!("could not build pause directive: {error:?}");
                let _ = client.disconnect(None);
                return;
            }
        };

        match pmix::allocation::job_control(&[], std::slice::from_ref(&pause)) {
            Ok(results) => println!("pause all procs: {} result entries", results.len()),
            Err(pmix::PmixStatus::Known(pmix::PmixError::ErrNotSupported)) => {
                println!("pause all procs: PMIX_ERR_NOT_SUPPORTED (job control is unavailable)");
                let _ = client.disconnect(None);
                println!("job_control done");
                return;
            }
            Err(error) => println!("pause all procs failed: {error:?}; continuing"),
        }

        let resume = match directive(JOB_CTRL_RESUME) {
            Ok(info) => info,
            Err(error) => {
                eprintln!("could not build resume directive: {error:?}");
                let _ = client.disconnect(None);
                return;
            }
        };

        match pmix::allocation::job_control(&[], std::slice::from_ref(&resume)) {
            Ok(results) => println!("resume all procs: {} result entries", results.len()),
            Err(pmix::PmixStatus::Known(pmix::PmixError::ErrNotSupported)) => {
                println!("resume all procs: PMIX_ERR_NOT_SUPPORTED (job control is unavailable)");
            }
            Err(error) => println!("resume all procs failed: {error:?}; continuing"),
        }

        let (sender, receiver) = mpsc::channel();
        match pmix::allocation::job_control_nb(
            &[],
            std::slice::from_ref(&resume),
            Box::new(JobControlCompletion { sender }),
        ) {
            Ok(()) => match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok((status, count)) => {
                    println!("non-blocking resume: status {status:?}, {count} result entries")
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    println!("non-blocking resume submitted but no completion arrived within 2s")
                }
                Err(error) => println!("non-blocking resume callback channel closed: {error}"),
            },
            Err(pmix::PmixStatus::Known(pmix::PmixError::ErrNotSupported)) => {
                println!(
                    "non-blocking resume: PMIX_ERR_NOT_SUPPORTED (job control is unavailable)"
                );
            }
            Err(error) => println!("non-blocking resume submission failed: {error:?}"),
        }
    }

    match client.disconnect(None) {
        Ok(()) => println!("disconnected"),
        Err(error) => println!("disconnect failed: {error:?}"),
    }
    println!("job_control done");
}

// No daemon tests: this file is the runnable artifact.
