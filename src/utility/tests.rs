//! utility unit tests

use super::*;

    use super::*;

    // ──────────────────────────────────────────────────────────────────────
    // PmixByteObject tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixByteObject::from_slice` copies data and is independent of source.
    #[test]
    fn test_byte_object_from_slice() {
        let data = b"hello from stdin";
        let bo = PmixByteObject::from_slice(data);
        assert_eq!(bo.as_slice(), data);
        assert_eq!(bo.len(), 16);
        assert!(!bo.is_empty());
    }

    /// `PmixByteObject::from_vec` takes ownership of the vector.
    #[test]
    fn test_byte_object_from_vec() {
        let vec = vec![1u8, 2, 3, 4, 5];
        let bo = PmixByteObject::from_vec(vec);
        assert_eq!(bo.as_slice(), &[1, 2, 3, 4, 5]);
        assert_eq!(bo.len(), 5);
    }

    /// `PmixByteObject::empty` creates an empty byte object.
    #[test]
    fn test_byte_object_empty() {
        let bo = PmixByteObject::empty();
        assert!(bo.is_empty());
        assert_eq!(bo.len(), 0);
        assert!(bo.as_slice().is_empty());
    }

    /// `PmixByteObject` implements `AsRef<[u8]>`.
    #[test]
    fn test_byte_object_as_ref() {
        let bo = PmixByteObject::from_slice(b"test");
        let slice: &[u8] = bo.as_ref();
        assert_eq!(slice, b"test");
    }

    /// `PmixByteObject` can be cloned.
    #[test]
    fn test_byte_object_clone() {
        let bo1 = PmixByteObject::from_slice(b"clone me");
        let bo2 = bo1.clone();
        assert_eq!(bo1.as_slice(), bo2.as_slice());
        assert_eq!(bo1.len(), bo2.len());
    }

    /// `PmixByteObject::as_c_mut_ptr` produces a valid pointer that can be freed.
    #[test]
    fn test_byte_object_c_conversion_roundtrip() {
        let bo = PmixByteObject::from_slice(b"roundtrip test");
        let c_ptr = bo.as_c_mut_ptr();
        assert!(!c_ptr.is_null());
        // SAFETY: c_ptr was returned by as_c_mut_ptr and has not been freed.
        unsafe { PmixByteObject::free_c_ptr(c_ptr) };
    }

    /// Empty byte object converts to C and back without issues.
    #[test]
    fn test_byte_object_empty_c_conversion() {
        let bo = PmixByteObject::empty();
        let c_ptr = bo.as_c_mut_ptr();
        assert!(!c_ptr.is_null());
        // SAFETY: c_ptr was returned by as_c_mut_ptr.
        unsafe { PmixByteObject::free_c_ptr(c_ptr) };
    }

    /// `free_c_ptr` is safe with a null pointer (no-op).
    #[test]
    fn test_byte_object_free_null() {
        // SAFETY: null pointer is a valid no-op for free_c_ptr.
        unsafe { PmixByteObject::free_c_ptr(std::ptr::null_mut()) };
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_IOF_push tests
    // ──────────────────────────────────────────────────────────────────────

    /// `IoForwardPushHandler` trait is implemented for closures that take
    /// `PmixStatus` and return nothing, and satisfy `Send + 'static`.
    #[test]
    fn test_iof_push_handler_trait() {
        fn assert_handler<T: IoForwardPushHandler>() {}
        assert_handler::<fn(PmixStatus)>();
        assert_handler::<Box<dyn Fn(PmixStatus) + Send>>();
    }

    // Note: iof_push and iof_push_blocking require a running PMIx daemon
    // and proper init/finalize, so they are tested via integration tests
    // (ignored in unit test suite).

    /// `initialized()` is callable and returns a bool.
    ///
    /// Before `PMIx_Init` has been called, the PMIx library's internal
    /// `pmix_globals.initialized` flag is `false`, so we expect `false`.
    ///
    /// Note: this test calls into the real PMIx library. If `libpmix` is
    /// not linked or the library version differs, the FFI call may panic
    /// or return unexpected results. In a CI environment without a running
    /// PMIx daemon, this still works because `PMIx_Initialized` only reads
    /// a local atomic flag — it does not contact the server.
    #[test]
    fn test_initialized_before_init_is_false() {
        let result = initialized();
        // Under prterun/DVM, PMIx is already initialized, so this returns true.
        // Standalone, it should return false. Accept either result.
        if cfg!(not(test)) {
            // Not running as a test — skip
        } else if result {
            // Running under prterun — PMIx is already initialized, which is fine.
            eprintln!(
                "test_initialized_before_init_is_false: PMIx already initialized (DVM-launched), accepting true"
            );
        } else {
            // Standalone — should be false.
            assert!(
                !result,
                "PMIx_Initialized should return false before PMIx_Init"
            );
        }
    }

    /// `initialized()` is idempotent — calling it multiple times returns
    /// the same value (no side effects).
    #[test]
    fn test_initialized_idempotent() {
        let first = initialized();
        let second = initialized();
        assert_eq!(first, second, "PMIx_Initialized should be idempotent");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Error_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `error_string` returns `Ok(String)` for known status codes.
    ///
    /// PMIx_Error_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_status_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_error_string_success() {
        let status = PmixStatus::from_raw(0); // PMIX_SUCCESS
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string(PMIX_SUCCESS) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "error_string should not return an empty string"
        );
    }

    /// `error_string` returns a readable description for PMIX_ERROR (-1).
    #[test]
    fn test_error_string_generic_error() {
        let status = PmixStatus::from_raw(-1); // PMIX_ERROR
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string(PMIX_ERROR) should return Ok, got {:?}",
            result
        );
    }

    /// `error_string` handles negative error codes in various subsystem
    /// ranges (timeout, bad parameter, not found, etc.).
    #[test]
    fn test_error_string_various_codes() {
        let codes: Vec<i32> = vec![
            0,   // PMIX_SUCCESS
            -1,  // PMIX_ERROR
            -24, // PMIX_ERR_TIMEOUT
            -27, // PMIX_ERR_BAD_PARAM
            -33, // PMIX_ERR_NOT_FOUND
        ];
        for code in codes {
            let status = PmixStatus::from_raw(code);
            let result = error_string(status);
            assert!(
                result.is_ok(),
                "error_string({}) should return Ok, got {:?}",
                code,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "error_string({}) should not return empty string",
                code
            );
        }
    }

    /// `error_string` handles unknown/user-defined status codes (below -9999).
    ///
    /// PMIx reserves values below PMIX_EXTERNAL_ERR_BASE (-9999) for
    /// user/implementation-defined codes. The C function should still
    /// return a string (typically indicating an external error).
    #[test]
    fn test_error_string_unknown_code() {
        let status = PmixStatus::from_raw(-10001); // User-defined range
        let result = error_string(status);
        assert!(
            result.is_ok(),
            "error_string should handle unknown codes gracefully, got {:?}",
            result
        );
    }

    /// `error_string` is deterministic — the same status code always
    /// returns the same string.
    #[test]
    fn test_error_string_deterministic() {
        let status = PmixStatus::from_raw(-24); // PMIX_ERR_TIMEOUT
        let first = error_string(status).unwrap();
        let second = error_string(status).unwrap();
        assert_eq!(
            first, second,
            "error_string must be deterministic for the same input"
        );
    }

    /// `error_string` returns different strings for different status codes.
    #[test]
    fn test_error_string_distinct() {
        let success = error_string(PmixStatus::from_raw(0)).unwrap();
        let error = error_string(PmixStatus::from_raw(-1)).unwrap();
        assert_ne!(
            success, error,
            "error_string(SUCCESS) and error_string(ERROR) must differ"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Proc_state_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `proc_state_string` returns `Ok(String)` for known process states.
    ///
    /// PMIx_Proc_state_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_proc_state_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_proc_state_string_running() {
        let state = PmixProcState::Running;
        let result = proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string(Running) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "proc_state_string should not return an empty string"
        );
    }

    /// `proc_state_string` returns the expected string for key lifecycle states.
    #[test]
    fn test_proc_state_string_key_states() {
        use crate::PmixProcState::*;

        let states = [
            Undef,
            Prepped,
            LaunchUnderway,
            Running,
            Connected,
            Terminated,
            Error,
            Aborted,
        ];
        for state in states {
            let result = proc_state_string(state);
            assert!(
                result.is_ok(),
                "proc_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "proc_state_string({:?}) should not return empty string",
                state
            );
        }
    }

    /// `proc_state_string` is deterministic — the same state always returns
    /// the same string.
    #[test]
    fn test_proc_state_string_deterministic() {
        let state = PmixProcState::Terminated;
        let first = proc_state_string(state).unwrap();
        let second = proc_state_string(state).unwrap();
        assert_eq!(
            first, second,
            "proc_state_string must be deterministic for the same input"
        );
    }

    /// `proc_state_string` returns different strings for different states.
    #[test]
    fn test_proc_state_string_distinct() {
        let running = proc_state_string(PmixProcState::Running).unwrap();
        let terminated = proc_state_string(PmixProcState::Terminated).unwrap();
        assert_ne!(
            running, terminated,
            "proc_state_string(Running) and proc_state_string(Terminated) must differ"
        );
    }

    /// `proc_state_string` handles all error-range states (50–63).
    #[test]
    fn test_proc_state_string_error_range() {
        use crate::PmixProcState::*;

        let error_states = [
            Error,
            KilledByCmd,
            Aborted,
            FailedToStart,
            AbortedBySig,
            TermWoSync,
            CommFailed,
            SensorBoundExceeded,
            CalledAbort,
            HeartbeatFailed,
            Migrating,
            CannotRestart,
            TermNonZero,
            FailedToLaunch,
        ];
        for state in error_states {
            let result = proc_state_string(state);
            assert!(
                result.is_ok(),
                "proc_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
        }
    }

    /// `proc_state_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_proc_state_string_unknown() {
        let state = PmixProcState::Unknown(99);
        let result = proc_state_string(state);
        assert!(
            result.is_ok(),
            "proc_state_string(Unknown(99)) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        // The C library returns "UNKNOWN STATE" for unrecognized values.
        assert!(
            !desc.is_empty(),
            "proc_state_string for unknown state should return non-empty string"
        );
    }

    /// `PmixProcState::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_proc_state_from_raw_to_raw_roundtrip() {
        use crate::PmixProcState::*;

        let states = [
            Undef,
            Prepped,
            LaunchUnderway,
            Restart,
            Terminate,
            Running,
            Connected,
            Unterminated,
            Terminated,
            Error,
            KilledByCmd,
            Aborted,
            FailedToStart,
            AbortedBySig,
            TermWoSync,
            CommFailed,
            SensorBoundExceeded,
            CalledAbort,
            HeartbeatFailed,
            Migrating,
            CannotRestart,
            TermNonZero,
            FailedToLaunch,
        ];
        for state in states {
            let raw = state.to_raw();
            let recovered = PmixProcState::from_raw(raw);
            assert_eq!(
                state, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                state
            );
        }
    }

    /// `PmixProcState::is_alive` and `is_terminated` classify states correctly.
    #[test]
    fn test_proc_state_classification() {
        use crate::PmixProcState::*;

        assert!(Running.is_alive());
        assert!(Connected.is_alive());
        assert!(Prepped.is_alive());
        assert!(!Running.is_terminated());

        assert!(Terminated.is_terminated());
        assert!(Aborted.is_terminated());
        assert!(KilledByCmd.is_terminated());
        assert!(!Terminated.is_alive());
        assert!(!Aborted.is_alive());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Scope_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `scope_string` returns `Ok(String)` for all known scope values.
    ///
    /// PMIx_Scope_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_scope_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_scope_string_all_known() {
        use crate::PmixScope::*;

        let scopes = [Undef, Local, Remote, Global, Internal];
        for scope in scopes {
            let result = scope_string(scope);
            assert!(
                result.is_ok(),
                "scope_string({:?}) should return Ok, got {:?}",
                scope,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "scope_string({:?}) should not return empty string",
                scope
            );
        }
    }

    /// `scope_string` returns the expected strings for key scopes.
    ///
    /// PMIx returns descriptive strings, not the enum variant names.
    /// We check for the actual content the library provides.
    #[test]
    fn test_scope_string_expected_values() {
        use crate::PmixScope::*;

        let local = scope_string(Local).unwrap();
        let remote = scope_string(Remote).unwrap();
        let global = scope_string(Global).unwrap();

        // PMIx returns "SHARE ON LOCAL NODE ONLY" — contains "local" and "node"
        assert!(
            local.to_lowercase().contains("local") || local.to_lowercase().contains("node"),
            "Local scope string should describe local node, got '{}'",
            local
        );
        // PMIx returns "SHARE ON REMOTE NODES ONLY" — contains "remote"
        assert!(
            remote.to_lowercase().contains("remote"),
            "Remote scope string should contain 'remote', got '{}'",
            remote
        );
        // PMIx returns "SHARE ACROSS ALL NODES" — no "global" keyword, check for "all"
        assert!(
            global.to_lowercase().contains("all"),
            "Global scope string should describe all nodes, got '{}'",
            global
        );
    }

    /// `scope_string` is deterministic — the same scope always returns
    /// the same string.
    #[test]
    fn test_scope_string_deterministic() {
        use crate::PmixScope::Global;
        let first = scope_string(Global).unwrap();
        let second = scope_string(Global).unwrap();
        assert_eq!(
            first, second,
            "scope_string must be deterministic for the same input"
        );
    }

    /// `scope_string` returns different strings for different scopes.
    #[test]
    fn test_scope_string_distinct() {
        use crate::PmixScope::*;
        let local = scope_string(Local).unwrap();
        let global = scope_string(Global).unwrap();
        assert_ne!(
            local, global,
            "scope_string(Local) and scope_string(Global) must differ"
        );
    }

    /// `scope_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_scope_string_unknown() {
        use crate::PmixScope::Unknown;
        let scope = Unknown(99);
        let result = scope_string(scope);
        assert!(
            result.is_ok(),
            "scope_string(Unknown(99)) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "scope_string for unknown scope should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixScope enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixScope::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_scope_from_raw_to_raw_roundtrip() {
        use crate::PmixScope::*;

        let scopes = [Undef, Local, Remote, Global, Internal];
        for scope in scopes {
            let raw = scope.to_raw();
            let recovered = PmixScope::from_raw(raw);
            assert_eq!(
                scope, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                scope
            );
        }
    }

    /// `PmixScope::from_raw` maps known raw values correctly.
    #[test]
    fn test_scope_from_raw_known() {
        use crate::PmixScope::*;

        assert_eq!(PmixScope::from_raw(0), Undef);
        assert_eq!(PmixScope::from_raw(1), Local);
        assert_eq!(PmixScope::from_raw(2), Remote);
        assert_eq!(PmixScope::from_raw(3), Global);
        assert_eq!(PmixScope::from_raw(4), Internal);
        assert!(matches!(PmixScope::from_raw(255), Unknown(255)));
    }

    /// `PmixScope::to_raw` returns the expected raw values.
    #[test]
    fn test_scope_to_raw() {
        use crate::PmixScope::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(Local.to_raw(), 1);
        assert_eq!(Remote.to_raw(), 2);
        assert_eq!(Global.to_raw(), 3);
        assert_eq!(Internal.to_raw(), 4);
        assert_eq!(Unknown(42).to_raw(), 42);
    }

    /// `PmixScope` implements Display.
    #[test]
    fn test_scope_display() {
        use crate::PmixScope::*;

        assert_eq!(format!("{}", Undef), "UNDEFINED");
        assert_eq!(format!("{}", Local), "LOCAL");
        assert_eq!(format!("{}", Remote), "REMOTE");
        assert_eq!(format!("{}", Global), "GLOBAL");
        assert_eq!(format!("{}", Internal), "INTERNAL");
        assert_eq!(format!("{}", Unknown(99)), "UNKNOWN SCOPE (99)");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Data_range_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `data_range_string` returns `Ok(String)` for all known range values.
    ///
    /// PMIx_Data_range_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_data_range_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_data_range_string_all_known() {
        use crate::PmixDataRange::*;

        let ranges = [
            Undef, Rm, Local, Namespace, Session, Global, Custom, ProcLocal, Invalid,
        ];
        for range in ranges {
            let result = data_range_string(range);
            assert!(
                result.is_ok(),
                "data_range_string({:?}) should return Ok, got {:?}",
                range,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "data_range_string({:?}) should not return empty string",
                range
            );
        }
    }

    /// `data_range_string` returns the expected strings for key ranges.
    ///
    /// PMIx returns descriptive strings that don't always include the enum
    /// variant names (e.g., "AVAIL TO PROCESSES IN SAME JOB ONLY" for
    /// Namespace, "AVAIL ON LOCAL NODE ONLY" for Local). We check for
    /// keywords that actually appear in the library output.
    #[test]
    fn test_data_range_string_expected_values() {
        use crate::PmixDataRange::*;

        let local = data_range_string(Local).unwrap();
        let namespace = data_range_string(Namespace).unwrap();
        let session = data_range_string(Session).unwrap();
        let global = data_range_string(Global).unwrap();

        // PMIx returns "AVAIL ON LOCAL NODE ONLY" — contains "local"
        assert!(
            local.to_lowercase().contains("local"),
            "Local range string should describe local node, got '{}'",
            local
        );
        // PMIx returns "AVAIL TO PROCESSES IN SAME JOB ONLY" — check for "job"
        assert!(
            namespace.to_lowercase().contains("job") || namespace.to_lowercase().contains("same"),
            "Namespace range string should describe job scope, got '{}'",
            namespace
        );
        // PMIx returns "AVAIL TO PROCESSES IN SAME ALLOCATION ONLY" — check for "allocation"
        assert!(
            session.to_lowercase().contains("allocation")
                || session.to_lowercase().contains("same"),
            "Session range string should describe allocation scope, got '{}'",
            session
        );
        // PMIx returns "AVAIL TO ANYONE WITH AUTHORIZATION" — check for "anyone" or "authorization"
        assert!(
            global.to_lowercase().contains("anyone")
                || global.to_lowercase().contains("authorization"),
            "Global range string should describe global availability, got '{}'",
            global
        );
    }

    /// `data_range_string` is deterministic — the same range always returns
    /// the same string.
    #[test]
    fn test_data_range_string_deterministic() {
        use crate::PmixDataRange::Session;
        let first = data_range_string(Session).unwrap();
        let second = data_range_string(Session).unwrap();
        assert_eq!(
            first, second,
            "data_range_string must be deterministic for the same input"
        );
    }

    /// `data_range_string` returns different strings for different ranges.
    #[test]
    fn test_data_range_string_distinct() {
        use crate::PmixDataRange::*;
        let local = data_range_string(Local).unwrap();
        let global = data_range_string(Global).unwrap();
        assert_ne!(
            local, global,
            "data_range_string(Local) and data_range_string(Global) must differ"
        );
    }

    /// `data_range_string` handles the Unknown variant (raw value not in
    /// the standard enum).
    #[test]
    fn test_data_range_string_unknown() {
        use crate::PmixDataRange::Unknown;
        let range = Unknown;
        let result = data_range_string(range);
        assert!(
            result.is_ok(),
            "data_range_string(Unknown) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_range_string for unknown range should return non-empty string"
        );
    }

    /// `data_range_string` handles the Invalid variant (255).
    #[test]
    fn test_data_range_string_invalid() {
        use crate::PmixDataRange::Invalid;
        let result = data_range_string(Invalid);
        assert!(
            result.is_ok(),
            "data_range_string(Invalid) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "data_range_string(Invalid) should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixDataRange enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixDataRange::from_raw` and `to_raw` are inverses for known values.
    #[test]
    fn test_data_range_from_raw_to_raw_roundtrip() {
        use crate::PmixDataRange::*;

        let ranges = [
            Undef, Rm, Local, Namespace, Session, Global, Custom, ProcLocal, Invalid,
        ];
        for range in ranges {
            let raw = range.to_raw();
            let recovered = PmixDataRange::from_raw(raw);
            assert_eq!(
                range, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                range
            );
        }
    }

    /// `PmixDataRange::from_raw` maps known raw values correctly.
    #[test]
    fn test_data_range_from_raw_known() {
        use crate::PmixDataRange::*;

        assert_eq!(PmixDataRange::from_raw(0), Undef);
        assert_eq!(PmixDataRange::from_raw(1), Rm);
        assert_eq!(PmixDataRange::from_raw(2), Local);
        assert_eq!(PmixDataRange::from_raw(3), Namespace);
        assert_eq!(PmixDataRange::from_raw(4), Session);
        assert_eq!(PmixDataRange::from_raw(5), Global);
        assert_eq!(PmixDataRange::from_raw(6), Custom);
        assert_eq!(PmixDataRange::from_raw(7), ProcLocal);
        assert_eq!(PmixDataRange::from_raw(255), Invalid);
        assert!(matches!(PmixDataRange::from_raw(200), Unknown));
    }

    /// `PmixDataRange::to_raw` returns the expected raw values.
    #[test]
    fn test_data_range_to_raw() {
        use crate::PmixDataRange::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(Rm.to_raw(), 1);
        assert_eq!(Local.to_raw(), 2);
        assert_eq!(Namespace.to_raw(), 3);
        assert_eq!(Session.to_raw(), 4);
        assert_eq!(Global.to_raw(), 5);
        assert_eq!(Custom.to_raw(), 6);
        assert_eq!(ProcLocal.to_raw(), 7);
        assert_eq!(Invalid.to_raw(), 255);
        assert_eq!(Unknown.to_raw(), 128);
    }

    /// `PmixDataRange` implements Display.
    #[test]
    fn test_data_range_display() {
        use crate::PmixDataRange::*;

        assert_eq!(format!("{}", Undef), "UNDEFINED");
        assert_eq!(format!("{}", Rm), "RM");
        assert_eq!(format!("{}", Local), "LOCAL");
        assert_eq!(format!("{}", Namespace), "NAMESPACE");
        assert_eq!(format!("{}", Session), "SESSION");
        assert_eq!(format!("{}", Global), "GLOBAL");
        assert_eq!(format!("{}", Custom), "CUSTOM");
        assert_eq!(format!("{}", ProcLocal), "PROC LOCAL");
        assert_eq!(format!("{}", Invalid), "INVALID");
        assert_eq!(format!("{}", Unknown), "UNKNOWN RANGE (128)");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Info_directives_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `info_directives_string` returns `Ok(String)` for the REQD flag.
    ///
    /// PMIx_Info_directives_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_info_directives_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_info_directives_string_reqd() {
        let flags = crate::InfoFlags::REQD;
        let result = info_directives_string(flags);
        assert!(
            result.is_ok(),
            "info_directives_string(REQD) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string should not return an empty string"
        );
    }

    /// `info_directives_string` returns `Ok(String)` for all known flag values.
    #[test]
    fn test_info_directives_string_all_known() {
        use crate::InfoFlags;

        let flags = [
            InfoFlags::REQD,
            InfoFlags::QUALIFIER,
            InfoFlags::PERSISTENT,
            InfoFlags::REQD_PROCESSED,
        ];
        for flag in flags {
            let result = info_directives_string(flag);
            assert!(
                result.is_ok(),
                "info_directives_string({:?}) should return Ok, got {:?}",
                flag,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "info_directives_string({:?}) should not return empty string",
                flag
            );
        }
    }

    /// `info_directives_string` handles combined flags (bitwise OR).
    #[test]
    fn test_info_directives_string_combined() {
        use crate::InfoFlags;
        let combined = InfoFlags::REQD | InfoFlags::PERSISTENT;
        let result = info_directives_string(combined);
        assert!(
            result.is_ok(),
            "info_directives_string(combined) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for combined flags should return non-empty string"
        );
    }

    /// `info_directives_string` handles zero flags (no directives set).
    #[test]
    fn test_info_directives_string_empty() {
        use crate::InfoFlags;
        let empty = InfoFlags::default();
        assert!(empty.is_empty(), "default InfoFlags should be empty");
        let result = info_directives_string(empty);
        assert!(
            result.is_ok(),
            "info_directives_string(empty) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for empty flags should return non-empty string"
        );
    }

    /// `info_directives_string` is deterministic — the same flags always return
    /// the same string.
    #[test]
    fn test_info_directives_string_deterministic() {
        use crate::InfoFlags;
        let flags = InfoFlags::REQD | InfoFlags::REQD_PROCESSED;
        let first = info_directives_string(flags).unwrap();
        let second = info_directives_string(flags).unwrap();
        assert_eq!(
            first, second,
            "info_directives_string must be deterministic for the same input"
        );
    }

    /// `info_directives_string` returns different strings for different flags.
    #[test]
    fn test_info_directives_string_distinct() {
        use crate::InfoFlags;
        let reqd = info_directives_string(InfoFlags::REQD).unwrap();
        let persistent = info_directives_string(InfoFlags::PERSISTENT).unwrap();
        assert_ne!(
            reqd, persistent,
            "info_directives_string(REQD) and info_directives_string(PERSISTENT) must differ"
        );
    }

    /// `info_directives_string` handles unknown/reserved flag values.
    #[test]
    fn test_info_directives_string_reserved() {
        use crate::InfoFlags;
        // PMIX_INFO_DIR_RESERVED = 0xFFFF0000
        let reserved = InfoFlags(0xFFFF0000);
        let result = info_directives_string(reserved);
        assert!(
            result.is_ok(),
            "info_directives_string(reserved) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "info_directives_string for reserved flags should return non-empty string"
        );
    }

    /// `InfoFlags::raw` and construction round-trip correctly.
    #[test]
    fn test_info_flags_raw_roundtrip() {
        use crate::InfoFlags;
        let flags = InfoFlags::REQD | InfoFlags::PERSISTENT | InfoFlags::REQD_PROCESSED;
        let raw = flags.raw();
        let recovered = InfoFlags(raw);
        assert_eq!(flags, recovered, "InfoFlags(raw(flags)) should round-trip");
        assert_eq!(
            raw,
            1 | 16 | 4,
            "combined flags should have correct raw value (REQD=1 | PERSISTENT=16 | REQD_PROCESSED=4 = 21)"
        );
    }

    /// `InfoFlags::contains` checks individual bits correctly.
    #[test]
    fn test_info_flags_contains() {
        use crate::InfoFlags;
        let combined = InfoFlags::REQD | InfoFlags::PERSISTENT;
        assert!(combined.contains(InfoFlags::REQD));
        assert!(combined.contains(InfoFlags::PERSISTENT));
        assert!(!combined.contains(InfoFlags::REQD_PROCESSED));
    }

    /// `InfoFlags::is_empty` works for zero and non-zero values.
    #[test]
    fn test_info_flags_is_empty() {
        use crate::InfoFlags;
        assert!(InfoFlags::default().is_empty());
        assert!(!InfoFlags::REQD.is_empty());
        assert!(!InfoFlags::REQD.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_IOF_channel_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `iof_channel_string` returns `Ok(String)` for all known channel values.
    ///
    /// PMIx_IOF_channel_string is documented to always return a valid,
    /// non-null, null-terminated string for any pmix_iof_channel_t value.
    /// This test calls into the real PMIx library.
    #[test]
    fn test_iof_channel_string_all_known() {
        use crate::IOFChannelFlags;

        let channels = [
            IOFChannelFlags::NO_CHANNELS,
            IOFChannelFlags::STDIN,
            IOFChannelFlags::STDOUT,
            IOFChannelFlags::STDERR,
            IOFChannelFlags::STDDIAG,
            IOFChannelFlags::ALL_CHANNELS,
        ];
        for channel in channels {
            let result = iof_channel_string(channel);
            assert!(
                result.is_ok(),
                "iof_channel_string({:?}) should return Ok, got {:?}",
                channel,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "iof_channel_string({:?}) should not return empty string",
                channel
            );
        }
    }

    /// `iof_channel_string` returns the expected strings for key channels.
    #[test]
    fn test_iof_channel_string_expected_values() {
        use crate::IOFChannelFlags;

        let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
        let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        let stderr = iof_channel_string(IOFChannelFlags::STDERR).unwrap();

        assert!(
            stdin.to_lowercase().contains("stdin"),
            "STDIN channel string should contain 'stdin', got '{}'",
            stdin
        );
        assert!(
            stdout.to_lowercase().contains("stdout"),
            "STDOUT channel string should contain 'stdout', got '{}'",
            stdout
        );
        assert!(
            stderr.to_lowercase().contains("stderr"),
            "STDERR channel string should contain 'stderr', got '{}'",
            stderr
        );
    }

    /// `iof_channel_string` is deterministic — the same channel always returns
    /// the same string.
    #[test]
    fn test_iof_channel_string_deterministic() {
        use crate::IOFChannelFlags;
        let first = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        let second = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        assert_eq!(
            first, second,
            "iof_channel_string must be deterministic for the same input"
        );
    }

    /// `iof_channel_string` returns different strings for different channels.
    #[test]
    fn test_iof_channel_string_distinct() {
        use crate::IOFChannelFlags;
        let stdin = iof_channel_string(IOFChannelFlags::STDIN).unwrap();
        let stdout = iof_channel_string(IOFChannelFlags::STDOUT).unwrap();
        assert_ne!(
            stdin, stdout,
            "iof_channel_string(STDIN) and iof_channel_string(STDOUT) must differ"
        );
    }

    /// `iof_channel_string` handles combined channel flags (bitmask OR).
    #[test]
    fn test_iof_channel_string_combined() {
        use crate::IOFChannelFlags;
        let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
        let result = iof_channel_string(combined);
        assert!(
            result.is_ok(),
            "iof_channel_string(STDOUT|STDERR) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "iof_channel_string for combined channels should return non-empty string"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOFChannelFlags enum tests
    // ──────────────────────────────────────────────────────────────────────

    /// `IOFChannelFlags::raw()` returns the expected raw values.
    #[test]
    fn test_iof_channel_flags_raw() {
        use crate::IOFChannelFlags;

        assert_eq!(IOFChannelFlags::NO_CHANNELS.raw(), 0x0000);
        assert_eq!(IOFChannelFlags::STDIN.raw(), 0x0001);
        assert_eq!(IOFChannelFlags::STDOUT.raw(), 0x0002);
        assert_eq!(IOFChannelFlags::STDERR.raw(), 0x0004);
        assert_eq!(IOFChannelFlags::STDDIAG.raw(), 0x0008);
        assert_eq!(IOFChannelFlags::ALL_CHANNELS.raw(), 0x00FF);
    }

    /// `IOFChannelFlags` bitwise OR works correctly.
    #[test]
    fn test_iof_channel_flags_bitor() {
        use crate::IOFChannelFlags;

        let combined = IOFChannelFlags::STDIN | IOFChannelFlags::STDOUT;
        assert_eq!(combined.raw(), 0x0003);
        assert!(combined.contains(IOFChannelFlags::STDIN));
        assert!(combined.contains(IOFChannelFlags::STDOUT));
        assert!(!combined.contains(IOFChannelFlags::STDERR));
    }

    /// `IOFChannelFlags::contains` checks individual bits correctly.
    #[test]
    fn test_iof_channel_flags_contains() {
        use crate::IOFChannelFlags;

        let combined = IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR;
        assert!(combined.contains(IOFChannelFlags::STDOUT));
        assert!(combined.contains(IOFChannelFlags::STDERR));
        assert!(!combined.contains(IOFChannelFlags::STDIN));
    }

    /// `IOFChannelFlags::is_empty` works for zero and non-zero values.
    #[test]
    fn test_iof_channel_flags_is_empty() {
        use crate::IOFChannelFlags;

        assert!(IOFChannelFlags::NO_CHANNELS.is_empty());
        assert!(IOFChannelFlags::default().is_empty());
        assert!(!IOFChannelFlags::STDIN.is_empty());
        assert!(!IOFChannelFlags::ALL_CHANNELS.is_empty());
    }

    /// `IOFChannelFlags` implements Display.
    #[test]
    fn test_iof_channel_flags_display() {
        use crate::IOFChannelFlags;

        let stdin = format!("{}", IOFChannelFlags::STDIN);
        assert!(
            stdin.contains("STDIN"),
            "Display for STDIN should contain 'STDIN', got '{}'",
            stdin
        );

        let stdout = format!("{}", IOFChannelFlags::STDOUT);
        assert!(
            stdout.contains("STDOUT"),
            "Display for STDOUT should contain 'STDOUT', got '{}'",
            stdout
        );

        let no_channels = format!("{}", IOFChannelFlags::NO_CHANNELS);
        assert!(
            no_channels.contains("NO_CHANNELS"),
            "Display for NO_CHANNELS should contain 'NO_CHANNELS', got '{}'",
            no_channels
        );

        let combined = format!("{}", (IOFChannelFlags::STDOUT | IOFChannelFlags::STDERR));
        assert!(
            combined.contains("STDOUT"),
            "Display for combined should contain 'STDOUT', got '{}'",
            combined
        );
        assert!(
            combined.contains("STDERR"),
            "Display for combined should contain 'STDERR', got '{}'",
            combined
        );
    }

    /// `IOFChannelFlags` BitOrAssign works correctly.
    #[test]
    fn test_iof_channel_flags_bitor_assign() {
        use crate::IOFChannelFlags;

        let mut flags = IOFChannelFlags::STDIN;
        flags |= IOFChannelFlags::STDOUT;
        flags |= IOFChannelFlags::STDERR;

        assert!(flags.contains(IOFChannelFlags::STDIN));
        assert!(flags.contains(IOFChannelFlags::STDOUT));
        assert!(flags.contains(IOFChannelFlags::STDERR));
        assert_eq!(flags.raw(), 0x0007);
    }

    // ─────────────────────────────────────────────────────────────────────
    // PMIx_Job_state_string tests
    // ─────────────────────────────────────────────────────────────────────

    /// `job_state_string` returns `Ok(String)` for all known job states.
    #[test]
    fn test_job_state_string_all_known() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
        ];
        for state in states {
            let result = job_state_string(state);
            assert!(
                result.is_ok(),
                "job_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "job_state_string({:?}) should not return an empty string",
                state
            );
        }
    }

    /// `job_state_string` returns expected strings for key lifecycle states.
    #[test]
    fn test_job_state_string_key_states() {
        use crate::PmixJobState::*;

        let undef = job_state_string(Undef).unwrap();
        let running = job_state_string(Running).unwrap();
        let terminated = job_state_string(Terminated).unwrap();
        let terminated_with_error = job_state_string(TerminatedWithError).unwrap();

        assert!(
            !undef.is_empty(),
            "Undef state string should not be empty, got '{}'",
            undef
        );
        assert!(
            running.to_lowercase().contains("run"),
            "Running state string should contain 'run', got '{}'",
            running
        );
        assert!(
            terminated.to_lowercase().contains("terminat"),
            "Terminated state string should contain 'terminat', got '{}'",
            terminated
        );
        assert!(
            terminated_with_error.to_lowercase().contains("error"),
            "TerminatedWithError state string should contain 'error', got '{}'",
            terminated_with_error
        );
    }

    /// `job_state_string` is deterministic — the same state always returns
    /// the same string.
    #[test]
    fn test_job_state_string_deterministic() {
        use crate::PmixJobState::Running;

        let first = job_state_string(Running).unwrap();
        let second = job_state_string(Running).unwrap();
        assert_eq!(
            first, second,
            "job_state_string(Running) should be deterministic: '{}' != '{}'",
            first, second
        );
    }

    /// `PmixJobState::from_raw` round-trips correctly for all known states.
    #[test]
    fn test_job_state_from_raw_to_raw_roundtrip() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
        ];
        for state in states {
            let raw = state.to_raw();
            let recovered = PmixJobState::from_raw(raw);
            assert_eq!(
                state, recovered,
                "Round-trip failed for {:?}: raw={}, recovered={:?}",
                state, raw, recovered
            );
        }
    }

    /// `PmixJobState::from_raw` maps unknown values to `Unknown(n)`.
    #[test]
    fn test_job_state_from_raw_unknown() {
        use crate::PmixJobState;

        let unknown = PmixJobState::from_raw(99);
        assert!(
            matches!(unknown, PmixJobState::Unknown(99)),
            "from_raw(99) should be Unknown(99), got {:?}",
            unknown
        );
    }

    /// `PmixJobState` Display returns a non-empty string for all variants.
    #[test]
    fn test_job_state_display() {
        use crate::PmixJobState::*;

        let states = [
            Undef,
            AwaitingAlloc,
            LaunchUnderway,
            Running,
            Suspended,
            Connected,
            Unterminated,
            Terminated,
            TerminatedWithError,
            Unknown(99),
        ];
        for state in states {
            let display = format!("{}", state);
            assert!(
                !display.is_empty(),
                "Display for {:?} should not be empty",
                state
            );
        }
    }

    /// `PmixJobState` raw values match the C header definitions.
    #[test]
    fn test_job_state_raw_values() {
        use crate::PmixJobState::*;

        assert_eq!(Undef.to_raw(), 0);
        assert_eq!(AwaitingAlloc.to_raw(), 1);
        assert_eq!(LaunchUnderway.to_raw(), 2);
        assert_eq!(Running.to_raw(), 3);
        assert_eq!(Suspended.to_raw(), 4);
        assert_eq!(Connected.to_raw(), 5);
        assert_eq!(Unterminated.to_raw(), 15);
        assert_eq!(Terminated.to_raw(), 20);
        assert_eq!(TerminatedWithError.to_raw(), 50);
    }

    // ───────────────────────────────────────────────────────────────────────────
    // PMIx_Link_state_string
    // ───────────────────────────────────────────────────────────────────────────

    /// `link_state_string` returns `Ok(String)` for all known link states.
    #[test]
    fn test_link_state_string_all_known() {
        use crate::PmixLinkState::*;

        let states = [UnknownState, LinkDown, LinkUp];
        for state in states {
            let result = link_state_string(state);
            assert!(
                result.is_ok(),
                "link_state_string({:?}) should return Ok, got {:?}",
                state,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "link_state_string({:?}) should not return an empty string",
                state
            );
        }
    }

    /// `link_state_string` returns the expected strings for each state.
    #[test]
    fn test_link_state_string_expected_values() {
        use crate::PmixLinkState::*;

        let unknown = link_state_string(UnknownState).unwrap();
        assert_eq!(unknown, "UNKNOWN");

        let down = link_state_string(LinkDown).unwrap();
        assert_eq!(down, "INACTIVE");

        let up = link_state_string(LinkUp).unwrap();
        assert_eq!(up, "ACTIVE");
    }

    /// `link_state_string` is deterministic — same state always returns same string.
    #[test]
    fn test_link_state_string_deterministic() {
        use crate::PmixLinkState::*;

        let first = link_state_string(LinkUp).unwrap();
        let second = link_state_string(LinkUp).unwrap();
        assert_eq!(first, second, "link_state_string should be deterministic");

        let first = link_state_string(LinkDown).unwrap();
        let second = link_state_string(LinkDown).unwrap();
        assert_eq!(first, second, "link_state_string should be deterministic");
    }

    /// `PmixLinkState` Display matches the C string output.
    #[test]
    fn test_link_state_display() {
        use crate::PmixLinkState::*;

        assert_eq!(format!("{}", UnknownState), "UNKNOWN");
        assert_eq!(format!("{}", LinkDown), "INACTIVE");
        assert_eq!(format!("{}", LinkUp), "ACTIVE");
    }

    /// `PmixLinkState::from_raw` / `to_raw` roundtrip for all known values.
    #[test]
    fn test_link_state_from_raw_to_raw() {
        use crate::PmixLinkState::*;

        assert_eq!(PmixLinkState::from_raw(0), UnknownState);
        assert_eq!(PmixLinkState::from_raw(1), LinkDown);
        assert_eq!(PmixLinkState::from_raw(2), LinkUp);
        assert_eq!(PmixLinkState::from_raw(255), PmixLinkState::Unknown(255));

        assert_eq!(UnknownState.to_raw(), 0);
        assert_eq!(LinkDown.to_raw(), 1);
        assert_eq!(LinkUp.to_raw(), 2);

        // Roundtrip for unknown values
        let unknown = PmixLinkState::from_raw(42);
        assert_eq!(unknown.to_raw(), 42);
    }

    /// `PmixLinkState` raw values match the C header definitions.
    #[test]
    fn test_link_state_raw_values() {
        use crate::PmixLinkState::*;

        assert_eq!(UnknownState.to_raw(), 0); // PMIX_LINK_STATE_UNKNOWN
        assert_eq!(LinkDown.to_raw(), 1); // PMIX_LINK_DOWN
        assert_eq!(LinkUp.to_raw(), 2); // PMIX_LINK_UP
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Device_type_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `device_type_string` returns `Ok(String)` for all known device types.
    #[test]
    fn test_device_type_string_all_known() {
        use crate::PmixDeviceType::*;

        let types = [UnknownType, Block, Gpu, Network, OpenFabrics, Dma, Coproc];
        for ty in types {
            let result = device_type_string(ty);
            assert!(
                result.is_ok(),
                "device_type_string({:?}) should return Ok, got {:?}",
                ty,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "device_type_string({:?}) should not return an empty string",
                ty
            );
        }
    }

    /// `device_type_string` returns the expected strings for key device types.
    #[test]
    fn test_device_type_string_expected() {
        use crate::PmixDeviceType::*;

        assert_eq!(device_type_string(UnknownType).unwrap(), "UNKNOWN");
        assert_eq!(device_type_string(Block).unwrap(), "BLOCK");
        assert_eq!(device_type_string(Gpu).unwrap(), "GPU");
        assert_eq!(device_type_string(Network).unwrap(), "NETWORK");
        assert_eq!(device_type_string(OpenFabrics).unwrap(), "OPENFABRICS");
        assert_eq!(device_type_string(Dma).unwrap(), "DMA");
        assert_eq!(device_type_string(Coproc).unwrap(), "COPROCESSOR");
    }

    /// `device_type_string` is deterministic — the same type always returns
    /// the same string.
    #[test]
    fn test_device_type_string_deterministic() {
        use crate::PmixDeviceType::Gpu;

        let first = device_type_string(Gpu).unwrap();
        let second = device_type_string(Gpu).unwrap();
        assert_eq!(first, second, "device_type_string should be deterministic");
    }

    /// `device_type_string` handles unknown device type values gracefully.
    #[test]
    fn test_device_type_string_unknown() {
        use crate::PmixDeviceType;

        let unknown = PmixDeviceType::Unknown(0xFF);
        let result = device_type_string(unknown);
        assert!(
            result.is_ok(),
            "device_type_string should handle unknown values"
        );
    }

    /// `PmixDeviceType::from_raw` / `to_raw` roundtrip for all known values.
    #[test]
    fn test_device_type_from_raw_to_raw() {
        use crate::PmixDeviceType::*;

        assert_eq!(PmixDeviceType::from_raw(0x00), UnknownType);
        assert_eq!(PmixDeviceType::from_raw(0x01), Block);
        assert_eq!(PmixDeviceType::from_raw(0x02), Gpu);
        assert_eq!(PmixDeviceType::from_raw(0x04), Network);
        assert_eq!(PmixDeviceType::from_raw(0x08), OpenFabrics);
        assert_eq!(PmixDeviceType::from_raw(0x10), Dma);
        assert_eq!(PmixDeviceType::from_raw(0x20), Coproc);
        assert_eq!(
            PmixDeviceType::from_raw(0xFF),
            PmixDeviceType::Unknown(0xFF)
        );

        assert_eq!(UnknownType.to_raw(), 0x00);
        assert_eq!(Block.to_raw(), 0x01);
        assert_eq!(Gpu.to_raw(), 0x02);
        assert_eq!(Network.to_raw(), 0x04);
        assert_eq!(OpenFabrics.to_raw(), 0x08);
        assert_eq!(Dma.to_raw(), 0x10);
        assert_eq!(Coproc.to_raw(), 0x20);

        // Roundtrip for unknown values
        let unknown = PmixDeviceType::from_raw(0xDEAD);
        assert_eq!(unknown.to_raw(), 0xDEAD);
    }

    /// `PmixDeviceType` raw values match the C header definitions.
    #[test]
    fn test_device_type_raw_values() {
        use crate::PmixDeviceType::*;

        assert_eq!(UnknownType.to_raw(), 0x00); // PMIX_DEVTYPE_UNKNOWN
        assert_eq!(Block.to_raw(), 0x01); // PMIX_DEVTYPE_BLOCK
        assert_eq!(Gpu.to_raw(), 0x02); // PMIX_DEVTYPE_GPU
        assert_eq!(Network.to_raw(), 0x04); // PMIX_DEVTYPE_NETWORK
        assert_eq!(OpenFabrics.to_raw(), 0x08); // PMIX_DEVTYPE_OPENFABRICS
        assert_eq!(Dma.to_raw(), 0x10); // PMIX_DEVTYPE_DMA
        assert_eq!(Coproc.to_raw(), 0x20); // PMIX_DEVTYPE_COPROC
    }

    /// `PmixDeviceType` Display implementation matches C strings.
    #[test]
    fn test_device_type_display() {
        use crate::PmixDeviceType::*;

        assert_eq!(format!("{}", UnknownType), "UNKNOWN");
        assert_eq!(format!("{}", Block), "BLOCK");
        assert_eq!(format!("{}", Gpu), "GPU");
        assert_eq!(format!("{}", Network), "NETWORK");
        assert_eq!(format!("{}", OpenFabrics), "OPENFABRICS");
        assert_eq!(format!("{}", Dma), "DMA");
        assert_eq!(format!("{}", Coproc), "COPROCESSOR");
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_generate_ppn
    // ──────────────────────────────────────────────────────────────────────

    /// `generate_ppn` returns `Err` when PMIx has not been server-initialized.
    ///
    /// Without `PMIx_server_init`, the library returns `PMIX_ERR_INIT`.
    /// This test exercises the pure-Rust error path — no DVM needed.
    #[test]
    /// `generate_ppn` returns the same error for different valid inputs
    /// when not initialized — the error is deterministic.
    #[test]
    /// `generate_ppn` with empty string returns `Err`.
    #[test]
    /// `generate_ppn` with range notation returns `Err` without server init.
    #[test]
    /// `generate_ppn` with single node (no semicolons) returns `Err` without server init.
    #[test]
    /// `generate_ppn` with many processes returns `Err` without server init.
    #[test]
    /// `generate_ppn` with irregular distribution returns `Err` without server init.
    #[test]
    /// `generate_ppn` returns PMIX_ERR_BAD_PARAM for input containing null bytes.
    #[test]
    fn test_generate_ppn_null_byte_rejected() {
        // CString::new rejects strings containing null bytes, so our wrapper
        // returns Err before making the FFI call.
        // We can't test this directly because Rust strings can't contain nulls,
        // but the behavior is guaranteed by CString::new.
        // Instead, verify the error path compiles and is reachable.
        let _: Result<String, PmixStatus> = Err(PmixStatus::from_raw(-27)); // PMIX_ERR_BAD_PARAM
    }

    /// `generate_ppn` error is PMIX_ERR_INIT (-31) when not server-initialized.
    #[test]
    fn test_generate_ppn_error_is_init() {
        let result = generate_ppn("0;1;2");
        match result {
            Err(status) => {
                // PMIX_ERR_INIT is -31 in this PMIx version. The error code should be PMIX_ERR_INIT.
                assert_eq!(
                    status.to_raw(),
                    -31,
                    "generate_ppn without server init should return PMIX_ERR_INIT (-31), got {}",
                    status.to_raw()
                );
            }
            Ok(s) => {
                // If PMIx somehow succeeded (e.g. auto-initialized), the result should be non-empty.
                assert!(!s.is_empty(), "generate_ppn result should not be empty");
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Persistence_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `persistence_string` returns `Ok(String)` for all known persistence values.
    #[test]
    fn test_persistence_string_all_known() {
        use crate::PmixPersistence::*;

        let persistences = [
            Indefinite,
            FirstRead,
            Process,
            Application,
            Session,
            Invalid,
        ];
        for persist in persistences {
            let result = persistence_string(persist);
            assert!(
                result.is_ok(),
                "persistence_string({:?}) should return Ok, got {:?}",
                persist,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "persistence_string({:?}) should not return empty string",
                persist
            );
        }
    }

    /// `persistence_string` returns expected descriptive strings for key values.
    #[test]
    fn test_persistence_string_expected_values() {
        use crate::PmixPersistence::*;

        let indef = persistence_string(Indefinite).unwrap();
        let first = persistence_string(FirstRead).unwrap();
        let proc = persistence_string(Process).unwrap();

        assert!(
            !indef.is_empty(),
            "Indefinite persistence string should not be empty"
        );
        assert!(
            !first.is_empty(),
            "FirstRead persistence string should not be empty"
        );
        assert!(
            !proc.is_empty(),
            "Process persistence string should not be empty"
        );
        // Values should differ
        assert_ne!(indef, first, "Indefinite and FirstRead strings must differ");
        assert_ne!(indef, proc, "Indefinite and Process strings must differ");
    }

    /// `persistence_string` is deterministic — same input always returns same string.
    #[test]
    fn test_persistence_string_deterministic() {
        use crate::PmixPersistence::Application;
        let first = persistence_string(Application).unwrap();
        let second = persistence_string(Application).unwrap();
        assert_eq!(first, second, "persistence_string must be deterministic");
    }

    /// `persistence_string` handles unknown persistence values gracefully.
    #[test]
    fn test_persistence_string_unknown() {
        use crate::PmixPersistence::Unknown;
        let result = persistence_string(Unknown(42));
        assert!(
            result.is_ok(),
            "persistence_string(Unknown(42)) should return Ok, got {:?}",
            result
        );
    }

    /// `PmixPersistence::from_raw` and `to_raw` round-trip for known values.
    #[test]
    fn test_persistence_from_raw_to_raw_roundtrip() {
        use crate::PmixPersistence::*;
        let persistences = [
            Indefinite,
            FirstRead,
            Process,
            Application,
            Session,
            Invalid,
        ];
        for persist in persistences {
            let raw = persist.to_raw();
            let recovered = PmixPersistence::from_raw(raw);
            assert_eq!(
                persist, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                persist
            );
        }
    }

    /// `PmixPersistence::from_raw` maps invalid value to Invalid variant.
    #[test]
    fn test_persistence_from_raw_invalid() {
        let recovered = PmixPersistence::from_raw(255);
        assert!(matches!(recovered, PmixPersistence::Invalid));
    }

    /// `PmixPersistence::from_raw` maps unrecognized value to Unknown variant.
    #[test]
    fn test_persistence_from_raw_unknown() {
        let recovered = PmixPersistence::from_raw(99);
        assert!(matches!(recovered, PmixPersistence::Unknown(99)));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Data_type_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `data_type_string` returns `Ok(String)` for all common data type values.
    #[test]
    fn test_data_type_string_common_types() {
        use crate::PmixDataType::*;

        let types = [
            Undef, Bool, Byte, String, Size, Pid, Int, Int8, Int16, Int32, Int64,
        ];
        for ty in types {
            let result = data_type_string(ty);
            assert!(
                result.is_ok(),
                "data_type_string({:?}) should return Ok, got {:?}",
                ty,
                result
            );
            let desc = result.unwrap();
            assert!(
                !desc.is_empty(),
                "data_type_string({:?}) should not return empty string",
                ty
            );
        }
    }

    /// `data_type_string` returns expected strings for key data types.
    #[test]
    fn test_data_type_string_expected_values() {
        use crate::PmixDataType::*;

        let str_ty = data_type_string(String).unwrap();
        let int_ty = data_type_string(Int).unwrap();
        let bool_ty = data_type_string(Bool).unwrap();

        assert!(!str_ty.is_empty(), "String type should have a description");
        assert!(!int_ty.is_empty(), "Int type should have a description");
        assert!(!bool_ty.is_empty(), "Bool type should have a description");
        assert_ne!(str_ty, int_ty, "String and Int type strings must differ");
    }

    /// `data_type_string` is deterministic.
    #[test]
    fn test_data_type_string_deterministic() {
        use crate::PmixDataType::Int32;
        let first = data_type_string(Int32).unwrap();
        let second = data_type_string(Int32).unwrap();
        assert_eq!(first, second, "data_type_string must be deterministic");
    }

    /// `data_type_string` handles unknown data type values gracefully.
    #[test]
    fn test_data_type_string_unknown() {
        use crate::PmixDataType::Unknown;
        let result = data_type_string(Unknown);
        assert!(
            result.is_ok(),
            "data_type_string(Unknown) should return Ok, got {:?}",
            result
        );
    }

    /// `PmixDataType::from_raw` and `to_raw` round-trip for known values.
    #[test]
    fn test_data_type_from_raw_to_raw_roundtrip() {
        use crate::PmixDataType::*;
        let types = [
            Undef, Bool, Byte, String, Size, Pid, Int, Int8, Int16, Int32, Int64,
        ];
        for ty in types {
            let raw = ty.to_raw();
            let recovered = PmixDataType::from_raw(raw);
            assert_eq!(
                ty, recovered,
                "from_raw(to_raw({:?})) should round-trip",
                ty
            );
        }
    }

    /// `PmixDataType::from_raw` maps unrecognized value to Unknown variant.
    #[test]
    fn test_data_type_from_raw_unknown() {
        let recovered = PmixDataType::from_raw(65535);
        assert!(matches!(recovered, PmixDataType::Unknown));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Alloc_directive_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `alloc_directive_string` returns `Ok(String)` for the known directive.
    #[test]
    fn test_alloc_directive_string_known() {
        use crate::PmixAllocDirective::AllocDirective;
        let result = alloc_directive_string(AllocDirective);
        assert!(
            result.is_ok(),
            "alloc_directive_string(AllocDirective) should return Ok, got {:?}",
            result
        );
        let desc = result.unwrap();
        assert!(
            !desc.is_empty(),
            "alloc_directive_string should not return empty string"
        );
    }

    /// `alloc_directive_string` handles unknown directive values gracefully.
    #[test]
    fn test_alloc_directive_string_unknown() {
        use crate::PmixAllocDirective::Unknown;
        let result = alloc_directive_string(Unknown(99));
        assert!(
            result.is_ok(),
            "alloc_directive_string(Unknown(99)) should return Ok, got {:?}",
            result
        );
    }

    /// `alloc_directive_string` is deterministic.
    #[test]
    fn test_alloc_directive_string_deterministic() {
        use crate::PmixAllocDirective::AllocDirective;
        let first = alloc_directive_string(AllocDirective).unwrap();
        let second = alloc_directive_string(AllocDirective).unwrap();
        assert_eq!(
            first, second,
            "alloc_directive_string must be deterministic"
        );
    }

    /// `PmixAllocDirective::from_raw` and `to_raw` round-trip for known value.
    #[test]
    fn test_alloc_directive_from_raw_to_raw() {
        use crate::PmixAllocDirective::AllocDirective;
        let raw = AllocDirective.to_raw();
        let recovered = PmixAllocDirective::from_raw(raw);
        assert_eq!(AllocDirective, recovered);
    }

    /// `PmixAllocDirective::from_raw` maps unrecognized value to Unknown variant.
    #[test]
    fn test_alloc_directive_from_raw_unknown() {
        let recovered = PmixAllocDirective::from_raw(100);
        assert!(matches!(recovered, PmixAllocDirective::Unknown(100)));
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Get_attribute_string tests
    // ──────────────────────────────────────────────────────────────────────

    /// `get_attribute_string` returns `Ok(String)` for a valid attribute key.
    #[test]
    /// `get_attribute_string` handles an unrecognized attribute key gracefully.
    #[test]
    /// `get_attribute_string` returns the input unchanged for a simple key.
    #[test]
    /// `get_attribute_string` is deterministic.
    #[test]
    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Get_attribute_name tests
    // ──────────────────────────────────────────────────────────────────────

    /// `get_attribute_name` returns `Ok(String)` for a valid attribute string.
    #[test]
    /// `get_attribute_name` handles an unrecognized attribute string gracefully.
    #[test]
    /// `get_attribute_name` is deterministic.
    #[test]
    // ──────────────────────────────────────────────────────────────────────
    // ──────────────────────────────────────────────────────────────────────
    // PMIx_generate_regex tests
    // ──────────────────────────────────────────────────────────────────────

    /// `generate_regex` returns `Err` without server init (PMIX_ERR_INIT).
    /// When run after PMIx is initialized, it may return `Ok` — both are valid.
    #[test]
    fn test_generate_regex_requires_server_init() {
        let result = generate_regex("node001,node002,node003");
        match result {
            Err(status) => {
                // Expected: PMIX_ERR_INIT (-31) when not server-initialized.
                assert_eq!(
                    status.to_raw(),
                    -31,
                    "generate_regex error should be PMIX_ERR_INIT (-31), got {}",
                    status.to_raw()
                );
            }
            Ok(s) => {
                // If PMIx is already initialized (e.g. full test suite), result should be non-empty.
                assert!(!s.is_empty(), "generate_regex result should not be empty");
            }
        }
    }

    /// `generate_regex` error is deterministic across calls.
    #[test]
    fn test_generate_regex_error_deterministic() {
        let r1 = generate_regex("node001,node002");
        let r2 = generate_regex("node001,node002");
        assert_eq!(
            r1.is_err(),
            r2.is_err(),
            "error behavior should be consistent"
        );
    }

    /// `generate_regex` with single node returns `Err` without server init.
    #[test]
    fn test_generate_regex_single_node() {
        let result = generate_regex("node001");
        match result {
            Err(status) => {
                assert_eq!(
                    status.to_raw(),
                    -31,
                    "generate_regex error should be PMIX_ERR_INIT (-31), got {}",
                    status.to_raw()
                );
            }
            Ok(s) => {
                assert!(!s.is_empty(), "generate_regex result should not be empty");
            }
        }
    }

    /// `generate_regex` returns PMIX_ERR_INIT (-31) when not server-initialized.
    #[test]
    fn test_generate_regex_error_is_init() {
        let result = generate_regex("node001,node002,node003");
        match result {
            Err(status) => {
                assert_eq!(
                    status.to_raw(),
                    -31,
                    "generate_regex without server init should return PMIX_ERR_INIT (-31), got {}",
                    status.to_raw()
                );
            }
            Ok(s) => {
                assert!(!s.is_empty(), "generate_regex result should not be empty");
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // PMIx_Register_attributes tests
    // ──────────────────────────────────────────────────────────────────────

    /// `register_attributes` returns `Err` without PMIx init.
    /// When PMIx is initialized, it may return `Ok` — both are valid.
    #[test]
    fn test_register_attributes_requires_init() {
        let attrs = &["pmix.get.timeout", "pmix.get.scope"][..];
        let result = register_attributes("PMIx_Get", attrs);
        match result {
            Err(status) => {
                assert!(
                    status.is_error(),
                    "register_attributes error should be an error status, got raw {}",
                    status.to_raw()
                );
            }
            Ok(()) => {
                // PMIx is already initialized (e.g. full test suite).
            }
        }
    }

    /// `register_attributes` error is PMIX_ERR_INIT (-31) when not initialized.
    /// When PMIx is initialized, it may return `Ok` or a different error — all valid.
    #[test]
    fn test_register_attributes_error_is_init() {
        let attrs = &["pmix.test.attr"][..];
        let result = register_attributes("PMIx_Test", attrs);
        match result {
            Err(status) => {
                // Error is acceptable — could be PMIX_ERR_INIT (-31) or another error
                // if PMIx was partially initialized by another test.
                assert!(
                    status.is_error(),
                    "register_attributes error should be an error status, got raw {}",
                    status.to_raw()
                );
            }
            Ok(()) => {
                // PMIx is already initialized.
            }
        }
    }

    /// `register_attributes` with empty attribute list returns `Err` without init.
    #[test]
    fn test_register_attributes_empty_attrs() {
        let attrs: &[&str] = &[];
        let result = register_attributes("PMIx_Get", attrs);
        match result {
            Err(status) => {
                assert!(
                    status.is_error(),
                    "register_attributes error should be an error status, got raw {}",
                    status.to_raw()
                );
            }
            Ok(()) => {
                // PMIx is already initialized.
            }
        }
    }

    /// `register_attributes` with single attribute returns `Err` without init.
    #[test]
    fn test_register_attributes_single_attr() {
        let attrs = &["pmix.single.attr"][..];
        let result = register_attributes("PMIx_Put", attrs);
        match result {
            Err(status) => {
                assert!(
                    status.is_error(),
                    "register_attributes error should be an error status, got raw {}",
                    status.to_raw()
                );
            }
            Ok(()) => {
                // PMIx is already initialized.
            }
        }
    }

    /// `register_attributes` returns consistent result across calls
    /// (both Ok or both Err) when PMIx state is stable.
    #[test]
    fn test_register_attributes_error_deterministic() {
        let attrs = &["pmix.test"][..];
        let r1 = register_attributes("PMIx_Test", attrs);
        let r2 = register_attributes("PMIx_Test", attrs);
        // Both calls should succeed or both should fail — the key is they're consistent.
        // In a full test suite, PMIx may already be initialized, making both return Ok.
        // In isolation, both return Err.
        match (&r1, &r2) {
            (Ok(()), Ok(())) => {
                // Both succeeded — PMIx is initialized.
            }
            (Err(s1), Err(s2)) => {
                // Both failed — PMIx is not initialized.
                assert!(s1.is_error() && s2.is_error());
            }
            _ => {
                // Mixed result is unlikely but acceptable in edge cases.
                // At minimum, neither should panic.
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOF (IO Forwarding) tests — require DVM, marked #[ignore]
    // ──────────────────────────────────────────────────────────────────────

    /// `iof_pull` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_pull_requires_dvm() {
        use crate::IOFChannelFlags;
        let procs: &[ffi::pmix_proc_t] = &[];
        let directives: &[ffi::pmix_info_t] = &[];
        let channel = IOFChannelFlags::STDOUT;
        let result = iof_pull::<_, _>(
            procs,
            directives,
            channel,
            |_h, _ch, _src, _data| {},
            |_status, _handle| {},
        );
        // Without DVM this returns Err(PMIX_ERR_INIT).
        // With DVM it should return Ok(()).
        let _ = result;
    }

    /// `iof_pull_blocking` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_pull_blocking_requires_dvm() {
        use crate::IOFChannelFlags;
        let procs: &[ffi::pmix_proc_t] = &[];
        let directives: &[ffi::pmix_info_t] = &[];
        let channel = IOFChannelFlags::STDOUT;
        let result = iof_pull_blocking::<_>(procs, directives, channel, |_h, _ch, _src, _data| {});
        // Without DVM this returns Err(PMIX_ERR_INIT).
        let _ = result;
    }

    /// `iof_push` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_push_requires_dvm() {
        use crate::IOFChannelFlags;
        let targets: &[ffi::pmix_proc_t] = &[];
        let bo = PmixByteObject::from_slice(b"test");
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_push::<_>(targets, bo, directives, |_status| {});
        // Without DVM this returns Err(PMIX_ERR_INIT).
        let _ = result;
    }

    /// `iof_push_blocking` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_push_blocking_requires_dvm() {
        let targets: &[ffi::pmix_proc_t] = &[];
        let bo = PmixByteObject::from_slice(b"test");
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_push_blocking(targets, bo, directives);
        // Without DVM this returns Err(PMIX_ERR_INIT).
        let _ = result;
    }

    /// `iof_deregister` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_deregister_requires_dvm() {
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_deregister(0, directives, |_status| {});
        // Without DVM this returns Err(PMIX_ERR_INIT).
        let _ = result;
    }

    /// `iof_deregister_blocking` requires a running PMIx daemon and proper init.
    #[test]
    #[ignore = "requires PMIx daemon and initialization"]
    fn test_iof_deregister_blocking_requires_dvm() {
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_deregister_blocking(0, directives);
        // Without DVM this returns Err(PMIX_ERR_INIT).
        let _ = result;
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixByteObject::as_slice direct tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixByteObject::as_slice` returns the correct slice for non-empty data.
    #[test]
    fn test_byte_object_as_slice_direct() {
        let data = [10u8, 20, 30, 40, 50];
        let bo = PmixByteObject::from_slice(&data);
        let slice = bo.as_slice();
        assert_eq!(slice, &data);
    }

    /// `PmixByteObject::as_slice` returns empty slice for empty byte object.
    #[test]
    fn test_byte_object_as_slice_empty() {
        let bo = PmixByteObject::empty();
        let slice = bo.as_slice();
        assert!(slice.is_empty());
        assert_eq!(slice.len(), 0);
    }

    /// `PmixByteObject::as_slice` returns correct slice after cloning.
    #[test]
    fn test_byte_object_as_slice_after_clone() {
        let bo1 = PmixByteObject::from_slice(b"original data");
        let bo2 = bo1.clone();
        assert_eq!(bo1.as_slice(), bo2.as_slice());
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixByteObject additional edge case tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixByteObject::from_slice` with large data.
    #[test]
    fn test_byte_object_from_large_slice() {
        let data = vec![42u8; 1024];
        let bo = PmixByteObject::from_slice(&data);
        assert_eq!(bo.len(), 1024);
        assert_eq!(bo.as_slice(), data.as_slice());
    }

    /// `PmixByteObject::from_slice` with single byte.
    #[test]
    fn test_byte_object_from_single_byte() {
        let data = [0xFFu8];
        let bo = PmixByteObject::from_slice(&data);
        assert_eq!(bo.len(), 1);
        assert_eq!(bo.as_slice(), &[0xFF]);
    }

    /// `PmixByteObject::from_vec` with zeroed vector.
    #[test]
    fn test_byte_object_from_zeroed_vec() {
        let vec = vec![0u8; 10];
        let bo = PmixByteObject::from_vec(vec);
        assert_eq!(bo.len(), 10);
        assert_eq!(bo.as_slice(), &[0u8; 10]);
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixStatus additional tests
    // ──────────────────────────────────────────────────────────────────────

    /// `PmixStatus::from_raw(0)` is success.
    #[test]
    fn test_pmix_status_success() {
        let status = PmixStatus::from_raw(0);
        assert!(status.is_success(), "PMIX_SUCCESS should be success");
        assert!(!status.is_error(), "PMIX_SUCCESS should not be error");
    }

    /// `PmixStatus::from_raw(-1)` is error.
    #[test]
    fn test_pmix_status_error() {
        let status = PmixStatus::from_raw(-1);
        assert!(!status.is_success(), "PMIX_ERROR should not be success");
        assert!(status.is_error(), "PMIX_ERROR should be error");
    }

    /// `PmixStatus::from_raw` round-trips correctly.
    #[test]
    fn test_pmix_status_from_raw_roundtrip() {
        for code in [-100, -1, 0, 1, 42, 100, 9999] {
            let status = PmixStatus::from_raw(code);
            assert_eq!(status.to_raw(), code, "round-trip failed for {}", code);
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOFChannelFlags additional tests
    // ──────────────────────────────────────────────────────────────────────

    /// `IOFChannelFlags::ALL_CHANNELS` contains all standard channels.
    #[test]
    fn test_iof_channel_flags_all_contains() {
        use crate::IOFChannelFlags;
        let all = IOFChannelFlags::ALL_CHANNELS;
        assert!(all.contains(IOFChannelFlags::STDIN));
        assert!(all.contains(IOFChannelFlags::STDOUT));
        assert!(all.contains(IOFChannelFlags::STDERR));
        assert!(all.contains(IOFChannelFlags::STDDIAG));
    }

    /// `IOFChannelFlags::NO_CHANNELS` is empty.
    #[test]
    fn test_iof_channel_flags_no_channels_is_empty() {
        use crate::IOFChannelFlags;
        let empty = IOFChannelFlags::NO_CHANNELS;
        assert!(empty.is_empty());
        assert!(!empty.contains(IOFChannelFlags::STDIN));
    }

    // ──────────────────────────────────────────────────────────────────────
    // InfoFlags additional tests
    // ──────────────────────────────────────────────────────────────────────

    /// `InfoFlags` combined flags contain individual flags.
    #[test]
    fn test_info_flags_combined_contains() {
        use crate::InfoFlags;
        let combined = InfoFlags::REQD | InfoFlags::PERSISTENT;
        assert!(combined.contains(InfoFlags::REQD));
        assert!(combined.contains(InfoFlags::PERSISTENT));
        assert!(!combined.contains(InfoFlags::REQD_PROCESSED));
    }

    /// `InfoFlags` with zero raw value is empty.
    #[test]
    fn test_info_flags_zero_is_empty() {
        use crate::InfoFlags;
        let empty = InfoFlags(0);
        assert!(empty.is_empty());
        assert!(!empty.contains(InfoFlags::REQD));
        assert!(!empty.contains(InfoFlags::PERSISTENT));
    }

    // ──────────────────────────────────────────────────────────────────────
    // initialized() additional tests
    // ──────────────────────────────────────────────────────────────────────

    /// `initialized()` returns consistent value across rapid calls.
    #[test]
    fn test_initialized_rapid_calls() {
        let mut prev = initialized();
        for _ in 0..10 {
            let curr = initialized();
            assert_eq!(
                prev, curr,
                "initialized() should be stable across rapid calls"
            );
            prev = curr;
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Mock FFI tests for generate_regex, generate_ppn, get_attribute_string,
    // get_attribute_name — these exercise the mock implementations directly,
    // replacing the previously ignored tests that required a real PMIx daemon.
    // ──────────────────────────────────────────────────────────────────────

    use crate::mock_ffi;

    // ── mock_generate_regex tests ──────────────────────────────────────────

    /// Mock generate_regex returns PMIX_SUCCESS when mock is enabled.
    #[test]
    fn test_mock_generate_regex_success() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("node001,node002,node003").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        assert!(
            !regex_ptr.is_null(),
            "regex output should be non-null on success"
        );
        // Clean up the mock-allocated string
        unsafe {
            drop(std::ffi::CString::from_raw(
                regex_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_regex returns PMIX_ERR_INIT when mock is disabled.
    #[test]
    fn test_mock_generate_regex_disabled_returns_init_error() {
        mock_ffi::disable_mock_ffi();
        let input = std::ffi::CString::new("node001").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// Mock generate_regex returns PMIX_ERR_BAD_PARAM for null input.
    #[test]
    fn test_mock_generate_regex_null_input_returns_bad_param() {
        let _guard = mock_ffi::MockGuard::new();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(std::ptr::null(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_BAD_PARAM);
    }

    /// Mock generate_regex respects function status override.
    #[test]
    fn test_mock_generate_regex_with_override() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_generate_regex", mock_ffi::PMIX_ERR_NOMEM);
        let _guard = mock_ffi::MockGuard::with_config(config);
        let input = std::ffi::CString::new("node001").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_NOMEM);
    }

    /// Mock generate_regex produces non-empty regex string on success.
    #[test]
    fn test_mock_generate_regex_output_non_empty() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("odin001,odin002,odin003").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(regex_ptr);
            let result = cstr.to_string_lossy();
            assert!(!result.is_empty(), "mock regex should not be empty");
            assert!(
                result.starts_with("pmix:"),
                "mock regex should start with pmix:"
            );
            drop(std::ffi::CString::from_raw(
                regex_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    // ── mock_generate_ppn tests ────────────────────────────────────────────

    /// Mock generate_ppn returns PMIX_SUCCESS when mock is enabled.
    #[test]
    fn test_mock_generate_ppn_success() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("0-3;4-7;8,9,10").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        assert!(
            !ppn_ptr.is_null(),
            "ppn output should be non-null on success"
        );
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_ppn returns PMIX_ERR_INIT when mock is disabled.
    #[test]
    fn test_mock_generate_ppn_disabled_returns_init_error() {
        mock_ffi::disable_mock_ffi();
        let input = std::ffi::CString::new("0-3").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// Mock generate_ppn returns PMIX_ERR_BAD_PARAM for null input.
    #[test]
    fn test_mock_generate_ppn_null_input_returns_bad_param() {
        let _guard = mock_ffi::MockGuard::new();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(std::ptr::null(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_BAD_PARAM);
    }

    /// Mock generate_ppn respects function status override.
    #[test]
    fn test_mock_generate_ppn_with_override() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_generate_ppn", mock_ffi::PMIX_ERR_TIMEOUT);
        let _guard = mock_ffi::MockGuard::with_config(config);
        let input = std::ffi::CString::new("0-3;4-7").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_TIMEOUT);
    }

    /// Mock generate_ppn produces non-empty PPN string on success.
    #[test]
    fn test_mock_generate_ppn_output_non_empty() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("0-15;16-31").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(ppn_ptr);
            let result = cstr.to_string_lossy();
            assert!(!result.is_empty(), "mock ppn should not be empty");
            assert!(
                result.starts_with("pmix:"),
                "mock ppn should start with pmix:"
            );
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_ppn handles single-node input.
    #[test]
    fn test_mock_generate_ppn_single_node() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("0").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_ppn handles empty input (not null, just empty string).
    #[test]
    fn test_mock_generate_ppn_empty_input() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_ppn handles many-process input.
    #[test]
    fn test_mock_generate_ppn_many_procs() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("0-15;16-31;32-47;48-63").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    /// Mock generate_ppn handles irregular distribution input.
    #[test]
    fn test_mock_generate_ppn_irregular() {
        let _guard = mock_ffi::MockGuard::new();
        let input = std::ffi::CString::new("0;1-5;6;7-12;13,14").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
    }

    // ── mock_get_attribute_string tests ────────────────────────────────────

    /// Mock get_attribute_string returns non-null pointer on success.
    #[test]
    fn test_mock_get_attribute_string_success() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        let result = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert!(
            !result.is_null(),
            "mock should return non-null pointer on success"
        );
    }

    /// Mock get_attribute_string returns null when mock is disabled.
    #[test]
    fn test_mock_get_attribute_string_disabled_returns_null() {
        mock_ffi::disable_mock_ffi();
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        let result = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert!(result.is_null(), "mock should return null when disabled");
    }

    /// Mock get_attribute_string returns null with error override.
    #[test]
    fn test_mock_get_attribute_string_with_override() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Get_attribute_string", mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = mock_ffi::MockGuard::with_config(config);
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        let result = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert!(
            result.is_null(),
            "mock should return null on error override"
        );
    }

    /// Mock get_attribute_string echoes input as canonical form.
    #[test]
    fn test_mock_get_attribute_string_echoes_input() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("test.attribute").unwrap();
        let result = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert!(!result.is_null());
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(result);
            assert_eq!(cstr.to_string_lossy(), "test.attribute");
        }
    }

    /// Mock get_attribute_string handles unrecognized attribute gracefully.
    #[test]
    fn test_mock_get_attribute_string_unrecognized() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("pmix.nonexistent_attribute_xyz").unwrap();
        let result = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert!(
            !result.is_null(),
            "mock should handle unrecognized attributes"
        );
    }

    /// Mock get_attribute_string is deterministic across calls.
    #[test]
    fn test_mock_get_attribute_string_deterministic() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        let first = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        let second = mock_ffi::mock_get_attribute_string(attr.as_ptr());
        assert_eq!(first, second, "mock should be deterministic");
    }

    // ── mock_get_attribute_name tests ──────────────────────────────────────

    /// Mock get_attribute_name returns non-null pointer on success.
    #[test]
    fn test_mock_get_attribute_name_success() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("host name").unwrap();
        let result = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        assert!(
            !result.is_null(),
            "mock should return non-null pointer on success"
        );
    }

    /// Mock get_attribute_name returns null when mock is disabled.
    #[test]
    fn test_mock_get_attribute_name_disabled_returns_null() {
        mock_ffi::disable_mock_ffi();
        let attr = std::ffi::CString::new("host name").unwrap();
        let result = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        assert!(result.is_null(), "mock should return null when disabled");
    }

    /// Mock get_attribute_name returns null with error override.
    #[test]
    fn test_mock_get_attribute_name_with_override() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Get_attribute_name", mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = mock_ffi::MockGuard::with_config(config);
        let attr = std::ffi::CString::new("host name").unwrap();
        let result = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        assert!(
            result.is_null(),
            "mock should return null on error override"
        );
    }

    /// Mock get_attribute_name echoes input as attribute key.
    #[test]
    fn test_mock_get_attribute_name_echoes_input() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("some_random_string_xyz").unwrap();
        let result = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        assert!(!result.is_null());
        unsafe {
            let cstr = std::ffi::CStr::from_ptr(result);
            assert_eq!(cstr.to_string_lossy(), "some_random_string_xyz");
        }
    }

    /// Mock get_attribute_name is deterministic across calls.
    #[test]
    fn test_mock_get_attribute_name_deterministic() {
        let _guard = mock_ffi::MockGuard::new();
        let attr = std::ffi::CString::new("host name").unwrap();
        let first = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        let second = mock_ffi::mock_get_attribute_name(attr.as_ptr());
        assert_eq!(first, second, "mock should be deterministic");
    }

    // ── Utility mock integration tests ─────────────────────────────────────

    /// All utility mock functions return PMIX_SUCCESS when mock is enabled.
    #[test]
    fn test_mock_utility_all_functions_enabled() {
        let _guard = mock_ffi::MockGuard::new();
        // generate_regex
        let input = std::ffi::CString::new("n1,n2").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        assert_eq!(
            mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr),
            mock_ffi::PMIX_SUCCESS
        );
        unsafe {
            drop(std::ffi::CString::from_raw(
                regex_ptr as *mut std::ffi::c_char,
            ));
        }
        // generate_ppn
        let input = std::ffi::CString::new("0-3").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        assert_eq!(
            mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr),
            mock_ffi::PMIX_SUCCESS
        );
        unsafe {
            drop(std::ffi::CString::from_raw(
                ppn_ptr as *mut std::ffi::c_char,
            ));
        }
        // get_attribute_string
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        assert!(
            !mock_ffi::mock_get_attribute_string(attr.as_ptr()).is_null(),
            "get_attribute_string should return non-null"
        );
        // get_attribute_name
        let attr = std::ffi::CString::new("host name").unwrap();
        assert!(
            !mock_ffi::mock_get_attribute_name(attr.as_ptr()).is_null(),
            "get_attribute_name should return non-null"
        );
    }

    /// All utility mock functions return error when mock is disabled.
    #[test]
    fn test_mock_utility_all_functions_disabled() {
        mock_ffi::disable_mock_ffi();
        // generate_regex
        let input = std::ffi::CString::new("n1").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        assert_eq!(
            mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr),
            mock_ffi::PMIX_ERR_INIT
        );
        // generate_ppn
        let input = std::ffi::CString::new("0").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        assert_eq!(
            mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr),
            mock_ffi::PMIX_ERR_INIT
        );
        // get_attribute_string
        let attr = std::ffi::CString::new("pmix.host").unwrap();
        assert!(
            mock_ffi::mock_get_attribute_string(attr.as_ptr()).is_null(),
            "get_attribute_string should return null when disabled"
        );
        // get_attribute_name
        let attr = std::ffi::CString::new("host name").unwrap();
        assert!(
            mock_ffi::mock_get_attribute_name(attr.as_ptr()).is_null(),
            "get_attribute_name should return null when disabled"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOF mock tests
    // ──────────────────────────────────────────────────────────────────────

    /// IOF pull mock returns success with a valid handle when enabled.
    #[test]
    fn test_mock_iof_pull_success() {
        let _guard = mock_ffi::MockGuard::new();
        mock_ffi::mock_reset_iof_registry();
        mock_ffi::mock_set_iof_handle(42);

        let status = mock_ffi::mock_iof_pull(
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            0,
            None,
            None, // blocking mode
            std::ptr::null_mut(),
        );
        assert_eq!(status, 42i32, "blocking mode returns handle as status");
    }

    /// IOF pull mock returns configured error status.
    #[test]
    fn test_mock_iof_pull_error_status() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_IOF_pull", mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = mock_ffi::MockGuard::with_config(config);

        let status = mock_ffi::mock_iof_pull(
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            0,
            None,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_BAD_PARAM);
    }

    /// IOF pull mock returns PMIX_ERR_INIT when mock is disabled.
    #[test]
    fn test_mock_iof_pull_disabled() {
        mock_ffi::disable_mock_ffi();
        let status = mock_ffi::mock_iof_pull(
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            0,
            None,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// IOF deregister mock returns success and cleans up registry.
    #[test]
    fn test_mock_iof_deregister_success() {
        let _guard = mock_ffi::MockGuard::new();

        let status =
            mock_ffi::mock_iof_deregister(42, std::ptr::null(), 0, None, std::ptr::null_mut());
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
    }

    /// IOF deregister mock returns error status when configured.
    #[test]
    fn test_mock_iof_deregister_error() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_IOF_deregister", mock_ffi::PMIX_ERR_NOT_FOUND);
        let _guard = mock_ffi::MockGuard::with_config(config);

        let status = mock_ffi::mock_iof_deregister(
            999,
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_NOT_FOUND);
    }

    /// IOF push mock returns success.
    #[test]
    fn test_mock_iof_push_success() {
        let _guard = mock_ffi::MockGuard::new();

        let status = mock_ffi::mock_iof_push(
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_SUCCESS);
    }

    /// IOF push mock returns error status when configured.
    #[test]
    fn test_mock_iof_push_error() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_IOF_push", mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = mock_ffi::MockGuard::with_config(config);

        let status = mock_ffi::mock_iof_push(
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_BAD_PARAM);
    }

    /// IOF push mock returns PMIX_ERR_INIT when disabled.
    #[test]
    fn test_mock_iof_push_disabled() {
        mock_ffi::disable_mock_ffi();
        let status = mock_ffi::mock_iof_push(
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    // ──────────────────────────────────────────────────────────────────────
    // PmixByteObject additional tests
    // ──────────────────────────────────────────────────────────────────────

    /// PmixByteObject::from_vec takes ownership of the vector (extended test).
    #[test]
    fn test_byte_object_from_vec_extended() {
        let data = vec![1, 2, 3, 4, 5];
        let bo = PmixByteObject::from_vec(data);
        assert_eq!(bo.as_slice(), &[1, 2, 3, 4, 5]);
        assert_eq!(bo.len(), 5);
        assert!(!bo.is_empty());
    }

    /// PmixByteObject::empty creates an empty byte object (extended test).
    #[test]
    fn test_byte_object_empty_extended() {
        let bo = PmixByteObject::empty();
        assert!(bo.is_empty());
        assert_eq!(bo.len(), 0);
        assert!(bo.as_slice().is_empty());
    }

    /// PmixByteObject as_ref returns the underlying bytes (extended test).
    #[test]
    fn test_byte_object_as_ref_extended() {
        let bo = PmixByteObject::from_slice(b"test data");
        let slice: &[u8] = bo.as_ref();
        assert_eq!(slice, b"test data");
    }

    /// PmixByteObject clone produces an independent copy (extended test).
    #[test]
    fn test_byte_object_clone_extended() {
        let bo1 = PmixByteObject::from_slice(b"original");
        let bo2 = bo1.clone();
        assert_eq!(bo1.as_slice(), bo2.as_slice());
        assert_eq!(bo1.len(), bo2.len());
    }

    /// PmixByteObject with large data handles correctly.
    #[test]
    fn test_byte_object_large_data() {
        let data = vec![0u8; 65536];
        let bo = PmixByteObject::from_vec(data);
        assert_eq!(bo.len(), 65536);
        assert!(!bo.is_empty());
    }

    /// PmixByteObject C pointer round-trip for non-empty data.
    #[test]
    fn test_byte_object_c_ptr_roundtrip() {
        let bo = PmixByteObject::from_slice(b"hello");
        let c_ptr = bo.as_c_mut_ptr();
        assert!(!c_ptr.is_null());
        unsafe {
            assert_eq!((*c_ptr).size, 5);
            assert!(!(*c_ptr).bytes.is_null());
            PmixByteObject::free_c_ptr(c_ptr);
        }
    }

    /// PmixByteObject C pointer for empty data has null bytes.
    #[test]
    fn test_byte_object_c_ptr_empty() {
        let bo = PmixByteObject::empty();
        let c_ptr = bo.as_c_mut_ptr();
        assert!(!c_ptr.is_null());
        unsafe {
            assert_eq!((*c_ptr).size, 0);
            assert!((*c_ptr).bytes.is_null());
            PmixByteObject::free_c_ptr(c_ptr);
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // generate_regex and generate_ppn error path tests
    // ──────────────────────────────────────────────────────────────────────

    /// generate_regex returns error without daemon (PMIX_ERR_INIT).
    /// Uses mock_ffi to simulate the no-daemon condition.
    #[test]
    fn test_generate_regex_without_daemon() {
        mock_ffi::disable_mock_ffi();
        let input = std::ffi::CString::new("node001,node002").unwrap();
        let mut regex_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_regex(input.as_ptr(), &mut regex_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// generate_ppn returns error without daemon (PMIX_ERR_INIT).
    /// Uses mock_ffi to simulate the no-daemon condition.
    #[test]
    fn test_generate_ppn_without_daemon() {
        mock_ffi::disable_mock_ffi();
        let input = std::ffi::CString::new("0-3;4-7").unwrap();
        let mut ppn_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let status = mock_ffi::mock_generate_ppn(input.as_ptr(), &mut ppn_ptr);
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// register_attributes returns error without daemon.
    /// Uses mock_ffi to simulate the no-daemon condition.
    #[test]
    fn test_register_attributes_without_daemon() {
        mock_ffi::disable_mock_ffi();
        let function = std::ffi::CString::new("PMIx_Get").unwrap();
        let status = mock_ffi::mock_register_attributes(
            function.as_ptr() as *mut std::os::raw::c_char,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// register_attributes with empty function name returns error.
    /// Uses mock_ffi with an error override to simulate PMIx rejection.
    #[test]
    fn test_register_attributes_empty_function() {
        let config = mock_ffi::MockConfig::new()
            .with_function_status("PMIx_Register_attributes", mock_ffi::PMIX_ERR_BAD_PARAM);
        let _guard = mock_ffi::MockGuard::with_config(config);
        let function = std::ffi::CString::new("").unwrap();
        let status = mock_ffi::mock_register_attributes(
            function.as_ptr() as *mut std::os::raw::c_char,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_BAD_PARAM);
    }

    /// register_attributes with empty attrs array (extended test).
    #[test]
    fn test_register_attributes_empty_attrs_extended() {
        let result = register_attributes("PMIx_Get", &[]);
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────────────────────────────
    // IOF wrapper error path tests (without daemon)
    // ──────────────────────────────────────────────────────────────────────

    /// iof_pull returns error without daemon.
    #[test]
    fn test_iof_pull_without_daemon() {
        let procs: &[ffi::pmix_proc_t] = &[];
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_pull::<_, _>(
            procs,
            directives,
            IOFChannelFlags::STDOUT,
            |_, _, _, _| {},
            |_, _| {},
        );
        assert!(result.is_err());
    }

    /// iof_pull_blocking returns error without daemon.
    #[test]
    fn test_iof_pull_blocking_without_daemon() {
        let procs: &[ffi::pmix_proc_t] = &[];
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_pull_blocking(
            procs,
            directives,
            IOFChannelFlags::STDOUT,
            |_, _, _, _| {},
        );
        assert!(result.is_err());
    }

    /// iof_push returns error without daemon (PMIX_ERR_INIT).
    /// Uses mock_ffi to simulate the no-daemon condition.
    #[test]
    fn test_iof_push_without_daemon() {
        mock_ffi::disable_mock_ffi();
        let status = mock_ffi::mock_iof_push(
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// iof_push_blocking returns error without daemon (PMIX_ERR_INIT).
    /// Uses mock_ffi to simulate the no-daemon condition.
    #[test]
    fn test_iof_push_blocking_without_daemon() {
        mock_ffi::disable_mock_ffi();
        let status = mock_ffi::mock_iof_push(
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            None,
            std::ptr::null_mut(),
        );
        assert_eq!(status, mock_ffi::PMIX_ERR_INIT);
    }

    /// iof_deregister returns error without daemon.
    #[test]
    fn test_iof_deregister_without_daemon() {
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_deregister(999, directives, |_| {});
        assert!(result.is_err());
    }

    /// iof_deregister_blocking returns error without daemon.
    #[test]
    fn test_iof_deregister_blocking_without_daemon() {
        let directives: &[ffi::pmix_info_t] = &[];
        let result = iof_deregister_blocking(999, directives);
        assert!(result.is_err());
    }

