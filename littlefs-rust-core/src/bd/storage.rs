use crate::error::Error;
use crate::types::lfs_block_t;

/// Block device storage backend.
///
/// Implement this trait to connect a flash chip, SD card, or any other block
/// device. See [`RamStorage`](crate::RamStorage) for a minimal example.
pub trait Storage {
    /// Read `buf.len()` bytes starting at `offset` within `block`.
    fn read(&mut self, block: lfs_block_t, offset: u32, buf: &mut [u8]) -> Result<(), Error>;

    /// Write `data` starting at `offset` within `block`.
    ///
    /// The block must have been erased before writing.
    fn write(&mut self, block: lfs_block_t, offset: u32, data: &[u8]) -> Result<(), Error>;

    /// Erase `block`, resetting all bytes to the erased state (typically `0xFF`).
    fn erase(&mut self, block: lfs_block_t) -> Result<(), Error>;

    /// Flush pending writes. The default implementation is a no-op.
    fn sync(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

impl<T: Storage> Storage for &mut T {
    fn read(&mut self, block: lfs_block_t, offset: u32, buf: &mut [u8]) -> Result<(), Error> {
        (*self).read(block, offset, buf)
    }

    fn write(&mut self, block: lfs_block_t, offset: u32, data: &[u8]) -> Result<(), Error> {
        (*self).write(block, offset, data)
    }

    fn erase(&mut self, block: lfs_block_t) -> Result<(), Error> {
        (*self).erase(block)
    }

    fn sync(&mut self) -> Result<(), Error> {
        (*self).sync()
    }
}
