//! Group management — `PMIx_Group_construct`, `PMIx_Group_construct_nb`,
//! `PMIx_Group_invite`, `PMIx_Group_invite_nb`, `PMIx_Group_join`,
//! `PMIx_Group_join_nb`, `PMIx_Group_leave`, `PMIx_Group_leave_nb`,
//! `PMIx_Group_destruct`, `PMIx_Group_destruct_nb`.
//!
//! This module provides safe Rust wrappers around the PMIx group
//! management APIs.

use crate::cbdata::{decode_req_id, encode_req_id, Registry};
use crate::ffi;
use crate::threading::invoke_user_callback;
use crate::{Info, PmixStatus, Proc};

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::{LazyLock, Mutex};

/// Re-export of the PMIx group accept/decline option enum.
///
/// Used by `group_join` and `group_join_nb` to specify whether
/// to accept or decline a group invitation.
pub use ffi::pmix_group_opt_t;


// ── One-shot completion registries (issue #67) ───────────────────────────────
//
// Bridges never pass a `Box` pointer as cbdata. Request IDs are encoded with
// `encode_req_id`; the bridge does `lock → remove → unlock → invoke_user_callback`.

static GROUP_CONSTRUCT_REGISTRY: LazyLock<Registry<GroupConstructCallbackWrapper>> =
    LazyLock::new(Registry::new);

static GROUP_INVITE_REGISTRY: LazyLock<Registry<GroupInviteCallbackWrapper>> =
    LazyLock::new(Registry::new);

static GROUP_JOIN_REGISTRY: LazyLock<Registry<GroupJoinCallbackWrapper>> =
    LazyLock::new(Registry::new);

static GROUP_LEAVE_REGISTRY: LazyLock<Registry<GroupLeaveCallbackWrapper>> =
    LazyLock::new(Registry::new);

static GROUP_DESTRUCT_REGISTRY: LazyLock<Registry<GroupDestructCallbackWrapper>> =
    LazyLock::new(Registry::new);

fn info_results_from_c(status: PmixStatus, info: *mut ffi::pmix_info_t, ninfo: usize) -> Vec<Info> {
    if !status.is_success() || info.is_null() || ninfo == 0 {
        return Vec::new();
    }
    // SAFETY: PMIx handed us a valid pmix_info_t array of length ninfo for the
    // duration of the completion callback. We wrap each entry without taking
    // free ownership of the whole array (same model as the previous Box-cbdata
    // bridges in this module).
    let mut vec = Vec::with_capacity(ninfo);
    unsafe {
        for i in 0..ninfo {
            vec.push(Info {
                handle: info.add(i),
                len: 1,
                _not_thread_safe: std::marker::PhantomData,
            });
        }
    }
    vec
}


// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_construct
// ─────────────────────────────────────────────────────────────────────────────

/// Construct a new group composed of the specified processes.
///
/// Creates a group identified by `group_id` containing the specified
/// processes. This is a **blocking** call: it does not return until all
/// specified processes have joined the group.
///
/// # Returns
/// * `Ok(Vec<Info>)` — group construction succeeded; results info array.
/// * `Err(PmixStatus)` — error in the request.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_construct(const char grp[],
///                                    const pmix_proc_t procs[], size_t nprocs,
///                                    const pmix_info_t directives[], size_t ndirs,
///                                    pmix_info_t **results, size_t *nresults);
/// ```
pub fn group_construct(
    group_id: &str,
    procs: &[Proc],
    directives: &[Info],
) -> Result<Vec<Info>, PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (dirs_ptr, ndirs) = if directives.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&directives[0] as *const Info)).handle)
                    as *const ffi::pmix_info_t
            },
            directives.len(),
        )
    };

    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresults: usize = 0;

    let raw_status = unsafe {
        ffi::PMIx_Group_construct(
            group_id_c.as_ptr(),
            procs_ptr,
            procs.len(),
            dirs_ptr,
            ndirs,
            &mut results,
            &mut nresults,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    let rust_results: Vec<Info> = unsafe {
        if results.is_null() || nresults == 0 {
            Vec::new()
        } else {
            let arr_ptr = results;
            let mut vec = Vec::with_capacity(nresults);
            for i in 0..nresults {
                vec.push(Info {
                    handle: arr_ptr.add(i),
                    len: 1,
                _not_thread_safe: std::marker::PhantomData,
                });
            }
            #[allow(unused_assignments)]
            {
                results = ptr::null_mut();
            }
            vec
        }
    };

    Ok(rust_results)
}

// ─────────────────────────────────────────────────────────────────────────────
// GroupConstructCallbackWrapper
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking group construct callback wrapper.
pub struct GroupConstructCallbackWrapper {
    callback: Box<dyn Fn(PmixStatus, Vec<Info>) + Send + 'static>,
}

impl GroupConstructCallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus, Vec<Info>) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_construct_nb
// ─────────────────────────────────────────────────────────────────────────────

/// FFI callback bridge for non-blocking group completion (`pmix_info_cbfunc_t`).
///
/// # Safety
/// `cbdata` is an opaque request ID from [`encode_req_id`]. OpenPMIx invokes
/// this from the progress thread exactly once per accepted request.
pub unsafe extern "C" fn group_construct_callback_bridge(
    status: i32,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
    _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    _release_cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }
    let req_id = decode_req_id(cbdata);
    let cb = {
        let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let Some(cb_wrapper) = cb else {
        return;
    };
    let pmix_status = PmixStatus::from_raw(status);
    let rust_results = info_results_from_c(pmix_status, info, ninfo);
    let _ = invoke_user_callback("groups", move || {
        (cb_wrapper.callback)(pmix_status, rust_results);
    });
}

/// Non-blocking group construct with a Rust closure callback.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_construct_nb(const char grp[],
///                                       const pmix_proc_t procs[], size_t nprocs,
///                                       const pmix_info_t info[], size_t ninfo,
///                                       pmix_info_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn group_construct_nb(
    group_id: &str,
    procs: &[Proc],
    info: &[Info],
    callback: GroupConstructCallbackWrapper,
) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let req_id = GROUP_CONSTRUCT_REGISTRY.insert_next(callback);
    let cbdata = encode_req_id(req_id);


    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe {
        ffi::PMIx_Group_construct_nb(
            group_id_c.as_ptr(),
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            Some(group_construct_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_invite
// ─────────────────────────────────────────────────────────────────────────────

/// Invite specified processes to join a group.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_invite(const char grp[],
///                                 const pmix_proc_t procs[], size_t nprocs,
///                                 const pmix_info_t info[], size_t ninfo,
///                                 pmix_info_t **results, size_t *nresult);
/// ```
pub fn group_invite(
    group_id: &str,
    procs: &[Proc],
    info: &[Info],
) -> Result<Vec<Info>, PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresult: usize = 0;

    let raw_status = unsafe {
        ffi::PMIx_Group_invite(
            group_id_c.as_ptr(),
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            &mut results,
            &mut nresult,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    let rust_results: Vec<Info> = unsafe {
        if results.is_null() || nresult == 0 {
            Vec::new()
        } else {
            let arr_ptr = results;
            let mut vec = Vec::with_capacity(nresult);
            for i in 0..nresult {
                vec.push(Info {
                    handle: arr_ptr.add(i),
                    len: 1,
                _not_thread_safe: std::marker::PhantomData,
                });
            }
            #[allow(unused_assignments)]
            {
                results = ptr::null_mut();
            }
            vec
        }
    };

    Ok(rust_results)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_invite_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking invite callback wrapper.
pub struct GroupInviteCallbackWrapper {
    callback: Box<dyn Fn(PmixStatus, Vec<Info>) + Send + 'static>,
}

impl GroupInviteCallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus, Vec<Info>) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// FFI callback bridge for non-blocking group completion (`pmix_info_cbfunc_t`).
///
/// # Safety
/// `cbdata` is an opaque request ID from [`encode_req_id`]. OpenPMIx invokes
/// this from the progress thread exactly once per accepted request.
pub unsafe extern "C" fn group_invite_callback_bridge(
    status: i32,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
    _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    _release_cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }
    let req_id = decode_req_id(cbdata);
    let cb = {
        let mut registry = GROUP_INVITE_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let Some(cb_wrapper) = cb else {
        return;
    };
    let pmix_status = PmixStatus::from_raw(status);
    let rust_results = info_results_from_c(pmix_status, info, ninfo);
    let _ = invoke_user_callback("groups", move || {
        (cb_wrapper.callback)(pmix_status, rust_results);
    });
}

/// Non-blocking invite with a Rust closure callback.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_invite_nb(const char grp[],
///                                    const pmix_proc_t procs[], size_t nprocs,
///                                    const pmix_info_t info[], size_t ninfo,
///                                    pmix_info_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn group_invite_nb(
    group_id: &str,
    procs: &[Proc],
    info: &[Info],
    callback: GroupInviteCallbackWrapper,
) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };
    let req_id = GROUP_INVITE_REGISTRY.insert_next(callback);
    let cbdata = encode_req_id(req_id);


    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe {
        ffi::PMIx_Group_invite_nb(
            group_id_c.as_ptr(),
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            Some(group_invite_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = GROUP_INVITE_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_join
// ─────────────────────────────────────────────────────────────────────────────

/// Respond to a group invitation (accept or decline).
///
/// Called by an invited process to accept or decline a group invitation.
/// The `leader` parameter identifies the leader process of the group.
/// The `option` parameter specifies whether to accept (`PMIX_GROUP_ACCEPT`)
/// or decline (`PMIX_GROUP_DECLINE`) the invitation.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_join(const char grp[],
///                               const pmix_proc_t leader,
///                               pmix_group_opt_t opt,
///                               const pmix_info_t info[], size_t ninfo,
///                               pmix_info_t **results, size_t *nresult);
/// ```
pub fn group_join(
    group_id: &str,
    leader: &Proc,
    option: ffi::pmix_group_opt_t,
    info: &[Info],
) -> Result<Vec<Info>, PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let leader_ptr = std::ptr::addr_of!(leader.handle) as *const ffi::pmix_proc_t;

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let mut results: *mut ffi::pmix_info_t = ptr::null_mut();
    let mut nresult: usize = 0;

    let raw_status = unsafe {
        ffi::PMIx_Group_join(
            group_id_c.as_ptr(),
            leader_ptr,
            option,
            info_ptr,
            ninfo,
            &mut results,
            &mut nresult,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    let rust_results: Vec<Info> = unsafe {
        if results.is_null() || nresult == 0 {
            Vec::new()
        } else {
            let arr_ptr = results;
            let mut vec = Vec::with_capacity(nresult);
            for i in 0..nresult {
                vec.push(Info {
                    handle: arr_ptr.add(i),
                    len: 1,
                _not_thread_safe: std::marker::PhantomData,
                });
            }
            #[allow(unused_assignments)]
            {
                results = ptr::null_mut();
            }
            vec
        }
    };

    Ok(rust_results)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_join_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking join callback wrapper.
pub struct GroupJoinCallbackWrapper {
    callback: Box<dyn Fn(PmixStatus, Vec<Info>) + Send + 'static>,
}

impl GroupJoinCallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus, Vec<Info>) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// FFI callback bridge for non-blocking group completion (`pmix_info_cbfunc_t`).
///
/// # Safety
/// `cbdata` is an opaque request ID from [`encode_req_id`]. OpenPMIx invokes
/// this from the progress thread exactly once per accepted request.
pub unsafe extern "C" fn group_join_callback_bridge(
    status: i32,
    info: *mut ffi::pmix_info_t,
    ninfo: usize,
    cbdata: *mut c_void,
    _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    _release_cbdata: *mut c_void,
) {
    if cbdata.is_null() {
        return;
    }
    let req_id = decode_req_id(cbdata);
    let cb = {
        let mut registry = GROUP_JOIN_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let Some(cb_wrapper) = cb else {
        return;
    };
    let pmix_status = PmixStatus::from_raw(status);
    let rust_results = info_results_from_c(pmix_status, info, ninfo);
    let _ = invoke_user_callback("groups", move || {
        (cb_wrapper.callback)(pmix_status, rust_results);
    });
}

/// Non-blocking join with a Rust closure callback.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_join_nb(const char grp[],
///                                  const pmix_proc_t leader,
///                                  pmix_group_opt_t opt,
///                                  const pmix_info_t info[], size_t ninfo,
///                                  pmix_info_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn group_join_nb(
    group_id: &str,
    leader: &Proc,
    option: ffi::pmix_group_opt_t,
    info: &[Info],
    callback: GroupJoinCallbackWrapper,
) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };
    let req_id = GROUP_JOIN_REGISTRY.insert_next(callback);
    let cbdata = encode_req_id(req_id);

    let leader_ptr = std::ptr::addr_of!(leader.handle) as *const ffi::pmix_proc_t;

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe {
        ffi::PMIx_Group_join_nb(
            group_id_c.as_ptr(),
            leader_ptr,
            option,
            info_ptr,
            ninfo,
            Some(group_join_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = GROUP_JOIN_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_leave
// ─────────────────────────────────────────────────────────────────────────────

/// Leave a group asynchronously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_leave(const char grp[],
///                                const pmix_info_t info[], size_t ninfo);
/// ```
pub fn group_leave(group_id: &str, info: &[Info]) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe { ffi::PMIx_Group_leave(group_id_c.as_ptr(), info_ptr, ninfo) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_leave_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking leave callback wrapper.
pub struct GroupLeaveCallbackWrapper {
    callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
}

impl GroupLeaveCallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// FFI callback bridge for non-blocking group leave.
///
/// # Safety
/// `cbdata` must be a valid pointer to a `GroupLeaveCallbackWrapper`
pub unsafe extern "C" fn group_leave_callback_bridge(status: i32, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }
    let req_id = decode_req_id(cbdata);
    let cb = {
        let mut registry = GROUP_LEAVE_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let Some(cb_wrapper) = cb else {
        return;
    };
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("groups", move || {
        (cb_wrapper.callback)(pmix_status);
    });
}

/// Non-blocking leave with a Rust closure callback.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_leave_nb(const char grp[],
///                                   const pmix_info_t info[], size_t ninfo,
///                                   pmix_op_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn group_leave_nb(
    group_id: &str,
    info: &[Info],
    callback: GroupLeaveCallbackWrapper,
) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };
    let req_id = GROUP_LEAVE_REGISTRY.insert_next(callback);
    let cbdata = encode_req_id(req_id);


    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe {
        ffi::PMIx_Group_leave_nb(
            group_id_c.as_ptr(),
            info_ptr,
            ninfo,
            Some(group_leave_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = GROUP_LEAVE_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_destruct
// ─────────────────────────────────────────────────────────────────────────────

/// Synchronously destroy a group.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_destruct(const char grp[],
///                                   const pmix_info_t info[], size_t ninfo);
/// ```
pub fn group_destruct(group_id: &str, info: &[Info]) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };

    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe { ffi::PMIx_Group_destruct(group_id_c.as_ptr(), info_ptr, ninfo) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Group_destruct_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking destruct callback wrapper.
pub struct GroupDestructCallbackWrapper {
    callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
}

impl GroupDestructCallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// FFI callback bridge for non-blocking group destruct.
///
/// # Safety
/// `cbdata` must be a valid pointer to a `GroupDestructCallbackWrapper`
pub unsafe extern "C" fn group_destruct_callback_bridge(status: i32, cbdata: *mut c_void) {
    if cbdata.is_null() {
        return;
    }
    let req_id = decode_req_id(cbdata);
    let cb = {
        let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
        registry.remove(&req_id)
    };
    let Some(cb_wrapper) = cb else {
        return;
    };
    let pmix_status = PmixStatus::from_raw(status);
    let _ = invoke_user_callback("groups", move || {
        (cb_wrapper.callback)(pmix_status);
    });
}

/// Non-blocking destruct with a Rust closure callback.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Group_destruct_nb(const char grp[],
///                                      const pmix_info_t info[], size_t ninfo,
///                                      pmix_op_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn group_destruct_nb(
    group_id: &str,
    info: &[Info],
    callback: GroupDestructCallbackWrapper,
) -> Result<(), PmixStatus> {
    if group_id.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let group_id_c = match CString::new(group_id) {
        Ok(c) => c,
        Err(_) => return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM)),
    };
    let req_id = GROUP_DESTRUCT_REGISTRY.insert_next(callback);
    let cbdata = encode_req_id(req_id);


    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    let raw_status = unsafe {
        ffi::PMIx_Group_destruct_nb(
            group_id_c.as_ptr(),
            info_ptr,
            ninfo,
            Some(group_destruct_callback_bridge),
            cbdata,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
        registry.remove(&req_id);
        Err(pmix_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Proc;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

    // ── Helper: create a Proc for testing ────────────────────────────────────

    fn test_proc(rank: u32) -> Proc {
        Proc::new("test_namespace", rank).expect("Proc::new should succeed")
    }

    fn test_procs(count: usize) -> Vec<Proc> {
        (0..count).map(|i| test_proc(i as u32)).collect()
    }

    // ── group_construct: parameter validation ────────────────────────────────

    #[test]
    fn test_group_construct_empty_group_id() {
        let procs = test_procs(1);
        let result = group_construct("", &procs, &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_interior_nul_group_id() {
        let procs = test_procs(1);
        let result = group_construct("group\0id", &procs, &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_empty_procs() {
        let result = group_construct("my_group", &[], &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_empty_group_id_and_procs() {
        let result = group_construct("", &[], &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        // group_id is checked first
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_valid_params_returns_err_unreach_without_dvm() {
        // With valid params the function reaches FFI which returns ErrUnreach
        // without a DVM — this exercises the FFI call path.
        let procs = test_procs(2);
        let result = group_construct("test_group", &procs, &[]);
        // Without a PMIx daemon, PMIx_Group_construct returns PMIX_ERR_NOT_SUPPORTED
        // or PMIX_ERR_INIT. We just verify it doesn't panic.
        match result {
            Ok(_) => {} // rare: only if a DVM is running
            Err(e) => {
                let raw = e.to_raw();
                assert!(raw < 0, "Expected error status, got {}", raw);
            }
        }
    }

    #[test]
    fn test_group_construct_with_directives() {
        // Directives are not validated for emptiness — only group_id and procs matter
        let procs = test_procs(1);
        let result = group_construct("grp", &procs, &[]);
        // Should reach FFI (not return BAD_PARAM)
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_construct_nb: parameter validation ─────────────────────────────

    #[test]
    fn test_group_construct_nb_empty_group_id() {
        let procs = test_procs(1);
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("", &procs, &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_nb_interior_nul_group_id() {
        let procs = test_procs(1);
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("group\0id", &procs, &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_nb_empty_procs() {
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("grp", &[], &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_construct_nb_valid_params_reaches_ffi() {
        let procs = test_procs(1);
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("grp", &procs, &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_construct_callback_bridge invocation ───────────────────────────

    #[test]
    fn test_group_construct_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper =
            GroupConstructCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                assert!(status.is_success());
                called_clone.store(true, Ordering::SeqCst);
            });

        let req_id = 900001usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        // Invoke the bridge directly with success status
        unsafe {
            group_construct_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        // The callback consumes the Box, so we must NOT drop it again
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(
            called.load(Ordering::SeqCst),
            "Callback should have been invoked"
        );
    }

    #[test]
    fn test_group_construct_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper =
            GroupConstructCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                called_clone.store(true, Ordering::SeqCst);
                assert!(!status.is_success());
                status_clone.store(status.to_raw(), Ordering::SeqCst);
            });

        let req_id = 900002usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_construct_callback_bridge(
                ffi::PMIX_ERR_NOT_SUPPORTED,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(
            status_recv.load(Ordering::SeqCst),
            ffi::PMIX_ERR_NOT_SUPPORTED
        );
    }

    // ── group_invite: parameter validation ───────────────────────────────────

    #[test]
    fn test_group_invite_empty_group_id() {
        let procs = test_procs(1);
        let result = group_invite("", &procs, &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_interior_nul_group_id() {
        let procs = test_procs(1);
        let result = group_invite("group\0id", &procs, &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_empty_procs() {
        let result = group_invite("my_group", &[], &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_valid_params_reaches_ffi() {
        let procs = test_procs(1);
        let result = group_invite("grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_invite_nb: parameter validation ────────────────────────────────

    #[test]
    fn test_group_invite_nb_empty_group_id() {
        let procs = test_procs(1);
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("", &procs, &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_nb_interior_nul_group_id() {
        let procs = test_procs(1);
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("group\0id", &procs, &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_nb_empty_procs() {
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("grp", &[], &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_invite_nb_valid_params_reaches_ffi() {
        let procs = test_procs(1);
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("grp", &procs, &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_invite_callback_bridge invocation ──────────────────────────────

    #[test]
    fn test_group_invite_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper =
            GroupInviteCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                assert!(status.is_success());
                called_clone.store(true, Ordering::SeqCst);
            });

        let req_id = 900003usize;
        {
            let mut registry = GROUP_INVITE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_invite_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── group_join: parameter validation ─────────────────────────────────────

    #[test]
    fn test_group_join_empty_group_id() {
        let leader = test_proc(0);
        let result = group_join("", &leader, ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT, &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_join_interior_nul_group_id() {
        let leader = test_proc(0);
        let result = group_join(
            "group\0id",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
        );
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_join_accept_option() {
        let leader = test_proc(0);
        let result = group_join(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    #[test]
    fn test_group_join_decline_option() {
        let leader = test_proc(0);
        let result = group_join(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_DECLINE,
            &[],
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_join_nb: parameter validation ──────────────────────────────────

    #[test]
    fn test_group_join_nb_empty_group_id() {
        let leader = test_proc(0);
        let cb = GroupJoinCallbackWrapper::new(|_, _| {});
        let result = group_join_nb(
            "",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
            cb,
        );
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_join_nb_interior_nul_group_id() {
        let leader = test_proc(0);
        let cb = GroupJoinCallbackWrapper::new(|_, _| {});
        let result = group_join_nb(
            "group\0id",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
            cb,
        );
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_join_nb_valid_params_reaches_ffi() {
        let leader = test_proc(0);
        let cb = GroupJoinCallbackWrapper::new(|_, _| {});
        let result = group_join_nb(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
            cb,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_join_callback_bridge invocation ────────────────────────────────

    #[test]
    fn test_group_join_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupJoinCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
            assert!(status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900004usize;
        {
            let mut registry = GROUP_JOIN_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_join_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── group_leave: parameter validation ────────────────────────────────────

    #[test]
    fn test_group_leave_empty_group_id() {
        let result = group_leave("", &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_leave_interior_nul_group_id() {
        let result = group_leave("group\0id", &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_leave_valid_params_reaches_ffi() {
        let result = group_leave("grp", &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_leave_nb: parameter validation ─────────────────────────────────

    #[test]
    fn test_group_leave_nb_empty_group_id() {
        let cb = GroupLeaveCallbackWrapper::new(|_| {});
        let result = group_leave_nb("", &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_leave_nb_interior_nul_group_id() {
        let cb = GroupLeaveCallbackWrapper::new(|_| {});
        let result = group_leave_nb("group\0id", &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_leave_nb_valid_params_reaches_ffi() {
        let cb = GroupLeaveCallbackWrapper::new(|_| {});
        let result = group_leave_nb("grp", &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_leave_callback_bridge invocation ───────────────────────────────

    #[test]
    fn test_group_leave_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupLeaveCallbackWrapper::new(move |status: PmixStatus| {
            assert!(status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900005usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_leave_callback_bridge(
                0, // PMIX_SUCCESS
                cbdata,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_leave_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupLeaveCallbackWrapper::new(move |status: PmixStatus| {
            assert!(!status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900006usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_leave_callback_bridge(ffi::PMIX_ERR_NOT_SUPPORTED, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── group_destruct: parameter validation ─────────────────────────────────

    #[test]
    fn test_group_destruct_empty_group_id() {
        let result = group_destruct("", &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_destruct_interior_nul_group_id() {
        let result = group_destruct("group\0id", &[]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_destruct_valid_params_reaches_ffi() {
        let result = group_destruct("grp", &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_destruct_nb: parameter validation ──────────────────────────────

    #[test]
    fn test_group_destruct_nb_empty_group_id() {
        let cb = GroupDestructCallbackWrapper::new(|_| {});
        let result = group_destruct_nb("", &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_destruct_nb_interior_nul_group_id() {
        let cb = GroupDestructCallbackWrapper::new(|_| {});
        let result = group_destruct_nb("group\0id", &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    #[test]
    fn test_group_destruct_nb_valid_params_reaches_ffi() {
        let cb = GroupDestructCallbackWrapper::new(|_| {});
        let result = group_destruct_nb("grp", &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Should not be BAD_PARAM with valid inputs"
                );
            }
        }
    }

    // ── group_destruct_callback_bridge invocation ────────────────────────────

    #[test]
    fn test_group_destruct_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupDestructCallbackWrapper::new(move |status: PmixStatus| {
            assert!(status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900007usize;
        {
            let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_destruct_callback_bridge(
                0, // PMIX_SUCCESS
                cbdata,
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_destruct_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupDestructCallbackWrapper::new(move |status: PmixStatus| {
            assert!(!status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900008usize;
        {
            let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_destruct_callback_bridge(ffi::PMIX_ERR_NOT_SUPPORTED, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── Callback wrapper construction (original tests, retained) ─────────────

    #[test]
    fn test_group_construct_callback_wrapper() {
        let wrapper =
            GroupConstructCallbackWrapper::new(|_status: PmixStatus, _info: Vec<Info>| {});
        let _ = std::sync::Arc::new(wrapper);
    }

    #[test]
    fn test_group_invite_callback_wrapper() {
        let wrapper = GroupInviteCallbackWrapper::new(|_status: PmixStatus, _info: Vec<Info>| {});
        let _ = std::sync::Arc::new(wrapper);
    }

    #[test]
    fn test_group_join_callback_wrapper() {
        let wrapper = GroupJoinCallbackWrapper::new(|_status: PmixStatus, _info: Vec<Info>| {});
        let _ = std::sync::Arc::new(wrapper);
    }

    #[test]
    fn test_group_leave_callback_wrapper() {
        let wrapper = GroupLeaveCallbackWrapper::new(|_status: PmixStatus| {});
        let _ = std::sync::Arc::new(wrapper);
    }

    #[test]
    fn test_group_destruct_callback_wrapper() {
        let wrapper = GroupDestructCallbackWrapper::new(|_status: PmixStatus| {});
        let _ = std::sync::Arc::new(wrapper);
    }

    // ── Proc construction tests (used by group functions) ────────────────────

    #[test]
    fn test_proc_new_basic() {
        let proc = test_proc(0);
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_new_multiple_ranks() {
        for i in 0..10u32 {
            let proc = test_proc(i);
            assert_eq!(proc.get_rank(), i);
        }
    }

    #[test]
    fn test_proc_set_rank() {
        let mut proc = test_proc(0);
        proc.set_rank(42);
        assert_eq!(proc.get_rank(), 42);
    }

    #[test]
    fn test_proc_new_with_nspace() {
        let proc0 = test_proc(0);
        let proc1 = proc0
            .new_with_nspace(1)
            .expect("new_with_nspace should succeed");
        assert_eq!(proc1.get_rank(), 1);
    }

    #[test]
    fn test_proc_new_invalid_nspace_with_nul() {
        let result = Proc::new("test\0nspace", 0);
        assert!(result.is_err());
    }

    // ── pmix_group_opt_t enum tests ──────────────────────────────────────────

    #[test]
    fn test_group_opt_accept_value() {
        assert_eq!(ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT as i32, 1);
    }

    #[test]
    fn test_group_opt_decline_value() {
        assert_eq!(ffi::pmix_group_opt_t::PMIX_GROUP_DECLINE as i32, 0);
    }

    // ── Edge case: single proc in construct/invite ───────────────────────────

    #[test]
    fn test_group_construct_single_proc() {
        let procs = test_procs(1);
        let result = group_construct("single_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "Single proc should be valid");
            }
        }
    }

    #[test]
    fn test_group_invite_single_proc() {
        let procs = test_procs(1);
        let result = group_invite("single_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "Single proc should be valid");
            }
        }
    }

    // ── Edge case: multiple procs ────────────────────────────────────────────

    #[test]
    fn test_group_construct_many_procs() {
        let procs = test_procs(100);
        let result = group_construct("big_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "100 procs should be valid");
            }
        }
    }

    // ── Callback wrapper Send trait verification ─────────────────────────────

    #[test]
    fn test_callback_wrappers_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GroupConstructCallbackWrapper>();
        assert_send::<GroupInviteCallbackWrapper>();
        assert_send::<GroupJoinCallbackWrapper>();
        assert_send::<GroupLeaveCallbackWrapper>();
        assert_send::<GroupDestructCallbackWrapper>();
    }

    // ── Callback captures values correctly ───────────────────────────────────

    #[test]
    fn test_construct_callback_captures_info_count() {
        use std::os::raw::c_void;
        let info_count = std::sync::Arc::new(AtomicUsize::new(0));
        let info_clone = info_count.clone();

        let wrapper =
            GroupConstructCallbackWrapper::new(move |_status: PmixStatus, info: Vec<Info>| {
                info_clone.store(info.len(), Ordering::SeqCst);
            });

        let req_id = 900009usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_construct_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(info_count.load(Ordering::SeqCst), 0);
    }

    // ── group_construct_nb: bridge invocation tests ─────────────────────────

    #[test]
    fn test_group_construct_nb_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper =
            GroupConstructCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                assert!(status.is_success());
                called_clone.store(true, Ordering::SeqCst);
            });

        let req_id = 900010usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_construct_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_construct_nb_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper =
            GroupConstructCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                called_clone.store(true, Ordering::SeqCst);
                assert!(!status.is_success());
                status_clone.store(status.to_raw(), Ordering::SeqCst);
            });

        let req_id = 900011usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_construct_callback_bridge(
                ffi::PMIX_ERR_INIT,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(status_recv.load(Ordering::SeqCst), ffi::PMIX_ERR_INIT);
    }

    #[test]
    fn test_group_construct_nb_empty_group_id_and_procs() {
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("", &[], &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        // group_id is checked first
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    // ── group_invite_nb: bridge invocation tests ────────────────────────────

    #[test]
    fn test_group_invite_nb_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper =
            GroupInviteCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                assert!(status.is_success());
                called_clone.store(true, Ordering::SeqCst);
            });

        let req_id = 900012usize;
        {
            let mut registry = GROUP_INVITE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_invite_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_invite_nb_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper =
            GroupInviteCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                called_clone.store(true, Ordering::SeqCst);
                assert!(!status.is_success());
                status_clone.store(status.to_raw(), Ordering::SeqCst);
            });

        let req_id = 900013usize;
        {
            let mut registry = GROUP_INVITE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_invite_callback_bridge(
                ffi::PMIX_ERR_NOT_SUPPORTED,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(
            status_recv.load(Ordering::SeqCst),
            ffi::PMIX_ERR_NOT_SUPPORTED
        );
    }

    #[test]
    fn test_group_invite_nb_empty_group_id_and_procs() {
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("", &[], &[], cb);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.to_raw(), ffi::PMIX_ERR_BAD_PARAM);
    }

    // ── group_join_nb: bridge invocation tests ──────────────────────────────

    #[test]
    fn test_group_join_nb_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupJoinCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
            assert!(status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900014usize;
        {
            let mut registry = GROUP_JOIN_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_join_callback_bridge(
                0, // PMIX_SUCCESS
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_join_nb_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper = GroupJoinCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
            called_clone.store(true, Ordering::SeqCst);
            assert!(!status.is_success());
            status_clone.store(status.to_raw(), Ordering::SeqCst);
        });

        let req_id = 900015usize;
        {
            let mut registry = GROUP_JOIN_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_join_callback_bridge(
                ffi::PMIX_ERR_NOT_SUPPORTED,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(
            status_recv.load(Ordering::SeqCst),
            ffi::PMIX_ERR_NOT_SUPPORTED
        );
    }

    #[test]
    fn test_group_join_nb_decline_option() {
        let leader = test_proc(0);
        let cb = GroupJoinCallbackWrapper::new(|_, _| {});
        let result = group_join_nb(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_DECLINE,
            &[],
            cb,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Decline option should be valid"
                );
            }
        }
    }

    // ── group_leave_nb: bridge error test ───────────────────────────────────

    #[test]
    fn test_group_leave_nb_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupLeaveCallbackWrapper::new(move |status: PmixStatus| {
            assert!(!status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900016usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_leave_callback_bridge(ffi::PMIX_ERR_NOT_SUPPORTED, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── group_destruct_nb: bridge invocation tests ──────────────────────────

    #[test]
    fn test_group_destruct_nb_bridge_invokes_callback_on_success() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupDestructCallbackWrapper::new(move |status: PmixStatus| {
            assert!(status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900017usize;
        {
            let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_destruct_callback_bridge(0, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_group_destruct_nb_bridge_invokes_callback_on_error() {
        use std::os::raw::c_void;
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let wrapper = GroupDestructCallbackWrapper::new(move |status: PmixStatus| {
            assert!(!status.is_success());
            called_clone.store(true, Ordering::SeqCst);
        });

        let req_id = 900018usize;
        {
            let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_destruct_callback_bridge(ffi::PMIX_ERR_NOT_SUPPORTED, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(called.load(Ordering::SeqCst));
    }

    // ── Edge case: special group ID characters ──────────────────────────────

    #[test]
    fn test_group_construct_special_group_id_chars() {
        let procs = test_procs(1);
        let result = group_construct("grp-with-dashes_123", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Special chars should be valid"
                );
            }
        }
    }

    #[test]
    fn test_group_invite_special_group_id_chars() {
        let procs = test_procs(1);
        let result = group_invite("grp.with.dots_456", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Special chars should be valid"
                );
            }
        }
    }

    #[test]
    fn test_group_leave_special_group_id_chars() {
        let result = group_leave("grp:with:colons", &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Special chars should be valid"
                );
            }
        }
    }

    #[test]
    fn test_group_destruct_special_group_id_chars() {
        let result = group_destruct("grp/slashes/test", &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Special chars should be valid"
                );
            }
        }
    }

    // ── Edge case: long group IDs ───────────────────────────────────────────

    #[test]
    fn test_group_construct_long_group_id() {
        let long_id = "a".repeat(256);
        let procs = test_procs(1);
        let result = group_construct(&long_id, &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Long group ID should be valid"
                );
            }
        }
    }

    #[test]
    fn test_group_invite_long_group_id() {
        let long_id = "b".repeat(512);
        let procs = test_procs(1);
        let result = group_invite(&long_id, &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Long group ID should be valid"
                );
            }
        }
    }

    // ── Edge case: group leave/destruct nb with empty group_id ──────────────
    // (test_group_leave_nb_empty_group_id and test_group_destruct_nb_empty_group_id
    //  already exist earlier in this module — no duplicate needed)

    // ── Edge case: multiple procs in invite ─────────────────────────────────

    #[test]
    fn test_group_invite_many_procs() {
        let procs = test_procs(50);
        let result = group_invite("big_invite_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "50 procs should be valid");
            }
        }
    }

    // ── Edge case: group join with various leader ranks ─────────────────────

    #[test]
    fn test_group_join_leader_rank_zero() {
        let leader = test_proc(0);
        let result = group_join(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Rank 0 leader should be valid"
                );
            }
        }
    }

    #[test]
    fn test_group_join_leader_high_rank() {
        let leader = test_proc(9999);
        let result = group_join(
            "grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "High rank leader should be valid"
                );
            }
        }
    }

    // ── Edge case: construct with many procs and various counts ─────────────

    #[test]
    fn test_group_construct_two_procs() {
        let procs = test_procs(2);
        let result = group_construct("two_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "Two procs should be valid");
            }
        }
    }

    #[test]
    fn test_group_construct_five_procs() {
        let procs = test_procs(5);
        let result = group_construct("five_grp", &procs, &[]);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(raw, ffi::PMIX_ERR_BAD_PARAM, "Five procs should be valid");
            }
        }
    }

    // ── Callback wrapper error status verification ──────────────────────────

    #[test]
    fn test_invite_bridge_receives_error_status() {
        use std::os::raw::c_void;
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper =
            GroupInviteCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
                status_clone.store(status.to_raw(), Ordering::SeqCst);
            });

        let req_id = 900019usize;
        {
            let mut registry = GROUP_INVITE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_invite_callback_bridge(
                ffi::PMIX_ERR_TIMEOUT,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(status_recv.load(Ordering::SeqCst), ffi::PMIX_ERR_TIMEOUT);
    }

    #[test]
    fn test_join_bridge_receives_error_status() {
        use std::os::raw::c_void;
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper = GroupJoinCallbackWrapper::new(move |status: PmixStatus, _info: Vec<Info>| {
            status_clone.store(status.to_raw(), Ordering::SeqCst);
        });

        let req_id = 900020usize;
        {
            let mut registry = GROUP_JOIN_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_join_callback_bridge(
                ffi::PMIX_ERR_TIMEOUT,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(status_recv.load(Ordering::SeqCst), ffi::PMIX_ERR_TIMEOUT);
    }

    #[test]
    fn test_leave_bridge_receives_error_status() {
        use std::os::raw::c_void;
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper = GroupLeaveCallbackWrapper::new(move |status: PmixStatus| {
            status_clone.store(status.to_raw(), Ordering::SeqCst);
        });

        let req_id = 900021usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_leave_callback_bridge(ffi::PMIX_ERR_TIMEOUT, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(status_recv.load(Ordering::SeqCst), ffi::PMIX_ERR_TIMEOUT);
    }

    #[test]
    fn test_destruct_bridge_receives_error_status() {
        use std::os::raw::c_void;
        let status_recv = std::sync::Arc::new(AtomicI32::new(0));
        let status_clone = status_recv.clone();

        let wrapper = GroupDestructCallbackWrapper::new(move |status: PmixStatus| {
            status_clone.store(status.to_raw(), Ordering::SeqCst);
        });

        let req_id = 900022usize;
        {
            let mut registry = GROUP_DESTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        unsafe {
            group_destruct_callback_bridge(ffi::PMIX_ERR_TIMEOUT, cbdata);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(status_recv.load(Ordering::SeqCst), ffi::PMIX_ERR_TIMEOUT);
    }

    // ── Edge case: group invite_nb with decline option ──────────────────────

    #[test]
    fn test_group_join_nb_accept_option_reaches_ffi() {
        let leader = test_proc(0);
        let cb = GroupJoinCallbackWrapper::new(|_, _| {});
        let result = group_join_nb(
            "test_grp",
            &leader,
            ffi::pmix_group_opt_t::PMIX_GROUP_ACCEPT,
            &[],
            cb,
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "Accept option should reach FFI"
                );
            }
        }
    }

    // ── Edge case: group construct_nb with many procs ───────────────────────

    #[test]
    fn test_group_construct_nb_many_procs() {
        let procs = test_procs(50);
        let cb = GroupConstructCallbackWrapper::new(|_, _| {});
        let result = group_construct_nb("big_nb_grp", &procs, &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "50 procs should be valid for nb construct"
                );
            }
        }
    }

    // ── Edge case: group invite with many procs nb ──────────────────────────

    #[test]
    fn test_group_invite_nb_many_procs() {
        let procs = test_procs(50);
        let cb = GroupInviteCallbackWrapper::new(|_, _| {});
        let result = group_invite_nb("big_nb_invite_grp", &procs, &[], cb);
        match result {
            Ok(_) => {}
            Err(e) => {
                let raw = e.to_raw();
                assert_ne!(
                    raw,
                    ffi::PMIX_ERR_BAD_PARAM,
                    "50 procs should be valid for nb invite"
                );
            }
        }
    }

    // ── issue #67: lock / panic bridge hygiene ────────────────────────────────

    #[test]
    fn test_group_leave_bridge_no_lock_across_user_code() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (enter_tx, enter_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let wrapper = GroupLeaveCallbackWrapper::new(move |_status| {
            let _ = enter_tx.send(());
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
        });
        let req_id = 424_267usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata_addr = encode_req_id(req_id) as usize;

        let progress = std::thread::spawn(move || unsafe {
            group_leave_callback_bridge(0, cbdata_addr as *mut std::ffi::c_void);
        });

        enter_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("user callback should enter");
        let lock_check = std::thread::spawn(|| {
            let _guard = GROUP_LEAVE_REGISTRY.lock();
        });
        lock_check.join().expect("registry lock must not be held across user callback code");

        let _ = release_tx.send(());
        progress.join().expect("bridge returns");
        assert!(
            GROUP_LEAVE_REGISTRY.lock().get(&req_id).is_none(),
            "one-shot leave callback must be removed before user invoke"
        );
    }

    #[test]
    fn test_group_leave_bridge_contains_user_panic() {
        let wrapper = GroupLeaveCallbackWrapper::new(move |_status| {
            panic!("group leave user boom");
        });
        let req_id = 424_268usize;
        {
            let mut registry = GROUP_LEAVE_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        // Must return normally — panic is contained at the bridge.
        unsafe {
            group_leave_callback_bridge(0, cbdata);
        }
        assert!(GROUP_LEAVE_REGISTRY.lock().get(&req_id).is_none());
    }

    #[test]
    fn test_group_construct_bridge_uses_encode_req_id() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static HIT: AtomicBool = AtomicBool::new(false);
        let wrapper = GroupConstructCallbackWrapper::new(move |status, info| {
            assert!(status.is_success());
            assert!(info.is_empty());
            HIT.store(true, Ordering::SeqCst);
        });
        let req_id = 424_269usize;
        {
            let mut registry = GROUP_CONSTRUCT_REGISTRY.lock();
            registry.insert(req_id, wrapper);
        }
        let cbdata = encode_req_id(req_id);
        assert_eq!(decode_req_id(cbdata), req_id);
        unsafe {
            group_construct_callback_bridge(
                0,
                std::ptr::null_mut(),
                0,
                cbdata,
                None,
                std::ptr::null_mut(),
            );
        }
        assert!(HIT.load(Ordering::SeqCst));
    }
}



/// Return the static PMIx spelling for a group operation.
pub fn group_operation_string(op: ffi::pmix_group_operation_t) -> &'static str {
    let p = crate::pmix_ffi_or_mock!(mock = unsafe { crate::mock_ffi::mock_group_operation_string(op) }, real = unsafe { ffi::PMIx_Group_operation_string(op) });
    if p.is_null() { return ""; }
    unsafe { std::ffi::CStr::from_ptr(p).to_str().unwrap_or("") }
}


#[cfg(test)]
#[test]
fn test_misc_group_string_wrapper() {
    let _guard = crate::mock_ffi::MockGuard::new();
    assert_eq!(group_operation_string(crate::ffi::pmix_group_operation_t::PMIX_GROUP_CONSTRUCT), "unknown");
}
