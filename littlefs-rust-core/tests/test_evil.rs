//! Upstream: tests/test_evil.toml
//!
//! Evil/corruption tests: invalid pointers, loops in metadata.
//! These corrupt metadata on disk via raw block writes or lfs_dir_commit
//! with forged tags, then verify mount/open/stat return appropriate errors.

mod common;

use common::{
    assert_err, assert_ok, default_config, init_context, path_bytes, LFS_O_CREAT, LFS_O_RDONLY,
    LFS_O_WRONLY,
};
use littlefs_rust_core::lfs_type::lfs_type::*;
use littlefs_rust_core::{
    lfs_ctz_fromle32, lfs_deinit, lfs_dir_commit, lfs_dir_fetch, lfs_dir_get, lfs_dir_open,
    lfs_file_close, lfs_file_open, lfs_file_read, lfs_file_write, lfs_format, lfs_fs_prepmove,
    lfs_init, lfs_mkdir, lfs_mktag, lfs_mount, lfs_pair_fromle32, lfs_stat, lfs_tole32,
    lfs_unmount, Lfs, LfsCaches, LfsConfig, LfsCtz, LfsDir, LfsFile, LfsInfo, LfsMdir,
    LFS_ERR_CORRUPT,
};
use littlefs_rust_core::{lfs_mattr, Storage};

const BLOCK_SIZE: u32 = 512;
const BLOCK_COUNT: u32 = 256;

/// Upstream: [cases.test_evil_invalid_tail_pointer]
///
/// defines.TAIL_TYPE = [LFS_TYPE_HARDTAIL, LFS_TYPE_SOFTTAIL]
/// defines.INVALSET = [0x3, 0x1, 0x2]
///
/// Format, then commit a TAIL_TYPE tag with invalid pair to root metadata.
/// Expect lfs_mount to return LFS_ERR_CORRUPT.
#[test]
fn test_evil_invalid_tail_pointer() {
    for &tail_type in &[LFS_TYPE_HARDTAIL, LFS_TYPE_SOFTTAIL] {
        for &invalset in &[0x3u32, 0x1, 0x2] {
            unsafe { evil_invalid_tail_pointer(tail_type, invalset) };
        }
    }
}

unsafe fn evil_invalid_tail_pointer(tail_type: u32, invalset: u32) {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));

    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    let invalid_pair: [u32; 2] = [
        if invalset & 0x1 != 0 { 0xcccccccc } else { 0 },
        if invalset & 0x2 != 0 { 0xcccccccc } else { 0 },
    ];
    let attrs = [lfs_mattr {
        tag: lfs_mktag(tail_type, 0x3ff, 8),
        buffer: invalid_pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    assert_err(LFS_ERR_CORRUPT, lfs_mount(&mut lfs, &mut caches, cfg));
}

/// Upstream: [cases.test_evil_invalid_dir_pointer]
///
/// defines.INVALSET = [0x3, 0x1, 0x2]
///
/// Format, create "dir_here", commit a DIRSTRUCT tag with invalid pair.
/// Mount succeeds, stat works, but dir_open/stat-child/file_open fail
/// with LFS_ERR_CORRUPT.
#[test]
fn test_evil_invalid_dir_pointer() {
    for &invalset in &[0x3u32, 0x1, 0x2] {
        unsafe { evil_invalid_dir_pointer(invalset) };
    }
}

unsafe fn evil_invalid_dir_pointer(invalset: u32) {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    let dir_name = path_bytes("dir_here");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, dir_name.as_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Corrupt the dir pointer
    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    // Verify id 1 == our directory
    let mut buffer = [0u8; 1024];
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x700, 0x3ff, 0),
        lfs_mktag(LFS_TYPE_NAME, 1, 8), // strlen("dir_here") == 8
        buffer.as_mut_ptr() as *mut core::ffi::c_void,
    );
    assert_eq!(tag, lfs_mktag(LFS_TYPE_DIR, 1, 8) as i32);
    assert_eq!(&buffer[..8], b"dir_here");

    let invalid_pair: [u32; 2] = [
        if invalset & 0x1 != 0 { 0xcccccccc } else { 0 },
        if invalset & 0x2 != 0 { 0xcccccccc } else { 0 },
    ];
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_DIRSTRUCT, 1, 8),
        buffer: invalid_pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    // Verify corruption behavior
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(lfs_stat(
        &mut lfs,
        &mut caches,
        dir_name.as_ptr(),
        info.as_mut_ptr(),
    ));
    let info_ref = &*info.as_ptr();
    let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
    assert_eq!(&info_ref.name[..nul], b"dir_here");
    assert_eq!(info_ref.type_, LFS_TYPE_DIR as u8);

    let mut dir = core::mem::MaybeUninit::<LfsDir>::zeroed();
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_dir_open(&mut lfs, &mut caches, dir.as_mut_ptr(), dir_name.as_ptr()),
    );

    let child_file = path_bytes("dir_here/file_here");
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_stat(
            &mut lfs,
            &mut caches,
            child_file.as_ptr(),
            info.as_mut_ptr(),
        ),
    );

    let child_dir = path_bytes("dir_here/dir_here");
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_dir_open(&mut lfs, &mut caches, dir.as_mut_ptr(), child_dir.as_ptr()),
    );

    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            child_file.as_ptr(),
            LFS_O_RDONLY,
        ),
    );
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_file_open(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            child_file.as_ptr(),
            LFS_O_WRONLY | LFS_O_CREAT,
        ),
    );

    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_evil_invalid_file_pointer]
///
/// defines.SIZE = [10, 1000, 100000]
///
/// Create "file_here" (empty). Corrupt its CTZSTRUCT to point at 0xcccccccc
/// with faked size. Mount + stat succeed. file_read fails with LFS_ERR_CORRUPT.
/// If SIZE > 2*BLOCK_SIZE, mkdir also fails (GC triggers corrupt read).
#[test]
fn test_evil_invalid_file_pointer() {
    for &size in &[10u32, 1000, 100000] {
        unsafe { evil_invalid_file_pointer(size) };
    }
}

unsafe fn evil_invalid_file_pointer(size: u32) {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let file_name = path_bytes("file_here");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        file_name.as_ptr(),
        LFS_O_WRONLY | LFS_O_CREAT,
    ));
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Corrupt the file pointer
    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    // Verify id 1 == our file
    let mut buffer = [0u8; 1024];
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x700, 0x3ff, 0),
        lfs_mktag(LFS_TYPE_NAME, 1, 9), // strlen("file_here") == 9
        buffer.as_mut_ptr() as *mut core::ffi::c_void,
    );
    assert_eq!(tag, lfs_mktag(LFS_TYPE_REG, 1, 9) as i32);
    assert_eq!(&buffer[..9], b"file_here");

    // Forge a CTZSTRUCT with invalid head and faked size
    let fake_ctz = LfsCtz {
        head: 0xcccccccc,
        size: lfs_tole32(size),
    };
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_CTZSTRUCT, 1, core::mem::size_of::<LfsCtz>() as u32),
        buffer: &fake_ctz as *const _ as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    // Verify corruption behavior
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(lfs_stat(
        &mut lfs,
        &mut caches,
        file_name.as_ptr(),
        info.as_mut_ptr(),
    ));
    let info_ref = &*info.as_ptr();
    let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
    assert_eq!(&info_ref.name[..nul], b"file_here");
    assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
    assert_eq!(info_ref.size, size);

    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        file_name.as_ptr(),
        LFS_O_RDONLY,
    ));
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_file_read(&mut lfs, &mut caches, file.as_mut_ptr(), &mut buffer),
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));

    if size > 2 * BLOCK_SIZE {
        let dir_name = path_bytes("dir_here");
        assert_err(
            LFS_ERR_CORRUPT,
            lfs_mkdir(&mut lfs, &mut caches, dir_name.as_ptr()),
        );
    }

    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_evil_invalid_ctz_pointer]
///
/// defines.SIZE = [2*BLOCK_SIZE, 3*BLOCK_SIZE, 4*BLOCK_SIZE]
///
/// Create file of SIZE bytes. Corrupt the CTZ skip-list head block by writing
/// invalid block pointers into it. Mount + stat succeed. File read fails
/// with LFS_ERR_CORRUPT. If SIZE > 2*BLOCK_SIZE, mkdir also fails.
#[test]
fn test_evil_invalid_ctz_pointer() {
    for &size in &[2 * BLOCK_SIZE, 3 * BLOCK_SIZE, 4 * BLOCK_SIZE] {
        unsafe { evil_invalid_ctz_pointer(size) };
    }
}

unsafe fn evil_invalid_ctz_pointer(size: u32) {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let file_name = path_bytes("file_here");
    let mut file = core::mem::MaybeUninit::<LfsFile>::zeroed();
    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        file_name.as_ptr(),
        LFS_O_WRONLY | LFS_O_CREAT,
    ));
    for _ in 0..size {
        let n = lfs_file_write(&mut lfs, &mut caches, file.as_mut_ptr(), b"c");
        assert_eq!(n, 1);
    }
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Read the CTZ struct and corrupt the head block
    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    // Verify id 1 == our file
    let mut buffer = vec![0u8; 4 * BLOCK_SIZE as usize];
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x700, 0x3ff, 0),
        lfs_mktag(LFS_TYPE_NAME, 1, 9),
        buffer.as_mut_ptr() as *mut core::ffi::c_void,
    );
    assert_eq!(tag, lfs_mktag(LFS_TYPE_REG, 1, 9) as i32);
    assert_eq!(&buffer[..9], b"file_here");

    // Get CTZ struct
    let mut ctz = LfsCtz { head: 0, size: 0 };
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x700, 0x3ff, 0),
        lfs_mktag(LFS_TYPE_STRUCT, 1, core::mem::size_of::<LfsCtz>() as u32),
        &mut ctz as *mut _ as *mut core::ffi::c_void,
    );
    assert_eq!(
        tag,
        lfs_mktag(LFS_TYPE_CTZSTRUCT, 1, core::mem::size_of::<LfsCtz>() as u32) as i32
    );
    lfs_ctz_fromle32(&mut ctz);

    // Rewrite ctz.head block with bad pointers at offsets 0 and 4
    let mut bbuffer = vec![0u8; BLOCK_SIZE as usize];
    assert!(lfs.storage.read(ctz.head, 0, &mut bbuffer).is_ok());
    let bad: u32 = lfs_tole32(0xcccccccc);
    bbuffer[0..4].copy_from_slice(&bad.to_ne_bytes());
    bbuffer[4..8].copy_from_slice(&bad.to_ne_bytes());
    assert!(lfs.storage.erase(ctz.head).is_ok());
    assert!(lfs.storage.write(ctz.head, 0, &bbuffer).is_ok());
    assert_ok(lfs_deinit(&mut lfs));

    // Verify corruption behavior
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));

    let mut info = core::mem::MaybeUninit::<LfsInfo>::zeroed();
    assert_ok(lfs_stat(
        &mut lfs,
        &mut caches,
        file_name.as_ptr(),
        info.as_mut_ptr(),
    ));
    let info_ref = &*info.as_ptr();
    let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
    assert_eq!(&info_ref.name[..nul], b"file_here");
    assert_eq!(info_ref.type_, LFS_TYPE_REG as u8);
    assert_eq!(info_ref.size, size);

    assert_ok(lfs_file_open(
        &mut lfs,
        &mut caches,
        file.as_mut_ptr(),
        file_name.as_ptr(),
        LFS_O_RDONLY,
    ));
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_file_read(
            &mut lfs,
            &mut caches,
            file.as_mut_ptr(),
            &mut buffer[..size as usize],
        ),
    );
    assert_ok(lfs_file_close(&mut lfs, &mut caches, file.as_mut_ptr()));

    if size > 2 * BLOCK_SIZE {
        let dir_name = path_bytes("dir_here");
        assert_err(
            LFS_ERR_CORRUPT,
            lfs_mkdir(&mut lfs, &mut caches, dir_name.as_ptr()),
        );
    }

    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_evil_invalid_gstate_pointer]
///
/// defines.INVALSET = [0x3, 0x1, 0x2]
///
/// Corrupt gstate via lfs_fs_prepmove with invalid move pointer.
/// Mount may succeed but first lfs_mkdir fails with LFS_ERR_CORRUPT.
#[test]
fn test_evil_invalid_gstate_pointer() {
    for &invalset in &[0x3u32, 0x1, 0x2] {
        unsafe { evil_invalid_gstate_pointer(invalset) };
    }
}

unsafe fn evil_invalid_gstate_pointer(invalset: u32) {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));

    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    let invalid_pair: [u32; 2] = [
        if invalset & 0x1 != 0 { 0xcccccccc } else { 0 },
        if invalset & 0x2 != 0 { 0xcccccccc } else { 0 },
    ];
    lfs_fs_prepmove(&mut lfs, 1, &invalid_pair);
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        core::ptr::null(),
        0,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    let dir_name = path_bytes("should_fail");
    assert_err(
        LFS_ERR_CORRUPT,
        lfs_mkdir(&mut lfs, &mut caches, dir_name.as_ptr()),
    );
    assert_ok(lfs_unmount(&mut lfs));
}

/// Upstream: [cases.test_evil_mdir_loop]
///
/// Change root tail to point at (0, 1) (itself), forming a 1-length
/// metadata loop. Expect mount to fail with LFS_ERR_CORRUPT.
#[test]
fn test_evil_mdir_loop() {
    unsafe { evil_mdir_loop() };
}

unsafe fn evil_mdir_loop() {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));

    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &pair,
    ));

    let self_pair: [u32; 2] = [0, 1];
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_HARDTAIL, 0x3ff, 8),
        buffer: self_pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    assert_err(LFS_ERR_CORRUPT, lfs_mount(&mut lfs, &mut caches, cfg));
}

/// Upstream: [cases.test_evil_mdir_loop2]
///
/// Create "child" dir. Corrupt child's tail to point at root (0, 1),
/// forming a 2-length loop. Expect mount to fail with LFS_ERR_CORRUPT.
#[test]
fn test_evil_mdir_loop2() {
    unsafe { evil_mdir_loop2() };
}

unsafe fn evil_mdir_loop2() {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    let child = path_bytes("child");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, child.as_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Find child's block pair
    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let root_pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &root_pair,
    ));

    let mut child_pair: [u32; 2] = [0; 2];
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x7ff, 0x3ff, 0),
        lfs_mktag(
            LFS_TYPE_DIRSTRUCT,
            1,
            core::mem::size_of::<[u32; 2]>() as u32,
        ),
        child_pair.as_mut_ptr() as *mut core::ffi::c_void,
    );
    assert_eq!(
        tag,
        lfs_mktag(
            LFS_TYPE_DIRSTRUCT,
            1,
            core::mem::size_of::<[u32; 2]>() as u32
        ) as i32
    );
    lfs_pair_fromle32(&mut child_pair);

    // Corrupt child's tail to point at root
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &child_pair,
    ));
    let root_ptr: [u32; 2] = [0, 1];
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_HARDTAIL, 0x3ff, 8),
        buffer: root_ptr.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    assert_err(LFS_ERR_CORRUPT, lfs_mount(&mut lfs, &mut caches, cfg));
}

/// Upstream: [cases.test_evil_mdir_loop_child]
///
/// Create "child" dir. Corrupt child's tail to point at itself (child's
/// own block pair), forming a 1-length child loop. Expect mount to fail
/// with LFS_ERR_CORRUPT.
#[test]
fn test_evil_mdir_loop_child() {
    unsafe { evil_mdir_loop_child() };
}

unsafe fn evil_mdir_loop_child() {
    let mut env = default_config(BLOCK_COUNT);
    init_context(&mut env);
    let cfg = &env.config as *const LfsConfig;

    let mut lfs = Lfs::new(&mut env.ram);
    let mut caches = LfsCaches::default();
    assert_ok(lfs_format(&mut lfs, &mut caches, cfg));
    assert_ok(lfs_mount(&mut lfs, &mut caches, cfg));
    let child = path_bytes("child");
    assert_ok(lfs_mkdir(&mut lfs, &mut caches, child.as_ptr()));
    assert_ok(lfs_unmount(&mut lfs));

    // Find child's block pair
    assert_ok(lfs_init(&mut lfs, &mut caches, cfg));
    let mut mdir = core::mem::MaybeUninit::<LfsMdir>::zeroed();
    let root_pair: [u32; 2] = [0, 1];
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &root_pair,
    ));

    let mut child_pair: [u32; 2] = [0; 2];
    let tag = lfs_dir_get(
        &mut lfs,
        &mut caches,
        mdir.as_ptr(),
        lfs_mktag(0x7ff, 0x3ff, 0),
        lfs_mktag(
            LFS_TYPE_DIRSTRUCT,
            1,
            core::mem::size_of::<[u32; 2]>() as u32,
        ),
        child_pair.as_mut_ptr() as *mut core::ffi::c_void,
    );
    assert_eq!(
        tag,
        lfs_mktag(
            LFS_TYPE_DIRSTRUCT,
            1,
            core::mem::size_of::<[u32; 2]>() as u32
        ) as i32
    );
    lfs_pair_fromle32(&mut child_pair);

    // Corrupt child's tail to point at itself
    assert_ok(lfs_dir_fetch(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        &child_pair,
    ));
    let attrs = [lfs_mattr {
        tag: lfs_mktag(LFS_TYPE_HARDTAIL, 0x3ff, 8),
        buffer: child_pair.as_ptr() as *const core::ffi::c_void,
    }];
    assert_ok(lfs_dir_commit(
        &mut lfs,
        &mut caches,
        mdir.as_mut_ptr(),
        attrs.as_ptr() as *const core::ffi::c_void,
        1,
    ));
    assert_ok(lfs_deinit(&mut lfs));

    assert_err(LFS_ERR_CORRUPT, lfs_mount(&mut lfs, &mut caches, cfg));
}
