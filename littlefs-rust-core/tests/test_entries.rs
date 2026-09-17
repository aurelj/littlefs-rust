//! Entry/inline file corner case tests.
//!
//! Upstream: tests/test_entries.toml
//! Source: https://github.com/littlefs-project/littlefs/blob/master/tests/test_entries.toml
//!
//! Metadata spill (4 files × 200B inline) and directory compaction.

mod common;

use common::{
    assert_ok, config_with_cache, init_context, init_logger, path_bytes, LFS_O_CREAT, LFS_O_RDONLY,
    LFS_O_TRUNC, LFS_O_WRONLY,
};
use littlefs_rust_core::{
    lfs_file_close, lfs_file_open, lfs_file_read, lfs_file_write, lfs_format, lfs_mount,
    lfs_remove, lfs_unmount, Lfs, LfsCaches, LfsConfig, LfsFile,
};

fn env_with_cache_512() -> common::TestEnv {
    config_with_cache(512, 128)
}

/// 2048 blocks matches upstream C test geometry (ERASE_COUNT=1M/512).
fn env_with_cache_512_2048_blocks() -> common::TestEnv {
    config_with_cache(512, 2048)
}

// --- test_entries_grow ---
#[tokio::test]
async fn test_entries_grow() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let buf = [b'c'; 1024];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = 20usize;
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
            )
            .await,
        );
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..size]).await;
        assert_eq!(n, size as i32);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut rb = [0u8; 256];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..20]).await;
    assert_eq!(n, 20);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..200]).await;
    assert_eq!(n, 200);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = if i == 1 { 200 } else { 20 };
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_RDONLY,
            )
            .await,
        );
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..size]).await;
        assert_eq!(n, size as i32);
        assert_eq!(&rb[..n as usize], &buf[..size]);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_shrink ---
#[tokio::test]
async fn test_entries_shrink() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let buf = [b'c'; 1024];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = if i == 1 { 200 } else { 20 };
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
            )
            .await,
        );
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..size]).await;
        assert_eq!(n, size as i32);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut rb = [0u8; 256];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..200]).await;
    assert_eq!(n, 200);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..20]).await;
    assert_eq!(n, 20);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = 20;
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_RDONLY,
            )
            .await,
        );
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..size]).await;
        assert_eq!(n, size as i32);
        assert_eq!(&rb[..n as usize], &buf[..size]);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_spill ---
#[tokio::test]
async fn test_entries_spill() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let buf = [b'c'; 256];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
            )
            .await,
        );
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..200]).await;
        assert_eq!(n, 200);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    let mut rb = [0u8; 256];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_RDONLY,
            )
            .await,
        );
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..200]).await;
        assert_eq!(n, 200);
        assert_eq!(&rb[..200], &buf[..200]);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_push_spill ---
#[tokio::test]
async fn test_entries_push_spill() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let buf = [b'c'; 256];
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi0").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..200]).await;
    assert_eq!(n, 200);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    for i in 1..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = if i == 1 { 20 } else { 200 };
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
            )
            .await,
        );
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..size]).await;
        assert_eq!(n, size as i32);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut rb = [0u8; 256];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..20]).await;
    assert_eq!(n, 20);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..200]).await;
    assert_eq!(n, 200);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_RDONLY,
            )
            .await,
        );
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..200]).await;
        assert_eq!(n, 200);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_drop ---
#[tokio::test]
async fn test_entries_drop() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let buf = [b'c'; 256];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let size = if i == 1 { 200 } else { 20 };
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
            )
            .await,
        );
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..size]).await;
        assert_eq!(n, size as i32);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_remove(&mut lfs, &mut caches, path_bytes("hi1").as_ptr()).await);
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("hi1").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &buf[..20]).await;
    assert_eq!(n, 20);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut rb = [0u8; 256];
    for i in 0..4 {
        let path = path_bytes(&format!("hi{i}"));
        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path.as_ptr(),
                LFS_O_RDONLY,
            )
            .await,
        );
        let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rb[..20]).await;
        assert_eq!(n, 20);
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    }

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_create_too_big ---
// Upstream: [cases.test_entries_create_too_big]
#[tokio::test]
async fn test_entries_create_too_big() {
    init_logger();
    let mut env = env_with_cache_512();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let path = path_bytes(&"m".repeat(200));
    let size = 400usize;
    let wbuf = [b'c'; 1024];
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &wbuf[..size]).await;
    assert_eq!(n, size as i32);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut rbuf = [0u8; 1024];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..size]).await;
    assert_eq!(n, size as i32);
    assert_eq!(&rbuf[..size], &wbuf[..size]);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_entries_resize_too_big ---
// Upstream: [cases.test_entries_resize_too_big]
// 200-byte path needs ample blocks; 2048 matches upstream geometry (ERASE_COUNT=1M/512).
#[tokio::test]
async fn test_entries_resize_too_big() {
    init_logger();
    let mut env = env_with_cache_512_2048_blocks();
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let path = path_bytes(&"m".repeat(200));
    let wbuf = [b'c'; 1024];
    let mut rbuf = [0u8; 1024];

    // Create with 40 bytes
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &wbuf[..40]).await;
    assert_eq!(n, 40);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    // Read 40 bytes
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..40]).await;
    assert_eq!(n, 40);
    assert_eq!(&rbuf[..40], &wbuf[..40]);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    // Truncate and write 400 bytes
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), &wbuf[..400]).await;
    assert_eq!(n, 400);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    // Read 400 bytes
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path.as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut rbuf[..400]).await;
    assert_eq!(n, 400);
    assert_eq!(&rbuf[..400], &wbuf[..400]);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_ok(lfs_unmount(&mut lfs));
}
