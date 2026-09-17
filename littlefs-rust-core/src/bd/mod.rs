//! Block device layer. Per lfs.c lfs_bd_*, lfs_cache_*.

#[allow(clippy::module_inception)]
pub(crate) mod bd;
mod lfs_cache;
mod storage;

pub use lfs_cache::LfsCache;
pub use storage::Storage;
