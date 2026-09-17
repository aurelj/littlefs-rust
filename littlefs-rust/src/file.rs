use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::future::Future;
use core::mem::MaybeUninit;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use littlefs_rust_core::{LfsFile, LfsFileConfig};

use crate::error::{from_lfs_result, from_lfs_size, Error};
use crate::filesystem::Filesystem;
use crate::metadata::{OpenFlags, SeekFrom};
use crate::storage::Storage;

pub(crate) struct FileAllocation {
    pub(crate) file: MaybeUninit<LfsFile>,
    _cache: Vec<u8>,
    pub(crate) file_config: LfsFileConfig,
}

impl FileAllocation {
    pub(crate) fn new(cache_size: u32) -> Self {
        let mut cache = vec![0u8; cache_size as usize];
        let file_config = LfsFileConfig {
            buffer: cache.as_mut_ptr() as *mut c_void,
            attrs: core::ptr::null_mut(),
            attr_count: 0,
        };
        Self {
            file: MaybeUninit::zeroed(),
            _cache: cache,
            file_config,
        }
    }
}

/// An open file handle.
///
/// Obtained from [`Filesystem::open`]. Automatically closed on drop; call
/// [`File::close`] explicitly to check for errors.
pub struct File<'a, S: Storage> {
    fs: &'a Filesystem<S>,
    alloc: Box<FileAllocation>,
    closed: bool,
}

impl<'a, S: Storage> File<'a, S> {
    pub(crate) async fn open(
        fs: &'a Filesystem<S>,
        path: &str,
        flags: OpenFlags,
    ) -> Result<Self, Error> {
        let mut alloc = Box::new(FileAllocation::new(fs.cache_size()));
        let path_bytes = null_terminate(path);
        {
            let inner = &mut *fs.inner.borrow_mut();
            let rc = littlefs_rust_core::lfs_file_opencfg(
                &mut inner.lfs,
                &mut inner.caches,
                alloc.file.as_mut_ptr(),
                path_bytes.as_ptr(),
                flags.bits() as i32,
                &alloc.file_config as *const LfsFileConfig,
            )
            .await;
            from_lfs_result(rc)?;
        }
        Ok(File {
            fs,
            alloc,
            closed: false,
        })
    }

    /// Read up to `buf.len()` bytes from the current position.
    /// Returns the number of bytes actually read.
    pub async fn read(&self, buf: &mut [u8]) -> Result<u32, Error> {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_read(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
            buf,
        )
        .await;
        from_lfs_size(rc)
    }

    /// Write `data` at the current position. Returns the number of bytes written.
    pub async fn write(&self, data: &[u8]) -> Result<u32, Error> {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_write(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
            data,
        )
        .await;
        from_lfs_size(rc)
    }

    /// Seek to a position. Returns the new absolute offset.
    pub async fn seek(&self, pos: SeekFrom) -> Result<u32, Error> {
        let (off, whence) = match pos {
            SeekFrom::Start(n) => (
                n as i32,
                littlefs_rust_core::lfs_type::lfs_whence_flags::LFS_SEEK_SET,
            ),
            SeekFrom::Current(n) => (
                n,
                littlefs_rust_core::lfs_type::lfs_whence_flags::LFS_SEEK_CUR,
            ),
            SeekFrom::End(n) => (
                n,
                littlefs_rust_core::lfs_type::lfs_whence_flags::LFS_SEEK_END,
            ),
        };
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_seek(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
            off,
            whence,
        )
        .await;
        from_lfs_size(rc)
    }

    /// Return the current read/write position.
    pub fn tell(&self) -> u32 {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_tell(
            &mut inner.lfs,
            self.alloc.file.as_ptr() as *mut LfsFile,
        );
        rc as u32
    }

    /// Return the file size in bytes.
    pub fn size(&self) -> u32 {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_size(
            &mut inner.lfs,
            self.alloc.file.as_ptr() as *mut LfsFile,
        );
        rc as u32
    }

    /// Flush cached writes to storage.
    pub async fn sync(&self) -> Result<(), Error> {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_sync(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
        )
        .await;
        from_lfs_result(rc)
    }

    /// Truncate or extend the file to `size` bytes.
    pub async fn truncate(&self, size: u32) -> Result<(), Error> {
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_truncate(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
            size,
        )
        .await;
        from_lfs_result(rc)
    }

    /// Close the file, flushing any pending writes. Consumes `self`.
    ///
    /// Dropping a [`File`] also closes it, but errors are silently ignored.
    pub async fn close(mut self) -> Result<(), Error> {
        self.closed = true;
        let inner = &mut *self.fs.inner.borrow_mut();
        let rc = littlefs_rust_core::lfs_file_close(
            &mut inner.lfs,
            &mut inner.caches,
            self.alloc.file.as_ptr() as *mut LfsFile,
        )
        .await;
        from_lfs_result(rc)
    }
}

impl<S: Storage> Drop for File<'_, S> {
    fn drop(&mut self) {
        if !self.closed {
            if let Ok(inner) = self.fs.inner.try_borrow_mut().as_deref_mut() {
                let _ = block_on(littlefs_rust_core::lfs_file_close(
                    &mut inner.lfs,
                    &mut inner.caches,
                    self.alloc.file.as_ptr() as *mut LfsFile,
                ));
            }
        }
    }
}

fn block_on<F: Future>(mut future: F) -> F::Output {
    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { core::pin::Pin::new_unchecked(&mut future) };
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => core::hint::spin_loop(),
        }
    }
}

unsafe fn noop_raw_waker() -> RawWaker {
    unsafe fn clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}

    RawWaker::new(
        core::ptr::null(),
        &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
    )
}

fn null_terminate(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.bytes().collect();
    v.push(0);
    v
}
