//! Compile-time `Send`/`Sync` matrix for concrete public PMIx handles and values.
//!
//! This is intentionally centralized: when a public wrapper is added or its
//! ownership model changes, add its intended auto-traits here.  The negative
//! assertions are as important as the positive ones.  In particular, they
//! prevent a raw-pointer-bearing wrapper from silently becoming movable or
//! shareable after a representation change.
//!
//! The matrix covers concrete public values, handles, and callback carrier
//! structs. Public callback traits declare their required `Send` bounds at
//! their definitions; trait objects inherit those bounds and are governed by
//! their explicit object-type bounds.
//!
//! See [THREADING.md](../THREADING.md), issue #50, and issue #66.

#[cfg(test)]
mod asserts {
    use static_assertions::{assert_impl_all, assert_not_impl_any};

    // ── Sessions, identities, POD, and scalar values ──────────────────────
    //
    // These values contain either process-safe session state, copied process
    // identity, scalar data, or Rust-owned memory.  They are deliberately
    // usable from worker threads.
    assert_impl_all!(crate::PmixClient: Clone, Send, Sync);
    assert_impl_all!(crate::PmixClientState: Send, Sync);
    assert_impl_all!(crate::server::PmixServer: Clone, Send, Sync);
    assert_impl_all!(crate::server::PmixServerHandle: Clone, Send, Sync);
    assert_impl_all!(crate::server::PmixServerState: Send, Sync);
    assert_impl_all!(crate::tool::PmixTool: Clone, Send, Sync);
    assert_impl_all!(crate::tool::PmixToolState: Send, Sync);
    assert_impl_all!(crate::tool::PmixToolHandle: Clone, Send, Sync);
    assert_impl_all!(crate::tool::PmixServerHandle: Clone, Send, Sync);
    assert_impl_all!(crate::Proc: Clone, Send, Sync);

    assert_impl_all!(crate::PmixError: Send, Sync);
    assert_impl_all!(crate::PmixStatus: Send, Sync);
    assert_impl_all!(crate::PmixProcState: Send, Sync);
    assert_impl_all!(crate::PmixScope: Send, Sync);
    assert_impl_all!(crate::PmixJobState: Send, Sync);
    assert_impl_all!(crate::PmixLinkState: Send, Sync);
    assert_impl_all!(crate::PmixDeviceType: Send, Sync);
    assert_impl_all!(crate::PmixPersistence: Send, Sync);
    assert_impl_all!(crate::PmixDataRange: Send, Sync);
    assert_impl_all!(crate::PmixDataType: Send, Sync);
    assert_impl_all!(crate::PmixAllocDirective: Send, Sync);
    assert_impl_all!(crate::allocation::PmixAllocDirective: Send, Sync);
    assert_impl_all!(crate::allocation::PmixJobCtrlAction: Send, Sync);
    assert_impl_all!(crate::cpu_locality::PmixBindEnvelope: Send, Sync);
    assert_impl_all!(crate::cpu_locality::PmixLocality: Send, Sync);
    assert_impl_all!(crate::IOFChannelFlags: Send, Sync);
    assert_impl_all!(crate::InfoFlags: Send, Sync);
    assert_impl_all!(crate::BuilderError: Send, Sync);
    assert_impl_all!(crate::ValueError: Send, Sync);
    assert_impl_all!(crate::PmixTimeval: Send, Sync);
    assert_impl_all!(crate::PmixEnvar: Send, Sync);
    assert_impl_all!(crate::InitOptions: Send, Sync);

    // Rust-owned wrappers and materialized results.  These do not retain a
    // PMIx allocation after construction.  `utility::PmixByteObject` is the
    // exception to the similarly named data-serialization wrapper: it owns a
    // Rust `Vec<u8>` and is only used to create a short-lived FFI copy.
    assert_impl_all!(crate::security::PmixCredential: Clone, Send, Sync);
    assert_impl_all!(crate::utility::PmixByteObject: Clone, Send, Sync);
    assert_impl_all!(crate::data_serialization::PmixPrintOutput: Send, Sync);
    assert_impl_all!(crate::fabric::PmixDeviceDistance: Send, Sync);
    assert_impl_all!(crate::process_mgmt::PmixApp: Send, Sync);
    assert_impl_all!(crate::process_mgmt::PmixAppBuilder: Send, Sync);
    assert_impl_all!(crate::data_serialization::PmixProcRef<'static>: Send, Sync);
    assert_impl_all!(crate::threading::ProgressContext: Send, Sync);

    // ── Function pointers and callback carriers ───────────────────────────
    //
    // A function pointer has no captured state.  Callback wrapper structs are
    // movable because their trait objects require `Send`, but they are not
    // advertised as concurrently shareable (`Sync` is intentionally absent).
    assert_impl_all!(crate::events::EventHandlerRef: Send, Sync);
    assert_impl_all!(crate::events::NotificationFn: Send, Sync);
    assert_impl_all!(crate::events::HandlerRegCbFn: Send, Sync);
    assert_impl_all!(crate::events::OpCbFn: Send, Sync);
    assert_impl_all!(crate::events::pmix_event_notification_cbfunc_fn_t: Send, Sync);
    assert_impl_all!(crate::groups::pmix_group_opt_t: Send, Sync);
    assert_impl_all!(crate::process_mgmt::SpawnCallback: Send, Sync);
    assert_impl_all!(crate::groups::GroupConstructCallbackWrapper: Send);
    assert_impl_all!(crate::groups::GroupInviteCallbackWrapper: Send);
    assert_impl_all!(crate::groups::GroupJoinCallbackWrapper: Send);
    assert_impl_all!(crate::groups::GroupLeaveCallbackWrapper: Send);
    assert_impl_all!(crate::groups::GroupDestructCallbackWrapper: Send);
    assert_impl_all!(crate::process_mgmt::SpawnCallbackWrapper: Send);
    assert_impl_all!(crate::process_mgmt::ConnectCallbackWrapper: Send);
    assert_impl_all!(crate::process_mgmt::DisconnectCallbackWrapper: Send);
    assert_impl_all!(crate::server::FenceNbCallbackWrapper: Send);

    assert_not_impl_any!(crate::groups::GroupConstructCallbackWrapper: Sync);
    assert_not_impl_any!(crate::groups::GroupInviteCallbackWrapper: Sync);
    assert_not_impl_any!(crate::groups::GroupJoinCallbackWrapper: Sync);
    assert_not_impl_any!(crate::groups::GroupLeaveCallbackWrapper: Sync);
    assert_not_impl_any!(crate::groups::GroupDestructCallbackWrapper: Sync);
    assert_not_impl_any!(crate::process_mgmt::SpawnCallbackWrapper: Sync);
    assert_not_impl_any!(crate::process_mgmt::ConnectCallbackWrapper: Sync);
    assert_not_impl_any!(crate::process_mgmt::DisconnectCallbackWrapper: Sync);
    assert_not_impl_any!(crate::server::FenceNbCallbackWrapper: Sync);

    // `Receiver<T>` is movable but not shareable.  This locks the intended
    // hop-off design without pretending that arbitrary `T` is thread-safe.
    assert_impl_all!(crate::threading::CallbackChannel<u8>: Send);
    assert_not_impl_any!(crate::threading::CallbackChannel<u8>: Sync);

    // ── C-owned or raw-pointer-bearing values ─────────────────────────────
    //
    // These types either own PMIx memory, release C allocations in `Drop`,
    // contain a borrowed/opaque C pointer, or contain a raw `pmix_value_t`
    // whose active union arm controls ownership.  They must stay on the
    // creating/application thread.  Convert their contents to Rust-owned
    // data before sending callback results to another thread.
    assert_not_impl_any!(crate::Info: Send);
    assert_not_impl_any!(crate::Info: Sync);
    assert_not_impl_any!(crate::InfoBuilder: Send);
    assert_not_impl_any!(crate::InfoBuilder: Sync);
    assert_not_impl_any!(crate::PmixPayload: Send);
    assert_not_impl_any!(crate::PmixPayload: Sync);
    assert_not_impl_any!(crate::PmixValueBuilder: Send);
    assert_not_impl_any!(crate::PmixValueBuilder: Sync);
    assert_not_impl_any!(crate::PmixOwnedValue: Send);
    assert_not_impl_any!(crate::PmixOwnedValue: Sync);

    assert_not_impl_any!(crate::data_ops::PmixPdata: Send);
    assert_not_impl_any!(crate::data_ops::PmixPdata: Sync);
    assert_not_impl_any!(crate::data_serialization::PmixByteObject: Send);
    assert_not_impl_any!(crate::data_serialization::PmixByteObject: Sync);
    assert_not_impl_any!(crate::data_serialization::PmixDataBuffer: Send);
    assert_not_impl_any!(crate::data_serialization::PmixDataBuffer: Sync);
    assert_not_impl_any!(crate::fabric::PmixFabric: Send);
    assert_not_impl_any!(crate::fabric::PmixFabric: Sync);
    assert_not_impl_any!(crate::fabric::PmixTopology: Send);
    assert_not_impl_any!(crate::fabric::PmixTopology: Sync);
    assert_not_impl_any!(crate::fabric::PmixCpuset: Send);
    assert_not_impl_any!(crate::fabric::PmixCpuset: Sync);
    assert_not_impl_any!(crate::fabric::DeviceDistances: Send);
    assert_not_impl_any!(crate::fabric::DeviceDistances: Sync);
    assert_not_impl_any!(crate::query_log::PmixQuery: Send);
    assert_not_impl_any!(crate::query_log::PmixQuery: Sync);
    assert_not_impl_any!(crate::query_log::QueryResults: Send);
    assert_not_impl_any!(crate::query_log::QueryResults: Sync);
    assert_not_impl_any!(crate::allocation::AllocationResults: Send);
    assert_not_impl_any!(crate::allocation::AllocationResults: Sync);
    assert_not_impl_any!(crate::allocation::JobControlResults: Send);
    assert_not_impl_any!(crate::allocation::JobControlResults: Sync);
    assert_not_impl_any!(crate::allocation::SessionControlResults: Send);
    assert_not_impl_any!(crate::allocation::SessionControlResults: Sync);
    assert_not_impl_any!(crate::monitoring::MonitorResults: Send);
    assert_not_impl_any!(crate::monitoring::MonitorResults: Sync);
    assert_not_impl_any!(crate::security::CredentialResults: Send);
    assert_not_impl_any!(crate::security::CredentialResults: Sync);
    assert_not_impl_any!(crate::security::ValidationResults: Send);
    assert_not_impl_any!(crate::security::ValidationResults: Sync);
    assert_not_impl_any!(crate::server::CollectInventoryResults: Send);
    assert_not_impl_any!(crate::server::CollectInventoryResults: Sync);
}
