#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]

use std::os::raw::c_char;

// pmix_proc_state_t is typedef'd as uint8_t in pmix_common.h
pub type pmix_proc_state_t = u8;

// pmix_scope_t is typedef'd as uint8_t in pmix_common.h
pub type pmix_scope_t = u8;

// Manual FFI declarations for functions not yet covered by bindgen.
// When bindgen becomes available, these can be removed in favor of
// the auto-generated declarations.
extern "C" {
    pub fn PMIx_Initialized() -> ::std::os::raw::c_int;
    pub fn PMIx_Error_string(status: i32) -> *const c_char;
    pub fn PMIx_Proc_state_string(state: pmix_proc_state_t) -> *const c_char;
    pub fn PMIx_Scope_string(scope: pmix_scope_t) -> *const c_char;
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
