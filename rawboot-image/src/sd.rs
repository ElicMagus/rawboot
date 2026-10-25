// SPDX-License-Identifier: Unlicense

//! The SD boot prefix: an MBR that points at an existing FAT partition, with the
//! sealed SD/SPI loader placed at sector 16 where BROM looks for it.
//!
//! The prefix carries no FAT volume and no payload. It only points BROM at a loader and the loader
//! at a partition that some other tool has formatted: writing the prefix over a card keeps the
//! partition and its contents intact (ADR-0011).

use alloc::vec;
use alloc::vec::Vec;

use crate::bytes::write_u32_le;
use crate::egon::{HeaderError, verify_sealed};

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
    /// The loader is empty or is not a sealed eGON image (see [`crate::egon::verify_sealed`]).
    LoaderNotSealed,
    /// `SD_LOADER_OFFSET + loader.len()` reaches the partition start, so the loader and the
    /// partition would overlap.
    LoaderOverlapsPartition,
    /// The partition start is zero, which would put the loader inside the MBR.
    ZeroStartLba,
    /// The partition sector count is zero.
    ZeroPartitionSectors,
    /// The prefix length does not fit the sector address width.
    SizeOverflow,
}

impl core::fmt::Display for SdImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            SdImageError::LoaderNotSealed => {
                "loader is not a sealed eGON image (build one with `cargo xtask image`)"
            }
            SdImageError::LoaderOverlapsPartition => {
                "loader at sector 16 reaches into the partition, raise `--start-lba`"
            }
            SdImageError::ZeroStartLba => "`--start-lba` must be non-zero",
            SdImageError::ZeroPartitionSectors => "`--partition-sectors` must be non-zero",
            SdImageError::SizeOverflow => "image size overflows the sector address width",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for SdImageError {}

impl From<HeaderError> for SdImageError {
    fn from(_: HeaderError) -> Self {
        Self::LoaderNotSealed
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
    let loader_end = SD_LOADER_OFFSET
        .checked_add(loader.len())
        .ok_or(SdImageError::SizeOverflow)?;

    verify_sealed(loader)?;
    if loader_end > image_len {
        return Err(SdImageError::LoaderOverlapsPartition);
    }

    let mut image = vec![0u8; image_len];
    write_mbr(&mut image, options);
    image[SD_LOADER_OFFSET..loader_end].copy_from_slice(loader);
    Ok(image)
}

fn write_mbr(image: &mut [u8], options: SdImageOptions) {
    let entry = &mut image[PARTITION_TABLE_OFFSET..PARTITION_TABLE_OFFSET + PARTITION_ENTRY_SIZE];
    entry[0] = 0x00;
    entry[1..4].copy_from_slice(&CHS_MAX);
    entry[4] = PARTITION_TYPE_FAT32;
    entry[5..8].copy_from_slice(&CHS_MAX);
    write_u32_le(entry, 8, options.start_lba);
    write_u32_le(entry, 12, options.partition_sectors);

    image[DISK_SIGNATURE_OFFSET..DISK_SIGNATURE_OFFSET + 4].fill(0);
    image[MBR_SIGNATURE_OFFSET..MBR_SIGNATURE_OFFSET + 2].copy_from_slice(&MBR_SIGNATURE);
}

const _: () = {
    assert!(PARTITION_TABLE_OFFSET + PARTITION_ENTRY_SIZE <= MBR_SIGNATURE_OFFSET);
    assert!(SD_LOADER_OFFSET / SECTOR_SIZE == 16);
};
