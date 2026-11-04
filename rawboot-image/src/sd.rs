// SPDX-License-Identifier: CC-BY-4.0

//! The SD boot prefix: an MBR that points at an existing FAT partition, with the
//! sealed SD/SPI loader placed at sector 16 where BROM looks for it.
//!
//! The prefix carries no FAT volume and no payload. It only points BROM at a loader and the loader
//! at a partition that some other tool has formatted: writing the prefix over a card keeps the
//! partition and its contents intact (ADR-0011).

use alloc::vec;
use alloc::vec::Vec;

use crate::bytes::write_u32_le;
use crate::egon::{VerifyError, verify_sealed};

/// The 512-byte logical sector BROM and MBR use.
pub const SECTOR_SIZE: usize = 512;

/// BROM reads the SD/SPI boot image from sector 16.
pub const SD_LOADER_OFFSET: usize = 8192;

/// The default start of the FAT partition, in sectors (1 MiB).
pub const DEFAULT_START_LBA: u32 = 2048;

/// MBR offset of the first partition-table entry.
pub const PARTITION_TABLE_OFFSET: usize = 446;

/// One MBR partition-table entry is 16 bytes.
pub const PARTITION_ENTRY_SIZE: usize = 16;

/// MBR offset of the disk signature.
pub const DISK_SIGNATURE_OFFSET: usize = 440;

/// MBR offset of the boot signature.
pub const MBR_SIGNATURE_OFFSET: usize = 510;

/// The `55 AA` boot signature at the end of the MBR.
pub const MBR_SIGNATURE: [u8; 2] = [0x55, 0xAA];

/// The partition type for a FAT32 partition (LBA addressing).
pub const PARTITION_TYPE_FAT32: u8 = 0x0C;

/// `CHS start/end` used for a partition that does not need a usable CHS address.
const CHS_MAX: [u8; 3] = [0xFE, 0xFF, 0xFF];

/// Offsets inside an MBR partition-table entry.
mod entry {
    pub const BOOT_FLAG: usize = 0;
    pub const START_CHS: usize = 1;
    pub const PARTITION_TYPE: usize = 4;
    pub const END_CHS: usize = 5;
    pub const START_LBA: usize = 8;
    pub const SECTOR_COUNT: usize = 12;
}

/// How [`build_sd_image`] lays out the prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdImageOptions {
    /// First sector of the FAT partition the MBR points at.
    pub start_lba: u32,
    /// Sectors in that partition.
    pub partition_sectors: u32,
}

impl SdImageOptions {
    /// Options for a partition starting at `start_lba` and spanning `partition_sectors`.
    pub const fn new(start_lba: u32, partition_sectors: u32) -> Self {
        Self {
            start_lba,
            partition_sectors,
        }
    }
}

/// Why an SD boot prefix could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdImageError {
    /// The loader is empty, so there is nothing to place at sector 16.
    LoaderEmpty,
    /// The loader is not a sealed eGON image (see [`crate::egon::verify_sealed`]).
    LoaderNotSealed(VerifyError),
    /// `SD_LOADER_OFFSET + loader.len()` reaches the partition start, so the loader and the
    /// partition would overlap.
    LoaderOverlapsPartition,
    /// The partition start is zero, which would put the loader inside the MBR.
    ZeroStartLba,
    /// The partition sector count is zero.
    ZeroPartitionSectors,
    /// The prefix length or the partition's last sector does not fit the sector address width.
    SizeOverflow,
}

impl core::fmt::Display for SdImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SdImageError::LoaderEmpty => f.write_str("loader image is empty"),
            SdImageError::LoaderNotSealed(err) => {
                write!(f, "loader is not a sealed eGON image: {err}")
            }
            SdImageError::LoaderOverlapsPartition => {
                f.write_str("loader at sector 16 reaches into the partition")
            }
            SdImageError::ZeroStartLba => f.write_str("partition start LBA must be non-zero"),
            SdImageError::ZeroPartitionSectors => {
                f.write_str("partition sector count must be non-zero")
            }
            SdImageError::SizeOverflow => {
                f.write_str("image size or partition end overflows the sector address width")
            }
        }
    }
}

impl core::error::Error for SdImageError {}

impl From<VerifyError> for SdImageError {
    fn from(err: VerifyError) -> Self {
        Self::LoaderNotSealed(err)
    }
}

/// Build the SD boot prefix: a zero-filled image exactly `start_lba` sectors long, with an MBR in
/// sector 0 and `loader` copied verbatim at [`SD_LOADER_OFFSET`] (sector 16).
///
/// The loader must be a sealed eGON image and must fit entirely before the partition, so writing
/// this prefix over a medium leaves the partition that starts at `start_lba` intact.
pub fn build_sd_image(loader: &[u8], options: SdImageOptions) -> Result<Vec<u8>, SdImageError> {
    if options.start_lba == 0 {
        return Err(SdImageError::ZeroStartLba);
    }
    if options.partition_sectors == 0 {
        return Err(SdImageError::ZeroPartitionSectors);
    }

    let image_len = options
        .start_lba
        .checked_mul(SECTOR_SIZE as u32)
        .ok_or(SdImageError::SizeOverflow)? as usize;
    options
        .start_lba
        .checked_add(options.partition_sectors - 1)
        .ok_or(SdImageError::SizeOverflow)?;
    if loader.is_empty() {
        return Err(SdImageError::LoaderEmpty);
    }
    let loader_end = SD_LOADER_OFFSET
        .checked_add(loader.len())
        .ok_or(SdImageError::SizeOverflow)?;

    verify_sealed(loader)?;
    // Ending exactly at the partition start is fine: the loader fills the prefix up to the
    // partition without writing a byte into it.
    if loader_end > image_len {
        return Err(SdImageError::LoaderOverlapsPartition);
    }

    let mut image = vec![0u8; image_len];
    write_mbr(&mut image, options);
    image[SD_LOADER_OFFSET..loader_end].copy_from_slice(loader);
    Ok(image)
}

fn write_mbr(image: &mut [u8], options: SdImageOptions) {
    let mbr = &mut image[PARTITION_TABLE_OFFSET..PARTITION_TABLE_OFFSET + PARTITION_ENTRY_SIZE];
    mbr[entry::BOOT_FLAG] = 0x00;
    mbr[entry::START_CHS..entry::START_CHS + CHS_MAX.len()].copy_from_slice(&CHS_MAX);
    mbr[entry::PARTITION_TYPE] = PARTITION_TYPE_FAT32;
    mbr[entry::END_CHS..entry::END_CHS + CHS_MAX.len()].copy_from_slice(&CHS_MAX);
    write_u32_le(mbr, entry::START_LBA, options.start_lba);
    write_u32_le(mbr, entry::SECTOR_COUNT, options.partition_sectors);

    image[MBR_SIGNATURE_OFFSET..MBR_SIGNATURE_OFFSET + 2].copy_from_slice(&MBR_SIGNATURE);
}

const _: () = {
    assert!(PARTITION_TABLE_OFFSET + PARTITION_ENTRY_SIZE <= MBR_SIGNATURE_OFFSET);
    assert!(SD_LOADER_OFFSET / SECTOR_SIZE == 16);
};
