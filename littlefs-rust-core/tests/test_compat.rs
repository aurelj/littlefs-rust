//! Upstream: tests/test_compat.toml
//!
//! Version edge-case tests. The 14 forward/backward compat tests live in
//! littlefs-rust-core-compat where they test actual C ↔ Rust interop.
//! These 3 remaining tests exercise superblock version field handling.

mod common;

use common::{assert_err, assert_ok, default_config, init_context, init_logger, path_bytes};
use littlefs_rust_core::lfs_type::lfs_type::LFS_TYPE_INLINESTRUCT;
use littlefs_rust_core::{
    lfs_dir_commit, lfs_dir_fetch, lfs_format, lfs_fs_stat, lfs_mattr, lfs_mktag, lfs_mount,
    lfs_superblock_tole32, lfs_unmount, Lfs, LfsCaches, LfsConfig, LfsFsinfo, LfsMdir,
    LfsSuperblock, LFS_DISK_VERSION, LFS_ERR_INVAL,
};

/// Upstream: [cases.test_compat_major_incompat]
///
/// Bump major version in superblock, verify mount rejects with LFS_ERR_INVAL.
#[test]
fn test_compat_major_incompat() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::default();
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

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
    assert_ok(lfs_dir_fetch(&mut lfs, &mut caches, &mut mdir, &root_pair));

    let cfg = unsafe { &*lfs.cfg };
    let mut superblock = LfsSuperblock {
        version: LFS_DISK_VERSION + 0x0001_0000,
        block_size: cfg.block_size,
        block_count: cfg.block_count,
        name_max: lfs.name_max,
        file_max: lfs.file_max,
        attr_max: lfs.attr_max,
    };
    lfs_superblock_tole32(&mut superblock);
    let attrs = [lfs_mattr {
        tag: lfs_mktag(
            LFS_TYPE_INLINESTRUCT,
            0,
            core::mem::size_of::<LfsSuperblock>() as u32,
        ),
        buffer: &superblock as *const LfsSuperblock as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        &mut mdir,
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_unmount(&mut lfs));

    assert_err(LFS_ERR_INVAL, lfs_mount(&mut lfs, &mut caches, cfg));
}

/// Upstream: [cases.test_compat_minor_incompat]
///
/// Bump minor version in superblock beyond what we support, verify mount rejects.
#[test]
fn test_compat_minor_incompat() {
    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::default();
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

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
    assert_ok(lfs_dir_fetch(&mut lfs, &mut caches, &mut mdir, &root_pair));

    let cfg = unsafe { &*lfs.cfg };
    let mut superblock = LfsSuperblock {
        version: LFS_DISK_VERSION + 0x0000_0001,
        block_size: cfg.block_size,
        block_count: cfg.block_count,
        name_max: lfs.name_max,
        file_max: lfs.file_max,
        attr_max: lfs.attr_max,
    };
    lfs_superblock_tole32(&mut superblock);
    let attrs = [lfs_mattr {
        tag: lfs_mktag(
            LFS_TYPE_INLINESTRUCT,
            0,
            core::mem::size_of::<LfsSuperblock>() as u32,
        ),
        buffer: &superblock as *const LfsSuperblock as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        &mut mdir,
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_unmount(&mut lfs));

    assert_err(LFS_ERR_INVAL, lfs_mount(&mut lfs, &mut caches, cfg));
}

/// Upstream: [cases.test_compat_minor_bump]
///
/// Downgrade minor version in superblock, mount works, write triggers minor bump.
#[test]
fn test_compat_minor_bump() {
    use littlefs_rust_core::lfs_type::lfs_open_flags::{
        LFS_O_CREAT, LFS_O_EXCL, LFS_O_RDONLY, LFS_O_TRUNC, LFS_O_WRONLY,
    };
    use littlefs_rust_core::{lfs_file_close, lfs_file_open, lfs_file_read, lfs_file_write};

    init_logger();
    let mut env = default_config(128);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::default();
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let mut file = core::mem::MaybeUninit::<littlefs_rust_core::LfsFile>::zeroed();
    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        path_bytes("test").as_ptr(),
        LFS_O_WRONLY | LFS_O_CREAT | LFS_O_EXCL,
    ));
    assert_eq!(
        lfs_file_write(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            b"testtest".as_ptr() as *const core::ffi::c_void,
            8,
        ),
        8
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Write old minor version to superblock
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
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
    assert_ok(lfs_dir_fetch(&mut lfs, &mut caches, &mut mdir, &root_pair));

    let config_ref = unsafe { &*lfs.cfg };
    let mut superblock = LfsSuperblock {
        version: LFS_DISK_VERSION - 1,
        block_size: config_ref.block_size,
        block_count: config_ref.block_count,
        name_max: lfs.name_max,
        file_max: lfs.file_max,
        attr_max: lfs.attr_max,
    };
    lfs_superblock_tole32(&mut superblock);
    let attrs = [lfs_mattr {
        tag: lfs_mktag(
            LFS_TYPE_INLINESTRUCT,
            0,
            core::mem::size_of::<LfsSuperblock>() as u32,
        ),
        buffer: &superblock as *const LfsSuperblock as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        &mut mdir,
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_unmount(&mut lfs));

    // Mount should work
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let mut fsinfo = core::mem::MaybeUninit::<LfsFsinfo>::zeroed();
    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(
        unsafe { (*fsinfo.as_ptr()).disk_version },
        LFS_DISK_VERSION - 1
    );

    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        path_bytes("test").as_ptr(),
        LFS_O_RDONLY,
    ));
    let mut buf = [0u8; 8];
    assert_eq!(
        lfs_file_read(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            8,
        ),
        8
    );
    assert_eq!(&buf, b"testtest");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));

    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(
        unsafe { (*fsinfo.as_ptr()).disk_version },
        LFS_DISK_VERSION - 1
    );
    assert_ok(lfs_unmount(&mut lfs));

    // Write should bump minor version
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(
        unsafe { (*fsinfo.as_ptr()).disk_version },
        LFS_DISK_VERSION - 1
    );

    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        path_bytes("test").as_ptr(),
        LFS_O_WRONLY | LFS_O_TRUNC,
    ));
    assert_eq!(
        lfs_file_write(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            b"teeeeest".as_ptr() as *const core::ffi::c_void,
            8,
        ),
        8
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));

    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(unsafe { (*fsinfo.as_ptr()).disk_version }, LFS_DISK_VERSION);
    assert_ok(lfs_unmount(&mut lfs));

    // Remount, verify version stayed bumped
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(unsafe { (*fsinfo.as_ptr()).disk_version }, LFS_DISK_VERSION);

    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        path_bytes("test").as_ptr(),
        LFS_O_RDONLY,
    ));
    assert_eq!(
        lfs_file_read(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            8,
        ),
        8
    );
    assert_eq!(&buf, b"teeeeest");
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));

    assert_ok(lfs_fs_stat(&mut lfs, &mut caches, fsinfo.as_mut_ptr()));
    assert_eq!(unsafe { (*fsinfo.as_ptr()).disk_version }, LFS_DISK_VERSION);
    assert_ok(lfs_unmount(&mut lfs));
}
