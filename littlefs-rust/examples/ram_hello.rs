//! Write and read a file on a RAM-backed littlefs filesystem.

use littlefs_rust::{Config, Filesystem, RamStorage};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // RamStorage is an in-memory block device — useful for tests and examples.
    // 128 blocks of 512 bytes each = 64 KB.
    let block_size = 512;
    let block_count = 128;
    let mut storage = RamStorage::new(block_size, block_count);
    let config = Config::new(block_size, block_count);

    // Format lays down the superblock; mount opens the filesystem for use.
    Filesystem::format(&mut storage, &config)
        .await
        .expect("format failed");
    let fs = Filesystem::mount(storage, config)
        .await
        .map_err(|(e, _)| e)
        .expect("mount failed");

    // write_file / read_to_vec are convenience wrappers that handle
    // open, write/read, and close in one call.
    fs.write_file("/hello.txt", b"Hello, littlefs!")
        .await
        .expect("write failed");

    let data = fs.read_to_vec("/hello.txt").await.expect("read failed");
    println!("{}", core::str::from_utf8(&data).unwrap());

    // Unmount returns ownership of the storage back to the caller.
    let storage = fs.unmount().expect("unmount failed");
    println!(
        "Storage: {} blocks x {} bytes = {} bytes total",
        storage.block_count(),
        storage.block_size(),
        storage.data().len()
    );
}
