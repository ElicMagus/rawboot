// SPDX-License-Identifier: Unlicense

//! FAT partition discovery.
//!
//! The card may carry a bare FAT volume (the whole card is the filesystem) or an
//! MBR with a FAT partition. `fatfs` mounts from the start of whatever device it
//! is given, so this module only locates candidate volumes; the block adapter
//! adds the partition offset.

/// The SD block size, and the unit of MBR partition addresses.
pub const BLOCK_SIZE: u64 = 512;

const BLOCK_SIZE_USIZE: usize = BLOCK_SIZE as usize;
/// Offset of the `55 AA` boot signature in the first sector.
pub const SIGNATURE_OFFSET: usize = BLOCK_SIZE_USIZE - 2;
/// The `55 AA` boot signature.
pub const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xaa];
const MBR_PARTITION_TABLE: usize = 446;
const MBR_PARTITION_ENTRIES: usize = 4;
pub const MAX_CANDIDATES: usize = 1 + MBR_PARTITION_ENTRIES;

/// A candidate FAT volume.
#[derive(Clone, Copy, Debug)]
pub struct Partition {
    /// First SD block of the volume.
    pub base_lba: u64,
    /// Volume length in bytes.
    pub len_bytes: u64,
}

/// One FAT partition-table entry, before any clamping to the card size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MbrFatPartition {
    /// First SD block of the partition.
    pub base_lba: u64,
    /// Sectors the MBR declares for the partition.
    pub sectors: u32,
    /// MBR partition type byte.
    pub partition_type: u8,
}

/// Returns the first FAT partition in the MBR in the same priority order as
/// [`candidates`]: FAT32 before FAT12/16, then table order.
///
/// The extent is not clamped to the card, so a caller that is about to write
/// can check the declared partition against the real capacity.
pub fn first_fat_partition(sector0: &[u8; BLOCK_SIZE_USIZE]) -> Option<MbrFatPartition> {
    if sector0[SIGNATURE_OFFSET..] != BOOT_SIGNATURE {
        return None;
    }

    for priority in [2, 1] {
        for index in 0..MBR_PARTITION_ENTRIES {
            let entry = MBR_PARTITION_TABLE + index * 16;
            let partition_type = sector0[entry + 4];
            if fat_type_priority(partition_type) != Some(priority) {
                continue;
            }

            let base_lba = le32(sector0, entry + 8) as u64;
            let sectors = le32(sector0, entry + 12);
            if base_lba == 0 || sectors == 0 {
                continue;
            }
            return Some(MbrFatPartition {
                base_lba,
                sectors,
                partition_type,
            });
        }
    }

    None
}

/// Lists candidate FAT volumes described by the card's first block.
///
/// A bare FAT volume comes first, followed by MBR partitions with FAT type
/// bytes, FAT32 before FAT12/16. Candidates are not validated beyond the
/// partition table: the caller must mount each one and keep the first that
/// `fatfs` accepts.
pub fn candidates(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mbr(entries: &[(u8, u32, u32)]) -> [u8; BLOCK_SIZE_USIZE] {
        let mut sector = [0u8; BLOCK_SIZE_USIZE];
        for (index, (partition_type, base_lba, sectors)) in entries.iter().enumerate() {
            let entry = MBR_PARTITION_TABLE + index * 16;
            sector[entry + 4] = *partition_type;
            sector[entry + 8..entry + 12].copy_from_slice(&base_lba.to_le_bytes());
            sector[entry + 12..entry + 16].copy_from_slice(&sectors.to_le_bytes());
        }
        sector[SIGNATURE_OFFSET..].copy_from_slice(&BOOT_SIGNATURE);
        sector
    }

    #[test]
    fn prefers_fat32_over_fat16_regardless_of_table_order() {
        let sector = mbr(&[(0x06, 100, 200), (0x0c, 2048, 4096)]);

        let partition = first_fat_partition(&sector).expect("a FAT partition");
        assert_eq!(partition.base_lba, 2048);
        assert_eq!(partition.sectors, 4096);
        assert_eq!(partition.partition_type, 0x0c);
    }

    #[test]
    fn skips_empty_and_non_fat_entries() {
        let sector = mbr(&[(0x83, 100, 200), (0x0c, 0, 200), (0x06, 300, 0)]);

        assert_eq!(first_fat_partition(&sector), None);
    }

    #[test]
    fn requires_the_mbr_signature() {
        let mut sector = mbr(&[(0x0c, 2048, 4096)]);
        sector[SIGNATURE_OFFSET] = 0;

        assert_eq!(first_fat_partition(&sector), None);
    }

    #[test]
    fn returns_a_declared_extent_without_clamping_the_card_size() {
        let sector = mbr(&[(0x0c, 2048, 10_000_000)]);

        let partition = first_fat_partition(&sector).expect("a FAT partition");
        assert_eq!(partition.sectors, 10_000_000);
    }
}
