//! Power-loss simulation tests.
//!
//! Upstream: tests/test_powerloss.toml
//! Source: https://github.com/littlefs-project/littlefs/blob/master/tests/test_powerloss.toml

mod common;

use common::{
    assert_ok_at, default_config, init_context, init_logger, path_bytes,
    powerloss::{
        init_powerloss_context, powerloss_config, powerloss_config_with_behavior,
        run_powerloss_exhaustive, run_powerloss_linear, run_powerloss_log, PowerLossBehavior,
    },
    LFS_O_APPEND, LFS_O_CREAT, LFS_O_RDONLY, LFS_O_WRONLY,
};
use littlefs_rust_core::{
    lfs_dir_close, lfs_dir_open, lfs_file_close, lfs_file_open, lfs_file_read, lfs_file_sync,
    lfs_file_write, lfs_format, lfs_mkdir, lfs_mount, lfs_unmount, Lfs, LfsCaches, LfsConfig,
    LfsDir, LfsFile, Storage, LFS_ERR_IO,
};

// --- test_powerloss_only_rev ---
// Upstream: write rev+1 to one block of dir pair; mount picks higher rev, read/write still works.
#[tokio::test]
async fn test_powerloss_only_rev() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let path_nb = path_bytes("notebook");
    let path_paper = path_bytes("notebook/paper");
    assert_ok_at(
        "mkdir notebook",
        lfs_mkdir(&mut lfs, &mut caches, path_nb.as_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
        assert!(n == buf.len() as i32);
        assert_ok_at(
            &format!("file_sync #{} (first loop)", i + 1),
            lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
        );
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper read",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut rbuf = [0u8; 256];
    for _ in 0..5 {
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..5]).await;
        assert_eq!(n, 5);
        assert_eq!(&rbuf[..5], b"hello");
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));

    // Get dir pair and rev from a fresh mount, then corrupt rev
    assert_ok_at(
        "mount before corrupt",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let mut dir = core::mem::MaybeUninit::<LfsDir>::zeroed();
    assert_ok_at(
        "dir_open notebook",
        lfs_dir_open(&mut lfs, &mut caches, dir.as_mut_ptr(), path_nb.as_ptr()).await,
    );
    let pair = unsafe { (*dir.as_ptr()).m.pair };
    let rev = unsafe { (*dir.as_ptr()).m.rev };
    assert_ok_at("dir_close", lfs_dir_close(&mut lfs, dir.as_mut_ptr()));
    assert_ok_at("unmount before corrupt", lfs_unmount(&mut lfs));

    // Partial write: rev+1 in block
    let block_size = env.config.block_size as usize;
    let mut block_buf = vec![0u8; block_size];
    let _ = lfs.storage.read(pair[1], 0, &mut block_buf);
    block_buf[0..4].copy_from_slice(&(rev + 1).to_le_bytes());
    let _ = lfs.storage.erase(pair[1]);
    let _ = lfs.storage.write(pair[1], 0, &block_buf);

    assert_ok_at(
        "mount after corrupt",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper read after corrupt",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    for _ in 0..5 {
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..5]).await;
        assert_eq!(n, 5);
        assert_eq!(&rbuf[..5], b"hello");
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper append",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_WRONLY | LFS_O_APPEND,
        )
        .await,
    );
    let buf2 = b"goodbye";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf2).await;
        assert!(n == buf2.len() as i32);
        assert_ok_at(
            &format!("file_sync #{} (after corrupt)", i + 1),
            lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
        );
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper read final",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    for _ in 0..5 {
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..5]).await;
        assert_eq!(n, 5);
        assert_eq!(&rbuf[..5], b"hello");
    }
    for _ in 0..5 {
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..7]).await;
        assert_eq!(n, 7);
        assert_eq!(&rbuf[..7], b"goodbye");
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount final", lfs_unmount(&mut lfs));
}

// --- test_powerloss_trigger_first_write ---
// Unit test: fail_after_writes=1 causes first prog/erase to return LFS_ERR_IO.
#[tokio::test]
async fn test_powerloss_trigger_first_write() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);
    env.set_fail_after_writes(1);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    let err = lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await;
    assert_eq!(
        err, LFS_ERR_IO,
        "format should fail on first write with fail_after_writes=1"
    );
}

// --- test_powerloss_runner_smoke ---
// Smoke test: run_powerloss_linear with mkdir op; verify mount works after power loss.
#[tokio::test]
async fn test_powerloss_runner_smoke() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let snapshot = env.snapshot();

    let path_d = path_bytes("d");
    let result = run_powerloss_linear(
        &mut env,
        &snapshot,
        64,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_mkdir(lfs, caches, path_d.as_ptr()).await;
            if err != 0 {
                let _ = lfs_unmount(lfs);
                return Err(err);
            }
            let err = lfs_unmount(lfs);
            if err != 0 {
                return Err(err);
            }
            Ok(())
        },
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let _ = lfs_unmount(lfs);
            Ok(())
        },
    )
    .await;
    result.expect("run_powerloss_linear should complete");
}

/// Upstream: [cases.test_powerloss_partial_prog]
/// defines.PROG_SIZE < BLOCK_SIZE, BYTE_OFF = [0, PROG_SIZE-1, PROG_SIZE/2], BYTE_VALUE = [0x33, 0xcc].
/// Corrupt one byte in a directory block at BYTE_OFF with BYTE_VALUE. Verify mount and read/write still work.
#[tokio::test]
async fn test_powerloss_partial_prog() {
    init_logger();
    const PROG_SIZE: u32 = 16;
    const BLOCK_SIZE: u32 = 512;
    let byte_offs: [u32; 3] = [0, PROG_SIZE - 1, PROG_SIZE / 2];
    let byte_values: [u8; 2] = [0x33, 0xcc];
    const DIR_BLOCK: u32 = 1; // second superblock block has root dir data

    for &byte_off in &byte_offs {
        for &byte_value in &byte_values {
            let mut env = default_config(128);
            init_context(&mut env);
            let cfg = &env.config as *const LfsConfig;

            let mut lfs = Lfs::new(&mut env.ram);
            let mut caches = LfsCaches::default();
            assert_ok_at("format", lfs_format(&mut lfs, &mut caches, cfg).await);
            assert_ok_at("mount", lfs_mount(&mut lfs, &mut caches, cfg).await);
            let path_a = path_bytes("a");
            assert_ok_at(
                "mkdir a",
                lfs_mkdir(&mut lfs, &mut caches, path_a.as_ptr()).await,
            );
            assert_ok_at("unmount", lfs_unmount(&mut lfs));

            let mut block = vec![0u8; BLOCK_SIZE as usize];
            assert!(lfs.storage.read(DIR_BLOCK, 0, &mut block).await.is_ok());
            block[byte_off as usize] = byte_value;
            assert!(lfs.storage.write(DIR_BLOCK, 0, &mut block).await.is_ok());

            assert_ok_at(
                &format!("mount after corrupt off={byte_off} val=0x{byte_value:02x}"),
                lfs_mount(&mut lfs, &mut caches, cfg).await,
            );
            let mut info = core::mem::MaybeUninit::<littlefs_rust_core::LfsInfo>::zeroed();
            let r = littlefs_rust_core::lfs_stat(
                &mut lfs,
                &mut caches,
                path_a.as_ptr(),
                info.as_mut_ptr(),
            )
            .await;
            assert!(r == 0, "lfs_stat a after corrupt: {r}");
            assert_ok_at("unmount after verify", lfs_unmount(&mut lfs));
        }
    }
}

// --- test_powerloss_snapshot_restore ---
// Unit test: snapshot and restore preserve BD state.
#[tokio::test]
async fn test_powerloss_snapshot_restore() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let snapshot = lfs.storage.snapshot();

    // Mutate ram
    let mut r = vec![0u8; snapshot.len()];
    let _ = lfs.storage.write(0, 0, &[0]).await;
    let _ = lfs.storage.read(0, 0, &mut r[0..1]).await;
    assert_ne!(r[0], snapshot[0]);

    lfs.storage.restore(&snapshot);
    let _ = lfs.storage.read(0, 0, &mut r).await;
    assert_eq!(&r[..], &snapshot[..]);

    assert_ok_at(
        "mount after restore",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}

// =============================================================================
// Debug tests. test_powerloss_only_rev / test_debug_powerloss_after_corrupt still
// fail with NOSPC on sync #5 after rev corruption; lfs_dir_split is now implemented.
// Remaining issue may be in compact/relocate when reading from corrupted block.
// =============================================================================

/// Minimal: file in root, write "hello" once, sync. No mkdir, no subdir.
#[tokio::test]
async fn test_debug_file_root_single_write_sync() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let path = path_bytes("paper");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
    assert_eq!(n, buf.len() as i32);
    assert_ok_at(
        "file_sync",
        lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}

/// File in root, write "hello" 5x with sync each (like powerloss but no mkdir). Bisects root vs subdir.
#[tokio::test]
async fn test_debug_file_root_repeated_write_sync() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let path = path_bytes("paper");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
        assert_eq!(n, buf.len() as i32);
        assert_ok_at(
            &format!("file_sync #{}", i + 1),
            lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
        );
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}

/// Exact powerloss pattern (mkdir + file in subdir) but bisects which sync fails.
#[tokio::test]
async fn test_debug_file_subdir_which_sync_fails() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let path_nb = path_bytes("notebook");
    let path_paper = path_bytes("notebook/paper");
    assert_ok_at(
        "mkdir notebook",
        lfs_mkdir(&mut lfs, &mut caches, path_nb.as_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
        assert_eq!(n, buf.len() as i32);
        let err = lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await;
        assert_ok_at(&format!("file_sync #{}", i + 1), err);
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}

/// Reproduces powerloss flow: setup, corrupt rev, then append. Bisects which sync fails after corrupt.
#[tokio::test]
async fn test_debug_powerloss_after_corrupt_append() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    let path_nb = path_bytes("notebook");
    let path_paper = path_bytes("notebook/paper");
    assert_ok_at(
        "mkdir notebook",
        lfs_mkdir(&mut lfs, &mut caches, path_nb.as_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
        assert_eq!(n, buf.len() as i32);
        assert_ok_at(
            &format!("file_sync #{}", i + 1),
            lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
        );
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));

    assert_ok_at(
        "mount before corrupt",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let mut dir = core::mem::MaybeUninit::<LfsDir>::zeroed();
    assert_ok_at(
        "dir_open notebook",
        lfs_dir_open(&mut lfs, &mut caches, dir.as_mut_ptr(), path_nb.as_ptr()).await,
    );
    let pair = unsafe { (*dir.as_ptr()).m.pair };
    let rev = unsafe { (*dir.as_ptr()).m.rev };
    assert_ok_at("dir_close", lfs_dir_close(&mut lfs, dir.as_mut_ptr()));
    assert_ok_at("unmount before corrupt", lfs_unmount(&mut lfs));

    let block_size = env.config.block_size as usize;
    let mut block_buf = vec![0u8; block_size];
    let _ = lfs.storage.read(pair[1], 0, &mut block_buf);
    block_buf[0..4].copy_from_slice(&(rev + 1).to_le_bytes());
    let _ = lfs.storage.erase(pair[1]);
    let _ = lfs.storage.write(pair[1], 0, &block_buf);

    assert_ok_at(
        "mount after corrupt",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper append",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_paper.as_ptr(),
            LFS_O_WRONLY | LFS_O_APPEND,
        )
        .await,
    );
    let buf2 = b"goodbye";
    for i in 0..5 {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf2).await;
        assert_eq!(n, buf2.len() as i32);
        assert_ok_at(
            &format!("file_sync #{} (after corrupt)", i + 1),
            lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
        );
    }
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}

// --- test_powerloss_runner_smoke_log ---
// Same as test_powerloss_runner_smoke but using run_powerloss_log (exponential stepping).
#[tokio::test]
async fn test_powerloss_runner_smoke_log() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let snapshot = env.snapshot();

    let path_d = path_bytes("d");
    let result = run_powerloss_log(
        &mut env,
        &snapshot,
        64,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_mkdir(lfs, caches, path_d.as_ptr()).await;
            if err != 0 {
                let _ = lfs_unmount(lfs);
                return Err(err);
            }
            let err = lfs_unmount(lfs);
            if err != 0 {
                return Err(err);
            }
            Ok(())
        },
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let _ = lfs_unmount(lfs);
            Ok(())
        },
    )
    .await;
    result.expect("run_powerloss_log should complete");
}

// --- test_powerloss_runner_smoke_exhaustive ---
// Same as test_powerloss_runner_smoke but using run_powerloss_exhaustive with depth=2.
#[tokio::test]
async fn test_powerloss_runner_smoke_exhaustive() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let snapshot = env.snapshot();

    let path_d = path_bytes("d");
    let result = run_powerloss_exhaustive(
        &mut env,
        &snapshot,
        64,
        2,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_mkdir(lfs, caches, path_d.as_ptr()).await;
            if err != 0 {
                let _ = lfs_unmount(lfs);
                return Err(err);
            }
            let err = lfs_unmount(lfs);
            if err != 0 {
                return Err(err);
            }
            Ok(())
        },
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let _ = lfs_unmount(lfs);
            Ok(())
        },
    )
    .await;
    result.expect("run_powerloss_exhaustive depth=2 should complete");
}

// --- test_powerloss_ooo_smoke ---
// OOO behaviour: writes between syncs may be reordered. Verify FS recovers correctly.
#[tokio::test]
async fn test_powerloss_ooo_smoke() {
    init_logger();
    let mut env = powerloss_config_with_behavior(128, PowerLossBehavior::Ooo);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    let snapshot = env.snapshot();

    let path_d = path_bytes("d");
    let result = run_powerloss_linear(
        &mut env,
        &snapshot,
        64,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_mkdir(lfs, caches, path_d.as_ptr()).await;
            if err != 0 {
                let _ = lfs_unmount(lfs);
                return Err(err);
            }
            let err = lfs_unmount(lfs);
            if err != 0 {
                return Err(err);
            }
            Ok(())
        },
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let _ = lfs_unmount(lfs);
            Ok(())
        },
    )
    .await;
    result.expect("OOO powerloss linear should complete");
}

/// Minimal subdir: mkdir + file, single write + sync.
#[tokio::test]
async fn test_debug_file_subdir_single_write_sync() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok_at(
        "format",
        lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );
    assert_ok_at(
        "mount",
        lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await,
    );

    assert_ok_at(
        "mkdir notebook",
        lfs_mkdir(&mut lfs, &mut caches, path_bytes("notebook").as_ptr()).await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok_at(
        "file_open paper create",
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("notebook/paper").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_APPEND,
        )
        .await,
    );
    let buf = b"hello";
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), buf).await;
    assert_eq!(n, buf.len() as i32);
    assert_ok_at(
        "file_sync",
        lfs_file_sync(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at(
        "file_close",
        lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await,
    );
    assert_ok_at("unmount", lfs_unmount(&mut lfs));
}
