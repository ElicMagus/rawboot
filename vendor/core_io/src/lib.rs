// SPDX-License-Identifier: CC-BY-4.0

//! Minimal `std::io` subset for `no_std`.
//!
//! `fatfs` 0.3.6 reaches `std::io` through the `core_io` crate when built
//! without `std`. The published `core_io` 0.1 is generated from a historical
//! libstd snapshot, needs nightly features and fails on stable Rust. This crate
//! provides only the items `fatfs` uses, with the same trait shapes, and is
//! patched over `core_io` by the workspace root.

#![no_std]

use core::fmt;

/// A specialized [`Result`] for I/O operations.
pub type Result<T> = core::result::Result<T, Error>;

/// A broad category of I/O error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The requested entity was not found.
    NotFound,
    /// A read ran past the end of the input.
    UnexpectedEof,
    /// An argument was invalid for the operation.
    InvalidInput,
    /// The operation is not supported by this device.
    Unsupported,
    /// The entity already exists.
    AlreadyExists,
    /// The operation was interrupted and may be retried.
    Interrupted,
    /// Any other error.
    Other,
}

/// An I/O error.
#[derive(Clone, Copy, Debug)]
pub struct Error {
    kind: ErrorKind,
    message: &'static str,
}

impl Error {
    /// Creates an error of `kind` with a static diagnostic message.
    pub fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }

    /// Returns the error's category.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

/// A stream position for [`Seek::seek`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekFrom {
    /// Absolute byte offset from the start.
    Start(u64),
    /// Byte offset from the end.
    End(i64),
    /// Byte offset from the current position.
    Current(i64),
}

/// The `Read` behaviour used by `fatfs`.
pub trait Read {
    /// Pulls bytes into `buf`, returning how many were read; `Ok(0)` is EOF.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Fills `buf` completely or returns [`ErrorKind::UnexpectedEof`].
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "failed to fill whole buffer",
                    ));
                }
                Ok(n) => {
                    buf = &mut buf[n..];
                }
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

/// The `Write` behaviour used by `fatfs`.
pub trait Write {
    /// Writes bytes from `buf`, returning how many were accepted.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Flushes buffered data to the underlying device.
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// Writes `buf` completely.
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::new(ErrorKind::Other, "failed to write whole buffer"));
                }
                Ok(n) => {
                    buf = &buf[n..];
                }
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

/// The `Seek` behaviour used by `fatfs`.
pub trait Seek {
    /// Moves the stream position and returns the new absolute offset.
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    /// Returns the current absolute offset.
    fn stream_position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

/// An in-memory reader over a byte slice.
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    /// Wraps `inner` at position zero.
    pub fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let data = self.inner.as_ref();
        let start = (self.pos as usize).min(data.len());
        let len = buf.len().min(data.len() - start);
        buf[..len].copy_from_slice(&data[start..start + len]);
        self.pos += len as u64;
        Ok(len)
    }
}

impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let len = self.inner.as_ref().len() as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.pos as i64 + offset,
            SeekFrom::End(offset) => len + offset,
        };
        if new_pos < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "cannot seek before the start",
            ));
        }
        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}

impl<T: Read + ?Sized> Read for &mut T {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }
}

impl<T: Write + ?Sized> Write for &mut T {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }
}

impl<T: Seek + ?Sized> Seek for &mut T {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        (**self).seek(pos)
    }
}

/// The common I/O traits, mirroring `std::io::prelude`.
pub mod prelude {
    pub use super::{Read, Seek, SeekFrom, Write};
}
