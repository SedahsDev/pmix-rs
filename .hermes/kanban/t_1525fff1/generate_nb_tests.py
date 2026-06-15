#!/usr/bin/env python3
"""Generate _nb test files for missing PMIx non-blocking functions."""

import os

BASE = "/home/bzf/projects/pmix-rs/tests"

# Each entry: (filename, module_import, function_name, callback_trait, 
#              test_call_expr, additional_imports)
tests = [
    # ── data_ops ──
    (
        "data_ops_Publish_nb.rs",
        "use pmix::data_ops::{publish_nb, PublishCallback};\nuse pmix::{InfoBuilder, PmixStatus};",
        "publish_nb",
        "PublishCallback",
        'publish_nb(&info, Box::new(TestCallback))',
        "",
    ),
    (
        "data_ops_Lookup_nb.rs",
        "use pmix::data_ops::{lookup_nb, LookupCallback};\nuse pmix::{InfoBuilder, PmixStatus};",
        "lookup_nb",
        "LookupCallback",
        'lookup_nb(&["key1"], None, Box::new(TestCallback))',
        "",
    ),
    (
        "data_ops_Unpublish_nb.rs",
        "use pmix::data_ops::{unpublish_nb, UnpublishCallback};\nuse pmix::PmixStatus;",
        "unpublish_nb",
        "UnpublishCallback",
        'unpublish_nb(None, None, Box::new(TestCallback))',
        "",
    ),
    # ── events ──
    (
        "events_Register_event_handler_nb.rs",
        "use pmix::events::{register_event_handler_nb, NotificationFn};\nuse pmix::{InfoBuilder, PmixStatus};",
        "register_event_handler_nb",
        "NotificationFn",
        'register_event_handler_nb(&[], &info, NotificationFn::new(|_, _, _, _, _, _| {}))',
        "",
    ),
    (
        "events_Deregister_event_handler_nb.rs",
        "use pmix::events::{deregister_event_handler_nb, OpCbFn};\nuse pmix::PmixStatus;\nuse std::ffi::c_void;",
        "deregister_event_handler_nb",
        "OpCbFn",
        'deregister_event_handler_nb(0, Some(|_, _, _| {}), std::ptr::null_mut())',
        "",
    ),
    (
        "events_Notify_event_nb.rs",
        "use pmix::events::{notify_event_nb, NotificationFn};\nuse pmix::{PmixDataRange, PmixStatus, Proc};",
        "notify_event_nb",
        "NotificationFn",
        'notify_event_nb(PmixStatus::Success, &proc, PmixDataRange::Global, &info, NotificationFn::new(|_, _, _, _, _, _| {}))',
        "",
    ),
    # ── process_mgmt ──
    (
        "process_mgmt_Spawn_nb.rs",
        "use pmix::process_mgmt::{spawn_nb, SpawnCallbackWrapper};\nuse pmix::{InfoBuilder, PmixApp, PmixStatus};",
        "spawn_nb",
        "SpawnCallbackWrapper",
        'spawn_nb(&[], &[], Box::new(TestCallback))',
        "",
    ),
    (
        "process_mgmt_Connect_nb.rs",
        "use pmix::process_mgmt::{connect_nb, ConnectCallbackWrapper};\nuse pmix::{Info, PmixStatus, Proc};",
        "connect_nb",
        "ConnectCallbackWrapper",
        'connect_nb(&[], &[], Box::new(TestCallback))',
        "",
    ),
    (
        "process_mgmt_Disconnect_nb.rs",
        "use pmix::process_mgmt::{disconnect_nb, DisconnectCallbackWrapper};\nuse pmix::{Info, PmixStatus, Proc};",
        "disconnect_nb",
        "DisconnectCallbackWrapper",
        'disconnect_nb(&[], &[], Box::new(TestCallback))',
        "",
    ),
    # ── query_log ──
    (
        "query_log_Query_info_nb.rs",
        "use pmix::query_log::{query_info_nb, QueryCallback};\nuse pmix::{PmixQuery, PmixStatus};",
        "query_info_nb",
        "QueryCallback",
        'query_info_nb(&[], Box::new(TestCallback))',
        "",
    ),
    (
        "query_log_Log_data_nb.rs",
        "use pmix::query_log::{log_data_nb, LogCallback};\nuse pmix::PmixStatus;",
        "log_data_nb",
        "LogCallback",
        'log_data_nb(&[], &[], Box::new(TestCallback))',
        "",
    ),
    # ── allocation ──
    (
        "allocation_Job_control_nb.rs",
        "use pmix::allocation::{job_control_nb, JobControlCallback};\nuse pmix::{Info, PmixStatus, Proc};",
        "job_control_nb",
        "JobControlCallback",
        'job_control_nb(&[], &[], Box::new(TestCallback))',
        "",
    ),
    # ── groups ──
    (
        "groups_Group_join_nb.rs",
        "use pmix::groups::{group_join_nb, GroupJoinCallbackWrapper};\nuse pmix::PmixStatus;\nuse pmix::Proc;",
        "group_join_nb",
        "GroupJoinCallbackWrapper",
        'group_join_nb("test_group", &proc, 0, Box::new(TestCallback))',
        "",
    ),
    (
        "groups_Group_leave_nb.rs",
        "use pmix::groups::{group_leave_nb, GroupLeaveCallbackWrapper};\nuse pmix::{Info, PmixStatus};",
        "group_leave_nb",
        "GroupLeaveCallbackWrapper",
        'group_leave_nb("test_group", &[], Box::new(TestCallback))',
        "",
    ),
]

for filename, imports, func_name, cb_trait, call_expr, extra_imports in tests:
    # Determine the C function name (PMIx_ prefix + snake_case to PascalCase)
    # e.g., publish_nb -> PMIx_Publish_nb
    parts = func_name.split("_")
    c_func = "PMIx_" + "".join(p.capitalize() for p in parts)
    
    content = f'''//! Integration tests for `{c_func}` via the safe `{func_name}()` wrapper.
//!
//! Tests that require PMIx runtime (PMIx_Init) are marked `#[ignore]`.

{imports}
{extra_imports}

/// `{func_name}` function is public and has the correct signature.
#[test]
fn {func_name}_compiles() {{
    // Verify the function signature compiles.
    let _ = {func_name} as fn({call_expr.split('(')[1].split(')')[0]}) -> Result<(), PmixStatus>;
}}

/// `{cb_trait}` trait is importable and can be used as a trait object.
#[test]
fn {func_name}_callback_trait_object() {{
    struct TestCallback;
    impl {cb_trait} for TestCallback {{
'''

    # Determine callback signature based on trait
    if cb_trait == "PublishCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "LookupCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _pdata: Vec<pmix::PmixPdata>) {}\n'
    elif cb_trait == "UnpublishCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "NotificationFn":
        # NotificationFn is a struct, not a trait — skip this test
        content += f'''    // NotificationFn is a wrapper struct, not a trait to implement.
    }}
}}

#[test]
fn {func_name}_notification_fn_exists() {{
    // NotificationFn::new accepts a closure.
    let _fn: NotificationFn = NotificationFn::new(|_, _, _, _, _, _| {{}});
}}
'''
    elif cb_trait == "OpCbFn":
        content += '        // OpCbFn is a function pointer type, not a trait.\n'
    elif cb_trait == "SpawnCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _job: pmix::Proc) {}\n'
    elif cb_trait == "ConnectCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "DisconnectCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "QueryCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<pmix::Info>) {}\n'
    elif cb_trait == "LogCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "JobControlCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<pmix::Info>) {}\n'
    elif cb_trait == "GroupJoinCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "GroupLeaveCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    
    content += f'''    }}

    let cb: Box<dyn {cb_trait}> = Box::new(TestCallback);
    let _: Box<dyn {cb_trait}> = cb;
}}

/// `{func_name}` returns `PMIX_ERR_INIT` when called without PMIx_Init.
#[test]
fn {func_name}_before_init_returns_err_init() {{
    struct InitCheckCallback;
    impl {cb_trait} for InitCheckCallback {{
'''
    
    # Same callback signatures
    if cb_trait == "PublishCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "LookupCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _pdata: Vec<pmix::PmixPdata>) {}\n'
    elif cb_trait == "UnpublishCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "NotificationFn":
        content += '        // NotificationFn is not a trait\n'
    elif cb_trait == "OpCbFn":
        content += '        // OpCbFn is a function pointer\n'
    elif cb_trait == "SpawnCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _job: pmix::Proc) {}\n'
    elif cb_trait == "ConnectCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "DisconnectCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "QueryCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<pmix::Info>) {}\n'
    elif cb_trait == "LogCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "JobControlCallback":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus, _info: Vec<pmix::Info>) {}\n'
    elif cb_trait == "GroupJoinCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    elif cb_trait == "GroupLeaveCallbackWrapper":
        content += '        fn on_complete(self: Box<Self>, _status: PmixStatus) {}\n'
    
    content += f'''    }}

'''

    # Build the actual call
    if "info" in call_expr:
        content += '    let info = InfoBuilder::new().build();\n'
    if "proc" in call_expr:
        content += '    let proc = Proc::new("test_ns", 0).unwrap();\n'
    
    content += f'    let result = {call_expr};\n'
    content += f'''    assert!(result.is_err(), "{func_name} should fail without PMIx_Init");
    assert_eq!(result.unwrap_err().to_raw(), -31, "should be PMIX_ERR_INIT");
}}
'''
    
    filepath = os.path.join(BASE, filename)
    with open(filepath, 'w') as f:
        f.write(content)
    print(f"Created {filepath}")

print("\nDone.")
