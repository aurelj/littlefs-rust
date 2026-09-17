//! Rust (littlefs-rust-core) wrapper for compat tests.

use std::mem::MaybeUninit;

use littlefs_rust_core::Storage;

use crate::storage::{check, path_cstr, prng_verify, test_prng, SharedStorage};

const LFS_O_RDONLY: i32 = 1;
const LFS_O_WRONLY: i32 = 2;
const LFS_O_CREAT: i32 = 0x0100;
const LFS_O_EXCL: i32 = 0x0200;
const LFS_ERR_EXIST: i32 = -17;

// ── Operation-level helpers (phase 2) ───────────────────────────────────

pub async fn format(storage: &SharedStorage) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_dir_names(storage: &SharedStorage, path: &str) -> Result<Vec<String>, i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let names = dir_names_mounted(&mut lfs, &mut caches, path).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(names)
}

pub async fn mount_read_file(storage: &SharedStorage, path: &str) -> Result<Vec<u8>, i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let data = read_file_mounted(&mut lfs, &mut caches, path).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(data)
}

pub async fn format_mkdir_unmount(storage: &SharedStorage, dir_name: &str) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    mkdir_mounted(&mut lfs, &mut caches, dir_name).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_mkdir_file_unmount(
    storage: &SharedStorage,
    dir_name: &str,
    file_name: &str,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    mkdir_mounted(&mut lfs, &mut caches, dir_name).await?;
    create_empty_file_mounted(&mut lfs, &mut caches, file_name).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_file_mkdir_unmount(
    storage: &SharedStorage,
    file_name: &str,
    dir_name: &str,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    create_empty_file_mounted(&mut lfs, &mut caches, file_name).await?;
    mkdir_mounted(&mut lfs, &mut caches, dir_name).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_three_unmount(storage: &SharedStorage) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for name in ["aaa", "zzz", "mmm"] {
        create_empty_file_mounted(&mut lfs, &mut caches, name).await?;
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_rename_unmount(
    storage: &SharedStorage,
    old_name: &str,
    new_name: &str,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    create_empty_file_mounted(&mut lfs, &mut caches, old_name).await?;
    let old = path_cstr(old_name);
    let new = path_cstr(new_name);
    check(littlefs_rust_core::lfs_rename(&mut lfs, &mut caches, old.as_ptr(), new.as_ptr()).await)?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_remove_unmount(storage: &SharedStorage, path: &str) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    create_empty_file_mounted(&mut lfs, &mut caches, path).await?;
    let p = path_cstr(path);
    check(littlefs_rust_core::lfs_remove(&mut lfs, &mut caches, p.as_ptr()).await)?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_write_unmount(
    storage: &SharedStorage,
    path: &str,
    content: &[u8],
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    write_file_mounted(&mut lfs, &mut caches, path, content).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_nested_dir_file_unmount(
    storage: &SharedStorage,
    parent: &str,
    child: &str,
    file_name: &str,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    mkdir_mounted(&mut lfs, &mut caches, parent).await?;
    let child_path = format!("{parent}/{child}");
    mkdir_mounted(&mut lfs, &mut caches, &child_path).await?;
    let file_path = format!("{child_path}/{file_name}");
    create_empty_file_mounted(&mut lfs, &mut caches, &file_path).await?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_mkdir_file_rmdir_unmount(
    storage: &SharedStorage,
    dir_name: &str,
    file_name: &str,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    mkdir_mounted(&mut lfs, &mut caches, dir_name).await?;
    let file_path = format!("{dir_name}/{file_name}");
    create_empty_file_mounted(&mut lfs, &mut caches, &file_path).await?;
    let fp = path_cstr(&file_path);
    check(littlefs_rust_core::lfs_remove(&mut lfs, &mut caches, fp.as_ptr()).await)?;
    let dp = path_cstr(dir_name);
    check(littlefs_rust_core::lfs_remove(&mut lfs, &mut caches, dp.as_ptr()).await)?;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_mkdir_expect_exist(storage: &SharedStorage, path: &str) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let p = path_cstr(path);
    let res = littlefs_rust_core::lfs_mkdir(&mut lfs, &mut caches, p.as_ptr()).await;
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    if res == LFS_ERR_EXIST {
        Ok(())
    } else if res == 0 {
        Err(-1)
    } else {
        Err(res)
    }
}

// ── Compat-level helpers (phase 3) ──────────────────────────────────────

pub async fn format_only(storage: &SharedStorage) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();
    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    Ok(())
}

pub async fn format_create_n_dirs(storage: &SharedStorage, count: usize) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in 0..count {
        mkdir_mounted(&mut lfs, &mut caches, &format!("dir{i}")).await?;
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_n_files_prng(
    storage: &SharedStorage,
    count: usize,
    size: u32,
    chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in 0..count {
        write_prng_file_mounted(
            &mut lfs,
            &mut caches,
            &format!("file{i}"),
            size,
            chunk,
            (i + 1) as u32,
        )
        .await?;
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn format_create_n_dirs_with_files_prng(
    storage: &SharedStorage,
    count: usize,
    size: u32,
    chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_format(&mut lfs, &mut caches, &env.config).await)?;
    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in 0..count {
        let dir = format!("dir{i}");
        mkdir_mounted(&mut lfs, &mut caches, &dir).await?;
        write_prng_file_mounted(
            &mut lfs,
            &mut caches,
            &format!("{dir}/file"),
            size,
            chunk,
            (i + 1) as u32,
        )
        .await?;
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_verify_n_empty_dirs(storage: &SharedStorage, count: usize) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        count,
        "expected {count} dirs, got {}",
        root.len()
    );
    for i in 0..count {
        let name = format!("dir{i}");
        assert!(root.contains(&name), "missing {name}");
        let contents = dir_names_mounted(&mut lfs, &mut caches, &name).await?;
        assert!(
            contents.is_empty(),
            "dir {name} should be empty, got {contents:?}"
        );
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_verify_n_files_prng(
    storage: &SharedStorage,
    count: usize,
    size: u32,
    _chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        count,
        "expected {count} files, got {}",
        root.len()
    );
    for i in 0..count {
        let path = format!("file{i}");
        let data = read_file_mounted(&mut lfs, &mut caches, &path).await?;
        assert_eq!(data.len(), size as usize, "file {path} size mismatch");
        prng_verify(&data, (i + 1) as u32);
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_verify_n_dirs_with_files_prng(
    storage: &SharedStorage,
    count: usize,
    size: u32,
    _chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        count,
        "expected {count} dirs, got {}",
        root.len()
    );
    for i in 0..count {
        let dir = format!("dir{i}");
        let contents = dir_names_mounted(&mut lfs, &mut caches, &dir).await?;
        assert_eq!(contents.len(), 1, "dir {dir} should have 1 file");
        let data = read_file_mounted(&mut lfs, &mut caches, &format!("{dir}/file")).await?;
        assert_eq!(data.len(), size as usize);
        prng_verify(&data, (i + 1) as u32);
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_create_dirs_and_list(
    storage: &SharedStorage,
    start: usize,
    count: usize,
    expected: usize,
) -> Result<Vec<String>, i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in start..(start + count) {
        mkdir_mounted(&mut lfs, &mut caches, &format!("dir{i}")).await?;
    }
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        expected,
        "expected {expected} entries, got {}",
        root.len()
    );
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(root)
}

pub async fn mount_create_files_prng_and_verify_all(
    storage: &SharedStorage,
    start: usize,
    count: usize,
    total: usize,
    size: u32,
    chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in start..(start + count) {
        write_prng_file_mounted(
            &mut lfs,
            &mut caches,
            &format!("file{i}"),
            size,
            chunk,
            (i + 1) as u32,
        )
        .await?;
    }
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        total,
        "expected {total} files, got {}",
        root.len()
    );
    for i in 0..total {
        let data = read_file_mounted(&mut lfs, &mut caches, &format!("file{i}")).await?;
        assert_eq!(data.len(), size as usize);
        prng_verify(&data, (i + 1) as u32);
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

pub async fn mount_create_dirs_files_prng_and_verify_all(
    storage: &SharedStorage,
    start: usize,
    count: usize,
    total: usize,
    size: u32,
    chunk: u32,
) -> Result<(), i32> {
    let env = storage.build_rust_env();
    let mut lfs =
        littlefs_rust_core::Lfs::new(unsafe { &mut *(env.config.context as *mut SharedStorage) });
    let mut caches = littlefs_rust_core::LfsCaches::default();

    check(littlefs_rust_core::lfs_mount(&mut lfs, &mut caches, &env.config).await)?;
    for i in start..(start + count) {
        let dir = format!("dir{i}");
        mkdir_mounted(&mut lfs, &mut caches, &dir).await?;
        write_prng_file_mounted(
            &mut lfs,
            &mut caches,
            &format!("{dir}/file"),
            size,
            chunk,
            (i + 1) as u32,
        )
        .await?;
    }
    let root = dir_names_mounted(&mut lfs, &mut caches, "/").await?;
    assert_eq!(
        root.len(),
        total,
        "expected {total} dirs, got {}",
        root.len()
    );
    for i in 0..total {
        let dir = format!("dir{i}");
        let data = read_file_mounted(&mut lfs, &mut caches, &format!("{dir}/file")).await?;
        assert_eq!(data.len(), size as usize);
        prng_verify(&data, (i + 1) as u32);
    }
    check(littlefs_rust_core::lfs_unmount(&mut lfs))?;
    Ok(())
}

// ── Internal helpers ────────────────────────────────────────────────────

async fn mkdir_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
) -> Result<(), i32> {
    let p = path_cstr(path);
    check(littlefs_rust_core::lfs_mkdir(lfs, caches, p.as_ptr()).await)
}

async fn create_empty_file_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
) -> Result<(), i32> {
    let p = path_cstr(path);
    let flags = LFS_O_WRONLY | LFS_O_CREAT | LFS_O_EXCL;
    let mut file = MaybeUninit::<littlefs_rust_core::LfsFile>::zeroed();
    check(
        littlefs_rust_core::lfs_file_open(lfs, caches, file.as_mut_ptr(), p.as_ptr(), flags).await,
    )?;
    check(littlefs_rust_core::lfs_file_close(lfs, caches, file.as_mut_ptr()).await)
}

async fn write_file_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
    content: &[u8],
) -> Result<(), i32> {
    let p = path_cstr(path);
    let flags = LFS_O_WRONLY | LFS_O_CREAT | LFS_O_EXCL;
    let mut file = MaybeUninit::<littlefs_rust_core::LfsFile>::zeroed();
    check(
        littlefs_rust_core::lfs_file_open(lfs, caches, file.as_mut_ptr(), p.as_ptr(), flags).await,
    )?;
    let n = littlefs_rust_core::lfs_file_write(lfs, caches, file.as_mut_ptr(), content).await;
    check(littlefs_rust_core::lfs_file_close(lfs, caches, file.as_mut_ptr()).await)?;
    if n < 0 {
        return Err(n);
    }
    assert_eq!(n as usize, content.len(), "short write");
    Ok(())
}

async fn write_prng_file_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
    size: u32,
    chunk: u32,
    seed: u32,
) -> Result<(), i32> {
    let p = path_cstr(path);
    let flags = LFS_O_WRONLY | LFS_O_CREAT | LFS_O_EXCL;
    let mut file = MaybeUninit::<littlefs_rust_core::LfsFile>::zeroed();
    check(
        littlefs_rust_core::lfs_file_open(lfs, caches, file.as_mut_ptr(), p.as_ptr(), flags).await,
    )?;

    let mut prng = seed;
    let mut buf = vec![0u8; chunk as usize];
    let mut i: u32 = 0;
    while i < size {
        let c = std::cmp::min(chunk, size - i);
        for slot in buf[..c as usize].iter_mut() {
            *slot = (test_prng(&mut prng) & 0xff) as u8;
        }
        let n =
            littlefs_rust_core::lfs_file_write(lfs, caches, file.as_mut_ptr(), &buf[..c as usize])
                .await;
        assert_eq!(n, c as i32, "short write at offset {i}");
        i += c;
    }
    check(littlefs_rust_core::lfs_file_close(lfs, caches, file.as_mut_ptr()).await)
}

async fn read_file_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
) -> Result<Vec<u8>, i32> {
    let p = path_cstr(path);
    let mut file = MaybeUninit::<littlefs_rust_core::LfsFile>::zeroed();
    check(
        littlefs_rust_core::lfs_file_open(lfs, caches, file.as_mut_ptr(), p.as_ptr(), LFS_O_RDONLY)
            .await,
    )?;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let n = littlefs_rust_core::lfs_file_read(lfs, caches, file.as_mut_ptr(), &mut chunk).await;
        if n < 0 {
            let _ = littlefs_rust_core::lfs_file_close(lfs, caches, file.as_mut_ptr()).await;
            return Err(n);
        }
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n as usize]);
    }
    check(littlefs_rust_core::lfs_file_close(lfs, caches, file.as_mut_ptr()).await)?;
    Ok(buf)
}

async fn dir_names_mounted<S: Storage>(
    lfs: &mut littlefs_rust_core::Lfs<S>,
    caches: &mut littlefs_rust_core::LfsCaches,
    path: &str,
) -> Result<Vec<String>, i32> {
    let p = path_cstr(path);
    let mut dir = MaybeUninit::<littlefs_rust_core::LfsDir>::zeroed();
    check(littlefs_rust_core::lfs_dir_open(lfs, caches, dir.as_mut_ptr(), p.as_ptr()).await)?;

    let mut names = Vec::new();
    let mut info = MaybeUninit::<littlefs_rust_core::LfsInfo>::zeroed();
    loop {
        let res =
            littlefs_rust_core::lfs_dir_read(lfs, caches, dir.as_mut_ptr(), info.as_mut_ptr())
                .await;
        if res == 0 {
            break;
        }
        if res < 0 {
            let _ = littlefs_rust_core::lfs_dir_close(lfs, dir.as_mut_ptr());
            return Err(res);
        }
        let info_ref = unsafe { &*info.as_ptr() };
        let nul = info_ref.name.iter().position(|&b| b == 0).unwrap_or(256);
        let name = core::str::from_utf8(&info_ref.name[..nul])
            .unwrap_or("")
            .to_string();
        if name != "." && name != ".." {
            names.push(name);
        }
    }
    let _ = littlefs_rust_core::lfs_dir_close(lfs, dir.as_mut_ptr());
    Ok(names)
}
