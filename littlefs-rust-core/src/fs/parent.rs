//! FS parent. Per lfs.c lfs_fs_pred, lfs_fs_parent.

use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

use crate::bd::Storage;

/// Per lfs.c lfs_fs_pred (lines 4796-4833)
///
/// C:
/// ```c
/// static int lfs_fs_pred(lfs_t *lfs,
///         const lfs_block_t pair[2], lfs_mdir_t *pdir) {
///     // iterate over all directory directory entries
///     pdir->tail[0] = 0;
///     pdir->tail[1] = 1;
///     struct lfs_tortoise_t tortoise = {
///         .pair = {LFS_BLOCK_NULL, LFS_BLOCK_NULL},
///         .i = 1,
///         .period = 1,
///     };
///     int err = LFS_ERR_OK;
///     while (!lfs_pair_isnull(pdir->tail)) {
///         err = lfs_tortoise_detectcycles(pdir, &tortoise);
///         if (err < 0) {
///             return LFS_ERR_CORRUPT;
///         }
///
///         if (lfs_pair_cmp(pdir->tail, pair) == 0) {
///             return 0;
///         }
///
///         int err = lfs_dir_fetch(lfs, pdir, pdir->tail);
///         if (err) {
///             return err;
///         }
///     }
///
///     return LFS_ERR_NOENT;
/// }
/// #endif
/// ```
pub async fn lfs_fs_pred<S: Storage>(
    lfs: &mut crate::fs::Lfs<S>,
    caches: &mut crate::fs::LfsCaches,
    pair: &[crate::types::lfs_block_t; 2],
    pdir: *mut crate::dir::LfsMdir,
) -> i32 {
    use crate::dir::fetch::lfs_dir_fetch;
    use crate::fs::mount::{lfs_tortoise_detectcycles, LfsTortoise};
    use crate::types::LFS_BLOCK_NULL;
    use crate::util::{lfs_pair_cmp, lfs_pair_isnull};

    unsafe {
        (*pdir).tail = [0, 1];
        let mut tortoise = LfsTortoise {
            pair: [LFS_BLOCK_NULL, LFS_BLOCK_NULL],
            i: 1,
            period: 1,
        };
        let mut have_fetched = false;
        #[cfg(feature = "loop_limits")]
        const MAX_PARENT_ITER: u32 = 2048;
        #[cfg(feature = "loop_limits")]
        let mut iter: u32 = 0;

        while !lfs_pair_isnull(&(*pdir).tail) {
            #[cfg(feature = "loop_limits")]
            {
                if iter >= MAX_PARENT_ITER {
                    panic!(
                        "loop_limits: MAX_PARENT_ITER ({}) exceeded in lfs_fs_parent",
                        MAX_PARENT_ITER
                    );
                }
                iter += 1;
            }
            let err = lfs_tortoise_detectcycles(pdir, &mut tortoise);
            if err < 0 {
                return crate::error::LFS_ERR_CORRUPT;
            }

            if lfs_pair_cmp(&(*pdir).tail, pair) == 0 {
                if !have_fetched {
                    // Matched before any fetch: tail [0,1] == pair (root).
                    // The root has no predecessor.
                    let err = lfs_dir_fetch(lfs, caches, pdir, &(*pdir).tail).await;
                    if err != 0 {
                        return crate::lfs_pass_err!(err);
                    }
                    if lfs_pair_isnull(&(*pdir).tail) {
                        return crate::error::LFS_ERR_NOENT;
                    }
                }
                return 0;
            }

            let err = lfs_dir_fetch(lfs, caches, pdir, &(*pdir).tail).await;
            if err != 0 {
                return crate::lfs_pass_err!(err);
            }
            have_fetched = true;
        }

        crate::error::LFS_ERR_NOENT
    }
}

/// C: lfs.c:4835-4853
#[repr(C)]
pub struct LfsFsParentMatch {
    pub pair: [crate::types::lfs_block_t; 2],
}

// Per lfs.c enum: LFS_CMP_EQ=0, LFS_CMP_LT=1, LFS_CMP_GT=2
const LFS_CMP_EQ: i32 = 0;
const LFS_CMP_LT: i32 = 1;

/// Per lfs.c lfs_fs_parent_match (lines 4835-4853)
///
/// C:
/// ```c
/// static int lfs_fs_parent_match(void *data,
///         lfs_tag_t tag, const void *buffer) {
///     struct lfs_fs_parent_match *find = data;
///     lfs_t *lfs = find->lfs;
///     const struct lfs_diskoff *disk = buffer;
///     (void)tag;
///     lfs_block_t child[2];
///     int err = lfs_bd_read(lfs, ...);
///     lfs_pair_fromle32(child);
///     return (lfs_pair_cmp(child, find->pair) == 0) ? LFS_CMP_EQ : LFS_CMP_LT;
/// }
/// ```
async fn lfs_fs_parent_match<S: Storage>(
    lfs: &mut crate::fs::Lfs<S>,
    caches: &mut crate::fs::LfsCaches,
    find: &LfsFsParentMatch,
    _tag: crate::types::lfs_tag_t,
    disk: &crate::tag::lfs_diskoff,
) -> i32 {
    use crate::bd::bd::lfs_bd_read;
    use crate::util::{lfs_pair_cmp, lfs_pair_fromle32};

    let mut child_buf = [0u8; 8];
    let err = lfs_bd_read(
        lfs,
        None,
        &mut caches.rcache,
        unsafe { lfs.cfg.as_ref() }.expect("cfg").block_size,
        disk.block,
        disk.off,
        &mut child_buf,
    )
    .await;
    if err != 0 {
        return crate::lfs_pass_err!(err);
    }
    let child = [
        u32::from_le_bytes([child_buf[0], child_buf[1], child_buf[2], child_buf[3]]),
        u32::from_le_bytes([child_buf[4], child_buf[5], child_buf[6], child_buf[7]]),
    ];
    if lfs_pair_cmp(&child, &find.pair) == 0 {
        LFS_CMP_EQ
    } else {
        LFS_CMP_LT
    }
}

/// Adapter binding `LfsFsParentMatch` as an `lfs_dir_fetchmatch` callback.
struct ParentMatchCb {
    find_match: LfsFsParentMatch,
}

impl<S: Storage> crate::dir::fetch::FetchCb<S> for ParentMatchCb {
    fn call<'a>(
        &'a mut self,
        lfs: &'a mut crate::fs::Lfs<S>,
        caches: &'a mut crate::fs::LfsCaches,
        tag: crate::types::lfs_tag_t,
        off: &'a crate::tag::lfs_diskoff,
    ) -> Pin<Box<dyn Future<Output = i32> + 'a>> {
        Box::pin(lfs_fs_parent_match(lfs, caches, &self.find_match, tag, off))
    }
}

/// Per lfs.c lfs_fs_parent (lines 4856-4892)
///
/// C:
/// ```c
/// static lfs_stag_t lfs_fs_parent(lfs_t *lfs, const lfs_block_t pair[2],
///         lfs_mdir_t *parent) {
///     // use fetchmatch with callback to find pairs
///     parent->tail[0] = 0;
///     parent->tail[1] = 1;
///     struct lfs_tortoise_t tortoise = {
///         .pair = {LFS_BLOCK_NULL, LFS_BLOCK_NULL},
///         .i = 1,
///         .period = 1,
///     };
///     int err = LFS_ERR_OK;
///     while (!lfs_pair_isnull(parent->tail)) {
///         err = lfs_tortoise_detectcycles(parent, &tortoise);
///         if (err < 0) {
///             return err;
///         }
///
///         lfs_stag_t tag = lfs_dir_fetchmatch(lfs, parent, parent->tail,
///                 LFS_MKTAG(0x7ff, 0, 0x3ff),
///                 LFS_MKTAG(LFS_TYPE_DIRSTRUCT, 0, 8),
///                 NULL,
///                 lfs_fs_parent_match, &(struct lfs_fs_parent_match){
///                     lfs, {pair[0], pair[1]}});
///         if (tag && tag != LFS_ERR_NOENT) {
///             return tag;
///         }
///     }
///
///     return LFS_ERR_NOENT;
/// }
/// #endif
/// ```
pub async fn lfs_fs_parent<S: Storage>(
    lfs: &mut crate::fs::Lfs<S>,
    caches: &mut crate::fs::LfsCaches,
    pair: *const [crate::types::lfs_block_t; 2],
    parent: *mut crate::dir::LfsMdir,
) -> crate::types::lfs_stag_t {
    use crate::dir::fetch::lfs_dir_fetchmatch;
    use crate::fs::mount::{lfs_tortoise_detectcycles, LfsTortoise};
    use crate::lfs_type::lfs_type::LFS_TYPE_DIRSTRUCT;
    use crate::tag::lfs_mktag;
    use crate::types::{lfs_block_t, LFS_BLOCK_NULL};
    use crate::util::lfs_pair_isnull;

    unsafe {
        (*parent).tail = [0, 1];
        let mut tortoise = LfsTortoise {
            pair: [LFS_BLOCK_NULL, LFS_BLOCK_NULL],
            i: 1,
            period: 1,
        };
        #[cfg(feature = "loop_limits")]
        const MAX_PARENT_ITER: u32 = 2048;
        #[cfg(feature = "loop_limits")]
        let mut iter: u32 = 0;

        while !lfs_pair_isnull(&(*parent).tail) {
            #[cfg(feature = "loop_limits")]
            {
                if iter >= MAX_PARENT_ITER {
                    panic!(
                        "loop_limits: MAX_PARENT_ITER ({}) exceeded in lfs_fs_parent (parent)",
                        MAX_PARENT_ITER
                    );
                }
                iter += 1;
            }
            let err = lfs_tortoise_detectcycles(parent, &mut tortoise);
            if err < 0 {
                return crate::lfs_pass_err!(err);
            }

            let find_match = LfsFsParentMatch {
                pair: [(*pair)[0], (*pair)[1]],
            };
            let tag = lfs_dir_fetchmatch(
                lfs,
                caches,
                parent,
                &(*parent).tail as *const _,
                lfs_mktag(0x7ff, 0, 0x3ff),
                lfs_mktag(LFS_TYPE_DIRSTRUCT, 0, 8),
                core::ptr::null_mut(),
                Some(&mut ParentMatchCb { find_match }),
            )
            .await;

            if tag != 0 && tag != crate::error::LFS_ERR_NOENT {
                return tag;
            }
        }

        crate::error::LFS_ERR_NOENT
    }
}
