//! Minimal test to isolate monitoring daemon hangs.

mod daemon_helper;

use pmix::monitoring::{heartbeat, process_monitor};

#[test]
#[ignore = "daemon isolation"]
fn test_tool_handle_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let handle = daemon_helper::get_tool_handle().expect("shared tool handle");
    eprintln!("[monitoring_minimal] tool handle acquired: {:?}", handle);
}

#[test]
#[ignore = "daemon isolation"]
fn test_heartbeat_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("shared tool handle");
    eprintln!("[monitoring_minimal] calling heartbeat...");
    let result = heartbeat();
    eprintln!("[monitoring_minimal] heartbeat returned: {:?}", result);
}

#[test]
#[ignore = "daemon isolation"]
fn test_process_monitor_only() {
    let _lock = daemon_helper::daemon_lock().expect("daemon lock");
    let _handle = daemon_helper::get_tool_handle().expect("shared tool handle");
    eprintln!("[monitoring_minimal] calling process_monitor...");
    let monitor_info = pmix::InfoBuilder::new().build();
    let result = process_monitor(
        &monitor_info,
        pmix::PmixStatus::Known(pmix::PmixError::MonitorHeartbeatAlert),
        &[],
    );
    eprintln!(
        "[monitoring_minimal] process_monitor returned: {:?}",
        result
    );
}
