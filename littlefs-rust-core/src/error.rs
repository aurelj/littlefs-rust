//! Error codes. Per lfs.h enum lfs_error.
//! Negative values allow positive return values (e.g. bytes read).

use core::fmt;

pub const LFS_ERR_OK: i32 = 0;
pub const LFS_ERR_IO: i32 = -5;
pub const LFS_ERR_CORRUPT: i32 = -84;
pub const LFS_ERR_NOENT: i32 = -2;
pub const LFS_ERR_EXIST: i32 = -17;
pub const LFS_ERR_NOTDIR: i32 = -20;
pub const LFS_ERR_ISDIR: i32 = -21;
pub const LFS_ERR_NOTEMPTY: i32 = -39;
pub const LFS_ERR_BADF: i32 = -9;
pub const LFS_ERR_FBIG: i32 = -27;
pub const LFS_ERR_INVAL: i32 = -22;
pub const LFS_ERR_NOSPC: i32 = -28;
pub const LFS_ERR_NOMEM: i32 = -12;
pub const LFS_ERR_NOATTR: i32 = -61;
pub const LFS_ERR_NAMETOOLONG: i32 = -36;

/// Positive return values for commit/orphan machinery. Per lfs.h enum lfs_error.
pub const LFS_OK_RELOCATED: i32 = 1;
pub const LFS_OK_DROPPED: i32 = 2;
pub const LFS_OK_ORPHANED: i32 = 3;

/// LittleFS operation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Io,
    Corrupt,
    NoEntry,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    Invalid,
    NoSpace,
    NoMemory,
    NoAttribute,
    NameTooLong,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io => write!(f, "I/O error"),
            Error::Corrupt => write!(f, "filesystem corrupt"),
            Error::NoEntry => write!(f, "no such file or directory"),
            Error::Exists => write!(f, "file or directory already exists"),
            Error::NotDir => write!(f, "not a directory"),
            Error::IsDir => write!(f, "is a directory"),
            Error::NotEmpty => write!(f, "directory not empty"),
            Error::Invalid => write!(f, "invalid parameter"),
            Error::NoSpace => write!(f, "no space left on device"),
            Error::NoMemory => write!(f, "out of memory"),
            Error::NoAttribute => write!(f, "no such attribute"),
            Error::NameTooLong => write!(f, "name too long"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<i32> for Error {
    fn from(code: i32) -> Self {
        match code {
            c if c == LFS_ERR_IO => Error::Io,
            c if c == LFS_ERR_CORRUPT => Error::Corrupt,
            c if c == LFS_ERR_NOENT => Error::NoEntry,
            c if c == LFS_ERR_EXIST => Error::Exists,
            c if c == LFS_ERR_NOTDIR => Error::NotDir,
            c if c == LFS_ERR_ISDIR => Error::IsDir,
            c if c == LFS_ERR_NOTEMPTY => Error::NotEmpty,
            c if c == LFS_ERR_INVAL => Error::Invalid,
            c if c == LFS_ERR_NOSPC => Error::NoSpace,
            c if c == LFS_ERR_NOMEM => Error::NoMemory,
            c if c == LFS_ERR_NOATTR => Error::NoAttribute,
            c if c == LFS_ERR_NAMETOOLONG => Error::NameTooLong,
            _ => panic!("unknown LFS error code: {}", code),
        }
    }
}

pub fn from_lfs_result(code: i32) -> Result<(), Error> {
    if code == 0 {
        Ok(())
    } else {
        Err(Error::from(code))
    }
}

pub fn from_lfs_size(code: i32) -> Result<u32, Error> {
    if code >= 0 {
        Ok(code as u32)
    } else {
        Err(Error::from(code))
    }
}

impl From<Error> for i32 {
    fn from(e: Error) -> Self {
        match e {
            Error::Io => LFS_ERR_IO,
            Error::Corrupt => LFS_ERR_CORRUPT,
            Error::NoEntry => LFS_ERR_NOENT,
            Error::Exists => LFS_ERR_EXIST,
            Error::NotDir => LFS_ERR_NOTDIR,
            Error::IsDir => LFS_ERR_ISDIR,
            Error::NotEmpty => LFS_ERR_NOTEMPTY,
            Error::Invalid => LFS_ERR_INVAL,
            Error::NoSpace => LFS_ERR_NOSPC,
            Error::NoMemory => LFS_ERR_NOMEM,
            Error::NoAttribute => LFS_ERR_NOATTR,
            Error::NameTooLong => LFS_ERR_NAMETOOLONG,
        }
    }
}

pub fn from_empty_result(res: Result<(), Error>) -> i32 {
    match res {
        Ok(()) => 0,
        Err(e) => i32::from(e),
    }
}

pub fn from_size_result(res: Result<u32, Error>) -> i32 {
    match res {
        Ok(size) => size as i32,
        Err(e) => i32::from(e),
    }
}
