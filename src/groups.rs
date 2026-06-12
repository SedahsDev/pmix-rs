//! Group management — `PMIx_Group_construct`, `PMIx_Group_construct_nb`,
//! `PMIx_Group_invite`, `PMIx_Group_invite_nb`, `PMIx_Group_join`,
//! `PMIx_Group_join_nb`, `PMIx_Group_leave`, `PMIx_Group_leave_nb`,
//! `PMIx_Group_destruct`, `PMIx_Group_destruct_nb`.
//!
//! This module provides safe Rust wrappers around the PMIx group
//! management APIs.

use crate::ffi;
use crate::{Info, PmixStatus, Proc};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;

/// Re-export of the PMIx group accept/decline option enum.
///
/// Used by `group_join` and `group_join_nb` to specify whether
/// to accept or decline a group invitation.
pub use ffi::pmix_group_opt_t;

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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

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
                });
            }
            results = ptr::null_mut();
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

    let cb_box: *mut GroupConstructCallbackWrapper = Box::into_raw(Box::new(callback));

    extern "C" fn group_construct_callback_bridge(
        status: i32,
        info: *mut ffi::pmix_info_t,
        ninfo: usize,
        cbdata: *mut c_void,
        _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
        _release_cbdata: *mut c_void,
    ) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut GroupConstructCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);

        let rust_results: Vec<Info> = if pmix_status.is_success() {
            unsafe {
                if info.is_null() || ninfo == 0 {
                    Vec::new()
                } else {
                    let mut vec = Vec::with_capacity(ninfo);
                    for i in 0..ninfo {
                        vec.push(Info {
                            handle: info.add(i),
                            len: 1,
                        });
                    }
                    vec
                }
            }
        } else {
            Vec::new()
        };

        (cb_wrapper.callback)(pmix_status, rust_results);
    }

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
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe { drop(Box::from_raw(cb_box)) }
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

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
                });
            }
            results = ptr::null_mut();
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");
    let cb_box: *mut GroupInviteCallbackWrapper = Box::into_raw(Box::new(callback));

    extern "C" fn group_invite_callback_bridge(
        status: i32,
        info: *mut ffi::pmix_info_t,
        ninfo: usize,
        cbdata: *mut c_void,
        _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
        _release_cbdata: *mut c_void,
    ) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut GroupInviteCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);

        let rust_results: Vec<Info> = if pmix_status.is_success() {
            unsafe {
                if info.is_null() || ninfo == 0 {
                    Vec::new()
                } else {
                    let mut vec = Vec::with_capacity(ninfo);
                    for i in 0..ninfo {
                        vec.push(Info {
                            handle: info.add(i),
                            len: 1,
                        });
                    }
                    vec
                }
            }
        } else {
            Vec::new()
        };

        (cb_wrapper.callback)(pmix_status, rust_results);
    }

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
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe { drop(Box::from_raw(cb_box)) }
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

    let leader_ptr = unsafe {
        std::ptr::addr_of!((*(&*leader as *const Proc)).handle) as *const ffi::pmix_proc_t
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
                });
            }
            results = ptr::null_mut();
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");
    let cb_box: *mut GroupJoinCallbackWrapper = Box::into_raw(Box::new(callback));

    extern "C" fn group_join_callback_bridge(
        status: i32,
        info: *mut ffi::pmix_info_t,
        ninfo: usize,
        cbdata: *mut c_void,
        _release_fn: Option<unsafe extern "C" fn(*mut c_void)>,
        _release_cbdata: *mut c_void,
    ) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut GroupJoinCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);

        let rust_results: Vec<Info> = if pmix_status.is_success() {
            unsafe {
                if info.is_null() || ninfo == 0 {
                    Vec::new()
                } else {
                    let mut vec = Vec::with_capacity(ninfo);
                    for i in 0..ninfo {
                        vec.push(Info {
                            handle: info.add(i),
                            len: 1,
                        });
                    }
                    vec
                }
            }
        } else {
            Vec::new()
        };

        (cb_wrapper.callback)(pmix_status, rust_results);
    }

    let leader_ptr = unsafe {
        std::ptr::addr_of!((*(&*leader as *const Proc)).handle) as *const ffi::pmix_proc_t
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
        ffi::PMIx_Group_join_nb(
            group_id_c.as_ptr(),
            leader_ptr,
            option,
            info_ptr,
            ninfo,
            Some(group_join_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe { drop(Box::from_raw(cb_box)) }
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");
    let cb_box: *mut GroupLeaveCallbackWrapper = Box::into_raw(Box::new(callback));

    extern "C" fn group_leave_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut GroupLeaveCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
    }

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
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe { drop(Box::from_raw(cb_box)) }
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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");

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

    let group_id_c = CString::new(group_id).expect("group_id must not contain interior NUL bytes");
    let cb_box: *mut GroupDestructCallbackWrapper = Box::into_raw(Box::new(callback));

    extern "C" fn group_destruct_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut GroupDestructCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
    }

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
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        unsafe { drop(Box::from_raw(cb_box)) }
        Err(pmix_status)
    }
}
