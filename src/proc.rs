//! PMIx process handle (`pmix_proc_t` wrapper).

use crate::ffi::*;
use std::ffi::{CStr, CString, NulError};
use std::mem;
use std::os::raw::c_char;

impl Proc {
    pub fn new(nspace: &str, rank: u32) -> Result<Self, NulError> {
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        let c_name = CString::new(nspace)?;
        unsafe {
            PMIx_Load_nspace(handle.nspace.as_mut_ptr(), c_name.as_ptr());
        }
        Ok(Proc { handle, len: 1 })
    }

    pub fn new_with_nspace(&self, rank: u32) -> Result<Self, NulError> {
        let mut handle: pmix_proc_t;
        unsafe {
            handle = mem::zeroed();
            PMIx_Proc_construct(&mut handle);
        }
        handle.rank = rank;
        unsafe {
            let src_handle = self.handle;
            PMIx_Load_nspace(handle.nspace.as_mut_ptr(), src_handle.nspace.as_ptr());
        }
        Ok(Proc { handle, len: 1 })
    }

    pub fn get_rank(&self) -> u32 {
        self.handle.rank
    }

    pub fn set_rank(&mut self, rank: u32) {
        self.handle.rank = rank;
    }
}

#[derive(Clone)]
pub struct Proc {
    pub(crate) handle: pmix_proc_t,
    pub(crate) len: usize,
}

