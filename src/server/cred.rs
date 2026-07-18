//! Server submodule: cred

use super::*;
#[cfg(any(test, feature = "mock_ffi"))]
use crate::mock_ffi;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Get_credential — get credential (server context)
// ─────────────────────────────────────────────────────────────────────────────

/// Retrieve a credential from a server context.
///
/// This wraps `PMIx_Get_credential` to be called from a server context.
/// Delegates to [`crate::security::get_credential`] for the actual implementation.
///
/// # Parameters
/// * `handle` — the server handle returned by [`server_init`].
/// * `info` — directives specifying which credential to retrieve.
///
/// # Returns
/// * `Ok(PmixCredential)` — the requested credential.
/// * `Err(PmixStatus)` — credential retrieval failed.
///
/// # C API
/// `pmix_status_t PMIx_Get_credential(const pmix_info_t info[], size_t ninfo,
///                                     pmix_byte_object_t *credential);`
pub fn server_get_credential(
    _handle: &PmixServerHandle,
    info: &[Info],
) -> Result<PmixCredential, PmixStatus> {
        #[cfg(any(test, feature = "mock_ffi"))]
    {
        if mock_ffi::is_mock_enabled() {
            let mut cred = unsafe { std::mem::zeroed::<ffi::pmix_byte_object_t>() };
        let status = unsafe {
            mock_ffi::mock_server_get_credential(
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        let pmix_status = PmixStatus::from_raw(status);
        if pmix_status.is_success() {
            Ok(PmixCredential::empty())
        } else {
            Err(pmix_status)
        }
        } else {
            crate::security::get_credential(info)
        }
    }
    #[cfg(not(any(test, feature = "mock_ffi")))]
    {
        crate::security::get_credential(info)
    }
}

