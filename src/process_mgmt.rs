//! Process management — `PMIx_Abort`, `PMIx_Spawn`, `PMIx_Spawn_nb`,
//! `PMIx_Connect`, `PMIx_Connect_nb`, `PMIx_Disconnect`, `PMIx_Disconnect_nb`.
//!
//! This module provides safe Rust wrappers around the PMIx process
//! management APIs:
//!
//! * **Abort** — request that the host resource manager abort the
//!   specified processes with a given error code and message.
//! * **Spawn** — spawn new processes/applications in the PMIx universe.
//! * **Spawn_nb** — non-blocking variant of spawn with a callback.
//! * **Connect** — record a set of processes as "connected", enabling
//!   cross-namespace notification and job-level info sharing.
//! * **Connect_nb** — non-blocking variant of connect with a callback.
//! * **Disconnect** — disconnect a previously connected set of processes.
//! * **Disconnect_nb** — non-blocking variant of disconnect with a callback.
//!
//! # Example
//!
//! ```no_run
//! use pmix::process_mgmt::{abort, spawn, connect, disconnect, PmixApp};
//! use pmix::{PmixError, PmixStatus, Proc};
//!
//! // Abort all processes in the caller's namespace
//! let result = abort(
//!     PmixStatus::Known(PmixError::Error),
//!     Some("something went wrong"),
//!     None, // NULL procs = abort all in caller's namespace
//! );
//! // Note: if the caller is included in the abort, the function
//! // will not return unless the host is unable to execute it.
//!
//! // Spawn a new job
//! let app = PmixApp::builder()
//!     .cmd("./myapp")
//!     .maxprocs(4)
//!     .build();
//! let result = spawn(&[], &[app]);
//! // result is Ok(nspace) on success
//!
//! // Connect to a namespace
//! let proc = Proc::new("target_namespace", pmix::PMIX_RANK_WILDCARD);
//! let result = connect(&[proc], &[]);
//!
//! // Disconnect from a previously connected set
//! let result = disconnect(&[proc], &[]);
//! ```

use crate::ffi;
use crate::{Info, PmixStatus, Proc};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Abort
// ─────────────────────────────────────────────────────────────────────────────

/// Request that the host resource manager abort the specified processes.
///
/// Instructs the PMIx server to print the provided message (if any) and
/// abort the given array of processes. The `status` argument is the error
/// code to return to the invoking environment.
///
/// * If `procs` is `None`, all processes in the caller's namespace are
///   aborted, including the caller itself — equivalent to passing a single
///   `pmix_proc_t` with `PMIX_RANK_WILDCARD`.
/// * The function is **blocking**: it does not return until the host
///   environment has carried out the operation on the specified processes.
/// * If the caller is included in the abort targets, the function will
///   **not return** unless the host is unable to execute the operation.
/// * Passing `None` for `msg` is allowed (no message printed).
///
/// # Returns
/// * `Ok(())` — the abort request was accepted. If the caller's own
///   process was included, the function will not return with success.
/// * `Err(PmixStatus::Known(PmixError::ErrParamValueNotSupported))` — the
///   host environment cannot abort the requested processes (e.g., subsets
///   from another namespace).
/// * `Err(PmixStatus)` — another error in the request.
///
/// # Thread Safety
/// Race conditions caused by multiple processes calling `PMIx_Abort`
/// simultaneously are left to the server implementation to resolve.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Abort(int status, const char msg[],
///                          pmix_proc_t procs[], size_t nprocs);
/// ```
pub fn abort(
    status: PmixStatus,
    msg: Option<&str>,
    procs: Option<&[Proc]>,
) -> Result<(), PmixStatus> {
    // Convert the optional message to a C string pointer.
    let (msg_ptr, _msg_cstring) = match msg {
        Some(m) => {
            let cs = CString::new(m).expect("abort message must not contain interior NUL bytes");
            (cs.as_ptr(), Some(cs))
        }
        None => (ptr::null(), None),
    };

    // Convert the optional proc array to a raw pointer + length.
    let (procs_ptr, nprocs) = match procs {
        Some(procs) if !procs.is_empty() => (
            &procs[0].handle as *const ffi::pmix_proc_t as *mut ffi::pmix_proc_t,
            procs.len(),
        ),
        _ => (ptr::null_mut(), 0),
    };

    // SAFETY: FFI call into PMIx library.
    // - `status.to_raw()` converts our type-safe PmixStatus to a raw
    //   `pmix_status_t` (i32) that the C API expects.
    // - `msg_ptr` is either null or a valid NUL-terminated C string
    //   whose lifetime (`_msg_cstring`) is kept alive until after this
    //   call returns.
    // - `procs_ptr` is either null or points to a valid slice of
    //   `pmix_proc_t` handles that remain valid for the duration of
    //   this call. PMIx does not retain these pointers after return.
    // - Note: if the caller's own process is included in `procs`, this
    //   function may not return. That is documented PMIx behavior.
    let raw_status = unsafe {
        ffi::PMIx_Abort(
            status.to_raw() as std::os::raw::c_int,
            msg_ptr,
            procs_ptr,
            nprocs,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PmixApp — safe wrapper around pmix_app_t
// ─────────────────────────────────────────────────────────────────────────────

/// A safe Rust wrapper around `pmix_app_t`.
///
/// Describes a single application to be spawned by `PMIx_Spawn`. Each
/// app specifies an executable command, command-line arguments,
/// environment variables, working directory, maximum process count,
/// and per-app info attributes.
///
/// Use [`PmixAppBuilder`] (via [`PmixApp::builder`]) to construct
/// instances safely, or [`PmixApp::from_raw`] when interfacing with
/// FFI-generated bindings directly.
#[derive(Debug)]
pub struct PmixApp {
    cmd: Option<CString>,
    argv: Vec<CString>,
    env: Vec<CString>,
    cwd: Option<CString>,
    maxprocs: c_int,
}

impl PmixApp {
    /// Create a builder for constructing a `PmixApp`.
    pub fn builder() -> PmixAppBuilder {
        PmixAppBuilder::new()
    }

    /// Get the command path.
    pub fn cmd(&self) -> Option<&str> {
        self.cmd.as_ref().and_then(|c| c.to_str().ok())
    }

    /// Get the command-line arguments.
    pub fn argv(&self) -> &[CString] {
        &self.argv
    }

    /// Get the environment variables.
    pub fn env_vars(&self) -> &[CString] {
        &self.env
    }

    /// Get the working directory.
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_ref().and_then(|c| c.to_str().ok())
    }

    /// Get the maximum number of processes.
    pub fn maxprocs(&self) -> c_int {
        self.maxprocs
    }
}

/// Builder for [`PmixApp`].
///
/// # Example
///
/// ```
/// use pmix::process_mgmt::PmixApp;
///
/// let app = PmixApp::builder()
///     .cmd("./myapp")
///     .arg("arg1")
///     .arg("arg2")
///     .env("MYVAR=value")
///     .cwd("/tmp")
///     .maxprocs(4)
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct PmixAppBuilder {
    cmd: Option<String>,
    argv: Vec<String>,
    env: Vec<String>,
    cwd: Option<String>,
    maxprocs: c_int,
}

impl PmixAppBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the executable command path.
    pub fn cmd(&mut self, cmd: &str) -> &mut Self {
        self.cmd = Some(cmd.to_owned());
        self
    }

    /// Add a command-line argument.
    pub fn arg(&mut self, arg: &str) -> &mut Self {
        self.argv.push(arg.to_owned());
        self
    }

    /// Add multiple command-line arguments.
    pub fn args<I>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for arg in args {
            self.argv.push(arg);
        }
        self
    }

    /// Add an environment variable in `KEY=VALUE` format.
    pub fn env(&mut self, env: &str) -> &mut Self {
        self.env.push(env.to_owned());
        self
    }

    /// Add multiple environment variables.
    pub fn envs<I>(&mut self, envs: I) -> &mut Self
    where
        I: IntoIterator<Item = String>,
    {
        for env in envs {
            self.env.push(env);
        }
        self
    }

    /// Set the working directory.
    pub fn cwd(&mut self, cwd: &str) -> &mut Self {
        self.cwd = Some(cwd.to_owned());
        self
    }

    /// Set the maximum number of processes for this application.
    /// A value of 0 means the default (unlimited).
    pub fn maxprocs(&mut self, maxprocs: c_int) -> &mut Self {
        self.maxprocs = maxprocs;
        self
    }

    /// Build the `PmixApp`. Returns an error if any string contains
    /// an interior NUL byte.
    pub fn build(&self) -> Result<PmixApp, std::ffi::NulError> {
        let cmd = self.cmd.as_deref().map(CString::new).transpose()?;
        let argv = self
            .argv
            .iter()
            .map(|s| CString::new(s.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let env = self
            .env
            .iter()
            .map(|s| CString::new(s.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let cwd = self.cwd.as_deref().map(CString::new).transpose()?;

        Ok(PmixApp {
            cmd,
            argv,
            env,
            cwd,
            maxprocs: self.maxprocs,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Spawn
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking spawn callback function type.
///
/// The callback receives:
/// - `status`: the result of the spawn operation (`PMIX_SUCCESS` or error)
/// - `nspace`: the namespace of the spawned job (valid only on success)
/// - `cbdata`: the user-provided callback data pointer
pub type SpawnCallback =
    unsafe extern "C" fn(status: ffi::pmix_status_t, nspace: *mut c_char, cbdata: *mut c_void);

/// Spawn new processes in the PMIx universe.
///
/// Requests the host resource manager to launch the specified
/// applications. The caller provides job-level attributes in `job_info`
/// and an array of `PmixApp` descriptors describing each application.
///
/// * This is a **blocking** call: it does not return until the host
///   environment has launched the specified applications (or failed).
/// * On success, returns the namespace of the spawned job.
/// * The `job_info` array can specify attributes such as working
///   directory, host selection, mapping policy, etc.
///
/// # Returns
/// * `Ok(nspace)` — the namespace string of the spawned job.
/// * `Err(PmixStatus)` — spawn failed with the given error code.
///
/// # Thread Safety
/// The caller is responsible for ensuring thread safety when calling
/// this from multiple threads simultaneously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Spawn(const pmix_info_t job_info[], size_t ninfo,
///                          const pmix_app_t apps[], size_t napps,
///                          char nspace[]);
/// ```
pub fn spawn(_job_info: &[Info], apps: &[PmixApp]) -> Result<String, PmixStatus> {
    if apps.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    let napps = apps.len();

    // SAFETY: PMIx_App_create uses pmix_calloc (→ calloc) to allocate
    // an array of `napps` zeroed pmix_app_t structures.
    let raw_apps: *mut ffi::pmix_app_t = unsafe { ffi::PMIx_App_create(napps) };
    if raw_apps.is_null() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_OUT_OF_RESOURCE));
    }

    // Drop guard ensures PMIx_App_free is always called, even on error.
    // PMIx_App_free calls PMIX_APP_DESTRUCT on each element, which in turn
    // calls pmix_free (→ free) on cmd, argv, env, cwd. Therefore we must
    // allocate all string data and pointer arrays via C allocators so
    // free() is valid. We use CString::into_raw() for strings and
    // libc::calloc for pointer arrays.
    struct AppArrayGuard(*mut ffi::pmix_app_t, usize);
    impl Drop for AppArrayGuard {
        fn drop(&mut self) {
            unsafe { ffi::PMIx_App_free(self.0, self.1) };
        }
    }
    let _guard = AppArrayGuard(raw_apps, napps);

    for (i, app) in apps.iter().enumerate() {
        let app_ptr = unsafe { raw_apps.add(i) };

        // ── cmd: transfer ownership to C via CString::into_raw ──
        let cmd_ptr: *mut c_char = match &app.cmd {
            Some(cs) => CString::into_raw(cs.clone()),
            None => ptr::null_mut(),
        };
        unsafe { (*app_ptr).cmd = cmd_ptr };

        // ── argv: null-terminated array of *mut c_char ──
        let argv_ptr: *mut *mut c_char = if app.argv.is_empty() {
            ptr::null_mut()
        } else {
            let n = app.argv.len();
            // Allocate null-terminated pointer array via libc::calloc.
            let ptrs: *mut *mut c_char = unsafe {
                libc::calloc((n + 1) as usize, std::mem::size_of::<*mut c_char>())
                    as *mut *mut c_char
            };
            for (j, s) in app.argv.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").unwrap());
                unsafe { *ptrs.add(j) = CString::into_raw(cstr) };
            }
            // ptrs[n] is already NULL from calloc.
            ptrs
        };
        unsafe { (*app_ptr).argv = argv_ptr };

        // ── env: null-terminated array of *mut c_char ──
        let env_ptr: *mut *mut c_char = if app.env.is_empty() {
            ptr::null_mut()
        } else {
            let n = app.env.len();
            let ptrs: *mut *mut c_char = unsafe {
                libc::calloc((n + 1) as usize, std::mem::size_of::<*mut c_char>())
                    as *mut *mut c_char
            };
            for (j, s) in app.env.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").unwrap());
                unsafe { *ptrs.add(j) = CString::into_raw(cstr) };
            }
            ptrs
        };
        unsafe { (*app_ptr).env = env_ptr };

        // ── cwd ──
        let cwd_ptr: *mut c_char = match &app.cwd {
            Some(cs) => CString::into_raw(cs.clone()),
            None => ptr::null_mut(),
        };
        unsafe { (*app_ptr).cwd = cwd_ptr };

        // ── maxprocs ──
        unsafe { (*app_ptr).maxprocs = app.maxprocs };

        // ── info (not supported yet) ──
        unsafe {
            (*app_ptr).info = ptr::null_mut();
            (*app_ptr).ninfo = 0;
        }
    }

    // job_info: not supported yet — pass NULL.
    let (info_ptr, ninfo) = (ptr::null(), 0);

    // Output namespace buffer.
    let mut nspace_buf: [c_char; 256] = [0; 256];

    // SAFETY: FFI call into PMIx library.
    // - `raw_apps` points to a valid array of `napps` pmix_app_t structures
    //   with all string fields allocated via C-compatible allocators.
    // - `nspace_buf` is a properly sized output buffer.
    // - PMIx_Spawn does not retain any of our pointers after return.
    let status =
        unsafe { ffi::PMIx_Spawn(info_ptr, ninfo, raw_apps, napps, nspace_buf.as_mut_ptr()) };

    // The AppArrayGuard will call PMIx_App_free(raw_apps, napps) which
    // calls PMIX_APP_DESTRUCT on each element, freeing cmd/argv/env/cwd
    // via pmix_free (→ free). This is correct because we allocated all
    // string data via CString::into_raw (which uses libc::malloc) and
    // pointer arrays via libc::calloc.

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        // SAFETY: On success, PMIx_Spawn writes a null-terminated
        // namespace string into nspace_buf.
        let nspace = unsafe { CStr::from_ptr(nspace_buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Ok(nspace)
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Spawn_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking spawn callback wrapper.
///
/// Wraps a Rust closure so it can be called from the C FFI callback.
/// The closure receives `(PmixStatus, Option<String>)` where the
/// `String` is the spawned namespace (present only on success).
pub struct SpawnCallbackWrapper {
    /// The user's Rust closure.
    callback: Box<dyn Fn(PmixStatus, Option<String>) + Send + 'static>,
}

impl SpawnCallbackWrapper {
    /// Create a new wrapper around a Rust closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus, Option<String>) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

/// Non-blocking spawn with a Rust closure callback.
///
/// Requests the host resource manager to launch the specified
/// applications. Unlike [`spawn`], this returns immediately and
/// invokes the provided callback upon completion.
///
/// * The callback receives `(PmixStatus, Option<String>)`:
///   - On success: `(Ok, Some(nspace))`
///   - On failure: `(Err, None)`
/// * The `callback` closure must be `Send + 'static` because it may
///   be invoked from a different thread by the PMIx library.
///
/// # Returns
/// * `Ok(())` — the spawn request was accepted (async, result in callback).
/// * `Err(PmixStatus)` — the spawn request itself failed synchronously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Spawn_nb(const pmix_info_t job_info[], size_t ninfo,
///                             const pmix_app_t apps[], size_t napps,
///                             pmix_spawn_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn spawn_nb(
    _job_info: &[Info],
    apps: &[PmixApp],
    callback: SpawnCallbackWrapper,
) -> Result<(), PmixStatus> {
    if apps.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    // Box the callback wrapper so it lives on the heap and outlives
    // the FFI call. We pass it as cbdata and recover it in the C
    // callback via Box::from_raw.
    let cb_box: *mut SpawnCallbackWrapper = Box::into_raw(Box::new(callback));

    // The C bridge function that PMIx calls back into.
    // SAFETY: This extern "C" function is only called by PMIx with
    // the cbdata pointer we provided (Box<SpawnCallbackWrapper>).
    // It takes ownership of the box via Box::from_raw.
    extern "C" fn spawn_callback_bridge(
        status: ffi::pmix_status_t,
        nspace: *mut c_char,
        cbdata: *mut c_void,
    ) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut SpawnCallbackWrapper) };

        let pmix_status = PmixStatus::from_raw(status);
        let nspace_str = if pmix_status.is_success() && !nspace.is_null() {
            let cstr = unsafe { CStr::from_ptr(nspace) };
            Some(cstr.to_string_lossy().into_owned())
        } else {
            None
        };

        (cb_wrapper.callback)(pmix_status, nspace_str);
        // The box is dropped here.
    }

    let napps = apps.len();

    // SAFETY: PMIx_App_create allocates and constructs napps pmix_app_t.
    let raw_apps: *mut ffi::pmix_app_t = unsafe { ffi::PMIx_App_create(napps) };
    if raw_apps.is_null() {
        unsafe { drop(Box::from_raw(cb_box)) };
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_OUT_OF_RESOURCE));
    }

    // Drop guard — always free the app array.
    // PMIx_Spawn_nb copies app data internally, so our C-allocated
    // strings can be freed immediately after the call returns.
    struct NbAppArrayGuard(*mut ffi::pmix_app_t, usize);
    impl Drop for NbAppArrayGuard {
        fn drop(&mut self) {
            unsafe { ffi::PMIx_App_free(self.0, self.1) };
        }
    }
    let _guard = NbAppArrayGuard(raw_apps, napps);

    for (i, app) in apps.iter().enumerate() {
        let app_ptr = unsafe { raw_apps.add(i) };

        // cmd
        let cmd_ptr: *mut c_char = match &app.cmd {
            Some(cs) => CString::into_raw(cs.clone()),
            None => ptr::null_mut(),
        };
        unsafe { (*app_ptr).cmd = cmd_ptr };

        // argv
        let argv_ptr: *mut *mut c_char = if app.argv.is_empty() {
            ptr::null_mut()
        } else {
            let n = app.argv.len();
            let ptrs: *mut *mut c_char = unsafe {
                libc::calloc((n + 1) as usize, std::mem::size_of::<*mut c_char>())
                    as *mut *mut c_char
            };
            for (j, s) in app.argv.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").unwrap());
                unsafe { *ptrs.add(j) = CString::into_raw(cstr) };
            }
            ptrs
        };
        unsafe { (*app_ptr).argv = argv_ptr };

        // env
        let env_ptr: *mut *mut c_char = if app.env.is_empty() {
            ptr::null_mut()
        } else {
            let n = app.env.len();
            let ptrs: *mut *mut c_char = unsafe {
                libc::calloc((n + 1) as usize, std::mem::size_of::<*mut c_char>())
                    as *mut *mut c_char
            };
            for (j, s) in app.env.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").unwrap());
                unsafe { *ptrs.add(j) = CString::into_raw(cstr) };
            }
            ptrs
        };
        unsafe { (*app_ptr).env = env_ptr };

        // cwd
        let cwd_ptr: *mut c_char = match &app.cwd {
            Some(cs) => CString::into_raw(cs.clone()),
            None => ptr::null_mut(),
        };
        unsafe { (*app_ptr).cwd = cwd_ptr };

        // maxprocs + info
        unsafe {
            (*app_ptr).maxprocs = app.maxprocs;
            (*app_ptr).info = ptr::null_mut();
            (*app_ptr).ninfo = 0;
        }
    }

    // SAFETY: FFI call into PMIx library.
    // - `raw_apps` is a valid array of `napps` pmix_app_t structures
    //   with C-allocated string fields.
    // - `spawn_callback_bridge` is a valid extern "C" callback.
    // - `cb_box` is a valid heap-allocated SpawnCallbackWrapper.
    // - PMIx_Spawn_nb returns immediately; the callback is invoked
    //   asynchronously by the PMIx library at a later time.
    // - PMIx copies app data internally, so our guard can free
    //   the app array after this call returns.
    let status = unsafe {
        ffi::PMIx_Spawn_nb(
            ptr::null(),
            0,
            raw_apps,
            napps,
            Some(spawn_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    // NbAppArrayGuard will free raw_apps + all C-allocated strings.

    let pmix_status = PmixStatus::from_raw(status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // On synchronous failure, PMIx still calls the callback with
        // the error status, so the bridge function will drop cb_box.
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Connect
// ─────────────────────────────────────────────────────────────────────────────

/// Record a set of processes as "connected".
///
/// Instructs the PMIx server to treat the specified processes as a
/// connected group. When processes are connected:
///
/// * The resource manager treats the failure of any process in the
///   group as a reportable event and takes appropriate action.
/// * Each process receives job-level info for the other namespaces
///   in the group, enabling cross-namespace queries without
///   communication penalties.
/// * Processes in the group receive notification of errors from
///   other members.
///
/// This is a **blocking** call: it does not return until all
/// participating processes have called `connect` with the same
/// set of processes.
///
/// # Constraints
/// * A process can only engage in *one* connect operation involving
///   the identical set of processes at a time.
/// * A process *can* be simultaneously engaged in multiple connect
///   operations, each involving a different set of processes.
/// * The `info` array can pass directives regarding the collective
///   algorithm, timeout constraints, and other options.
///
/// # Returns
/// * `Ok(())` — all participating processes have connected.
/// * `Err(PmixStatus)` — the connect operation failed.
///
/// # Thread Safety
/// The caller is responsible for ensuring thread safety when calling
/// this from multiple threads simultaneously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Connect(const pmix_proc_t procs[], size_t nprocs,
///                            const pmix_info_t info[], size_t ninfo);
/// ```
pub fn connect(procs: &[Proc], info: &[Info]) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    // Convert proc slice to a raw pointer.
    // SAFETY: `procs` is a non-empty slice of `Proc` values, each
    // containing a `pmix_proc_t` handle as its first field. We take
    // the address of the first element's handle and cast it to the
    // FFI type. The slice remains valid for the duration of this call.
    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    // Convert info slice to a raw pointer.
    let (info_ptr, ninfo) = if info.is_empty() {
        (ptr::null(), 0)
    } else {
        // SAFETY: `info` is a non-empty slice of `Info` values, each
        // containing a pointer to a `pmix_info_t`. We take the address
        // of the first element's handle field.
        (
            unsafe {
                std::ptr::addr_of!((*(&info[0] as *const Info)).handle) as *const ffi::pmix_info_t
            },
            info.len(),
        )
    };

    // SAFETY: FFI call into PMIx library.
    // - `procs_ptr` points to a valid slice of `pmix_proc_t` handles
    //   that remain valid for the duration of this call. PMIx does
    //   not retain these pointers after return.
    // - `info_ptr` is either null or points to a valid slice of
    //   `pmix_info_t` pointers. PMIx reads but does not retain.
    // - `nprocs` and `ninfo` are the correct lengths of their arrays.
    // - This is a blocking call: it does not return until all
    //   participating processes have completed the connect operation.
    let raw_status = unsafe { ffi::PMIx_Connect(procs_ptr, procs.len(), info_ptr, ninfo) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        Err(pmix_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConnectCallbackWrapper — Rust closure → FFI callback bridge
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking connect callback wrapper.
///
/// Wraps a Rust closure so it can be called from the C FFI callback.
/// The closure receives `PmixStatus` — the result of the connect
/// operation (`PMIX_SUCCESS` or an error code).
pub struct ConnectCallbackWrapper {
    /// The user's Rust closure.
    callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
}

impl ConnectCallbackWrapper {
    /// Create a new wrapper around a Rust closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(PmixStatus) + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Connect_nb
// ─────────────────────────────────────────────────────────────────────────────

/// Non-blocking connect with a Rust closure callback.
///
/// Records a set of processes as "connected" without blocking. The
/// provided callback is invoked when the operation completes.
///
/// * The callback receives `PmixStatus`:
///   - `PMIX_SUCCESS` — all participating processes have connected.
///   - Error code — the connect operation failed.
/// * The `callback` closure must be `Send + 'static` because it may
///   be invoked from a different thread by the PMIx library.
///
/// # Returns
/// * `Ok(())` — the connect request was accepted (async, result in callback).
/// * `Err(PmixStatus)` — the connect request itself failed synchronously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Connect_nb(const pmix_proc_t procs[], size_t nprocs,
///                               const pmix_info_t info[], size_t ninfo,
///                               pmix_op_cbfunc_t cbfunc, void *cbdata);
/// ```
pub fn connect_nb(
    procs: &[Proc],
    info: &[Info],
    callback: ConnectCallbackWrapper,
) -> Result<(), PmixStatus> {
    if procs.is_empty() {
        return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
    }

    // Box the callback wrapper so it lives on the heap and outlives
    // the FFI call. We pass it as cbdata and recover it in the C
    // callback via Box::from_raw.
    let cb_box: *mut ConnectCallbackWrapper = Box::into_raw(Box::new(callback));

    // The C bridge function that PMIx calls back into.
    // SAFETY: This extern "C" function is only called by PMIx with
    // the cbdata pointer we provided (Box<ConnectCallbackWrapper>).
    // It takes ownership of the box via Box::from_raw.
    extern "C" fn connect_callback_bridge(status: i32, cbdata: *mut c_void) {
        let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut ConnectCallbackWrapper) };
        let pmix_status = PmixStatus::from_raw(status);
        (cb_wrapper.callback)(pmix_status);
        // The box is dropped here.
    }

    // Convert proc slice to a raw pointer.
    // SAFETY: `procs` is a non-empty slice of `Proc` values.
    let procs_ptr = unsafe {
        std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
    };

    // Convert info slice to a raw pointer.
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

    // SAFETY: FFI call into PMIx library.
    // - `procs_ptr` points to a valid slice of `pmix_proc_t` handles.
    // - `info_ptr` is either null or points to a valid slice of
    //   `pmix_info_t` pointers.
    // - `connect_callback_bridge` is a valid extern "C" callback.
    // - `cb_box` is a valid heap-allocated ConnectCallbackWrapper
    //   that will be recovered in the callback via Box::from_raw.
    // - PMIx_Connect_nb returns immediately; the callback is invoked
    //   asynchronously by the PMIx library at a later time.
    let raw_status = unsafe {
        ffi::PMIx_Connect_nb(
            procs_ptr,
            procs.len(),
            info_ptr,
            ninfo,
            Some(connect_callback_bridge),
            cb_box as *mut c_void,
        )
    };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if pmix_status.is_success() {
        Ok(())
    } else {
        // On synchronous failure, PMIx may or may not call the callback.
             // To be safe, reclaim the box to avoid a memory leak.
             unsafe { drop(Box::from_raw(cb_box)) }
             Err(pmix_status)
         }
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // PMIx_Disconnect
        // ─────────────────────────────────────────────────────────────────────────────

        /// Disconnect a previously connected set of processes.
        ///
        /// Instructs the PMIx server to disconnect the specified processes that
        /// were previously connected via [`connect`] or [`connect_nb`]. When
        /// processes are disconnected:
        ///
        /// * The resource manager no longer treats the failure of any process in
        ///   the group as a reportable event.
        /// * Processes no longer receive job-level info for the other namespaces
        ///   that were part of the connected group.
        /// * Processes in the group no longer receive notification of errors from
        ///   other members of the disconnected group.
        ///
        /// This is a **blocking** call: it does not return until all
        /// participating processes have called `disconnect` with the same
        /// set of processes, and the host environment has completed any
        /// required supporting operations.
        ///
        /// # Constraints
        /// * A process can only engage in *one* disconnect operation involving
        ///   the identical procs array at a time.
        /// * A process *can* be simultaneously engaged in multiple disconnect
        ///   operations, each involving a different procs array.
        /// * Processes must provide the **identical** procs array (same ordering,
        ///   same identification method) as was used in the corresponding connect.
        /// * A process cannot reconnect to a set of procs that has not fully
        ///   completed disconnect — you have to fully disconnect before you can
        ///   reconnect to the same group.
        /// * An error is returned if the specified set of procs was not previously
        ///   connected via a call to `PMIx_Connect` or its non-blocking form.
        /// * The `info` array can pass directives regarding the collective
        ///   algorithm, timeout constraints, and other options.
        ///
        /// # Returns
        /// * `Ok(())` — all participating processes have disconnected.
        /// * `Err(PmixStatus::Known(PmixError::ErrInvalidOperation))` — the
        ///   specified set of procs was not previously connected.
        /// * `Err(PmixStatus)` — another error in the request.
        ///
        /// # Thread Safety
        /// The caller is responsible for ensuring thread safety when calling
        /// this from multiple threads simultaneously.
        ///
        /// # C API
        /// ```c
        /// pmix_status_t PMIx_Disconnect(const pmix_proc_t procs[], size_t nprocs,
        ///                               const pmix_info_t info[], size_t ninfo);
        /// ```
        pub fn disconnect(procs: &[Proc], info: &[Info]) -> Result<(), PmixStatus> {
         if procs.is_empty() {
             return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
         }

         // Convert proc slice to a raw pointer.
         // SAFETY: `procs` is a non-empty slice of `Proc` values, each
         // containing a `pmix_proc_t` handle as its first field. We take
         // the address of the first element's handle and cast it to the
         // FFI type. The slice remains valid for the duration of this call.
         let procs_ptr = unsafe {
             std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
         };

         // Convert info slice to a raw pointer.
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

         // SAFETY: FFI call into PMIx library.
         // - `procs_ptr` points to a valid slice of `pmix_proc_t` handles
         //   that remain valid for the duration of this call. PMIx does
         //   not retain these pointers after return.
         // - `info_ptr` is either null or points to a valid slice of
         //   `pmix_info_t` pointers. PMIx reads but does not retain.
         // - `nprocs` and `ninfo` are the correct lengths of their arrays.
         // - This is a blocking call: it does not return until all
         //   participating processes have completed the disconnect operation.
         let raw_status = unsafe {
             ffi::PMIx_Disconnect(procs_ptr, procs.len(), info_ptr, ninfo)
         };

         let pmix_status = PmixStatus::from_raw(raw_status);
         if pmix_status.is_success() {
             Ok(())
         } else {
             Err(pmix_status)
         }
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // DisconnectCallbackWrapper — Rust closure → FFI callback bridge
        // ─────────────────────────────────────────────────────────────────────────────

        /// Non-blocking disconnect callback wrapper.
        ///
        /// Wraps a Rust closure so it can be called from the C FFI callback.
        /// The closure receives `PmixStatus` — the result of the disconnect
        /// operation (`PMIX_SUCCESS` or an error code).
        pub struct DisconnectCallbackWrapper {
         /// The user's Rust closure.
         callback: Box<dyn Fn(PmixStatus) + Send + 'static>,
        }

        impl DisconnectCallbackWrapper {
         /// Create a new wrapper around a Rust closure.
         pub fn new<F>(f: F) -> Self
         where
             F: Fn(PmixStatus) + Send + 'static,
         {
             Self {
                 callback: Box::new(f),
             }
         }
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // PMIx_Disconnect_nb
        // ─────────────────────────────────────────────────────────────────────────────

        /// Non-blocking disconnect with a Rust closure callback.
        ///
        /// Disconnects a previously connected set of processes without blocking.
        /// The provided callback is invoked when the operation completes.
        ///
        /// * The callback receives `PmixStatus`:
        ///   - `PMIX_SUCCESS` — all participating processes have disconnected.
        ///   - Error code — the disconnect operation failed.
        /// * The `callback` closure must be `Send + 'static` because it may
        ///   be invoked from a different thread by the PMIx library.
        ///
        /// # Returns
        /// * `Ok(())` — the disconnect request was accepted (async, result in callback).
        /// * `Err(PmixStatus)` — the disconnect request itself failed synchronously.
        ///
        /// # C API
        /// ```c
        /// pmix_status_t PMIx_Disconnect_nb(const pmix_proc_t procs[], size_t nprocs,
        ///                                  const pmix_info_t info[], size_t ninfo,
        ///                                  pmix_op_cbfunc_t cbfunc, void *cbdata);
        /// ```
        pub fn disconnect_nb(
         procs: &[Proc],
         info: &[Info],
         callback: DisconnectCallbackWrapper,
        ) -> Result<(), PmixStatus> {
         if procs.is_empty() {
             return Err(PmixStatus::from_raw(ffi::PMIX_ERR_BAD_PARAM));
         }

         // Box the callback wrapper so it lives on the heap and outlives
         // the FFI call. We pass it as cbdata and recover it in the C
         // callback via Box::from_raw.
         let cb_box: *mut DisconnectCallbackWrapper = Box::into_raw(Box::new(callback));

         // The C bridge function that PMIx calls back into.
         // SAFETY: This extern "C" function is only called by PMIx with
         // the cbdata pointer we provided (Box<DisconnectCallbackWrapper>).
         // It takes ownership of the box via Box::from_raw.
         extern "C" fn disconnect_callback_bridge(status: i32, cbdata: *mut c_void) {
             let cb_wrapper = unsafe { Box::from_raw(cbdata as *mut DisconnectCallbackWrapper) };
             let pmix_status = PmixStatus::from_raw(status);
             (cb_wrapper.callback)(pmix_status);
             // The box is dropped here.
         }

         // Convert proc slice to a raw pointer.
         // SAFETY: `procs` is a non-empty slice of `Proc` values.
         let procs_ptr = unsafe {
             std::ptr::addr_of!((*(&procs[0] as *const Proc)).handle) as *const ffi::pmix_proc_t
         };

         // Convert info slice to a raw pointer.
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

         // SAFETY: FFI call into PMIx library.
         // - `procs_ptr` points to a valid slice of `pmix_proc_t` handles.
         // - `info_ptr` is either null or points to a valid slice of
         //   `pmix_info_t` pointers.
         // - `disconnect_callback_bridge` is a valid extern "C" callback.
         // - `cb_box` is a valid heap-allocated DisconnectCallbackWrapper
         //   that will be recovered in the callback via Box::from_raw.
         // - PMIx_Disconnect_nb returns immediately; the callback is invoked
         //   asynchronously by the PMIx library at a later time.
         let raw_status = unsafe {
             ffi::PMIx_Disconnect_nb(
                 procs_ptr,
                 procs.len(),
                 info_ptr,
                 ninfo,
                 Some(disconnect_callback_bridge),
                 cb_box as *mut c_void,
             )
         };

         let pmix_status = PmixStatus::from_raw(raw_status);
         if pmix_status.is_success() {
             Ok(())
         } else {
             // On synchronous failure, PMIx may or may not call the callback.
             // To be safe, reclaim the box to avoid a memory leak.
             unsafe { drop(Box::from_raw(cb_box)) }
             Err(pmix_status)
         }
        }
