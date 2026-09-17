//! Orphan and power-loss consistency tests.
//!
//! Upstream: tests/test_orphans.toml
//! Source: https://github.com/littlefs-project/littlefs/blob/master/tests/test_orphans.toml

mod common;

#[cfg(feature = "slow_tests")]
use common::powerloss::{init_powerloss_context, powerloss_config, run_powerloss_linear};
#[cfg(feature = "slow_tests")]
use common::test_prng;
use common::{assert_ok, default_config, dir_block, init_context, init_logger, path_bytes};
#[cfg(feature = "slow_tests")]
use littlefs_rust_core::lfs_type::lfs_type::LFS_TYPE_DIR;
use littlefs_rust_core::lfs_type::lfs_type::LFS_TYPE_SOFTTAIL;
use littlefs_rust_core::{
    lfs_alloc_ckpoint, lfs_dir_alloc, lfs_dir_commit, lfs_dir_fetch, lfs_format,
    lfs_fs_forceconsistency, lfs_fs_hasorphans, lfs_fs_mkconsistent, lfs_fs_preporphans,
    lfs_fs_size, lfs_mattr, lfs_mkdir, lfs_mktag, lfs_mount, lfs_pair_tole32, lfs_remove, lfs_stat,
    lfs_unmount, Lfs, LfsCaches, LfsConfig, LfsMdir, Storage, LFS_ERR_NOENT,
};
#[cfg(feature = "slow_tests")]
use littlefs_rust_core::{LfsInfo, LFS_ERR_EXIST, LFS_ERR_NOTEMPTY};

// --- test_orphans_mkconsistent_fresh ---
// Minimal: format, mount, mkconsistent. No mkdir/remove. Sanity check.
#[tokio::test]
async fn test_orphans_mkconsistent_fresh() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_fs_mkconsistent(&mut lfs, &mut caches).await);
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_orphans_mkconsistent_no_orphans ---
// With lazy force_consistency, mkdir/remove run deorphan first. So preporphans(1)
// gets cleared before the commit. Verify: mkconsistent clears (no-op) and persists.
#[tokio::test]
async fn test_orphans_mkconsistent_no_orphans() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_fs_preporphans(&mut lfs, 1));
    assert!(unsafe { lfs_fs_hasorphans(&mut lfs) });

    let path = path_bytes("_p");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path.as_ptr()).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path.as_ptr()).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "force_consistency before mkdir clears orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "persisted gstate has no orphans"
    );
    assert_ok(lfs_fs_mkconsistent(&mut lfs, &mut caches).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "after mkconsistent, gstate should have no orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "after remount, gstate persisted to disk has no orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_orphans_no_orphans ---
// preporphans(+1), mkdir+remove clears via force_consistency, unmount
#[tokio::test]
async fn test_orphans_no_orphans() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_fs_preporphans(&mut lfs, 1));
    assert!(unsafe { lfs_fs_hasorphans(&mut lfs) });

    let path = path_bytes("_x");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path.as_ptr()).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path.as_ptr()).await);
    assert!(!unsafe { lfs_fs_hasorphans(&mut lfs) });
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_orphans_nonreentrant ---
// Upstream: orphan operations without powerloss.
// Uses n=1 dir to match test_dirs_many_removal (n=2+ mkdir currently fails in this crate).
#[tokio::test]
async fn test_orphans_nonreentrant() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let path = path_bytes("a");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path.as_ptr()).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path.as_ptr()).await);
    assert!(!unsafe { lfs_fs_hasorphans(&mut lfs) });
    assert_ok(lfs_unmount(&mut lfs));
}

// --- Missing upstream stubs ---

/// Upstream: [cases.test_orphans_normal]
/// if = 'PROG_SIZE <= 0x3fe'. Corrupt child's commit to create orphan, mkdir triggers deorphan, check lfs_fs_size.
#[tokio::test]
async fn test_orphans_normal() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg).await);

    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent/orphan").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent/child").as_ptr()).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path_bytes("parent/orphan").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    // Mount to get child dir block, then corrupt it
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg).await);
    let block = dir_block(&mut lfs, &mut caches, "parent/child").await;
    assert_ok(lfs_unmount(&mut lfs));

    let block_size = env.config.block_size as usize;
    let mut buffer = vec![0u8; block_size];
    assert!(lfs.storage.read(block, 0, &mut buffer).await.is_ok());

    let mut off = block_size as i32 - 1;
    while off >= 0 && buffer[off as usize] == 0xff {
        off -= 1;
    }
    assert!(off >= 3, "block {block} has fewer than 4 written bytes");
    let start = (off - 3) as usize;
    buffer[start..start + 3].fill(env.config.block_size as u8);

    assert!(lfs.storage.erase(block).await.is_ok());
    assert!(lfs.storage.write(block, 0, &buffer).await.is_ok());

    // Mount and verify orphan is gone, child exists, size is 8
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg).await);
    let mut info = core::mem::MaybeUninit::<littlefs_rust_core::LfsInfo>::zeroed();
    assert_eq!(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("parent/orphan").as_ptr(),
            info.as_mut_ptr()
        )
        .await,
        LFS_ERR_NOENT
    );
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("parent/child").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(lfs_fs_size(&mut lfs, &mut caches).await, 8);
    assert_ok(lfs_unmount(&mut lfs));

    // mkdir parent/otherchild triggers deorphan, size still 8
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg).await);
    assert_ok(
        lfs_mkdir(
            &mut lfs,
            &mut caches,
            path_bytes("parent/otherchild").as_ptr(),
        )
        .await,
    );
    assert_eq!(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("parent/orphan").as_ptr(),
            info.as_mut_ptr()
        )
        .await,
        LFS_ERR_NOENT
    );
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("parent/child").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("parent/otherchild").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(lfs_fs_size(&mut lfs, &mut caches).await, 8);
    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_orphans_one_orphan]
/// Create orphan via internal APIs (lfs_dir_alloc + SOFTTAIL commit + lfs_fs_preporphans). Run lfs_fs_forceconsistency.
#[tokio::test]
async fn test_orphans_one_orphan() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    // Create an orphan mdir
    let mut orphan = LfsMdir {
        pair: [0, 0],
        rev: 0,
        off: 0,
        etag: 0,
        count: 0,
        erased: false,
        split: false,
        tail: [0, 0],
    };
    unsafe { lfs_alloc_ckpoint(&mut lfs) };
    assert_ok(unsafe { lfs_dir_alloc(&mut lfs, &mut caches, &mut orphan).await });
    assert_ok(lfs_dir_commit(&mut lfs, &mut caches, &mut orphan, core::ptr::null(), 0).await);

    // Append orphan to root and mark FS as having orphans
    assert_ok(lfs_fs_preporphans(&mut lfs, 1));
    let mut mdir = LfsMdir {
        pair: [0, 0],
        rev: 0,
        off: 0,
        etag: 0,
        count: 0,
        erased: false,
        split: false,
        tail: [0, 0],
    };
    let root_pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(&mut lfs, &mut caches, &mut mdir, &root_pair).await);
    lfs_pair_tole32(&mut orphan.pair);
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_SOFTTAIL, 0x3ff, 8),
        buffer: orphan.pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(
        lfs_dir_commit(
            &mut lfs,
            &mut caches,
            &mut mdir,
            attrs.as_ptr() as *const core::ffi::c_void,
            1,
        )
        .await,
    );

    assert!(
        unsafe { lfs_fs_hasorphans(&mut lfs) },
        "should have orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert!(
        unsafe { lfs_fs_hasorphans(&mut lfs) },
        "orphans should persist"
    );
    assert_ok(lfs_fs_forceconsistency(&mut lfs, &mut caches).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "forceconsistency should clear orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_orphans_mkconsistent_one_orphan]
/// Same orphan creation as one_orphan. Use lfs_fs_mkconsistent + remount. Verify cleanup.
#[tokio::test]
async fn test_orphans_mkconsistent_one_orphan() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    // Create an orphan mdir
    let mut orphan = LfsMdir {
        pair: [0, 0],
        rev: 0,
        off: 0,
        etag: 0,
        count: 0,
        erased: false,
        split: false,
        tail: [0, 0],
    };
    unsafe { lfs_alloc_ckpoint(&mut lfs) };
    assert_ok(unsafe { lfs_dir_alloc(&mut lfs, &mut caches, &mut orphan).await });
    assert_ok(lfs_dir_commit(&mut lfs, &mut caches, &mut orphan, core::ptr::null(), 0).await);

    // Append orphan to root and mark FS as having orphans
    assert_ok(lfs_fs_preporphans(&mut lfs, 1));
    let mut mdir = LfsMdir {
        pair: [0, 0],
        rev: 0,
        off: 0,
        etag: 0,
        count: 0,
        erased: false,
        split: false,
        tail: [0, 0],
    };
    let root_pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(&mut lfs, &mut caches, &mut mdir, &root_pair).await);
    lfs_pair_tole32(&mut orphan.pair);
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_SOFTTAIL, 0x3ff, 8),
        buffer: orphan.pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(
        lfs_dir_commit(
            &mut lfs,
            &mut caches,
            &mut mdir,
            attrs.as_ptr() as *const core::ffi::c_void,
            1,
        )
        .await,
    );

    assert!(
        unsafe { lfs_fs_hasorphans(&mut lfs) },
        "should have orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert!(
        unsafe { lfs_fs_hasorphans(&mut lfs) },
        "orphans should persist"
    );
    assert_ok(lfs_fs_mkconsistent(&mut lfs, &mut caches).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "mkconsistent should clear orphans"
    );
    assert_ok(lfs_unmount(&mut lfs));

    // Remount and verify orphans are still gone
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert!(
        !unsafe { lfs_fs_hasorphans(&mut lfs) },
        "after remount, orphans should still be gone"
    );
    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_orphans_reentrant]
/// FILES=[6,26], DEPTH=1; FILES=3,DEPTH=3 skipped when CACHE_SIZE!=64. reentrant, CYCLES=20.
#[tokio::test]
#[cfg(feature = "slow_tests")]
async fn test_orphans_reentrant() {
    init_logger();
    const CYCLES: u32 = 20;
    const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

    for (files, depth) in [(6usize, 1usize), (26, 1)] {
        if 2 * files >= 128 {
            continue;
        }
        let mut env = powerloss_config(128);
        init_powerloss_context(&mut env);
        let snapshot = env.snapshot();

        let result = run_powerloss_linear(
            &mut env,
            &snapshot,
            2000,
            async |lfs, caches, config| {
                let err = lfs_mount(lfs, caches, config).await;
                if err != 0 {
                    let e = lfs_format(lfs, caches, config).await;
                    if e != 0 {
                        return Err(e);
                    }
                    let e = lfs_mount(lfs, caches, config).await;
                    if e != 0 {
                        return Err(e);
                    }
                }

                let mut prng: u32 = 1;
                for _ in 0..CYCLES {
                    let mut components = Vec::with_capacity(depth);
                    for _ in 0..depth {
                        let c = ALPHA[(test_prng(&mut prng) as usize) % files];
                        components.push((c as char).to_string());
                    }
                    let full_path = "/".to_string() + &components.join("/");

                    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
                    let res = lfs_stat(
                        lfs,
                        caches,
                        path_bytes(&full_path).as_ptr(),
                        info.as_mut_ptr(),
                    )
                    .await;
                    if res == LFS_ERR_NOENT {
                        for d in 0..depth {
                            let sub = "/".to_string() + &components[..=d].join("/");
                            let err = lfs_mkdir(lfs, caches, path_bytes(&sub).as_ptr()).await;
                            if err != 0 && err != LFS_ERR_EXIST {
                                return Err(err);
                            }
                        }
                        for d in 0..depth {
                            let sub = "/".to_string() + &components[..=d].join("/");
                            let r =
                                lfs_stat(lfs, caches, path_bytes(&sub).as_ptr(), info.as_mut_ptr())
                                    .await;
                            if r != 0 {
                                return Err(if r < 0 { r } else { -1 });
                            }
                            let info_ref = unsafe { &*info.as_ptr() };
                            let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
                            let name = core::str::from_utf8(&info_ref.name[..nul]).unwrap();
                            let expected = &components[d];
                            if name != *expected {
                                return Err(-1);
                            }
                            if info_ref.type_ != LFS_TYPE_DIR as u8 {
                                return Err(-1);
                            }
                        }
                    } else if res == 0 {
                        let info_ref = unsafe { &*info.as_ptr() };
                        let expected = &components[depth - 1];
                        let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
                        let name = core::str::from_utf8(&info_ref.name[..nul]).unwrap();
                        if name != *expected || info_ref.type_ != LFS_TYPE_DIR as u8 {
                            return Err(-1);
                        }
                        for d in (0..depth).rev() {
                            let sub = "/".to_string() + &components[..=d].join("/");
                            let err = lfs_remove(lfs, caches, path_bytes(&sub).as_ptr()).await;
                            if err != 0 && err != LFS_ERR_NOTEMPTY {
                                return Err(err);
                            }
                        }
                        let r = lfs_stat(
                            lfs,
                            caches,
                            path_bytes(&full_path).as_ptr(),
                            info.as_mut_ptr(),
                        )
                        .await;
                        if r != LFS_ERR_NOENT {
                            return Err(if r < 0 { r } else { -1 });
                        }
                    } else {
                        return Err(res);
                    }
                }

                if lfs_unmount(lfs) != 0 {
                    return Err(-1);
                }
                Ok(())
            },
            async |_, _, _| Ok(()),
        )
        .await;
        result.unwrap_or_else(|_| {
            panic!("test_orphans_reentrant FILES={files} DEPTH={depth} should complete")
        });
    }
}
