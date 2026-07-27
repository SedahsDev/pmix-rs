//! Compile-time `Send`/`Sync` matrix for public session vs C-owned handle types.
//!
//! See [THREADING.md](../THREADING.md) and issue #50.

#[cfg(test)]
mod asserts {
    use static_assertions::{assert_impl_all, assert_not_impl_any};

    // ── Sessions: must stay shareable ─────────────────────────────────────
    use crate::{PmixClient, PmixClientState, Proc};
    use crate::server::{PmixServer, PmixServerState};
    use crate::tool::{PmixTool, PmixToolState};

    assert_impl_all!(PmixClient: Clone, Send, Sync);
    assert_impl_all!(PmixClientState: Send, Sync);
    assert_impl_all!(PmixServer: Clone, Send, Sync);
    assert_impl_all!(PmixServerState: Send, Sync);
    assert_impl_all!(PmixTool: Clone, Send, Sync);
    assert_impl_all!(PmixToolState: Send, Sync);
    // POD process identity — free-threaded by design
    assert_impl_all!(Proc: Clone, Send, Sync);

    // ── C-owned / exclusive handles: must NOT be free-threaded ────────────
    use crate::Info;
    use crate::InfoBuilder;
    use crate::PmixOwnedValue;
    use crate::allocation::{AllocationResults, JobControlResults, SessionControlResults};
    use crate::data_serialization::{PmixByteObject, PmixDataBuffer};
    use crate::fabric::{DeviceDistances, PmixCpuset, PmixFabric, PmixTopology};
    use crate::monitoring::MonitorResults;
    use crate::query_log::{PmixQuery, QueryResults};
    use crate::security::ValidationResults;
    use crate::server::CollectInventoryResults;

    assert_not_impl_any!(Info: Send);
    assert_not_impl_any!(Info: Sync);
    assert_not_impl_any!(InfoBuilder: Send);
    assert_not_impl_any!(InfoBuilder: Sync);
    assert_not_impl_any!(PmixOwnedValue: Send);
    assert_not_impl_any!(PmixOwnedValue: Sync);
    assert_not_impl_any!(PmixDataBuffer: Send);
    assert_not_impl_any!(PmixDataBuffer: Sync);
    assert_not_impl_any!(PmixByteObject: Send);
    assert_not_impl_any!(PmixByteObject: Sync);
    assert_not_impl_any!(PmixFabric: Send);
    assert_not_impl_any!(PmixFabric: Sync);
    assert_not_impl_any!(PmixTopology: Send);
    assert_not_impl_any!(PmixTopology: Sync);
    assert_not_impl_any!(PmixCpuset: Send);
    assert_not_impl_any!(PmixCpuset: Sync);
    assert_not_impl_any!(DeviceDistances: Send);
    assert_not_impl_any!(DeviceDistances: Sync);
    assert_not_impl_any!(QueryResults: Send);
    assert_not_impl_any!(QueryResults: Sync);
    assert_not_impl_any!(PmixQuery: Send);
    assert_not_impl_any!(PmixQuery: Sync);
    assert_not_impl_any!(AllocationResults: Send);
    assert_not_impl_any!(AllocationResults: Sync);
    assert_not_impl_any!(JobControlResults: Send);
    assert_not_impl_any!(JobControlResults: Sync);
    assert_not_impl_any!(SessionControlResults: Send);
    assert_not_impl_any!(SessionControlResults: Sync);
    assert_not_impl_any!(MonitorResults: Send);
    assert_not_impl_any!(MonitorResults: Sync);
    assert_not_impl_any!(ValidationResults: Send);
    assert_not_impl_any!(ValidationResults: Sync);
    assert_not_impl_any!(CollectInventoryResults: Send);
    assert_not_impl_any!(CollectInventoryResults: Sync);
}
