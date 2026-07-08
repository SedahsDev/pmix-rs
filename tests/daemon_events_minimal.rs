//! Minimal test to isolate which FFI call hangs in events daemon test.

mod daemon_helper;

use pmix::events::{deregister_event_handler, notify_event, register_event_handler};
use pmix::server::{PmixServerModule, server_finalize, server_init};
use pmix::{InfoBuilder, PmixDataRange, PmixStatus, Proc};

#[test]
#[ignore = "daemon isolation"]
fn test_server_init_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");
    eprintln!("[daemon_events_minimal] server_init succeeded");
    let _ = server_finalize(handle);
    eprintln!("[daemon_events_minimal] server_finalize succeeded");
}

#[test]
#[ignore = "daemon isolation"]
fn test_register_event_handler_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");
    eprintln!("[daemon_events_minimal] server_init succeeded");

    let codes = vec![PmixStatus::Known(pmix::PmixError::Error)];
    let reg_info = InfoBuilder::new().build();
    eprintln!("[daemon_events_minimal] calling register_event_handler...");
    let reg_result = register_event_handler(&codes, &reg_info, None, None);
    eprintln!(
        "[daemon_events_minimal] register_event_handler returned: {:?}",
        reg_result
    );

    if let Ok(evhdlr_ref) = reg_result {
        let _ = deregister_event_handler(evhdlr_ref, None);
        eprintln!("[daemon_events_minimal] deregister_event_handler succeeded");
    }

    let _ = server_finalize(handle);
    eprintln!("[daemon_events_minimal] server_finalize succeeded (register test)");
}

#[test]
#[ignore = "daemon isolation"]
fn test_notify_event_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let module = PmixServerModule::default();
    let info = InfoBuilder::new().build();
    let handle = server_init(Some(&module), &info).expect("server_init");
    eprintln!("[daemon_events_minimal] server_init succeeded");

    let source = Proc::new("test-nspace", 0).expect("proc");
    let notify_info = InfoBuilder::new().build();

    eprintln!("[daemon_events_minimal] calling notify_event with Session...");
    let _ = notify_event(
        PmixStatus::Known(pmix::PmixError::Error),
        &source,
        PmixDataRange::Session,
        &notify_info,
    );
    eprintln!("[daemon_events_minimal] notify_event Session returned");

    // Try other ranges
    for range in [
        PmixDataRange::Namespace,
        PmixDataRange::Global,
        PmixDataRange::Local,
        PmixDataRange::Rm,
        PmixDataRange::Custom,
        PmixDataRange::ProcLocal,
        PmixDataRange::Undef,
    ] {
        eprintln!(
            "[daemon_events_minimal] calling notify_event with {:?}...",
            range
        );
        let _ = notify_event(
            PmixStatus::Known(pmix::PmixError::Error),
            &source,
            range,
            &notify_info,
        );
        eprintln!("[daemon_events_minimal] notify_event {:?} returned", range);
    }

    let _ = server_finalize(handle);
    eprintln!("[daemon_events_minimal] server_finalize succeeded (notify test)");
}
