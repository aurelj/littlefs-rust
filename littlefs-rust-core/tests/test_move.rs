//! Move/rename tests.
//!
//! Upstream: tests/test_move.toml
//! Source: https://github.com/littlefs-project/littlefs/blob/master/tests/test_move.toml
//!
//! Corruption and powerloss tests; cross-dir rename implemented via lfs_rename_.
#![allow(clippy::manual_c_str_literals)]

mod common;

use common::{
    assert_err, assert_ok, config_with_wear_leveling, corrupt_block, default_config, dir_block,
    dir_entry_names, dir_pair, init_context, init_logger, init_wear_leveling_context, path_bytes,
    powerloss::{init_powerloss_context, powerloss_config, run_powerloss_linear},
    LFS_O_CREAT, LFS_O_RDONLY, LFS_O_TRUNC, LFS_O_WRONLY,
};
use littlefs_rust_core::lfs_type::lfs_type::{LFS_TYPE_DIR, LFS_TYPE_REG};
use littlefs_rust_core::LFS_ERR_NOENT;
use littlefs_rust_core::{
    lfs_dir_close, lfs_dir_open, lfs_dir_read, lfs_file_close, lfs_file_open, lfs_file_read,
    lfs_file_write, lfs_format, lfs_mkdir, lfs_mount, lfs_remove, lfs_rename, lfs_stat,
    lfs_unmount, Lfs, LfsCaches, LfsConfig, LfsDir, LfsFile, LfsInfo,
};

// --- test_move_nop ---
// Rename to self is legal
#[tokio::test]
async fn test_move_nop() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let hi = path_bytes("hi");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, hi.as_ptr()).await);
    assert_ok(lfs_rename(&mut lfs, &mut caches, hi.as_ptr(), hi.as_ptr()).await);

    let hi_hi = path_bytes("hi/hi");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, hi_hi.as_ptr()).await);
    assert_ok(lfs_rename(&mut lfs, &mut caches, hi_hi.as_ptr(), hi_hi.as_ptr()).await);

    let hi_hi_hi = path_bytes("hi/hi/hi");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, hi_hi_hi.as_ptr()).await);
    assert_ok(lfs_rename(&mut lfs, &mut caches, hi_hi_hi.as_ptr(), hi_hi_hi.as_ptr()).await);

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(lfs_stat(&mut lfs, &mut caches, hi_hi_hi.as_ptr(), info.as_mut_ptr()).await);
    let info = unsafe { info.assume_init() };
    let nul = info.name.iter().position(|&b| b == 0).unwrap_or(256);
    assert_eq!(core::str::from_utf8(&info.name[..nul]).unwrap(), "hi");
    assert_eq!(info.type_, LFS_TYPE_DIR as u8);

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_file ---
// Cross-dir rename a/hello -> c/hello
#[tokio::test]
async fn test_move_file() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);

    let a_hello = path_bytes("a/hello");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            a_hello.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n1 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"hola\n").await;
    assert_eq!(n1, 5);
    let n2 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"bonjour\n").await;
    assert_eq!(n2, 8);
    let n3 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"ohayo\n").await;
    assert_eq!(n3, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 0);
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 1);
    assert_eq!(c_names[0], "hello");

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    let info = unsafe { info.assume_init() };
    assert_eq!(info.size, 5 + 8 + 6);

    let mut info_dummy = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    let err_a = lfs_stat(
        &mut lfs,
        &mut caches,
        path_bytes("a/hello").as_ptr(),
        info_dummy.as_mut_ptr(),
    )
    .await;
    assert_err(LFS_ERR_NOENT, err_a);
    let err_b = lfs_stat(
        &mut lfs,
        &mut caches,
        path_bytes("b/hello").as_ptr(),
        info_dummy.as_mut_ptr(),
    )
    .await;
    assert_err(LFS_ERR_NOENT, err_b);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("c/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 32];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 5 + 8 + 6);
    assert_eq!(&buf[..5], b"hola\n");
    assert_eq!(&buf[5..13], b"bonjour\n");
    assert_eq!(&buf[13..19], b"ohayo\n");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    let mut file_dummy = core::mem::MaybeUninit::<LfsFile>::zeroed();
    let err_d = lfs_file_open(
        &mut lfs,
        &mut caches,
        file_dummy.as_mut_ptr(),
        path_bytes("d/hello").as_ptr(),
        LFS_O_RDONLY,
    )
    .await;
    assert_err(LFS_ERR_NOENT, err_d);
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_dir ---
// Cross-dir rename a/hi -> c/hi
#[tokio::test]
async fn test_move_dir() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/hola").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/bonjour").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/ohayo").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            path_bytes("c/hi").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let names = dir_entry_names(
        &mut lfs,
        &mut caches,
        &env.config as *const LfsConfig,
        "c/hi",
    )
    .await
    .unwrap();
    assert!(names.contains(&"bonjour".to_string()));
    assert!(names.contains(&"hola".to_string()));
    assert!(names.contains(&"ohayo".to_string()));
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_state_stealing ---
// Chain a->b->c->d then remove b,c
#[tokio::test]
async fn test_move_state_stealing() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("a/hello").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    let n1 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"hola\n").await;
    assert_eq!(n1, 5);
    let n2 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"bonjour\n").await;
    assert_eq!(n2, 8);
    let n3 = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"ohayo\n").await;
    assert_eq!(n3, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("b/hello").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("b/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("c/hello").as_ptr(),
            path_bytes("d/hello").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_remove(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("d/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 32];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 5 + 8 + 6);
    assert_eq!(&buf[..5], b"hola\n");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_create_delete_same ---
// Same-dir rename while files open
#[tokio::test]
async fn test_move_create_delete_same() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let f1 = path_bytes("1.move_me");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            f1.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let f0 = path_bytes("0.before");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            f0.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.1").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let f2 = path_bytes("2.in_between");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            f2.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.2").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let f4 = path_bytes("4.after");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            f4.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.3").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut fa = core::mem::MaybeUninit::<LfsFile>::zeroed();
    let mut fb = core::mem::MaybeUninit::<LfsFile>::zeroed();
    let mut fc = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fa.as_mut_ptr(),
            f0.as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fb.as_mut_ptr(),
            f2.as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fc.as_mut_ptr(),
            f4.as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    let _ = lfs_file_write(&mut lfs, &mut caches, fa.as_mut_ptr(), b"test.4").await;
    let _ = lfs_file_write(&mut lfs, &mut caches, fb.as_mut_ptr(), b"test.5").await;
    let _ = lfs_file_write(&mut lfs, &mut caches, fc.as_mut_ptr(), b"test.6").await;

    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("1.move_me").as_ptr(),
            path_bytes("3.move_me").as_ptr(),
        )
        .await,
    );

    assert_ok(lfs_file_close(&mut lfs, &mut caches, fa.as_mut_ptr()).await);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, fb.as_mut_ptr()).await);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, fc.as_mut_ptr()).await);

    let names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "/")
        .await
        .unwrap();
    assert!(names.contains(&"0.before".to_string()));
    assert!(names.contains(&"2.in_between".to_string()));
    assert!(names.contains(&"3.move_me".to_string()));
    assert!(names.contains(&"4.after".to_string()));

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("0.before").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 16];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 6);
    assert_eq!(&buf[..6], b"test.4");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_create_delete_delete_same ---
#[tokio::test]
async fn test_move_create_delete_delete_same() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("1.move_me").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("3.move_me").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"remove me").await;
    assert_eq!(n, 9);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("0.before").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.1").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("2.in_between").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.2").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("4.after").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"test.3").await;
    assert_eq!(n, 6);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut fa = core::mem::MaybeUninit::<LfsFile>::zeroed();
    let mut fb = core::mem::MaybeUninit::<LfsFile>::zeroed();
    let mut fc = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fa.as_mut_ptr(),
            path_bytes("0.before").as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fb.as_mut_ptr(),
            path_bytes("2.in_between").as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            fc.as_mut_ptr(),
            path_bytes("4.after").as_ptr(),
            LFS_O_WRONLY | LFS_O_TRUNC,
        )
        .await,
    );
    let _ = lfs_file_write(&mut lfs, &mut caches, fa.as_mut_ptr(), b"test.4").await;
    let _ = lfs_file_write(&mut lfs, &mut caches, fb.as_mut_ptr(), b"test.5").await;
    let _ = lfs_file_write(&mut lfs, &mut caches, fc.as_mut_ptr(), b"test.6").await;

    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("1.move_me").as_ptr(),
            path_bytes("3.move_me").as_ptr(),
        )
        .await,
    );

    assert_ok(lfs_file_close(&mut lfs, &mut caches, fa.as_mut_ptr()).await);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, fb.as_mut_ptr()).await);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, fc.as_mut_ptr()).await);

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("3.move_me").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    let info = unsafe { info.assume_init() };
    assert_eq!(info.size, 0);

    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_create_delete_different ---
// Cross-dir rename with overwrite
#[tokio::test]
async fn test_move_create_delete_different() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("dir.1").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("dir.2").as_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("dir.1/1.move_me").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("dir.2/1.move_me").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"remove me").await;
    assert_eq!(n, 9);
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("dir.1/1.move_me").as_ptr(),
            path_bytes("dir.2/1.move_me").as_ptr(),
        )
        .await,
    );

    let names = dir_entry_names(
        &mut lfs,
        &mut caches,
        &env.config as *const LfsConfig,
        "dir.2",
    )
    .await
    .unwrap();
    assert!(names.contains(&"1.move_me".to_string()));
    assert_ok(lfs_unmount(&mut lfs));
}

// --- Corruption: file rename ---

// Upstream: test_move_file_corrupt_source
// Corrupt source dir after rename; rename should stick.
#[tokio::test]
async fn test_move_file_corrupt_source() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("a/hello").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"hola\n").await,
        5
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"bonjour\n").await,
        8
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"ohayo\n").await,
        6
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 0);
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 1);
    assert_eq!(c_names[0], "hello");

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).size }, 5 + 8 + 6);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("c/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 32];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 5 + 8 + 6);
    assert_eq!(&buf[..5], b"hola\n");
    assert_eq!(&buf[5..13], b"bonjour\n");
    assert_eq!(&buf[13..19], b"ohayo\n");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_err(
        LFS_ERR_NOENT,
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("d/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// Upstream: test_move_file_corrupt_source_dest
// Corrupt both source and dest dirs; rename should roll back.
#[tokio::test]
async fn test_move_file_corrupt_source_dest() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("a/hello").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"hola\n").await,
        5
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"bonjour\n").await,
        8
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"ohayo\n").await,
        6
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    let cblock = dir_block(&mut lfs, &mut caches, "c").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;
    corrupt_block(&mut lfs.storage, cblock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 1);
    assert_eq!(a_names[0], "hello");
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 0);

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).size }, 5 + 8 + 6);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("a/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 32];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 5 + 8 + 6);
    assert_eq!(&buf[..5], b"hola\n");
    assert_eq!(&buf[5..13], b"bonjour\n");
    assert_eq!(&buf[13..19], b"ohayo\n");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_err(
        LFS_ERR_NOENT,
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("d/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// Upstream: test_move_file_after_corrupt
// Corrupt both, then redo rename; rename should succeed.
#[tokio::test]
async fn test_move_file_after_corrupt() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("a/hello").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC,
        )
        .await,
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"hola\n").await,
        5
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"bonjour\n").await,
        8
    );
    assert_eq!(
        lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"ohayo\n").await,
        6
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    let cblock = dir_block(&mut lfs, &mut caches, "c").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;
    corrupt_block(&mut lfs.storage, cblock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            path_bytes("c/hello").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 0);
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 1);
    assert_eq!(c_names[0], "hello");

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).size }, 5 + 8 + 6);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hello").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("c/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    let mut buf = [0u8; 32];
    let n = lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf).await;
    assert_eq!(n, 5 + 8 + 6);
    assert_eq!(&buf[..5], b"hola\n");
    assert_eq!(&buf[5..13], b"bonjour\n");
    assert_eq!(&buf[13..19], b"ohayo\n");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

    assert_err(
        LFS_ERR_NOENT,
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("d/hello").as_ptr(),
            LFS_O_RDONLY,
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_move_reentrant_file ---
// Power-loss at rename points; verify FS consistent after each simulated power loss.
#[tokio::test]
async fn test_move_reentrant_file() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("dir.1").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("dir.2").as_ptr()).await);
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            path_bytes("dir.1/1.move_me").as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        )
        .await,
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    let snapshot = env.snapshot();
    let path_src = path_bytes("dir.1/1.move_me");
    let path_dst = path_bytes("dir.2/1.move_me");

    let result = run_powerloss_linear(
        &mut env,
        &snapshot,
        128,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_rename(lfs, caches, path_src.as_ptr(), path_dst.as_ptr()).await;
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
    result.expect("test_move_reentrant_file should complete");
}

// Upstream: test_move_dir_corrupt_source
// Corrupt source dir after dir rename; rename should stick.
#[tokio::test]
async fn test_move_dir_corrupt_source() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/hola").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/bonjour").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/ohayo").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            path_bytes("c/hi").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 0);
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 1);
    assert_eq!(c_names[0], "hi");

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).type_ }, LFS_TYPE_DIR as u8);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let hi_names = dir_entry_names(
        &mut lfs,
        &mut caches,
        &env.config as *const LfsConfig,
        "c/hi",
    )
    .await
    .unwrap();
    assert!(hi_names.contains(&"hola".to_string()));
    assert!(hi_names.contains(&"bonjour".to_string()));
    assert!(hi_names.contains(&"ohayo".to_string()));

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("d/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// Upstream: test_move_dir_corrupt_source_dest
// Corrupt both source and dest; dir rename should roll back.
#[tokio::test]
async fn test_move_dir_corrupt_source_dest() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/hola").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/bonjour").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/ohayo").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            path_bytes("c/hi").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    let cblock = dir_block(&mut lfs, &mut caches, "c").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;
    corrupt_block(&mut lfs.storage, cblock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 1);
    assert_eq!(a_names[0], "hi");
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 0);

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).type_ }, LFS_TYPE_DIR as u8);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let hi_names = dir_entry_names(
        &mut lfs,
        &mut caches,
        &env.config as *const LfsConfig,
        "a/hi",
    )
    .await
    .unwrap();
    assert!(hi_names.contains(&"hola".to_string()));
    assert!(hi_names.contains(&"bonjour".to_string()));
    assert!(hi_names.contains(&"ohayo".to_string()));

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("d/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// Upstream: test_move_dir_after_corrupt
// Corrupt both, then redo dir rename; rename should succeed.
#[tokio::test]
async fn test_move_dir_after_corrupt() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/hola").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/bonjour").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/ohayo").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            path_bytes("c/hi").as_ptr(),
        )
        .await,
    );

    let ablock = dir_block(&mut lfs, &mut caches, "a").await;
    let cblock = dir_block(&mut lfs, &mut caches, "c").await;
    assert_ok(lfs_unmount(&mut lfs));
    corrupt_block(&mut lfs.storage, ablock).await;
    corrupt_block(&mut lfs.storage, cblock).await;

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(
        lfs_rename(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            path_bytes("c/hi").as_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    let a_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "a")
        .await
        .unwrap();
    assert_eq!(a_names.len(), 0);
    let c_names = dir_entry_names(&mut lfs, &mut caches, &env.config as *const LfsConfig, "c")
        .await
        .unwrap();
    assert_eq!(c_names.len(), 1);
    assert_eq!(c_names[0], "hi");

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("c/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_eq!(unsafe { (*info.as_ptr()).type_ }, LFS_TYPE_DIR as u8);

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("a/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("b/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );

    let hi_names = dir_entry_names(
        &mut lfs,
        &mut caches,
        &env.config as *const LfsConfig,
        "c/hi",
    )
    .await
    .unwrap();
    assert!(hi_names.contains(&"hola".to_string()));
    assert!(hi_names.contains(&"bonjour".to_string()));
    assert!(hi_names.contains(&"ohayo".to_string()));

    assert_err(
        LFS_ERR_NOENT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            path_bytes("d/hi").as_ptr(),
            info.as_mut_ptr(),
        )
        .await,
    );
    assert_ok(lfs_unmount(&mut lfs));
}

// --- test_reentrant_dir ---
// Power-loss at cross-dir dir rename points; verify FS consistent after each.
#[tokio::test]
async fn test_reentrant_dir() {
    init_logger();
    let mut env = powerloss_config(128);
    init_powerloss_context(&mut env);

    let mut lfs = Lfs::new(&mut env.ctx);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("b").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("c").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("d").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi").as_ptr()).await);
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("a/hi/hola").as_ptr()).await);
    assert_ok(lfs_unmount(&mut lfs));

    let snapshot = env.snapshot();
    let path_src = path_bytes("a/hi");
    let path_dst = path_bytes("c/hi");

    let result = run_powerloss_linear(
        &mut env,
        &snapshot,
        128,
        async |lfs, caches, config| {
            let err = lfs_mount(lfs, caches, config).await;
            if err != 0 {
                return Err(err);
            }
            let err = lfs_rename(lfs, caches, path_src.as_ptr(), path_dst.as_ptr()).await;
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
    result.expect("test_reentrant_dir should complete");
}

// --- Missing upstream stubs ---

/// Upstream: [cases.test_move_fix_relocation]
/// RELOCATIONS in 0..4, ERASE_CYCLES=0xffffffff. Force dir relocation via set_wear, then rename.
#[tokio::test]
async fn test_move_fix_relocation() {
    init_logger();
    const ERASE_CYCLES: u32 = 0xffffffff;
    let mut env = config_with_wear_leveling(256, ERASE_CYCLES);
    init_wear_leveling_context(&mut env);

    for relocations in 0..4u32 {
        let mut lfs = Lfs::new(&mut env.bd);
        let mut caches = LfsCaches::default();
        assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
        assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

        assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent").as_ptr()).await);
        assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent/child").as_ptr()).await);

        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path_bytes("parent/1.move_me").as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT,
            )
            .await,
        );
        assert_eq!(
            lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"move me\0").await,
            8
        );
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

        for (path, content) in [
            ("parent/0.before", b"test.1\0"),
            ("parent/2.after", b"test.2\0"),
            ("parent/child/0.before", b"test.3\0"),
            ("parent/child/2.after", b"test.4\0"),
        ] {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    file.as_mut_ptr(),
                    path_bytes(path).as_ptr(),
                    LFS_O_WRONLY | LFS_O_CREAT,
                )
                .await,
            );
            assert_eq!(
                lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), content).await,
                7
            );
            assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
        }

        let mut files = [
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
        ];
        let paths = [
            "parent/0.before",
            "parent/2.after",
            "parent/child/0.before",
            "parent/child/2.after",
        ];
        for (f, p) in files.iter_mut().zip(paths) {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    f.as_mut_ptr(),
                    path_bytes(p).as_ptr(),
                    LFS_O_WRONLY | LFS_O_TRUNC,
                )
                .await,
            );
        }
        for (f, content) in
            files
                .iter_mut()
                .zip([b"test.5\0", b"test.6\0", b"test.7\0", b"test.8\0"])
        {
            assert_eq!(
                lfs_file_write(&mut lfs, &mut caches, f.as_mut_ptr(), content).await,
                7
            );
        }

        if relocations & 1 != 0 {
            let pair = dir_pair(&mut lfs, &mut caches, "parent").await;
            lfs.storage.set_wear(pair[0], 0xffffffff);
            lfs.storage.set_wear(pair[1], 0xffffffff);
        }
        if relocations & 2 != 0 {
            let pair = dir_pair(&mut lfs, &mut caches, "parent/child").await;
            lfs.storage.set_wear(pair[0], 0xffffffff);
            lfs.storage.set_wear(pair[1], 0xffffffff);
        }

        assert_ok(
            lfs_rename(
                &mut lfs,
                &mut caches,
                path_bytes("parent/1.move_me").as_ptr(),
                path_bytes("parent/child/1.move_me").as_ptr(),
            )
            .await,
        );

        for f in &mut files {
            assert_ok(lfs_file_close(&mut lfs, &mut caches, f.as_mut_ptr()).await);
        }

        let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
        let mut dir = core::mem::MaybeUninit::<LfsDir>::zeroed();
        assert_ok(
            lfs_dir_open(
                &mut lfs,
                &mut caches,
                dir.as_mut_ptr(),
                path_bytes("parent").as_ptr(),
            )
            .await,
        );
        let expect_parent = ["0.before", "2.after", "child"];
        let mut idx = 0;
        loop {
            let n = lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await;
            assert!(n >= 0);
            if n == 0 {
                break;
            }
            let info_ref = unsafe { &*info.as_ptr() };
            let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
            let name = core::str::from_utf8(&info_ref.name[..nul]).unwrap();
            if name == "." || name == ".." {
                continue;
            }
            assert!(idx < expect_parent.len(), "extra entry: {name}");
            assert_eq!(name, expect_parent[idx]);
            if idx < 2 {
                assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
                assert_eq!(info_ref.size, 7);
            } else {
                assert_eq!(info_ref.type_, LFS_TYPE_DIR as u8);
            }
            idx += 1;
        }
        assert_eq!(idx, expect_parent.len());
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            0
        );
        assert_ok(lfs_dir_close(&mut lfs, dir.as_mut_ptr()));

        assert_ok(
            lfs_dir_open(
                &mut lfs,
                &mut caches,
                dir.as_mut_ptr(),
                path_bytes("parent/child").as_ptr(),
            )
            .await,
        );
        let expect_child = ["0.before", "1.move_me", "2.after"];
        let mut idx = 0;
        loop {
            let n = lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await;
            assert!(n >= 0);
            if n == 0 {
                break;
            }
            let info_ref = unsafe { &*info.as_ptr() };
            let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
            let name = core::str::from_utf8(&info_ref.name[..nul]).unwrap();
            if name == "." || name == ".." {
                continue;
            }
            assert!(idx < expect_child.len(), "extra entry: {name}");
            assert_eq!(name, expect_child[idx]);
            assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
            assert_eq!(info_ref.size, if name == "1.move_me" { 8 } else { 7 });
            idx += 1;
        }
        assert_eq!(idx, expect_child.len());
        assert_ok(lfs_dir_close(&mut lfs, dir.as_mut_ptr()));

        let mut buf = [0u8; 32];
        for (path, expected) in [
            ("parent/0.before", b"test.5\0"),
            ("parent/2.after", b"test.6\0"),
            ("parent/child/0.before", b"test.7\0"),
            ("parent/child/2.after", b"test.8\0"),
        ] {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    file.as_mut_ptr(),
                    path_bytes(path).as_ptr(),
                    LFS_O_RDONLY,
                )
                .await,
            );
            assert_eq!(
                lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf[..7]).await,
                7
            );
            assert_eq!(&buf[..6], &expected[..6]);
            assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
        }

        assert_ok(lfs_unmount(&mut lfs));
    }
}

/// Upstream: [cases.test_move_fix_relocation_predecessor]
/// RELOCATIONS in 0..8. Move sibling/1.move_me -> child/1.move_me with forced relocations.
#[tokio::test]
async fn test_move_fix_relocation_predecessor() {
    init_logger();
    const ERASE_CYCLES: u32 = 0xffffffff;
    let mut env = config_with_wear_leveling(256, ERASE_CYCLES);
    init_wear_leveling_context(&mut env);

    for relocations in 0..8u32 {
        let mut lfs = Lfs::new(&mut env.bd);
        let mut caches = LfsCaches::default();
        assert_ok(lfs_format(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);
        assert_ok(lfs_mount(&mut lfs, &mut caches, &env.config as *const LfsConfig).await);

        assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent").as_ptr()).await);
        assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent/child").as_ptr()).await);
        assert_ok(lfs_mkdir(&mut lfs, &mut caches, path_bytes("parent/sibling").as_ptr()).await);

        let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
        assert_ok(
            lfs_file_open(
                &mut lfs,
                &mut caches,
                file.as_mut_ptr(),
                path_bytes("parent/sibling/1.move_me").as_ptr(),
                LFS_O_WRONLY | LFS_O_CREAT,
            )
            .await,
        );
        assert_eq!(
            lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"move me\0").await,
            8
        );
        assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);

        for (path, content) in [
            ("parent/sibling/0.before", b"test.1\0"),
            ("parent/sibling/2.after", b"test.2\0"),
            ("parent/child/0.before", b"test.3\0"),
            ("parent/child/2.after", b"test.4\0"),
        ] {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    file.as_mut_ptr(),
                    path_bytes(path).as_ptr(),
                    LFS_O_WRONLY | LFS_O_CREAT,
                )
                .await,
            );
            assert_eq!(
                lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), content).await,
                7
            );
            assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
        }

        let mut files = [
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
            core::mem::MaybeUninit::<LfsFile>::zeroed(),
        ];
        let paths = [
            "parent/sibling/0.before",
            "parent/sibling/2.after",
            "parent/child/0.before",
            "parent/child/2.after",
        ];
        for (f, p) in files.iter_mut().zip(paths) {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    f.as_mut_ptr(),
                    path_bytes(p).as_ptr(),
                    LFS_O_WRONLY | LFS_O_TRUNC,
                )
                .await,
            );
        }
        for (f, content) in
            files
                .iter_mut()
                .zip([b"test.5\0", b"test.6\0", b"test.7\0", b"test.8\0"])
        {
            assert_eq!(
                lfs_file_write(&mut lfs, &mut caches, f.as_mut_ptr(), content).await,
                7
            );
        }

        if relocations & 1 != 0 {
            let pair = dir_pair(&mut lfs, &mut caches, "parent").await;
            lfs.storage.set_wear(pair[0], 0xffffffff);
            lfs.storage.set_wear(pair[1], 0xffffffff);
        }
        if relocations & 2 != 0 {
            let pair = dir_pair(&mut lfs, &mut caches, "parent/sibling").await;
            lfs.storage.set_wear(pair[0], 0xffffffff);
            lfs.storage.set_wear(pair[1], 0xffffffff);
        }
        if relocations & 4 != 0 {
            let pair = dir_pair(&mut lfs, &mut caches, "parent/child").await;
            lfs.storage.set_wear(pair[0], 0xffffffff);
            lfs.storage.set_wear(pair[1], 0xffffffff);
        }

        assert_ok(
            lfs_rename(
                &mut lfs,
                &mut caches,
                path_bytes("parent/sibling/1.move_me").as_ptr(),
                path_bytes("parent/child/1.move_me").as_ptr(),
            )
            .await,
        );

        for f in &mut files {
            assert_ok(lfs_file_close(&mut lfs, &mut caches, f.as_mut_ptr()).await);
        }

        let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
        let mut dir = core::mem::MaybeUninit::<LfsDir>::zeroed();
        assert_ok(
            lfs_dir_open(
                &mut lfs,
                &mut caches,
                dir.as_mut_ptr(),
                path_bytes("parent/sibling").as_ptr(),
            )
            .await,
        );
        // Skip . and ..
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            1
        );
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            1
        );
        let expect_sibling = ["0.before", "2.after"];
        for name in expect_sibling {
            assert_eq!(
                lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
                1
            );
            let info_ref = unsafe { &*info.as_ptr() };
            let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
            assert_eq!(core::str::from_utf8(&info_ref.name[..nul]).unwrap(), name);
            assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
            assert_eq!(info_ref.size, 7);
        }
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            0
        );
        assert_ok(lfs_dir_close(&mut lfs, dir.as_mut_ptr()));

        assert_ok(
            lfs_dir_open(
                &mut lfs,
                &mut caches,
                dir.as_mut_ptr(),
                path_bytes("parent/child").as_ptr(),
            )
            .await,
        );
        // Skip . and ..
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            1
        );
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            1
        );
        let expect_child = ["0.before", "1.move_me", "2.after"];
        for name in expect_child.iter() {
            assert_eq!(
                lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
                1
            );
            let info_ref = unsafe { &*info.as_ptr() };
            let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
            assert_eq!(core::str::from_utf8(&info_ref.name[..nul]).unwrap(), *name);
            assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
            if *name == "1.move_me" {
                assert_eq!(info_ref.size, 8);
            } else {
                assert_eq!(info_ref.size, 7);
            }
        }
        assert_eq!(
            lfs_dir_read(&mut lfs, &mut caches, dir.as_mut_ptr(), info.as_mut_ptr()).await,
            0
        );
        assert_ok(lfs_dir_close(&mut lfs, dir.as_mut_ptr()));

        let mut buf = [0u8; 32];
        for (path, expected) in [
            ("parent/sibling/0.before", b"test.5\0"),
            ("parent/sibling/2.after", b"test.6\0"),
            ("parent/child/0.before", b"test.7\0"),
            ("parent/child/2.after", b"test.8\0"),
        ] {
            assert_ok(
                lfs_file_open(
                    &mut lfs,
                    &mut caches,
                    file.as_mut_ptr(),
                    path_bytes(path).as_ptr(),
                    LFS_O_RDONLY,
                )
                .await,
            );
            assert_eq!(
                lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buf[..7]).await,
                7
            );
            assert_eq!(&buf[..6], &expected[..6]);
            assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()).await);
        }

        assert_ok(lfs_unmount(&mut lfs));
    }
}
