// SPDX-License-Identifier: Unlicense

//! Read-only block-device adapter for `fatfs`.
//!
//! `fatfs` seeks and reads arbitrary byte ranges; the SD card hands out whole
//! blocks. This adapter turns the latter into the former and offsets every
//! access by the FAT partition's start block.

use core::cmp::min;

use core_io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};

use crate::sdcard::{Sdcard_data, sdmmc_blk_read};

use super::partition::{BLOCK_SIZE, Partition};

const BLOCK_SIZE_USIZE: usize = BLOCK_SIZE as usize;

pub(crate) struct BlockDevice<'a> {
    sd: &'a mut Sdcard_data,
    partition: Partition,
    offset: u64,
    cached_lba: Option<u64>,
    cache: [u8; BLOCK_SIZE_USIZE],
}

impl<'a> BlockDevice<'a> {
    pub(crate) fn new(sd: &'a mut Sdcard_data, partition: Partition) -> Self {
        Self {
            sd,
            partition,
            offset: 0,
            cached_lba: None,
            cache: [0; BLOCK_SIZE_USIZE],
        }
    }

    fn len_bytes(&self) -> u64 {
        self.partition.len_bytes
    }

    fn block_lba(&self, offset: u64) -> u64 {
        self.partition.base_lba + offset / BLOCK_SIZE
    }

    fn read_block(&mut self, lba: u64) -> Result<&[u8; BLOCK_SIZE_USIZE]> {
        if self.cached_lba != Some(lba) {
            let read = sdmmc_blk_read(self.sd, self.cache.as_mut_ptr(), lba, 1);
            if read != 1 {
                return Err(read_error());
            }
            self.cached_lba = Some(lba);
        }
        Ok(&self.cache)
    }
}

fn read_error() -> Error {
    Error::new(ErrorKind::Other, "SD block read failed")
}

impl Read for BlockDevice<'_> {
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
                    let lba = self.block_lba(self.offset);
                    let read =
                        sdmmc_blk_read(self.sd, buf[done..].as_mut_ptr(), lba, blocks as u64);
                    if read != blocks as u64 {
                        return Err(read_error());
                    }
                    let bytes = blocks * BLOCK_SIZE_USIZE;
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

impl Write for BlockDevice<'_> {
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        Err(Error::new(ErrorKind::Unsupported, "SD card is read-only"))
    }
}

impl Seek for BlockDevice<'_> {
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
