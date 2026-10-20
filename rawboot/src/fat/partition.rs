// SPDX-License-Identifier: Unlicense

//! FAT partition discovery.
//!
//! The card may carry a bare FAT volume (the whole card is the filesystem) or an
//! MBR with a FAT partition. `fatfs` mounts from the start of whatever device it
//! is given, so this module only locates candidate volumes; the block adapter
//! adds the partition offset.

/// The SD block size, and the unit of MBR partition addresses.
pub(crate) const BLOCK_SIZE: u64 = 512;

const BLOCK_SIZE_USIZE: usize = BLOCK_SIZE as usize;
const SIGNATURE_OFFSET: usize = BLOCK_SIZE_USIZE - 2;
const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xaa];
const MBR_PARTITION_TABLE: usize = 446;
const MBR_PARTITION_ENTRIES: usize = 4;
pub(crate) const MAX_CANDIDATES: usize = 1 + MBR_PARTITION_ENTRIES;

/// A candidate FAT volume.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Partition {
    /// First SD block of the volume.
    pub base_lba: u64,
    /// Volume length in bytes.
    pub len_bytes: u64,
}

/// Lists candidate FAT volumes described by the card's first block.
///
/// A bare FAT volume comes first, followed by MBR partitions with FAT type
/// bytes, FAT32 before FAT12/16. Candidates are not validated beyond the
/// partition table: the caller must mount each one and keep the first that
/// `fatfs` accepts.
pub(crate) fn candidates(
    sector0: &[u8; BLOCK_SIZE_USIZE],
    card_bytes: u64,
) -> [Option<Partition>; MAX_CANDIDATES] {
    let mut candidates = [None; MAX_CANDIDATES];
    let mut count = 0;

    if let Some(len_bytes) = fat_boot_sector_len_bytes(sector0) {
        candidates[count] = Some(Partition {
            base_lba: 0,
            len_bytes: len_bytes.min(card_bytes),
        });
        count += 1;
    }

    if sector0[SIGNATURE_OFFSET..] != BOOT_SIGNATURE {
        return candidates;
    }

    for priority in [2, 1] {
        for index in 0..MBR_PARTITION_ENTRIES {
            let entry = MBR_PARTITION_TABLE + index * 16;
            if fat_type_priority(sector0[entry + 4]) != Some(priority) {
                continue;
            }

            let base_lba = le32(sector0, entry + 8) as u64;
            let sectors = le32(sector0, entry + 12) as u64;
            let base_byte = base_lba * BLOCK_SIZE;
            if base_lba == 0 || sectors == 0 || base_byte >= card_bytes {
                continue;
            }

            candidates[count] = Some(Partition {
                base_lba,
                len_bytes: (sectors * BLOCK_SIZE).min(card_bytes - base_byte),
            });
            count += 1;
        }
    }

    candidates
}

/// Returns the volume length when `sector` looks like a FAT boot sector.
fn fat_boot_sector_len_bytes(sector: &[u8; BLOCK_SIZE_USIZE]) -> Option<u64> {
    if sector[SIGNATURE_OFFSET..] != BOOT_SIGNATURE {
        return None;
    }
    // A FAT boot sector starts with a short jump over the BPB; an MBR does not.
    if sector[0] != 0xeb && sector[0] != 0xe9 {
        return None;
    }

    let bytes_per_sector = le16(sector, 11) as u64;
    if !(512..=4096).contains(&bytes_per_sector) || !bytes_per_sector.is_power_of_two() {
        return None;
    }

    let sectors_per_cluster = sector[13] as u64;
    if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
        return None;
    }

    let reserved_sectors = le16(sector, 14);
    let fat_count = sector[16];
    if reserved_sectors == 0 || fat_count == 0 {
        return None;
    }

    let sectors16 = le16(sector, 19) as u64;
    let sectors = if sectors16 != 0 {
        sectors16
    } else {
        le32(sector, 32) as u64
    };
    (sectors != 0).then_some(sectors * bytes_per_sector)
}

/// Rank of an MBR partition type in the FAT family; larger is preferred.
fn fat_type_priority(partition_type: u8) -> Option<u8> {
    match partition_type {
        0x0b | 0x0c | 0x1b | 0x1c => Some(2),
        0x01 | 0x04 | 0x06 | 0x0e | 0x11 | 0x14 | 0x16 => Some(1),
        _ => None,
    }
}

fn le16(buf: &[u8; BLOCK_SIZE_USIZE], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn le32(buf: &[u8; BLOCK_SIZE_USIZE], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}
