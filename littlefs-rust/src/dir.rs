use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem::MaybeUninit;

use littlefs_rust_core::{LfsDir, LfsInfo};

use crate::error::{from_lfs_result, Error};
use crate::filesystem::Filesystem;
use crate::metadata::{DirEntry, FileType};
use crate::storage::Storage;

pub(crate) struct DirAllocation {
    pub(crate) dir: MaybeUninit<LfsDir>,
}

impl DirAllocation {
    pub(crate) fn new() -> Self {
        Self {
            dir: MaybeUninit::zeroed(),
        }
    }
}

/// An open directory iterator.
///
/// Obtained from [`Filesystem::read_dir`]. Yields [`DirEntry`] items,
/// automatically skipping `.` and `..`. Closed on drop, or explicitly
/// via [`ReadDir::close`].
pub struct ReadDir<'a, S: Storage> {
    fs: &'a Filesystem<S>,
    alloc: Box<DirAllocation>,
    closed: bool,
}

impl<'a, S: Storage> ReadDir<'a, S> {
    pub(crate) async fn open(fs: &'a Filesystem<S>, path: &str) -> Result<Self, Error> {
        let mut alloc = Box::new(DirAllocation::new());
        let path_bytes = null_terminate(path);
        {
            let inner = &mut *fs.inner.borrow_mut();
            let rc = littlefs_rust_core::lfs_dir_open(
                &mut inner.lfs,
                &mut inner.caches,
                alloc.dir.as_mut_ptr(),
                path_bytes.as_ptr(),
            )
            .await;
            from_lfs_result(rc)?;
        }
        Ok(ReadDir {
            fs,
            alloc,
            closed: false,
        })
    }

    pub async fn next(&mut self) -> Option<Result<DirEntry, Error>> {
        loop {
            let mut info = MaybeUninit::<LfsInfo>::zeroed();
            let rc = {
                let inner = &mut *self.fs.inner.borrow_mut();
                littlefs_rust_core::lfs_dir_read(
                    &mut inner.lfs,
                    &mut inner.caches,
                    self.alloc.dir.as_mut_ptr(),
                    info.as_mut_ptr(),
                )
                .await
            };

            return match rc {
                0 => None,
                n if n < 0 => Some(Err(Error::from(n))),
                _ => {
                    let entry = dir_entry_from_info(unsafe { &*info.as_ptr() });
                    if entry.name == "." || entry.name == ".." {
                        continue;
                    }
                    Some(Ok(entry))
                }
            };
        }
    }

    pub async fn collect(&mut self) -> Result<Vec<DirEntry>, Error> {
        let mut entries = Vec::new();
        while let Some(result) = self.next().await {
            match result {
                Ok(entry) => entries.push(entry),
                Err(e) => return Err(e),
            }
        }
        Ok(entries)
    }

    /// Close the directory handle. Consumes `self`.
    ///
    /// Dropping a [`ReadDir`] also closes it, but errors are silently ignored.
    pub fn close(mut self) -> Result<(), Error> {
        self.closed = true;
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_dir_close(&mut inner.lfs, self.alloc.dir.as_mut_ptr());
        from_lfs_result(rc)
    }
}

impl<S: Storage> Drop for ReadDir<'_, S> {
    fn drop(&mut self) {
        if !self.closed {
            if let Ok(inner) = self.fs.inner.try_borrow_mut().as_deref_mut() {
                let _ =
                    littlefs_rust_core::lfs_dir_close(&mut inner.lfs, self.alloc.dir.as_mut_ptr());
            }
        }
    }
}

pub(crate) fn dir_entry_from_info(info: &LfsInfo) -> DirEntry {
    let nul = info
        .name
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(info.name.len());
    let name = core::str::from_utf8(&info.name[..nul]).unwrap_or("").into();
    let file_type = if info.type_ == littlefs_rust_core::lfs_type::lfs_type::LFS_TYPE_DIR as u8 {
        FileType::Dir
    } else {
        FileType::File
    };
    DirEntry {
        name,
        file_type,
        size: info.size,
    }
}

fn null_terminate(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.bytes().collect();
    v.push(0);
    v
}
