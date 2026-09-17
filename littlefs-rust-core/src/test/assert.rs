//! Assertion helpers for superblock magic and block content.

use crate::test::ram::{MAGIC, MAGIC_OFFSET};
use crate::{Lfs, Storage};

/// Read config's block at offset 0, return magic region (8 bytes at MAGIC_OFFSET).
fn read_magic_region<S: Storage>(lfs: &mut Lfs<S>, block: u32) -> Option<[u8; 8]> {
    let mut buf = [0u8; 24];
    lfs.storage
        .read(block, 0, &mut buf)
        .map(|_| buf[MAGIC_OFFSET as usize..][..8].try_into().unwrap())
        .ok()
}

/// Panics if block does not contain MAGIC at MAGIC_OFFSET.
pub fn assert_block_has_magic<S: Storage>(lfs: &mut Lfs<S>, block: u32) {
    let got = read_magic_region(lfs, block)
        .unwrap_or_else(|| panic!("read_block_raw failed for block {}", block));
    assert_eq!(
        &got, MAGIC,
        "block {}: expected MAGIC at offset {}, got {:?}",
        block, MAGIC_OFFSET, &got
    );
}

/// Panics if blocks 0 or 1 do not contain MAGIC.
pub fn assert_blocks_0_and_1_have_magic<S: Storage>(lfs: &mut Lfs<S>) {
    assert_block_has_magic(lfs, 0);
    assert_block_has_magic(lfs, 1);
}
