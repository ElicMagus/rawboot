// SPDX-License-Identifier: Unlicense

//! Block-device adapter for `fatfs`.
//!
//! `fatfs` seeks and reads or writes arbitrary byte ranges; the SD card hands
//! out whole blocks. This adapter turns the latter into the former, offsets
//! every access by the FAT partition's start block, and keeps one dirty block
//! cached until a sequential write completes or the caller flushes.
//!
//! [`BlockIo`] is the seam that keeps the adapter testable on the host: the SD
//! card provides an implementation backed by SDHCI, and a test supplies an
//! in-memory one.

use core::cmp::min;

use core_io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};

#[cfg(feature = "write")]
use crate::sdcard::{SdError, sdmmc_blk_write};
use crate::sdcard::{Sdcard_data, sdmmc_blk_read};

use super::partition::{BLOCK_SIZE, Partition};

const BLOCK_SIZE_USIZE: usize = BLOCK_SIZE as usize;

/// A device made of [`BLOCK_SIZE`]-byte blocks.
pub trait BlockIo {
    /// Reads whole blocks into `buf`, which must be a multiple of the block
    /// size.
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> core::result::Result<(), BlockError>;

    /// Writes whole blocks from `buf`, which must be a multiple of the block
    /// size.
    #[cfg(feature = "write")]
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> core::result::Result<(), BlockError>;
}

/// Why a block access failed, with the range that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// The card did not initialise.
    Offline,
    /// A block read failed.
    Read { lba: u64, blocks: u64 },
    /// A block write failed.
    Write { lba: u64, blocks: u64 },
}

impl BlockError {
    fn to_io(self) -> Error {
        match self {
            Self::Offline => Error::new(ErrorKind::Other, "SD/MMC card is not online"),
            Self::Read { .. } => Error::new(ErrorKind::Other, "SD block read failed"),
            Self::Write { .. } => Error::new(ErrorKind::Other, "SD block write failed"),
        }
    }
}

/// [`BlockIo`] over the SD/MMC driver.
pub struct SdCardIo<'a> {
    sd: &'a mut Sdcard_data,
}

impl<'a> SdCardIo<'a> {
    pub fn new(sd: &'a mut Sdcard_data) -> Self {
        Self { sd }
    }
}

impl BlockIo for SdCardIo<'_> {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> core::result::Result<(), BlockError> {
        let blocks = (buf.len() / BLOCK_SIZE_USIZE) as u64;
        if blocks == 0 {
            return Ok(());
        }
        if !self.sd.online {
            return Err(BlockError::Offline);
        }
        if sdmmc_blk_read(self.sd, buf.as_mut_ptr(), lba, blocks) != blocks {
            return Err(BlockError::Read { lba, blocks });
        }
        Ok(())
    }

    #[cfg(feature = "write")]
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> core::result::Result<(), BlockError> {
        let blocks = (buf.len() / BLOCK_SIZE_USIZE) as u64;
        if blocks == 0 {
            return Ok(());
        }
        sdmmc_blk_write(self.sd, buf.as_ptr(), lba, blocks).map_err(|err| match err {
            SdError::Offline => BlockError::Offline,
            SdError::Write { blkno, blkcnt } => BlockError::Write {
                lba: blkno,
                blocks: blkcnt,
            },
        })
    }
}

/// A byte-addressable view of a [`BlockIo`] partition with a write-back block
/// cache.
///
/// Reads never bypass an unflushed cache, and switching blocks flushes the
/// previous one first. [`Write::flush`] is the only place a dirty block is
/// written, so callers must flush before unmounting or handing the card to
/// another driver.
pub struct BlockDevice<B: BlockIo> {
    io: B,
    partition: Partition,
    offset: u64,
    cached_lba: Option<u64>,
    cache: [u8; BLOCK_SIZE_USIZE],
    dirty: bool,
    flush_on_drop: bool,
}

impl<B: BlockIo> BlockDevice<B> {
    pub fn new(io: B, partition: Partition) -> Self {
        Self {
            io,
            partition,
            offset: 0,
            cached_lba: None,
            cache: [0; BLOCK_SIZE_USIZE],
            dirty: false,
            flush_on_drop: false,
        }
    }

    /// A device that flushes its dirty cache when it is dropped.
    ///
    /// `fatfs` writes its FSInfo and dirty flag after `unmount`, but does not
    /// flush the underlying device; the flasher relies on this to persist those
    /// last writes after it has read the files back.
    pub fn new_flushing(io: B, partition: Partition) -> Self {
        let mut device = Self::new(io, partition);
        device.flush_on_drop = true;
        device
    }

    fn len_bytes(&self) -> u64 {
        self.partition.len_bytes
    }

    fn block_lba(&self, offset: u64) -> u64 {
        self.partition.base_lba + offset / BLOCK_SIZE
    }

    #[cfg(feature = "write")]
    fn flush_cache(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let lba = self.cached_lba.expect("a dirty cache always has a block");
        self.io
            .write_blocks(lba, &self.cache)
            .map_err(BlockError::to_io)?;
        self.dirty = false;
        Ok(())
    }

    /// Without the `write` feature the device is read-only, so the cache can
    /// never be dirty and there is nothing to flush.
    #[cfg(not(feature = "write"))]
    fn flush_cache(&mut self) -> Result<()> {
        debug_assert!(!self.dirty, "a read-only device cannot cache a write");
        Ok(())
    }

    fn ensure_block(&mut self, lba: u64) -> Result<()> {
        if self.cached_lba != Some(lba) {
            self.flush_cache()?;
            self.io
                .read_blocks(lba, &mut self.cache)
                .map_err(BlockError::to_io)?;
            self.cached_lba = Some(lba);
            self.dirty = false;
        }
        Ok(())
    }

    fn read_block(&mut self, lba: u64) -> Result<&[u8; BLOCK_SIZE_USIZE]> {
        self.ensure_block(lba)?;
        Ok(&self.cache)
    }
}

impl<B: BlockIo> Drop for BlockDevice<B> {
    fn drop(&mut self) {
        if self.flush_on_drop {
            // The files were read back before unmount; a failure to persist the
            // FSInfo flag is not worth hiding the drop, and there is no caller
            // left to report it to.
            let _ = self.flush_cache();
        }
    }
}

impl<B: BlockIo> Read for BlockDevice<B> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = min(
            buf.len() as u64,
            self.len_bytes().saturating_sub(self.offset),
        ) as usize;
        let mut done = 0;

        while done < len {
            let offset_in_block = (self.offset % BLOCK_SIZE) as usize;
            let left = len - done;

            if offset_in_block == 0 {
                let blocks = left / BLOCK_SIZE_USIZE;
                if blocks > 0 {
                    self.flush_cache()?;
                    let lba = self.block_lba(self.offset);
                    let bytes = blocks * BLOCK_SIZE_USIZE;
                    self.io
                        .read_blocks(lba, &mut buf[done..done + bytes])
                        .map_err(BlockError::to_io)?;
                    self.cached_lba = None;
                    done += bytes;
                    self.offset += bytes as u64;
                    continue;
                }
            }

            let lba = self.block_lba(self.offset);
            let count = min(BLOCK_SIZE_USIZE - offset_in_block, left);
            let block = self.read_block(lba)?;
            buf[done..done + count]
                .copy_from_slice(&block[offset_in_block..offset_in_block + count]);
            done += count;
            self.offset += count as u64;
        }

        Ok(len)
    }
}

#[cfg(feature = "write")]
impl<B: BlockIo> Write for BlockDevice<B> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = min(
            buf.len() as u64,
            self.len_bytes().saturating_sub(self.offset),
        ) as usize;
        let mut done = 0;

        while done < len {
            let offset_in_block = (self.offset % BLOCK_SIZE) as usize;
            let left = len - done;

            if offset_in_block == 0 {
                let blocks = left / BLOCK_SIZE_USIZE;
                if blocks > 0 {
                    self.flush_cache()?;
                    let lba = self.block_lba(self.offset);
                    let bytes = blocks * BLOCK_SIZE_USIZE;
                    self.io
                        .write_blocks(lba, &buf[done..done + bytes])
                        .map_err(BlockError::to_io)?;
                    self.cached_lba = None;
                    done += bytes;
                    self.offset += bytes as u64;
                    continue;
                }
            }

            let lba = self.block_lba(self.offset);
            let count = min(BLOCK_SIZE_USIZE - offset_in_block, left);
            self.ensure_block(lba)?;
            self.cache[offset_in_block..offset_in_block + count]
                .copy_from_slice(&buf[done..done + count]);
            self.dirty = true;
            done += count;
            self.offset += count as u64;
        }

        Ok(len)
    }

    fn flush(&mut self) -> Result<()> {
        self.flush_cache()
    }
}

#[cfg(not(feature = "write"))]
impl<B: BlockIo> Write for BlockDevice<B> {
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        Err(Error::new(ErrorKind::Unsupported, "SD card is read-only"))
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<B: BlockIo> Seek for BlockDevice<B> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_offset = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.offset as i64 + offset,
            SeekFrom::End(offset) => self.len_bytes() as i64 + offset,
        };
        if new_offset < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "cannot seek before the start of the partition",
            ));
        }
        self.offset = new_offset as u64;
        Ok(self.offset)
    }
}
