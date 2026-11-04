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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl MbrFatPartition {
    /// The declared length in bytes, without clamping to the card.
    pub const fn len_bytes(self) -> u64 {
        self.sectors as u64 * BLOCK_SIZE
    }

    /// This entry clamped to the card, or `None` when it starts past the card.
    pub fn partition(self, card_bytes: u64) -> Option<Partition> {
        let base_byte = self.base_lba.checked_mul(BLOCK_SIZE)?;
        if base_byte >= card_bytes {
            return None;
        }
        Some(Partition {
            base_lba: self.base_lba,
            len_bytes: self.len_bytes().min(card_bytes - base_byte),
        })
    }
}

/// Lists the MBR's FAT partitions in priority order: FAT32 before FAT12/16,
/// then table order. Extents are not clamped to the card, so a writer can
/// validate the declaration first.
pub fn fat_partitions(
    sector0: &[u8; BLOCK_SIZE_USIZE],
) -> [Option<MbrFatPartition>; MBR_PARTITION_ENTRIES] {
    let mut partitions = [None; MBR_PARTITION_ENTRIES];
    if sector0[SIGNATURE_OFFSET..] != BOOT_SIGNATURE {
        return partitions;
    }

    let mut count = 0;
    for priority in [2, 1] {
        for index in 0..MBR_PARTITION_ENTRIES {
            let entry = MBR_PARTITION_TABLE + index * 16;
            if fat_type_priority(sector0[entry + 4]) != Some(priority) {
                continue;
            }

            let base_lba = le32(sector0, entry + 8) as u64;
            let sectors = le32(sector0, entry + 12);
            if base_lba == 0 || sectors == 0 {
                continue;
            }
            partitions[count] = Some(MbrFatPartition { base_lba, sectors });
            count += 1;
        }
    }

    partitions
}

/// The whole-card FAT volume when sector 0 carries a FAT boot sector.
pub fn bare_fat_partition(sector0: &[u8; BLOCK_SIZE_USIZE], card_bytes: u64) -> Option<Partition> {
    let len_bytes = fat_boot_sector_len_bytes(sector0)?;
    Some(Partition {
        base_lba: 0,
        len_bytes: len_bytes.min(card_bytes),
    })
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

    if let Some(bare) = bare_fat_partition(sector0, card_bytes) {
        candidates[count] = Some(bare);
        count += 1;
    }

    for entry in fat_partitions(sector0).into_iter().flatten() {
        let Some(partition) = entry.partition(card_bytes) else {
            continue;
        };
        candidates[count] = Some(partition);
        count += 1;
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

        let partitions = fat_partitions(&sector);
        let first = partitions[0].expect("a FAT partition");
        assert_eq!(first.base_lba, 2048);
        assert_eq!(first.sectors, 4096);
    }

    #[test]
    fn skips_empty_and_non_fat_entries() {
        let sector = mbr(&[(0x83, 100, 200), (0x0c, 0, 200), (0x06, 300, 0)]);

        assert_eq!(fat_partitions(&sector), [None; MBR_PARTITION_ENTRIES]);
    }

    #[test]
    fn requires_the_mbr_signature() {
        let mut sector = mbr(&[(0x0c, 2048, 4096)]);
        sector[SIGNATURE_OFFSET] = 0;

        assert_eq!(fat_partitions(&sector), [None; MBR_PARTITION_ENTRIES]);
    }

    #[test]
    fn returns_a_declared_extent_without_clamping_the_card_size() {
        let sector = mbr(&[(0x0c, 2048, 10_000_000)]);

        let partition = fat_partitions(&sector)[0].expect("a FAT partition");
        assert_eq!(partition.sectors, 10_000_000);
        assert_eq!(partition.len_bytes(), 10_000_000 * BLOCK_SIZE);
        assert_eq!(
            partition.partition(2048 * BLOCK_SIZE + 4096),
            Some(Partition {
                base_lba: 2048,
                len_bytes: 4096,
            })
        );
    }

    #[test]
    fn an_entry_past_the_card_has_no_clamped_partition() {
        let sector = mbr(&[(0x0c, 4096, 100)]);

        let partition = fat_partitions(&sector)[0].expect("a FAT partition");
        assert_eq!(partition.partition(4096 * BLOCK_SIZE), None);
    }

    #[test]
    fn candidates_put_a_bare_volume_first_and_skip_past_card_entries() {
        let mut sector = mbr(&[(0x0c, 8192, 100), (0x06, 2048, 100)]);
        sector[0] = 0xeb;
        sector[11..13].copy_from_slice(&512u16.to_le_bytes());
        sector[13] = 1;
        sector[14..16].copy_from_slice(&1u16.to_le_bytes());
        sector[16] = 1;
        sector[19..21].copy_from_slice(&100u16.to_le_bytes());

        let candidates = candidates(&sector, 8192 * BLOCK_SIZE);
        assert_eq!(candidates[0].map(|p| p.base_lba), Some(0));
        assert_eq!(candidates[1].map(|p| p.base_lba), Some(2048));
        assert_eq!(candidates[2], None);
    }
}
