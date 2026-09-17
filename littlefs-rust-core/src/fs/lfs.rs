//! Main filesystem type. Per lfs.h typedef struct lfs.

use crate::bd::{LfsCache, Storage};
use crate::dir::LfsMlist;
use crate::lfs_config::LfsConfig;
use crate::lfs_gstate::LfsGstate;
use crate::types::lfs_block_t;

use super::lfs_lookahead::LfsLookahead;

/// Per lfs.h typedef struct lfs
#[repr(C)]
pub struct Lfs<S: Storage> {
    pub root: [lfs_block_t; 2],
    pub mlist: *mut LfsMlist,
    pub seed: u32,
    pub gstate: LfsGstate,
    pub gdisk: LfsGstate,
    pub gdelta: LfsGstate,
    pub lookahead: LfsLookahead,
    pub storage: S,
    pub cfg: *const LfsConfig,
    pub block_count: u32,
    pub name_max: u32,
    pub file_max: u32,
    pub attr_max: u32,
    pub inline_max: u32,
}

impl<S: Storage> Lfs<S> {
    pub fn new(storage: S) -> Self {
        Self {
            root: [0; 2],
            mlist: core::ptr::null_mut(),
            seed: 0,
            gstate: LfsGstate {
                tag: 0,
                pair: [0, 0],
            },
            gdisk: LfsGstate {
                tag: 0,
                pair: [0, 0],
            },
            gdelta: LfsGstate {
                tag: 0,
                pair: [0, 0],
            },
            lookahead: LfsLookahead::default(),
            storage,
            cfg: core::ptr::null(),
            block_count: 0,
            name_max: 0,
            file_max: 0,
            attr_max: 0,
            inline_max: 0,
        }
    }
}

#[derive(Default)]
pub struct LfsCaches {
    pub pcache: LfsCache,
    pub rcache: LfsCache,
}
