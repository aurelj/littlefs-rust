//! Block device config. Per lfs.h struct lfs_config.
//! Callbacks use raw function pointers for C-compatible layout.

#![allow(non_camel_case_types)]

use crate::types::{lfs_block_t, lfs_off_t, lfs_size_t};

/// Per lfs.h struct lfs_config.
/// Layout matches C for potential FFI. Callbacks use Option to allow null.
#[repr(C)]
pub struct LfsConfig {
    pub context: *mut core::ffi::c_void,
    pub read_size: lfs_size_t,
    pub prog_size: lfs_size_t,
    pub block_size: lfs_size_t,
    pub block_count: lfs_size_t,
    pub block_cycles: i32,
    pub cache_size: lfs_size_t,
    pub lookahead_size: lfs_size_t,
    pub compact_thresh: lfs_size_t,
    pub read_buffer: *mut core::ffi::c_void,
    pub prog_buffer: *mut core::ffi::c_void,
    pub lookahead_buffer: *mut core::ffi::c_void,
    pub name_max: lfs_size_t,
    pub file_max: lfs_size_t,
    pub attr_max: lfs_size_t,
    pub metadata_max: lfs_size_t,
    pub inline_max: lfs_size_t,
}
