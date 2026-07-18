//! Process management — `PMIx_Abort`, `PMIx_Spawn`, `PMIx_Spawn_nb`,
//! `PMIx_Connect`, `PMIx_Connect_nb`, `PMIx_Disconnect`, `PMIx_Disconnect_nb`,
//! `PMIx_Resolve_peers`, `PMIx_Resolve_nodes`.
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
//! use pmix::{InfoBuilder, PmixError, PmixStatus, Proc, RANK_WILDCARD};
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
//!     .build()
//!     .expect("valid app");
//! let job_info = vec![InfoBuilder::new().build()];
//! let result = spawn(&job_info, &[app]);
//! // result is Ok(nspace) on success
//!
//! // Connect to a namespace
//! let target = Proc::new("target_namespace", RANK_WILDCARD)
//!     .expect("valid proc");
//! let connect_info = InfoBuilder::new().build();
//! let result = connect(&[target.clone()], &[connect_info]);
//!
//! // Disconnect from a previously connected set
//! let disconnect_info = InfoBuilder::new().build();
//! let result = disconnect(&[target], &[disconnect_info]);
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
                libc::calloc(n + 1, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            for (j, s) in app.argv.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").expect("CString::new interior NUL (process_mgmt.rs)"));
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
                libc::calloc(n + 1, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            for (j, s) in app.env.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").expect("CString::new interior NUL (process_mgmt.rs)"));
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
                libc::calloc(n + 1, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            for (j, s) in app.argv.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").expect("CString::new interior NUL (process_mgmt.rs)"));
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
                libc::calloc(n + 1, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            for (j, s) in app.env.iter().enumerate() {
                let cstr = CString::new(s.as_bytes()).unwrap_or_else(|_| CString::new("").expect("CString::new interior NUL (process_mgmt.rs)"));
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
    let raw_status = unsafe { ffi::PMIx_Disconnect(procs_ptr, procs.len(), info_ptr, ninfo) };

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

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Resolve_peers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the array of processes within a specified namespace executing
/// on a given node.
///
/// Given a node name, return the array of processes within the specified
/// namespace that are executing on that node.
///
/// * If `nspace` is `None`, all processes on the node (across all known
///   namespaces) will be returned.
/// * If `nodename` is `None`, the current local node is used.
/// * If the specified node does not currently host any processes from the
///   given namespace, the returned vector will be empty.
///
/// The caller owns the returned `Vec<Proc>` — no explicit free is needed.
///
/// # Returns
/// * `Ok(Vec<Proc>)` — list of processes on the specified node/namespace.
///   May be empty if no processes match.
/// * `Err(PmixStatus::Known(PmixError::ErrInit))` — PMIx has not been
///   initialized via `PMIx_Init`.
/// * `Err(PmixStatus::Known(PmixError::ErrNotFound))` — `nspace` was
///   provided but no such namespace is known.
/// * `Err(PmixStatus)` — another error in the request.
///
/// # Thread Safety
/// The caller is responsible for ensuring thread safety when calling
/// this from multiple threads simultaneously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Resolve_peers(const char *nodename,
///                                  const pmix_nspace_t nspace,
///                                  pmix_proc_t **procs, size_t *nprocs);
/// ```
pub fn resolve_peers(
    nodename: Option<&str>,
    nspace: Option<&str>,
) -> Result<Vec<Proc>, PmixStatus> {
    // Convert nodename to C string if provided.
    let (nodename_ptr, _nodename_cstring) = match nodename {
        Some(n) => {
            let cs = CString::new(n).expect("nodename must not contain interior NUL bytes");
            (cs.as_ptr(), Some(cs))
        }
        None => (ptr::null::<c_char>(), None),
    };

    // Convert nspace to C string if provided.
    let (nspace_ptr, _nspace_cstring) = match nspace {
        Some(ns) => {
            let cs = CString::new(ns).expect("nspace must not contain interior NUL bytes");
            (cs.as_ptr(), Some(cs))
        }
        None => (ptr::null::<c_char>(), None),
    };

    let mut procs: *mut ffi::pmix_proc_t = ptr::null_mut();
    let mut nprocs: usize = 0;

    // SAFETY: FFI call into PMIx library.
    // - `nodename_ptr` is either null (use local node) or a valid C string.
    // - `nspace_ptr` is either null (all namespaces) or a valid C string.
    // - `procs` and `nprocs` are valid mutable references for output.
    // - On success, PMIx allocates a `pmix_proc_t` array that the caller
    //   owns and must free via PMIX_PROC_FREE (which calls pmix_free).
    let raw_status =
        unsafe { ffi::PMIx_Resolve_peers(nodename_ptr, nspace_ptr, &mut procs, &mut nprocs) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // Convert the C array to a Rust Vec<Proc>.
    // SAFETY: On success, PMIx_Resolve_peers allocates an array of nprocs
    // pmix_proc_t elements. We read each element, then free the C array.
    let rust_procs: Vec<Proc> = unsafe {
        if procs.is_null() || nprocs == 0 {
            // No processes found — return empty vec.
            Vec::new()
        } else {
            // Read each proc from the C array.
            let mut rust_vec = Vec::with_capacity(nprocs);
            for i in 0..nprocs {
                let c_proc = std::ptr::read_unaligned(procs.add(i));
                let proc = Proc {
                    handle: c_proc,
                    len: 1,
                };
                rust_vec.push(proc);
            }
            // Free the C-allocated array. PMIX_PROC_FREE macro calls pmix_free.
            ffi::free(procs as *mut std::ffi::c_void);
            rust_vec
        }
    };

    Ok(rust_procs)
}

// ─────────────────────────────────────────────────────────────────────────────
// PMIx_Resolve_nodes
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the list of nodes hosting processes within a given namespace.
///
/// Given a namespace, return the list of nodes that host processes
/// within that namespace. The returned string is a comma-delimited
/// list of node names.
///
/// * If the specified namespace does not exist or has no nodes,
///   an error is returned.
/// * The caller owns the returned `String` — no explicit free is needed.
///
/// # Returns
/// * `Ok(String)` — comma-delimited list of node names for the namespace.
/// * `Err(PmixStatus::Known(PmixError::ErrInit))` — PMIx has not been
///   initialized via `PMIx_Init`.
/// * `Err(PmixStatus::Known(PmixError::ErrNotFound))` — the specified
///   namespace is not known.
/// * `Err(PmixStatus)` — another error in the request.
///
/// # Thread Safety
/// The caller is responsible for ensuring thread safety when calling
/// this from multiple threads simultaneously.
///
/// # C API
/// ```c
/// pmix_status_t PMIx_Resolve_nodes(const pmix_nspace_t nspace,
///                                  char **nodelist);
/// ```
pub fn resolve_nodes(nspace: &str) -> Result<String, PmixStatus> {
    // Convert namespace to C string.
    let nspace_cs = CString::new(nspace).expect("nspace must not contain interior NUL bytes");

    let mut nodelist: *mut c_char = ptr::null_mut();

    // SAFETY: FFI call into PMIx library.
    // - `nspace_cs.as_ptr()` is a valid NUL-terminated C string whose
    //   lifetime (`nspace_cs`) is kept alive until after this call.
    // - `nodelist` is a valid mutable pointer for the output.
    // - On success, PMIx allocates a NUL-terminated string via pmix_malloc
    //   that the caller owns and must free via pmix_free.
    let raw_status = unsafe { ffi::PMIx_Resolve_nodes(nspace_cs.as_ptr(), &mut nodelist) };

    let pmix_status = PmixStatus::from_raw(raw_status);
    if !pmix_status.is_success() {
        return Err(pmix_status);
    }

    // Convert the C-allocated string to a Rust String.
    // SAFETY: On success, PMIx_Resolve_nodes writes a non-null,
    // NUL-terminated, C-allocated string into *nodelist.
    let node_list_str: String = unsafe {
        if nodelist.is_null() {
            // Should not happen on success, but be defensive.
            return Err(PmixStatus::from_raw(ffi::PMIX_ERR_NOT_FOUND));
        }
        let c_str = CStr::from_ptr(nodelist);
        let rust_str = c_str.to_string_lossy().into_owned();
        // Free the C-allocated string. PMIx uses pmix_free → free.
        ffi::free(nodelist as *mut std::ffi::c_void);
        rust_str
    };

    Ok(node_list_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmix_app_builder_new() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert_eq!(app.cmd(), None);
        assert!(app.argv().is_empty());
    }

    #[test]
    fn test_pmix_app_builder_cmd() {
        let app = PmixAppBuilder::new().cmd("/bin/echo").build().unwrap();
        assert_eq!(app.cmd(), Some("/bin/echo"));
    }

    #[test]
    fn test_pmix_app_builder_arg() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/echo")
            .arg("hello")
            .build()
            .unwrap();
        assert_eq!(app.argv().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_args() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/echo")
            .args(vec!["arg1".to_string(), "arg2".to_string()])
            .build()
            .unwrap();
        assert_eq!(app.argv().len(), 2);
    }

    #[test]
    fn test_pmix_app_builder_env() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/echo")
            .env("FOO=bar")
            .build()
            .unwrap();
        assert_eq!(app.env_vars().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_cwd() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/echo")
            .cwd("/tmp")
            .build()
            .unwrap();
        assert_eq!(app.cwd(), Some("/tmp"));
    }

    #[test]
    fn test_pmix_app_builder_maxprocs() {
        let app = PmixAppBuilder::new().maxprocs(4).build().unwrap();
        assert_eq!(app.maxprocs(), 4);
    }

    #[test]
    fn test_pmix_app_builder_nul_error() {
        let result = PmixAppBuilder::new().cmd("has\0null").build();
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixApp — accessor tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_app_accessors_default() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert_eq!(app.cmd(), None);
        assert!(app.argv().is_empty());
        assert!(app.env_vars().is_empty());
        assert_eq!(app.cwd(), None);
        assert_eq!(app.maxprocs(), 0);
    }

    #[test]
    fn test_pmix_app_accessors_full() {
        let app = PmixAppBuilder::new()
            .cmd("/usr/bin/test")
            .arg("--verbose")
            .arg("--count")
            .arg("5")
            .env("PATH=/usr/bin")
            .env("HOME=/root")
            .cwd("/tmp")
            .maxprocs(8)
            .build()
            .unwrap();
        assert_eq!(app.cmd(), Some("/usr/bin/test"));
        assert_eq!(app.argv().len(), 3);
        assert_eq!(app.env_vars().len(), 2);
        assert_eq!(app.cwd(), Some("/tmp"));
        assert_eq!(app.maxprocs(), 8);
    }

    #[test]
    fn test_pmix_app_builder_envs() {
        let app = PmixAppBuilder::new()
            .envs(vec![
                "A=1".to_string(),
                "B=2".to_string(),
                "C=3".to_string(),
            ])
            .build()
            .unwrap();
        assert_eq!(app.env_vars().len(), 3);
    }

    #[test]
    fn test_pmix_app_builder_nul_in_arg() {
        let result = PmixAppBuilder::new()
            .cmd("/bin/test")
            .arg("has\0null")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_app_builder_nul_in_env() {
        let result = PmixAppBuilder::new()
            .cmd("/bin/test")
            .env("KEY=\0value")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_app_builder_nul_in_cwd() {
        let result = PmixAppBuilder::new()
            .cmd("/bin/test")
            .cwd("/tmp/\0bad")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_app_builder_chain_order() {
        // Builder methods can be called in any order
        let app = PmixAppBuilder::new()
            .maxprocs(2)
            .cwd("/var")
            .cmd("/bin/ls")
            .arg("-la")
            .build()
            .unwrap();
        assert_eq!(app.cmd(), Some("/bin/ls"));
        assert_eq!(app.maxprocs(), 2);
        assert_eq!(app.cwd(), Some("/var"));
        assert_eq!(app.argv().len(), 1);
    }

    // ──────────────────────────────────────────────────────────────────────
    // spawn() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_empty_apps_returns_bad_param() {
        // spawn with no apps should return PMIX_ERR_BAD_PARAM
        let result = spawn(&[], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // connect() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_connect_empty_procs_returns_bad_param() {
        let result = connect(&[], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // disconnect() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_disconnect_empty_procs_returns_bad_param() {
        let result = disconnect(&[], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Callback wrapper construction tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_callback_wrapper_new() {
        let wrapper = SpawnCallbackWrapper::new(|_status, _nspace| {
            // callback body
        });
        // Wrapper should be constructible — actual invocation needs DVM
        drop(wrapper);
    }

    #[test]
    fn test_connect_callback_wrapper_new() {
        let wrapper = ConnectCallbackWrapper::new(|_status| {
            // callback body
        });
        drop(wrapper);
    }

    #[test]
    fn test_disconnect_callback_wrapper_new() {
        let wrapper = DisconnectCallbackWrapper::new(|_status| {
            // callback body
        });
        drop(wrapper);
    }

    // ──────────────────────────────────────────────────────────────────────
    // spawn_nb() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_nb_empty_apps_returns_bad_param() {
        let wrapper = SpawnCallbackWrapper::new(|_, _| {});
        let result = spawn_nb(&[], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // connect_nb() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_connect_nb_empty_procs_returns_bad_param() {
        let wrapper = ConnectCallbackWrapper::new(|_| {});
        let result = connect_nb(&[], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // disconnect_nb() — validation tests (no DVM required)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_disconnect_nb_empty_procs_returns_bad_param() {
        let wrapper = DisconnectCallbackWrapper::new(|_| {});
        let result = disconnect_nb(&[], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixApp — Debug and clone tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_app_builder_debug() {
        let mut builder = PmixAppBuilder::new();
        builder.cmd("/bin/echo").arg("hello");
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("/bin/echo"));
    }

    #[test]
    fn test_pmix_app_debug() {
        let app = PmixApp::builder().cmd("/bin/ls").build().unwrap();
        let debug_str = format!("{:?}", app);
        assert!(debug_str.contains("/bin/ls"));
    }

    #[test]
    fn test_pmix_app_builder_default_maxprocs() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert_eq!(app.maxprocs(), 0);
    }

    #[test]
    fn test_pmix_app_builder_maxprocs_zero() {
        let app = PmixAppBuilder::new().maxprocs(0).build().unwrap();
        assert_eq!(app.maxprocs(), 0);
    }

    #[test]
    fn test_pmix_app_builder_maxprocs_negative() {
        let app = PmixAppBuilder::new().maxprocs(-1).build().unwrap();
        assert_eq!(app.maxprocs(), -1);
    }

    #[test]
    fn test_pmix_app_builder_maxprocs_max() {
        let app = PmixAppBuilder::new()
            .maxprocs(std::i32::MAX)
            .build()
            .unwrap();
        assert_eq!(app.maxprocs(), std::i32::MAX);
    }

    #[test]
    fn test_pmix_app_builder_multiple_args() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .arg("arg1")
            .arg("arg2")
            .arg("arg3")
            .arg("arg4")
            .arg("arg5")
            .build()
            .unwrap();
        assert_eq!(app.argv().len(), 5);
    }

    #[test]
    fn test_pmix_app_builder_multiple_envs() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .env("A=1")
            .env("B=2")
            .env("C=3")
            .build()
            .unwrap();
        assert_eq!(app.env_vars().len(), 3);
    }

    #[test]
    fn test_pmix_app_builder_empty_cmd() {
        let app = PmixAppBuilder::new().cmd("").build().unwrap();
        assert_eq!(app.cmd(), Some(""));
    }

    #[test]
    fn test_pmix_app_builder_empty_cwd() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .cwd("")
            .build()
            .unwrap();
        assert_eq!(app.cwd(), Some(""));
    }

    #[test]
    fn test_pmix_app_builder_empty_arg() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .arg("")
            .build()
            .unwrap();
        assert_eq!(app.argv().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_empty_env() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .env("")
            .build()
            .unwrap();
        assert_eq!(app.env_vars().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_special_chars_in_cmd() {
        let app = PmixAppBuilder::new()
            .cmd("/path/with spaces/app")
            .build()
            .unwrap();
        assert_eq!(app.cmd(), Some("/path/with spaces/app"));
    }

    #[test]
    fn test_pmix_app_builder_special_chars_in_env() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .env("PATH=/usr/bin:/bin:/usr/local/bin")
            .build()
            .unwrap();
        assert_eq!(app.env_vars().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_unicode_in_cmd() {
        let app = PmixAppBuilder::new()
            .cmd("/path/日本語/app")
            .build()
            .unwrap();
        assert_eq!(app.cmd(), Some("/path/日本語/app"));
    }

    #[test]
    fn test_pmix_app_builder_combined_args_and_envs() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .args(vec!["a".to_string(), "b".to_string()])
            .arg("c")
            .envs(vec!["X=1".to_string(), "Y=2".to_string()])
            .env("Z=3")
            .build()
            .unwrap();
        assert_eq!(app.argv().len(), 3);
        assert_eq!(app.env_vars().len(), 3);
    }

    #[test]
    fn test_pmix_app_builder_args_empty_iterator() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .args(Vec::<String>::new())
            .build()
            .unwrap();
        assert!(app.argv().is_empty());
    }

    #[test]
    fn test_pmix_app_builder_envs_empty_iterator() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .envs(Vec::<String>::new())
            .build()
            .unwrap();
        assert!(app.env_vars().is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixAppBuilder — builder pattern tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_app_builder_returns_mut_self() {
        // Verify fluent interface works — each method returns &mut Self
        let _app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .arg("x")
            .env("K=V")
            .cwd("/tmp")
            .maxprocs(1)
            .build()
            .unwrap();
        // If we got here, the fluent interface works
    }

    #[test]
    fn test_pmix_app_builder_cmd_overwrite() {
        let app = PmixAppBuilder::new()
            .cmd("/first")
            .cmd("/second")
            .build()
            .unwrap();
        assert_eq!(app.cmd(), Some("/second"));
    }

    #[test]
    fn test_pmix_app_builder_cwd_overwrite() {
        let app = PmixAppBuilder::new()
            .cmd("/bin/test")
            .cwd("/first")
            .cwd("/second")
            .build()
            .unwrap();
        assert_eq!(app.cwd(), Some("/second"));
    }

    #[test]
    fn test_pmix_app_builder_maxprocs_overwrite() {
        let app = PmixAppBuilder::new()
            .maxprocs(1)
            .maxprocs(2)
            .build()
            .unwrap();
        assert_eq!(app.maxprocs(), 2);
    }

    #[test]
    fn test_pmix_app_builder_no_cmd() {
        let app = PmixAppBuilder::new().arg("x").build().unwrap();
        assert_eq!(app.cmd(), None);
        assert_eq!(app.argv().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_only_env() {
        let app = PmixAppBuilder::new().env("A=1").build().unwrap();
        assert_eq!(app.cmd(), None);
        assert!(app.argv().is_empty());
        assert_eq!(app.env_vars().len(), 1);
    }

    #[test]
    fn test_pmix_app_builder_nul_in_envs_iterator() {
        let result = PmixAppBuilder::new()
            .cmd("/bin/test")
            .envs(vec!["A=1".to_string(), "B=\0bad".to_string()])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_pmix_app_builder_nul_in_args_iterator() {
        let result = PmixAppBuilder::new()
            .cmd("/bin/test")
            .args(vec!["good".to_string(), "bad\0arg".to_string()])
            .build();
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixApp::builder() — convenience method
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_app_builder_static_method() {
        // PmixApp::builder() should be equivalent to PmixAppBuilder::new()
        let app = PmixApp::builder().cmd("/bin/test").build().unwrap();
        assert_eq!(app.cmd(), Some("/bin/test"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // spawn() — additional validation tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_with_job_info_and_empty_apps() {
        // Even with job_info, empty apps should fail
        let info = vec![crate::InfoBuilder::new().build()];
        let result = spawn(&info, &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_spawn_with_single_app() {
        // spawn with one app — should not return BAD_PARAM
        // (will fail at FFI level since no DVM, but not BAD_PARAM)
        let app = PmixApp::builder().cmd("/bin/echo").build().unwrap();
        let result = spawn(&[], &[app]);
        // Without DVM, this hits FFI and returns an error — but not BAD_PARAM
        assert!(result.is_err());
        // The error should NOT be ErrBadParam since apps is non-empty
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_spawn_with_multiple_apps() {
        let app1 = PmixApp::builder()
            .cmd("/bin/echo")
            .maxprocs(1)
            .build()
            .unwrap();
        let app2 = PmixApp::builder()
            .cmd("/bin/ls")
            .maxprocs(2)
            .build()
            .unwrap();
        let result = spawn(&[], &[app1, app2]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // spawn_nb() — additional validation tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_nb_with_job_info_and_empty_apps() {
        let info = vec![crate::InfoBuilder::new().build()];
        let wrapper = SpawnCallbackWrapper::new(|_, _| {});
        let result = spawn_nb(&info, &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_eq!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_spawn_nb_with_single_app() {
        let app = PmixApp::builder().cmd("/bin/echo").build().unwrap();
        let wrapper = SpawnCallbackWrapper::new(|_, _| {});
        let result = spawn_nb(&[], &[app], wrapper);
        // Without DVM, FFI call returns error — but not BAD_PARAM
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_spawn_nb_with_multiple_apps() {
        let app1 = PmixApp::builder().cmd("/bin/echo").build().unwrap();
        let app2 = PmixApp::builder().cmd("/bin/ls").build().unwrap();
        let wrapper = SpawnCallbackWrapper::new(|_, _| {});
        let result = spawn_nb(&[], &[app1, app2], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // connect() — tests with valid procs
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_connect_with_valid_proc() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let result = connect(&[proc], &[]);
        // Without DVM, FFI call returns error — but not BAD_PARAM
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_with_multiple_procs() {
        let p1 = crate::Proc::new("test_ns", 0).unwrap();
        let p2 = crate::Proc::new("test_ns", 1).unwrap();
        let result = connect(&[p1, p2], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_with_info() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let info = vec![crate::InfoBuilder::new().build()];
        let result = connect(&[proc], &info);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_with_wildcard_rank() {
        let proc = crate::Proc::new("test_ns", crate::RANK_WILDCARD).unwrap();
        let result = connect(&[proc], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_different_namespaces() {
        let p1 = crate::Proc::new("ns_a", 0).unwrap();
        let p2 = crate::Proc::new("ns_b", 0).unwrap();
        let result = connect(&[p1, p2], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // disconnect() — tests with valid procs
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_disconnect_with_valid_proc() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let result = disconnect(&[proc], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_disconnect_with_multiple_procs() {
        let p1 = crate::Proc::new("test_ns", 0).unwrap();
        let p2 = crate::Proc::new("test_ns", 1).unwrap();
        let result = disconnect(&[p1, p2], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_disconnect_with_info() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let info = vec![crate::InfoBuilder::new().build()];
        let result = disconnect(&[proc], &info);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_disconnect_with_wildcard_rank() {
        let proc = crate::Proc::new("test_ns", crate::RANK_WILDCARD).unwrap();
        let result = disconnect(&[proc], &[]);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // connect_nb() — tests with valid procs
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_connect_nb_with_valid_proc() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let wrapper = ConnectCallbackWrapper::new(|_| {});
        let result = connect_nb(&[proc], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_nb_with_info() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let info = vec![crate::InfoBuilder::new().build()];
        let wrapper = ConnectCallbackWrapper::new(|_| {});
        let result = connect_nb(&[proc], &info, wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    fn test_connect_nb_with_multiple_procs() {
        let p1 = crate::Proc::new("ns_a", 0).unwrap();
        let p2 = crate::Proc::new("ns_b", 0).unwrap();
        let wrapper = ConnectCallbackWrapper::new(|_| {});
        let result = connect_nb(&[p1, p2], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // disconnect_nb() — tests with valid procs
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    #[ignore] // Requires DVM — PMIx_Disconnect_nb segfaults without init
    fn test_disconnect_nb_with_valid_proc() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let wrapper = DisconnectCallbackWrapper::new(|_| {});
        let result = disconnect_nb(&[proc], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    #[ignore] // Requires DVM — PMIx_Disconnect_nb segfaults without init
    fn test_disconnect_nb_with_info() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let info = vec![crate::InfoBuilder::new().build()];
        let wrapper = DisconnectCallbackWrapper::new(|_| {});
        let result = disconnect_nb(&[proc], &info, wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    #[test]
    #[ignore] // Requires DVM — PMIx_Disconnect_nb segfaults without init
    fn test_disconnect_nb_with_multiple_procs() {
        let p1 = crate::Proc::new("ns_a", 0).unwrap();
        let p2 = crate::Proc::new("ns_b", 0).unwrap();
        let wrapper = DisconnectCallbackWrapper::new(|_| {});
        let result = disconnect_nb(&[p1, p2], &[], wrapper);
        assert!(result.is_err());
        if let Err(status) = result {
            assert_ne!(status, PmixStatus::Known(crate::PmixError::ErrBadParam));
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Callback wrapper — closure capture tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_callback_wrapper_captures_closure() {
        let wrapper = SpawnCallbackWrapper::new(|status, nspace| {
            // Verify closure can capture and use arguments
            let _ = status.is_success();
            let _ = nspace.is_some();
        });
        drop(wrapper);
    }

    #[test]
    fn test_connect_callback_wrapper_captures_closure() {
        let wrapper = ConnectCallbackWrapper::new(|status| {
            let _ = status.is_error();
        });
        drop(wrapper);
    }

    #[test]
    fn test_disconnect_callback_wrapper_captures_closure() {
        let wrapper = DisconnectCallbackWrapper::new(|status| {
            let _ = status.known();
        });
        drop(wrapper);
    }

    // ──────────────────────────────────────────────────────────────────────
    // abort() — parameter validation tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_abort_with_success_status() {
        // abort with success status — no procs (wildcard)
        let result = abort(PmixStatus::Known(crate::PmixError::Success), None, None);
        // Without DVM, FFI call returns error
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_error_status() {
        let result = abort(
            PmixStatus::Known(crate::PmixError::Error),
            Some("test abort"),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_message() {
        let result = abort(
            PmixStatus::Known(crate::PmixError::Error),
            Some("abort message with spaces"),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_empty_message() {
        let result = abort(PmixStatus::Known(crate::PmixError::Error), Some(""), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_procs() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        let result = abort(
            PmixStatus::Known(crate::PmixError::Error),
            Some("abort"),
            Some(&[proc]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_multiple_procs() {
        let p1 = crate::Proc::new("test_ns", 0).unwrap();
        let p2 = crate::Proc::new("test_ns", 1).unwrap();
        let result = abort(
            PmixStatus::Known(crate::PmixError::Error),
            None,
            Some(&[p1, p2]),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_abort_with_unknown_status() {
        let result = abort(PmixStatus::Unknown(-99999), None, None);
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // resolve_peers() and resolve_nodes() — no-DVM tests
    // ──────────────────────────────────────────────────────────────────────
    // NOTE: Without a DVM (prrte/pmix server), these calls fail. The exact
    // error code varies by PMIx version and state — PMIx 5.0.7 returns
    // ErrNotFound rather than ErrInit. We assert only that an error is
    // returned, not the specific error, to avoid version-dependent failures.

    #[test]
    fn test_resolve_peers_no_dvm() {
        let result = resolve_peers(None, None);
        assert!(result.is_err(), "resolve_peers() should fail without DVM");
    }

    #[test]
    fn test_resolve_peers_with_nodename() {
        let result = resolve_peers(Some("localhost"), None);
        assert!(result.is_err(), "resolve_peers() should fail without DVM");
    }

    #[test]
    fn test_resolve_peers_with_nspace() {
        let result = resolve_peers(None, Some("test_ns"));
        assert!(result.is_err(), "resolve_peers() should fail without DVM");
    }

    #[test]
    fn test_resolve_peers_with_both() {
        let result = resolve_peers(Some("node01"), Some("test_ns"));
        assert!(result.is_err(), "resolve_peers() should fail without DVM");
    }

    #[test]
    fn test_resolve_nodes_no_dvm() {
        let result = resolve_nodes("test_ns");
        assert!(result.is_err(), "resolve_nodes() should fail without DVM");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixStatus edge cases in process_mgmt context
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_error_is_not_success() {
        let result = spawn(&[], &[]);
        if let Err(status) = result {
            assert!(!status.is_success());
            assert!(status.is_error());
        }
    }

    #[test]
    fn test_connect_error_is_not_success() {
        let result = connect(&[], &[]);
        if let Err(status) = result {
            assert!(!status.is_success());
            assert!(status.is_error());
        }
    }

    #[test]
    fn test_disconnect_error_is_not_success() {
        let result = disconnect(&[], &[]);
        if let Err(status) = result {
            assert!(!status.is_success());
            assert!(status.is_error());
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Proc accessor tests (within process_mgmt context)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_proc_new_and_rank() {
        let proc = crate::Proc::new("test_ns", 42).unwrap();
        assert_eq!(proc.get_rank(), 42);
    }

    #[test]
    fn test_proc_set_rank() {
        let mut proc = crate::Proc::new("test_ns", 0).unwrap();
        proc.set_rank(99);
        assert_eq!(proc.get_rank(), 99);
    }

    #[test]
    fn test_proc_new_wildcard_rank() {
        let proc = crate::Proc::new("test_ns", crate::RANK_WILDCARD).unwrap();
        assert_eq!(proc.get_rank(), crate::RANK_WILDCARD);
    }

    #[test]
    fn test_proc_new_zero_rank() {
        let proc = crate::Proc::new("test_ns", 0).unwrap();
        assert_eq!(proc.get_rank(), 0);
    }

    #[test]
    fn test_proc_new_max_rank() {
        let proc = crate::Proc::new("test_ns", u32::MAX).unwrap();
        assert_eq!(proc.get_rank(), u32::MAX);
    }

    #[test]
    fn test_proc_clone() {
        let proc = crate::Proc::new("test_ns", 42).unwrap();
        let proc2 = proc.clone();
        assert_eq!(proc.get_rank(), proc2.get_rank());
    }

    #[test]
    fn test_proc_new_nul_error() {
        let result = crate::Proc::new("has\0null", 0);
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixApp edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pmix_app_cmd_returns_none_when_not_set() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert_eq!(app.cmd(), None);
    }

    #[test]
    fn test_pmix_app_argv_returns_empty_when_not_set() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert!(app.argv().is_empty());
    }

    #[test]
    fn test_pmix_app_env_vars_returns_empty_when_not_set() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert!(app.env_vars().is_empty());
    }

    #[test]
    fn test_pmix_app_cwd_returns_none_when_not_set() {
        let app = PmixAppBuilder::new().build().unwrap();
        assert_eq!(app.cwd(), None);
    }

    // ──────────────────────────────────────────────────────────────────────
    // SpawnCallback type alias test
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_spawn_callback_type_is_assignable() {
        // Verify SpawnCallback type alias is usable
        extern "C" fn dummy_spawn_cb(
            _status: crate::ffi::pmix_status_t,
            _nspace: *mut c_char,
            _cbdata: *mut c_void,
        ) {
        }
        let _cb: SpawnCallback = dummy_spawn_cb;
    }
}
